//! openGauss / GaussDB 的 sha256 与 SM3 认证（TODOs G3 / DC1）。
//!
//! 只存 sha256 口令的用户（`password_encryption_type = 2`，openGauss 的默认值），服务端发的是
//! openGauss 自己的认证请求：码 10，和 PostgreSQL 的 SASL 同一个码，内容却不是机制列表。sqlx 当成
//! SASL 去解，报 `unsupported SASL authentication mechanisms:`（列表是空的），libpq 也不认。
//! 只存 SM3 口令的用户（`password_encryption_type = 3`，信创国密配置），服务端发认证请求码 13
//! （`AUTH_REQ_SM3`），标准 PostgreSQL 驱动同样不认。
//!
//! 不改 sqlx：在本机开一个只给这条连接用的转发口，sqlx 连它。转发口自己去连服务端，把登录这一段
//! 做完，再把服务端的「认证成功」交给 sqlx，此后原样双向转发——sqlx 看到的是一个不要口令的服务端。
//! 只在普通连接报了上述两类拒连特征之一后才走这条路（`sqlx_pool::open`），PostgreSQL 与别的兼容库不受影响。
//!
//! 协议版本写成 3.51：openGauss 只在客户端声明 3.51 时才在认证请求里带上迭代次数，3.0 时它照自己存的
//! 次数算（`auth_iteration_count`，可配），客户端只能猜。只有这里改版本号，sqlx 发什么都不变。
//!
//! 算法是 openGauss 的 RFC 5802 变体（核对自 `openGauss-server` `sha2.cpp` / `fe-auth.cpp`）：
//! - `K = PBKDF2-HMAC-SHA1(口令, 盐, 迭代次数, 32)`，`ClientKey = HMAC-SHA256(K, "Client Key")`
//! - SHA256（码 10，存法 0 或 2）：`StoredKey = SHA256(ClientKey)`
//! - SM3（码 13，存法 3）：`StoredKey = SM3(ClientKey)`（仅此处替换为 GB/T 32905-2016 SM3）
//! - 两者最终均回答 `hex(HMAC-SHA256(StoredKey, token) XOR ClientKey)`。

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 普通连接撞上 openGauss sha256（码 10）或 SM3（码 13）认证时 sqlx 的原话
pub fn is_opengauss_auth_rejection(error: &str) -> bool {
  is_sha256_rejection(error) || is_sm3_rejection(error)
}

/// 普通连接撞上 sha256 认证时 sqlx 的原话（机制列表是空的）
pub fn is_sha256_rejection(error: &str) -> bool {
  error.contains("unsupported SASL authentication mechanisms")
}

/// 普通连接撞上 openGauss SM3 认证（码 13）时 sqlx 的原话
pub fn is_sm3_rejection(error: &str) -> bool {
  error.contains("unknown authentication method: 13")
}

/// 认证请求里的码。10 和 PostgreSQL 的 SASL 同一个数；13 是 openGauss 扩展的 SM3
const AUTH_REQUEST_SHA256: u32 = 10;
const AUTH_REQUEST_SM3: u32 = 13;
/// 口令在服务端的存法：明文（0）与 sha256（2）走 SHA256 StoredKey；sm3（3）走 SM3 StoredKey
const STORED_PLAIN: u32 = 0;
const STORED_SHA256: u32 = 2;
const STORED_SM3: u32 = 3;
/// PBKDF2 迭代数合理上限（openGauss 默认 10,000），防止错位或恶意包卡死 CPU
const MAX_PBKDF2_ITERATIONS: u32 = 10_000_000;
/// 3.51：openGauss 据此在认证请求里带上迭代次数
const PROTOCOL_3_51: u32 = (3 << 16) | 51;
const SSL_REQUEST: u32 = 80_877_103;
const GSS_ENC_REQUEST: u32 = 80_877_104;
const CANCEL_REQUEST: u32 = 80_877_102;
/// 启动包的长度上限。正常的只有几百字节；防一个坏长度让这里分配几个 GB
const MAX_STARTUP_LENGTH: usize = 10_000;

/// 转发口连谁、用什么口令登录
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Upstream {
  pub host: String,
  pub port: u16,
  pub password: String,
  pub tls: Tls,
}

/// 转发口到服务端这一段的 TLS，照连接串里的 `sslmode` 等几项，语义与 sqlx 相同：
/// `prefer` 服务端不肯就不加密，`prefer` 与 `require` 不校验证书，`verify-ca` 不核主机名
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Tls {
  pub mode: TlsMode,
  pub root_certificate: Option<String>,
  pub client_certificate: Option<String>,
  pub client_key: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TlsMode {
  Disable,
  Prefer,
  Require,
  VerifyCa,
  VerifyFull,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthHash {
  Sha256,
  Sm3,
}

/// 由 sha256 认证请求的正文（码之后的部分）算出回答。只认明文与 sha256 两种存法
pub fn sha256_response(password: &str, request: &[u8]) -> Result<Vec<u8>, String> {
  rfc5802_response(password, request, AuthHash::Sha256)
}

/// 由 SM3 认证请求的正文（码 13 之后的部分）算出回答。只认 SM3 存法（3）
pub fn sm3_response(password: &str, request: &[u8]) -> Result<Vec<u8>, String> {
  rfc5802_response(password, request, AuthHash::Sm3)
}

fn rfc5802_response(password: &str, request: &[u8], hash: AuthHash) -> Result<Vec<u8>, String> {
  let label = match hash {
    AuthHash::Sha256 => "sha256",
    AuthHash::Sm3 => "SM3",
  };
  let field = |range: std::ops::Range<usize>| {
    request.get(range).ok_or_else(|| {
      format!("openGauss {label} authentication request is only {} bytes", request.len())
    })
  };
  let stored = u32::from_be_bytes(be4(field(0..4)?));
  let supported = match hash {
    AuthHash::Sha256 => stored == STORED_SHA256 || stored == STORED_PLAIN,
    AuthHash::Sm3 => stored == STORED_SM3,
  };
  if !supported {
    return Err(format!(
      "openGauss password storage method {stored} is not supported for {label} authentication"
    ));
  }
  let salt = hex_decode(field(4..68)?)?;
  let token = hex_decode(field(68..76)?)?;
  let iterations = u32::from_be_bytes(be4(field(76..80)?));
  if iterations == 0 || iterations > MAX_PBKDF2_ITERATIONS {
    return Err(format!(
      "openGauss {label} authentication iteration count {iterations} is out of range (1..={MAX_PBKDF2_ITERATIONS})"
    ));
  }

  let mut key = [0u8; 32];
  pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password.as_bytes(), &salt, iterations, &mut key);
  let client_key = hmac_sha256(&key, b"Client Key")?;
  let stored_key: [u8; 32] = match hash {
    AuthHash::Sha256 => Sha256::digest(&client_key).into(),
    AuthHash::Sm3 => sm3_digest(&client_key),
  };
  let signature = hmac_sha256(&stored_key, &token)?;
  let proof: String = signature
    .iter()
    .zip(&client_key)
    .map(|(left, right)| format!("{:02x}", left ^ right))
    .collect();
  Ok(proof.into_bytes())
}

/// GB/T 32905-2016（GM/T 0004-2012）SM3 密码杂凑算法。
/// 仅用于把 openGauss SM3 认证的 `ClientKey`（32 字节）散列成 `StoredKey`，
/// 纯安全整数位运算，避免为一个 32 字节摘要引入额外外部依赖。
fn sm3_digest(message: &[u8]) -> [u8; 32] {
  const IV: [u32; 8] = [
    0x7380_166f,
    0x4914_b2b9,
    0x1724_42d7,
    0xda8a_0600,
    0xa96f_30bc,
    0x1631_38aa,
    0xe38d_ee4d,
    0xb0fb_0e4e,
  ];
  let p0 = |x: u32| x ^ x.rotate_left(9) ^ x.rotate_left(17);
  let p1 = |x: u32| x ^ x.rotate_left(15) ^ x.rotate_left(23);

  let bit_len = (message.len() as u64).wrapping_mul(8);
  let mut padded = Vec::with_capacity((message.len() + 9 + 63) & !63);
  padded.extend_from_slice(message);
  padded.push(0x80);
  while padded.len() % 64 != 56 {
    padded.push(0);
  }
  padded.extend_from_slice(&bit_len.to_be_bytes());

  let mut state = IV;
  for block in padded.as_chunks::<64>().0 {
    let mut w = [0u32; 68];
    for (slot, word) in w.iter_mut().zip(block.as_chunks::<4>().0) {
      *slot = u32::from_be_bytes(*word);
    }
    for j in 16..68 {
      let mixed = w[j - 16] ^ w[j - 9] ^ w[j - 3].rotate_left(15);
      w[j] = p1(mixed) ^ w[j - 13].rotate_left(7) ^ w[j - 6];
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
    for j in 0..64 {
      let t_j = if j < 16 { 0x79cc_4519u32 } else { 0x7a87_9d8au32 };
      let ss1 = a
        .rotate_left(12)
        .wrapping_add(e)
        .wrapping_add(t_j.rotate_left((j % 32) as u32))
        .rotate_left(7);
      let ss2 = ss1 ^ a.rotate_left(12);
      let ff = if j < 16 { a ^ b ^ c } else { (a & b) | (a & c) | (b & c) };
      let gg = if j < 16 { e ^ f ^ g } else { (e & f) | ((!e) & g) };
      let tt1 = ff.wrapping_add(d).wrapping_add(ss2).wrapping_add(w[j] ^ w[j + 4]);
      let tt2 = gg.wrapping_add(h).wrapping_add(ss1).wrapping_add(w[j]);
      d = c;
      c = b.rotate_left(9);
      b = a;
      a = tt1;
      h = g;
      g = f.rotate_left(19);
      f = e;
      e = p0(tt2);
    }

    let next = [a, b, c, d, e, f, g, h];
    for (slot, delta) in state.iter_mut().zip(next) {
      *slot ^= delta;
    }
  }

  let mut out = [0u8; 32];
  for (chunk, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(state) {
    *chunk = word.to_be_bytes();
  }
  out
}

fn be4(bytes: &[u8]) -> [u8; 4] {
  let mut out = [0u8; 4];
  out.copy_from_slice(&bytes[..4]);
  out
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
  let mut mac = Hmac::<Sha256>::new_from_slice(key).map_err(|error| error.to_string())?;
  mac.update(data);
  Ok(mac.finalize().into_bytes().to_vec())
}

fn hex_decode(text: &[u8]) -> Result<Vec<u8>, String> {
  let digit = |byte: u8| match byte {
    b'0'..=b'9' => Ok(byte - b'0'),
    b'a'..=b'f' => Ok(byte - b'a' + 10),
    b'A'..=b'F' => Ok(byte - b'A' + 10),
    _ => Err(format!("openGauss authentication request has a non-hex byte {byte:#04x}")),
  };
  text
    .chunks(2)
    .map(|pair| Ok(digit(pair[0])? << 4 | digit(*pair.get(1).unwrap_or(&b'0'))?))
    .collect()
}

/// 这个服务端的转发口，没有就开一个。同一个服务端与口令只开一个，连接池里每条连接都连它：
/// 转发口跟着应用走，不跟着池子走，数目以「连过几个这样的服务端」为上限。经 SSH 隧道时本机端口每次
/// 重连都不同，旧的那个留着空等——一个监听套接字，没有连接就不占别的
pub async fn local_port(upstream: Upstream) -> Result<u16, String> {
  static RELAYS: OnceLock<Mutex<HashMap<Upstream, u16>>> = OnceLock::new();
  let relays = RELAYS.get_or_init(Default::default);
  if let Some(port) = relays.lock().map_err(|error| error.to_string())?.get(&upstream) {
    return Ok(*port);
  }
  let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(|error| error.to_string())?;
  let port = listener.local_addr().map_err(|error| error.to_string())?.port();
  relays.lock().map_err(|error| error.to_string())?.insert(upstream.clone(), port);
  let upstream = Arc::new(upstream);
  tokio::spawn(async move {
    loop {
      let Ok((client, _)) = listener.accept().await else {
        continue;
      };
      let upstream = Arc::clone(&upstream);
      tokio::spawn(async move {
        if let Err(error) = relay(client, &upstream).await {
          log::warn!("openGauss relay: {error}");
        }
      });
    }
  });
  Ok(port)
}

/// 客户端发来的启动包（长度在内），以及它的码
async fn read_startup(stream: &mut TcpStream) -> std::io::Result<(Vec<u8>, u32)> {
  let length = stream.read_u32().await? as usize;
  if !(8..=MAX_STARTUP_LENGTH).contains(&length) {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      format!("startup packet length {length}"),
    ));
  }
  let mut packet = vec![0u8; length];
  packet[..4].copy_from_slice(&(length as u32).to_be_bytes());
  stream.read_exact(&mut packet[4..]).await?;
  let code = u32::from_be_bytes(be4(&packet[4..8]));
  Ok((packet, code))
}

/// 服务端的一条消息（类型字节、长度在内）
async fn read_message<S: AsyncRead + Unpin>(stream: &mut S) -> std::io::Result<Vec<u8>> {
  let kind = stream.read_u8().await?;
  let length = stream.read_u32().await? as usize;
  if length < 4 {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      format!("message length {length}"),
    ));
  }
  let mut message = Vec::with_capacity(length + 1);
  message.push(kind);
  message.extend_from_slice(&(length as u32).to_be_bytes());
  message.resize(length + 1, 0);
  stream.read_exact(&mut message[5..]).await?;
  Ok(message)
}

/// 给 sqlx 一条它会照原样报出来的错误（28P01：口令不对一类）
fn error_response(message: &str) -> Vec<u8> {
  let mut body = Vec::new();
  for (field, value) in [(b'S', "FATAL"), (b'V', "FATAL"), (b'C', "28P01"), (b'M', message)] {
    body.push(field);
    body.extend_from_slice(value.as_bytes());
    body.push(0);
  }
  body.push(0);
  let mut packet = vec![b'E'];
  packet.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
  packet.extend(body);
  packet
}

async fn relay(mut client: TcpStream, upstream: &Upstream) -> std::io::Result<()> {
  // sqlx 连转发口时按 `sslmode=disable` 不会先问 TLS；`prefer` 一类的客户端问了就答「不用」
  let (mut startup, mut code) = read_startup(&mut client).await?;
  while code == SSL_REQUEST || code == GSS_ENC_REQUEST {
    client.write_all(b"N").await?;
    (startup, code) = read_startup(&mut client).await?;
  }
  let mut server = match connect(upstream).await {
    Ok(server) => server,
    // 说给 sqlx 听：它等的是启动包的回答，一条错误消息会原样成为连接失败的原因
    Err(error) => {
      let reason = format!("openGauss relay could not reach the server: {error}");
      if code != CANCEL_REQUEST {
        client.write_all(&error_response(&reason)).await?;
      }
      return Err(std::io::Error::other(reason));
    }
  };
  if code == CANCEL_REQUEST {
    return server.write_all(&startup).await;
  }
  startup[4..8].copy_from_slice(&PROTOCOL_3_51.to_be_bytes());
  server.write_all(&startup).await?;
  authenticate(&mut client, &mut server, &upstream.password).await?;
  tokio::io::copy_bidirectional(&mut client, &mut server).await.map(|_| ())
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

/// 连上服务端，按 `sslmode` 先问 TLS（与 sqlx 的 `maybe_upgrade` 同一个流程）
async fn connect(upstream: &Upstream) -> Result<Box<dyn Stream>, String> {
  let mut tcp = TcpStream::connect((upstream.host.as_str(), upstream.port))
    .await
    .map_err(|error| error.to_string())?;
  if upstream.tls.mode == TlsMode::Disable {
    return Ok(Box::new(tcp));
  }
  let mut request = 8u32.to_be_bytes().to_vec();
  request.extend(SSL_REQUEST.to_be_bytes());
  tcp.write_all(&request).await.map_err(|error| error.to_string())?;
  match tcp.read_u8().await.map_err(|error| error.to_string())? {
    b'S' => {}
    b'N' if upstream.tls.mode == TlsMode::Prefer => return Ok(Box::new(tcp)),
    b'N' => return Err("server does not support TLS".to_string()),
    other => return Err(format!("unexpected response from SSLRequest: {other:#04x}")),
  }
  let config = tls_config(&upstream.tls)?;
  let name = rustls::pki_types::ServerName::try_from(upstream.host.clone())
    .map_err(|error| error.to_string())?;
  let stream = tokio_rustls::TlsConnector::from(Arc::new(config))
    .connect(name, tcp)
    .await
    .map_err(|error| error.to_string())?;
  Ok(Box::new(stream))
}

/// 照 sqlx 的 `tls_rustls::handshake` 配：同一个加密实现（ring）、同一套根证书（webpki-roots
/// 加上 `sslrootcert`）、同样三档校验
fn tls_config(tls: &Tls) -> Result<rustls::ClientConfig, String> {
  use rustls::pki_types::pem::PemObject;
  use rustls::pki_types::{CertificateDer, PrivateKeyDer};

  let provider = Arc::new(rustls::crypto::ring::default_provider());
  let builder = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
    .with_safe_default_protocol_versions()
    .map_err(|error| error.to_string())?;
  let read = |path: &str| std::fs::read(path).map_err(|error| format!("{path}: {error}"));
  let client_auth = match (&tls.client_certificate, &tls.client_key) {
    (Some(certificate), Some(key)) => {
      let chain = CertificateDer::pem_slice_iter(&read(certificate)?)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{certificate}: {error}"))?;
      let key =
        PrivateKeyDer::from_pem_slice(&read(key)?).map_err(|error| format!("{key}: {error}"))?;
      Some((chain, key))
    }
    (None, None) => None,
    _ => return Err("user auth key and certs must be given together".to_string()),
  };

  let verifier: Arc<dyn rustls::client::danger::ServerCertVerifier> = match tls.mode {
    TlsMode::Disable | TlsMode::Prefer | TlsMode::Require => Arc::new(AnyCertificate(provider)),
    TlsMode::VerifyCa | TlsMode::VerifyFull => {
      let mut roots =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
      if let Some(path) = &tls.root_certificate {
        for certificate in CertificateDer::pem_slice_iter(&read(path)?) {
          let certificate = certificate.map_err(|error| format!("{path}: {error}"))?;
          roots.add(certificate).map_err(|error| format!("{path}: {error}"))?;
        }
      }
      let webpki =
        rustls::client::WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
          .build()
          .map_err(|error| error.to_string())?;
      match tls.mode {
        TlsMode::VerifyCa => Arc::new(AnyHostname(webpki)),
        _ => webpki,
      }
    }
  };
  let builder = builder.dangerous().with_custom_certificate_verifier(verifier);
  match client_auth {
    Some((chain, key)) => {
      builder.with_client_auth_cert(chain, key).map_err(|error| error.to_string())
    }
    None => Ok(builder.with_no_client_auth()),
  }
}

/// `prefer` / `require`：加密，不校验证书（sqlx 的 `DummyTlsVerifier`）。握手签名照常验
#[derive(Debug)]
struct AnyCertificate(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for AnyCertificate {
  fn verify_server_cert(
    &self,
    _end_entity: &rustls::pki_types::CertificateDer<'_>,
    _intermediates: &[rustls::pki_types::CertificateDer<'_>],
    _server_name: &rustls::pki_types::ServerName<'_>,
    _ocsp_response: &[u8],
    _now: rustls::pki_types::UnixTime,
  ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
    Ok(rustls::client::danger::ServerCertVerified::assertion())
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    certificate: &rustls::pki_types::CertificateDer<'_>,
    signature: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls12_signature(
      message,
      certificate,
      signature,
      &self.0.signature_verification_algorithms,
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    certificate: &rustls::pki_types::CertificateDer<'_>,
    signature: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls13_signature(
      message,
      certificate,
      signature,
      &self.0.signature_verification_algorithms,
    )
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    self.0.signature_verification_algorithms.supported_schemes()
  }
}

/// `verify-ca`：校验证书链，不核主机名（sqlx 的 `NoHostnameTlsVerifier`）
#[derive(Debug)]
struct AnyHostname(Arc<rustls::client::WebPkiServerVerifier>);

impl rustls::client::danger::ServerCertVerifier for AnyHostname {
  fn verify_server_cert(
    &self,
    end_entity: &rustls::pki_types::CertificateDer<'_>,
    intermediates: &[rustls::pki_types::CertificateDer<'_>],
    server_name: &rustls::pki_types::ServerName<'_>,
    ocsp_response: &[u8],
    now: rustls::pki_types::UnixTime,
  ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
    match self.0.verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now) {
      Err(rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForName)) => {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
      }
      verdict => verdict,
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    certificate: &rustls::pki_types::CertificateDer<'_>,
    signature: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
    self.0.verify_tls12_signature(message, certificate, signature)
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    certificate: &rustls::pki_types::CertificateDer<'_>,
    signature: &rustls::DigitallySignedStruct,
  ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
    self.0.verify_tls13_signature(message, certificate, signature)
  }

  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    self.0.supported_verify_schemes()
  }
}

/// 答掉 openGauss sha256（码 10）或 SM3（码 13）的认证请求；别的消息原样交给客户端，此后的事归它
async fn authenticate<C, S>(client: &mut C, server: &mut S, password: &str) -> std::io::Result<()>
where
  C: AsyncWrite + Unpin,
  S: AsyncRead + AsyncWrite + Unpin,
{
  loop {
    let message = read_message(server).await?;
    let auth_hash = if message[0] == b'R' && message.len() >= 9 {
      match u32::from_be_bytes(be4(&message[5..9])) {
        AUTH_REQUEST_SHA256 => Some(AuthHash::Sha256),
        AUTH_REQUEST_SM3 => Some(AuthHash::Sm3),
        _ => None,
      }
    } else {
      None
    };
    let Some(hash) = auth_hash else {
      return client.write_all(&message).await;
    };
    match rfc5802_response(password, &message[9..], hash) {
      Ok(proof) => {
        let mut answer = vec![b'p'];
        answer.extend_from_slice(&((proof.len() + 5) as u32).to_be_bytes());
        answer.extend(proof);
        answer.push(0);
        server.write_all(&answer).await?;
      }
      Err(reason) => return client.write_all(&error_response(&reason)).await,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// 期望值由 Node 的 crypto 按官方驱动的算法独立算出（`pbkdf2Sync(…, 'sha1')` 与两次 HMAC-SHA256）
  #[test]
  fn answers_like_the_reference_implementation() {
    let mut request = STORED_SHA256.to_be_bytes().to_vec();
    request.extend(format!("3f2a{}beef", "0".repeat(56)).as_bytes());
    request.extend(b"a1b2c3d4");
    request.extend(10_000u32.to_be_bytes());
    let proof = sha256_response("Test@123", &request).expect("answer");
    assert_eq!(
      String::from_utf8(proof).expect("hex"),
      "5563331295fc62e3ed709545b53caa69845010319eadb1aa6c8e8efd32ed45f0"
    );
  }

  #[test]
  fn sm3_matches_gbt_32905_2016_official_vectors() {
    let hex = |bytes: [u8; 32]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    // GB/T 32905-2016 附录 A 示例 1："abc"
    assert_eq!(
      hex(sm3_digest(b"abc")),
      "66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0"
    );
    // GB/T 32905-2016 附录 A 示例 2："abcd" 重复 16 次（64 字节跨两块边界）
    assert_eq!(
      hex(sm3_digest(b"abcd".repeat(16).as_slice())),
      "debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732"
    );
  }

  /// 期望值由 OpenSSL 3（Python `hashlib.new("sm3")` + `pbkdf2_hmac("sha1")` + HMAC-SHA256）
  /// 按 `openGauss-server` `GsSm3Encrypt` + `fe-auth.cpp` 独立算出
  #[test]
  fn answers_sm3_like_the_reference_implementation() {
    let mut request = STORED_SM3.to_be_bytes().to_vec();
    request.extend(format!("3f2a{}beef", "0".repeat(56)).as_bytes());
    request.extend(b"a1b2c3d4");
    request.extend(10_000u32.to_be_bytes());
    let proof = sm3_response("Test@123", &request).expect("sm3 answer");
    assert_eq!(
      String::from_utf8(proof).expect("hex"),
      "7fecc63e76dc024dc765342834035c72aca17d20a75c1ec26393fd2116c24f6e"
    );
  }

  #[test]
  fn refuses_short_requests_wrong_storage_and_abnormal_iterations() {
    // 3.0 的请求里没有迭代次数；转发口总是声明 3.51，真碰上就说清楚，不猜次数
    let mut request = STORED_SHA256.to_be_bytes().to_vec();
    request.extend([b'0'; 72]);
    assert!(sha256_response("p", &request).is_err());
    assert!(sm3_response("p", &request).is_err());

    let mut md5 = 1u32.to_be_bytes().to_vec();
    md5.extend([b'0'; 76]);
    assert!(sha256_response("p", &md5).expect_err("md5").contains("method 1"));

    // SM3 请求不接受 SHA256 存法（2），反之亦然
    let mut sha256_req = STORED_SHA256.to_be_bytes().to_vec();
    sha256_req.extend([b'0'; 72]);
    sha256_req.extend(10_000u32.to_be_bytes());
    assert!(sm3_response("p", &sha256_req).expect_err("sha256 in sm3").contains("method 2"));

    // 异常迭代数（0 或超上限）拒绝计算，防止恶意/错位包卡死 CPU
    let mut zero_iter = STORED_SM3.to_be_bytes().to_vec();
    zero_iter.extend([b'0'; 72]);
    zero_iter.extend(0u32.to_be_bytes());
    assert!(sm3_response("p", &zero_iter).expect_err("zero iter").contains("out of range"));

    let mut huge_iter = STORED_SM3.to_be_bytes().to_vec();
    huge_iter.extend([b'0'; 72]);
    huge_iter.extend((MAX_PBKDF2_ITERATIONS + 1).to_be_bytes());
    assert!(sm3_response("p", &huge_iter).expect_err("huge iter").contains("out of range"));
  }

  #[test]
  fn recognizes_the_driver_message() {
    assert!(is_opengauss_auth_rejection(
      "error returned from database: unsupported SASL authentication mechanisms: "
    ));
    assert!(is_opengauss_auth_rejection("unknown authentication method: 13"));
    assert!(!is_opengauss_auth_rejection("password authentication failed for user \"u\""));
    assert!(!is_opengauss_auth_rejection("unknown authentication method: 99"));
  }

  /// DC0-E1 + DC1 端到端门禁：
  /// 1) 真实 `sqlx` 直连发出 `AUTH_REQ_SM3 (13)` 的 mock 服务端，确认报错能被 `is_opengauss_auth_rejection` 命中；
  /// 2) 真实 `sqlx_pool::open` 自动切本机 `opengauss_relay` 声明协议 `3.51` 并完成 SM3 握手。
  #[tokio::test]
  async fn sqlx_pool_open_completes_sm3_handshake_via_relay() {
    use sqlx::postgres::{PgConnectOptions, PgSslMode};
    use sqlx::Connection;

    let expected_proof = "7fecc63e76dc024dc765342834035c72aca17d20a75c1ec26393fd2116c24f6e";
    let salt_hex = format!("3f2a{}beef", "0".repeat(56));
    let token_hex = "a1b2c3d4";

    let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("bind mock opengauss");
    let port = listener.local_addr().expect("addr").port();

    let server = tokio::spawn(async move {
      // 允许处理 3 次连接：1) 裸 sqlx 探测失败 2) sqlx_pool::open 首次直连失败 3) 经 opengauss_relay 成功
      for _ in 0..3 {
        let Ok((mut sock, _)) = listener.accept().await else {
          break;
        };
        let (startup, code) = read_startup(&mut sock).await.expect("startup");
        assert_ne!(code, SSL_REQUEST, "test uses sslmode=disable");
        let protocol = u32::from_be_bytes(be4(&startup[4..8]));

        // 发送 AUTH_REQ_SM3 (13) 认证请求
        let mut body = Vec::new();
        body.extend_from_slice(&AUTH_REQUEST_SM3.to_be_bytes());
        body.extend_from_slice(&STORED_SM3.to_be_bytes());
        body.extend_from_slice(salt_hex.as_bytes());
        body.extend_from_slice(token_hex.as_bytes());
        body.extend_from_slice(&10_000u32.to_be_bytes());
        let mut msg = vec![b'R'];
        msg.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
        msg.extend_from_slice(&body);
        sock.write_all(&msg).await.expect("send auth req");

        if protocol != PROTOCOL_3_51 {
          // 裸 sqlx 收到码 13 会直接断开
          continue;
        }

        // 转发口声明了 3.51，校验其回发的 SM3 client proof
        let answer = read_message(&mut sock).await.expect("password message");
        assert_eq!(answer[0], b'p');
        let proof_str = std::str::from_utf8(&answer[5..answer.len() - 1]).expect("utf8 proof");
        assert_eq!(proof_str, expected_proof);

        // 回发 AuthenticationOk (R, len=8, 0) + ParameterStatus + ReadyForQuery (Z, len=5, 'I')
        let mut ok = vec![b'R'];
        ok.extend_from_slice(&8u32.to_be_bytes());
        ok.extend_from_slice(&0u32.to_be_bytes());
        ok.push(b'Z');
        ok.extend_from_slice(&5u32.to_be_bytes());
        ok.push(b'I');
        sock.write_all(&ok).await.expect("send auth ok");

        // 收到 sqlx 后续连接初始化/关闭时干净退出
        let mut buf = [0u8; 64];
        let _ =
          tokio::time::timeout(std::time::Duration::from_millis(200), sock.read(&mut buf)).await;
      }
    });

    // 1) 实测真实 sqlx 收到码 13 时的精确错误文本
    let direct_opts = PgConnectOptions::new()
      .host("127.0.0.1")
      .port(port)
      .username("sm3user")
      .password("Test@123")
      .database("postgres")
      .ssl_mode(PgSslMode::Disable);
    let direct_err = sqlx::PgConnection::connect_with(&direct_opts)
      .await
      .expect_err("direct sqlx must reject AUTH_REQ_SM3")
      .to_string();
    assert!(
      is_opengauss_auth_rejection(&direct_err),
      "unexpected sqlx SM3 rejection message: {direct_err}"
    );

    // 2) 实测经 sqlx_pool::open 自动切 opengauss_relay 完成 SM3 认证建连
    let url = format!("postgres://sm3user:Test%40123@127.0.0.1:{port}/postgres?sslmode=disable");
    let pool = crate::services::sqlx_pool::open(&url).await.expect("open via sm3 relay");
    drop(pool);
    server.await.expect("mock server");
  }
}
