//! Redis：第二种非关系型库。
//!
//! 连接由后端持有，与 MongoDB 同一个办法：`test_connection` 连上之后把 [`RedisPool`]
//! 登记在 [`RedisRegistry`] 里，键是不带口令的 `redis://user@host:port/库号`。
//!
//! 和 MongoDB 不同、值得记住的：
//! - **一个库号一条连接。** `SELECT` 改的是整条连接的状态，而 `ConnectionManager` 是
//!   多路复用的——在它上面切库，同时在跑的另一条命令就落到别的库里。所以每个库号
//!   各开一条，第一次用到时才开（`RedisPool::connection`）。
//! - **键和值都是字节串**，不一定是 UTF-8。往前端送的是 [`RedisBytes`]：原样的
//!   base64（拿去定位用）加一份给人看的文字；前端回传的永远是 base64，不是那份文字。
//! - **只用 `SCAN` 族，不用 `KEYS`。** `KEYS` 在大库上会把整台服务端卡住。

use crate::models::{ConnectionProfile, TlsMode};
use crate::services::pool_registry::PoolRegistry;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use redis::aio::{ConnectionLike, ConnectionManager, ConnectionManagerConfig};
use redis::cluster_async::ClusterConnection;
use redis::cluster_routing::{RoutingInfo, SingleNodeRoutingInfo};
use redis::{ConnectionAddr, ConnectionInfo, IntoConnectionInfo, RedisConnectionInfo, Value};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 与前端 `REDIS_SCHEME` 一致：连接串以它开头就归这里管
pub const REDIS_SCHEME: &str = "redis://";

/// 口令或用户名不对（`WRONGPASS`，或旧版的 `invalid password`）
pub const REDIS_AUTH_FAILED: &str = "DATAOMNI_REDIS_AUTH_FAILED";
/// 没给口令，而服务端要（`NOAUTH`）
pub const REDIS_AUTH_REQUIRED: &str = "DATAOMNI_REDIS_AUTH_REQUIRED";
/// 这个用户没有这条命令或这个键的权限（`NOPERM`，ACL）。冒号后面是服务端原话
pub const REDIS_NO_PERMISSION: &str = "DATAOMNI_REDIS_NO_PERMISSION";
/// 在时限内没连上：地址不通、TLS 对不上、服务没起
pub const REDIS_UNREACHABLE: &str = "DATAOMNI_REDIS_UNREACHABLE";
/// 服务端报错，冒号后面是 `错误码: 原话`
pub const REDIS_SERVER_ERROR: &str = "DATAOMNI_REDIS_SERVER_ERROR";
/// 超过了查询时限
pub const REDIS_TIMEOUT: &str = "DATAOMNI_REDIS_TIMEOUT";
/// 要看的键已经不在了（过期或被删）
pub const REDIS_KEY_GONE: &str = "DATAOMNI_REDIS_KEY_GONE";
/// 「库」那一格不是 0 或正整数
pub const REDIS_DATABASE_INVALID: &str = "DATAOMNI_REDIS_DATABASE_INVALID";
/// 前端传回来的键不是合法的 base64——只会是程序错误，报出来比拿错的键去查强
pub const REDIS_KEY_INVALID: &str = "DATAOMNI_REDIS_KEY_INVALID";
/// CA 证书文件读不了。冒号后面带着路径
pub const REDIS_TLS_FILE_INVALID: &str = "DATAOMNI_REDIS_TLS_FILE_INVALID";
/// 选了 Sentinel 却没填主节点名
pub const REDIS_SENTINEL_MASTER_MISSING: &str = "DATAOMNI_REDIS_SENTINEL_MASTER_MISSING";
/// 哨兵不认这个主节点名（`SENTINEL get-master-addr-by-name` 回的是空）。冒号后面是那个名字
pub const REDIS_SENTINEL_MASTER_UNKNOWN: &str = "DATAOMNI_REDIS_SENTINEL_MASTER_UNKNOWN";
/// 备用哨兵节点地址格式无效。冒号后面是出错的那一项
pub const REDIS_SENTINEL_NODES_INVALID: &str = "DATAOMNI_REDIS_SENTINEL_NODES_INVALID";
/// 默认 Sentinel 端口
const DEFAULT_SENTINEL_PORT: u16 = 26379;
/// 多哨兵场景下每台哨兵的寻主超时，防止第一台黑洞耗光整次连接超时
const PER_SENTINEL_TIMEOUT: Duration = Duration::from_secs(3);
/// 经 SSH 隧道连 Sentinel 或集群：隧道只通到填的那一个地址，哨兵报回的主节点、集群报回的
/// 其余节点都在隧道另一头，这边连不到
pub const REDIS_DISCOVERY_VIA_TUNNEL: &str = "DATAOMNI_REDIS_DISCOVERY_VIA_TUNNEL";
/// 集群只有 0 号库（`SELECT` 别的库号报错）
pub const REDIS_CLUSTER_DATABASE: &str = "DATAOMNI_REDIS_CLUSTER_DATABASE";
/// 连接串对应的连接不在（断开之后还有请求过来）
pub const REDIS_NOT_CONNECTED: &str = "DATAOMNI_DB_SESSION_NOT_CONNECTED";

/// 连上的等待上限。和前端建立会话的 15 秒错开，让这里先报出原因
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// 字符串值最多带多少字节过来。整份要看得另想办法（导出），界面上是一眼
const STRING_PREVIEW_BYTES: usize = 512 * 1024;

/// `SCAN` 一次最多让服务端看多少个槽位。`COUNT` 只是提示，服务端每次返回的个数不定
const SCAN_COUNT: u64 = 1_000;

/// 下一轮 `SCAN` 的 `COUNT`：按到目前为止的匹配密度，估「还差的个数要看多少槽位」。
///
/// 固定给 1000 时，键密的库里一轮就回来约 1000 个，一页远超前端的页大小；
/// 固定给页大小时，匹配很稀的模式又要多跑几倍的往返。第一轮当作全都匹配，
/// 之后按密度放大，一轮都没匹配上就直接给上限
fn next_scan_count(page: usize, collected: usize, scanned: u64) -> u64 {
  let remaining = page.saturating_sub(collected).max(1) as u64;
  let ceiling = SCAN_COUNT.max(remaining);
  if scanned == 0 {
    return remaining.min(ceiling);
  }
  if collected == 0 {
    return ceiling;
  }
  (remaining.saturating_mul(scanned) / collected as u64).clamp(remaining, ceiling)
}

pub type RedisRegistry = PoolRegistry<RedisPool>;

/// 单次 Redis Keyspace 抽样体检的键数硬上限（AN3 安全护栏）
pub const MAX_REDIS_ANALYSIS_SAMPLE: usize = 20_000;
/// 默认抽样键数
pub const DEFAULT_REDIS_ANALYSIS_SAMPLE: usize = 5_000;
/// 抽样体检单批 SCAN 大小
const ANALYSIS_BATCH_SIZE: usize = 200;

/// 托管在 Tauri `State` 中的 Redis 体检协作取消登记表
#[derive(Default)]
pub struct RedisAnalysisState {
  active: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl RedisAnalysisState {
  pub fn register(&self, request_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut guard) = self.active.lock() {
      guard.insert(request_id.to_string(), Arc::clone(&flag));
    }
    flag
  }

  pub fn cancel(&self, request_id: &str) -> bool {
    if let Ok(guard) = self.active.lock() {
      if let Some(flag) = guard.get(request_id) {
        flag.store(true, Ordering::SeqCst);
        return true;
      }
    }
    false
  }

  pub fn finish(&self, request_id: &str) {
    if let Ok(mut guard) = self.active.lock() {
      guard.remove(request_id);
    }
  }
}

#[derive(Clone)]
pub struct RedisTarget {
  host: String,
  port: u16,
  username: String,
  password: String,
  /// 表单上「库」那一格，默认 0。对象树里另外几个库各开各的连接
  database: i64,
  tls: TlsMode,
  ca_certificate_path: Option<String>,
  /// 经 Sentinel：`sentinel_nodes` 按顺序保存主表单地址与备用哨兵地址，主节点地址每次开连接时问它们
  sentinel_master: Option<String>,
  sentinel_nodes: Vec<(String, u16)>,
  /// 集群：`host` / `port` 是任意一个节点，驱动从它读出槽位分布
  cluster: bool,
}

/// 解析单个哨兵节点文本：`host:port`、`[ipv6]:port`、`[ipv6]` 或裸 `host`（默认端口 26379）
fn parse_sentinel_endpoint(raw: &str) -> Result<(String, u16), String> {
  let token = raw.trim();
  if token.is_empty() {
    return Err(format!("{REDIS_SENTINEL_NODES_INVALID}: {raw}"));
  }
  if let Some(rest) = token.strip_prefix('[') {
    let Some(end) = rest.find(']') else {
      return Err(format!("{REDIS_SENTINEL_NODES_INVALID}: {token}"));
    };
    let host = rest[..end].trim();
    if host.is_empty() {
      return Err(format!("{REDIS_SENTINEL_NODES_INVALID}: {token}"));
    }
    let after = &rest[end + 1..];
    if after.is_empty() {
      return Ok((host.to_string(), DEFAULT_SENTINEL_PORT));
    }
    let Some(port_text) = after.strip_prefix(':') else {
      return Err(format!("{REDIS_SENTINEL_NODES_INVALID}: {token}"));
    };
    let port = port_text
      .parse::<u16>()
      .ok()
      .filter(|port| *port > 0)
      .ok_or_else(|| format!("{REDIS_SENTINEL_NODES_INVALID}: {token}"))?;
    return Ok((host.to_string(), port));
  }

  // 未加方括号但含多个冒号（如裸 IPv6 `::1`）：按整串作为主机，使用默认哨兵端口
  if token.matches(':').count() > 1 {
    return Ok((token.to_string(), DEFAULT_SENTINEL_PORT));
  }

  if let Some((host, port_text)) = token.rsplit_once(':') {
    let host = host.trim();
    if host.is_empty() {
      return Err(format!("{REDIS_SENTINEL_NODES_INVALID}: {token}"));
    }
    let port = port_text
      .trim()
      .parse::<u16>()
      .ok()
      .filter(|port| *port > 0)
      .ok_or_else(|| format!("{REDIS_SENTINEL_NODES_INVALID}: {token}"))?;
    return Ok((host.to_string(), port));
  }

  Ok((token.to_string(), DEFAULT_SENTINEL_PORT))
}

/// 合并表单主地址与 `sentinelNodes` 备用哨兵列表（逗号、分号或换行分隔），保序去重
pub fn parse_sentinel_nodes(
  primary_host: &str,
  primary_port: u16,
  extra: Option<&str>,
) -> Result<Vec<(String, u16)>, String> {
  let mut nodes = Vec::new();
  let mut seen = HashSet::new();
  let mut push = |host: String, port: u16| {
    let key = (host.to_ascii_lowercase(), port);
    if seen.insert(key) {
      nodes.push((host, port));
    }
  };
  let primary = primary_host.trim();
  if !primary.is_empty() {
    push(primary.to_string(), primary_port);
  }
  if let Some(extra) = extra {
    for raw in extra.split([',', ';', '\n', '\r']) {
      let trimmed = raw.trim();
      if trimmed.is_empty() {
        continue;
      }
      let (host, port) = parse_sentinel_endpoint(trimmed)?;
      push(host, port);
    }
  }
  Ok(nodes)
}

impl RedisTarget {
  pub fn from_profile(profile: &ConnectionProfile) -> Result<Self, String> {
    let sentinel_master = match profile.redis_sentinel_master() {
      Some("") => return Err(REDIS_SENTINEL_MASTER_MISSING.to_string()),
      master => master.map(str::to_string),
    };
    let sentinel_nodes = if sentinel_master.is_some() {
      parse_sentinel_nodes(&profile.host, profile.port, profile.redis_sentinel_nodes())?
    } else {
      Vec::new()
    };
    let cluster = profile.redis_cluster();
    if (sentinel_master.is_some() || cluster) && profile.ssh_tunnel.is_some() {
      return Err(REDIS_DISCOVERY_VIA_TUNNEL.to_string());
    }
    let database = database_index(profile.database.as_deref())?;
    if cluster && database != 0 {
      return Err(REDIS_CLUSTER_DATABASE.to_string());
    }
    Ok(Self {
      host: profile.host.clone(),
      port: profile.port,
      username: profile.username.clone(),
      password: profile.password.clone(),
      database,
      tls: profile.effective_tls_mode(),
      ca_certificate_path: profile.ca_certificate_path.clone().filter(|path| !path.is_empty()),
      sentinel_master,
      sentinel_nodes,
      cluster,
    })
  }

  /// `credentials` 为假时不带用户名与口令：给没设口令的哨兵用
  fn connection_info(
    &self,
    host: &str,
    port: u16,
    database: i64,
    credentials: bool,
  ) -> Result<ConnectionInfo, String> {
    // `Required` 只加密不校验，与另外几家一致；`VerifyCa` 按完整校验（rustls 没有
    // 只放过主机名的开关），往严里走
    let address = match self.tls {
      TlsMode::Disabled => ConnectionAddr::Tcp(host.to_string(), port),
      mode => ConnectionAddr::TcpTls {
        host: host.to_string(),
        port,
        insecure: matches!(mode, TlsMode::Preferred | TlsMode::Required),
        tls_params: None,
      },
    };
    let mut settings = RedisConnectionInfo::default().set_db(database);
    if credentials && !self.username.is_empty() {
      settings = settings.set_username(&self.username);
    }
    if credentials && !self.password.is_empty() {
      settings = settings.set_password(&self.password);
    }
    Ok(address.into_connection_info().map_err(describe_error)?.set_redis_settings(settings))
  }

  /// 加密且给了 CA 时那份 CA 的内容
  fn root_certificate(&self) -> Result<Option<redis::TlsCertificates>, String> {
    match (&self.ca_certificate_path, self.tls) {
      (Some(path), mode) if mode != TlsMode::Disabled => {
        let root = std::fs::read(path)
          .map_err(|error| format!("{REDIS_TLS_FILE_INVALID}: {path}: {error}"))?;
        Ok(Some(redis::TlsCertificates { client_tls: None, root_cert: Some(root) }))
      }
      _ => Ok(None),
    }
  }

  fn client(
    &self,
    host: &str,
    port: u16,
    database: i64,
    credentials: bool,
  ) -> Result<redis::Client, String> {
    let info = self.connection_info(host, port, database, credentials)?;
    match self.root_certificate()? {
      Some(certificates) => redis::Client::build_with_tls(info, certificates).map_err(|error| {
        let path = self.ca_certificate_path.as_deref().unwrap_or_default();
        format!("{REDIS_TLS_FILE_INVALID}: {path}: {error}")
      }),
      None => redis::Client::open(info).map_err(describe_error),
    }
  }
}

/// 「库」那一格：空着是 0。别的写法当场拒——拿 `SELECT abc` 去问服务端，报出来的是
/// 一句「invalid DB index」，而且要等到连上之后
fn database_index(text: Option<&str>) -> Result<i64, String> {
  match text.map(str::trim).filter(|text| !text.is_empty()) {
    None => Ok(0),
    Some(text) => text
      .parse::<i64>()
      .ok()
      .filter(|index| *index >= 0)
      .ok_or_else(|| format!("{REDIS_DATABASE_INVALID}: {text}")),
  }
}

/// 单机（含经 Sentinel 找到的主节点）是一条 `ConnectionManager`；集群是驱动的集群连接，
/// 按键的槽位把命令发到对应的主节点。两种都多路复用、克隆一份就是同一条连接
#[derive(Clone)]
pub enum RedisConnection {
  Single(ConnectionManager),
  Cluster(ClusterConnection),
}

impl ConnectionLike for RedisConnection {
  fn req_packed_command<'a>(&'a mut self, cmd: &'a redis::Cmd) -> redis::RedisFuture<'a, Value> {
    match self {
      Self::Single(connection) => connection.req_packed_command(cmd),
      Self::Cluster(connection) => connection.req_packed_command(cmd),
    }
  }

  fn req_packed_commands<'a>(
    &'a mut self,
    pipeline: &'a redis::Pipeline,
    offset: usize,
    count: usize,
  ) -> redis::RedisFuture<'a, Vec<Value>> {
    match self {
      Self::Single(connection) => connection.req_packed_commands(pipeline, offset, count),
      Self::Cluster(connection) => connection.req_packed_commands(pipeline, offset, count),
    }
  }

  fn get_db(&self) -> i64 {
    match self {
      Self::Single(connection) => connection.get_db(),
      Self::Cluster(connection) => connection.get_db(),
    }
  }
}

/// 一个连接配置在后端的全部：连接参数加每个库号一条连接
pub struct RedisPool {
  target: RedisTarget,
  connections: tokio::sync::Mutex<HashMap<i64, RedisConnection>>,
}

impl RedisPool {
  /// 这个库号上的连接，第一次用到时才开。断了自己重连。
  /// 查缓存命中立即放锁，网络建连不持 `self.connections` 锁，避免阻塞同实例其他库号（H16 / P2-3）
  pub async fn connection(&self, database: i64) -> Result<RedisConnection, String> {
    if self.target.cluster && database != 0 {
      return Err(REDIS_CLUSTER_DATABASE.to_string());
    }
    {
      let connections = self.connections.lock().await;
      if let Some(connection) = connections.get(&database) {
        return Ok(connection.clone());
      }
    }
    let connection = open(&self.target, database).await?;
    let mut connections = self.connections.lock().await;
    Ok(connections.entry(database).or_insert_with(|| connection).clone())
  }

  /// 配置里的那个库号：对象树总是列出它，哪怕里面一个键都没有
  pub fn default_database(&self) -> i64 {
    self.target.database
  }

  pub fn is_sentinel(&self) -> bool {
    self.target.sentinel_master.is_some()
  }

  /// Sentinel 模式下主从切换后清空旧库连接缓存，重新向哨兵查询最新主节点并建连（DC3 / G8）
  pub async fn refresh_sentinel_connection(
    &self,
    database: i64,
  ) -> Result<RedisConnection, String> {
    if !self.is_sentinel() {
      return self.connection(database).await;
    }
    let mut connections = self.connections.lock().await;
    connections.remove(&database);
    let connection = open(&self.target, database).await?;
    connections.insert(database, connection.clone());
    Ok(connection)
  }
}

/// 判定是否为 Redis 从库拒写错误（`READONLY You can't write against a read only replica.`）
fn is_readonly_redis_error(error: &redis::RedisError) -> bool {
  error.code().is_some_and(|code| code.eq_ignore_ascii_case("READONLY"))
    || matches!(error.kind(), redis::ErrorKind::Server(redis::ServerErrorKind::ReadOnly))
}

fn is_readonly_value(value: &Value) -> bool {
  matches!(value, Value::ServerError(error) if error.code().eq_ignore_ascii_case("READONLY"))
}

fn is_readonly_error_message(error: &str) -> bool {
  error.starts_with(&format!("{REDIS_SERVER_ERROR}: READONLY:"))
}

/// 连上配置里的那个库并确认真的可用（`PING` 要认证，口令错在这一步报出来）
pub async fn connect(target: RedisTarget) -> Result<RedisPool, String> {
  let connection = open(&target, target.database).await?;
  let mut connections = HashMap::new();
  connections.insert(target.database, connection);
  Ok(RedisPool { target, connections: tokio::sync::Mutex::new(connections) })
}

/// 问单个哨兵主节点现在在哪。哨兵常常不设口令（数据节点设了）：带着口令去会被它拒
/// （「AUTH … called without any password configured」），那就不带再问一次——不另设一格
/// 哨兵口令。重估条件：有人的哨兵口令与数据节点不同
async fn query_single_sentinel(
  target: &RedisTarget,
  host: &str,
  port: u16,
  master: &str,
) -> Result<Option<(String, u16)>, String> {
  let ask = |credentials: bool| async move {
    let client = target.client(host, port, 0, credentials)?;
    let mut connection =
      client.get_multiplexed_async_connection().await.map_err(describe_connect_error)?;
    redis::cmd("SENTINEL")
      .arg("get-master-addr-by-name")
      .arg(master)
      .query_async::<Option<(String, u16)>>(&mut connection)
      .await
      .map_err(describe_error)
  };
  match ask(true).await {
    Err(error) if error.contains("without any password configured") => ask(false).await,
    result => result,
  }
}

/// 按顺序向候选哨兵列表查询主节点地址（DC3 / G8）：任一哨兵宕机、超时或不认该主节点名时
/// 自动切换下一台；全部失败时优先保留明确的主节点名未知错误，否则报最后一次连接错误
async fn master_address(target: &RedisTarget, master: &str) -> Result<(String, u16), String> {
  let fallback = [(target.host.clone(), target.port)];
  let candidates: &[(String, u16)] =
    if target.sentinel_nodes.is_empty() { &fallback } else { &target.sentinel_nodes };
  let per_node_timeout = if candidates.len() > 1 { PER_SENTINEL_TIMEOUT } else { CONNECT_TIMEOUT };

  let mut last_error: Option<String> = None;
  let mut saw_unknown_master = false;

  for (host, port) in candidates {
    let query = query_single_sentinel(target, host, *port, master);
    match tokio::time::timeout(per_node_timeout, query).await {
      Ok(Ok(Some(address))) => return Ok(address),
      Ok(Ok(None)) => {
        saw_unknown_master = true;
      }
      Ok(Err(error)) => {
        log::warn!("redis sentinel {host}:{port} lookup failed: {error}");
        last_error = Some(error);
      }
      Err(_) => {
        let error = format!("{REDIS_UNREACHABLE}: {host}:{port} ({}s)", per_node_timeout.as_secs());
        log::warn!("redis sentinel {host}:{port} lookup timed out");
        last_error = Some(error);
      }
    }
  }

  if saw_unknown_master {
    return Err(format!("{REDIS_SENTINEL_MASTER_UNKNOWN}: {master}"));
  }
  Err(last_error.unwrap_or_else(|| format!("{REDIS_SENTINEL_MASTER_UNKNOWN}: {master}")))
}

async fn open(target: &RedisTarget, database: i64) -> Result<RedisConnection, String> {
  if target.tls != TlsMode::Disabled {
    crate::services::sqlx_pool::ensure_tls_provider();
  }
  if target.cluster {
    return open_cluster(target).await;
  }
  // 不设响应时限：每条命令外面各套一层查询超时（默认的 500 毫秒对大库上的 SCAN
  // 太短）。重试只留一次：连不上时要尽快说出来，不是在后台退避六轮
  let config = ConnectionManagerConfig::new()
    .set_connection_timeout(Some(CONNECT_TIMEOUT))
    .set_response_timeout(None)
    .set_number_of_retries(1);
  let attempt = async {
    // 每开一条连接都问一次：主从切换之后，「重新连接」就连到新的主节点上
    let client = match &target.sentinel_master {
      Some(master) => {
        let (host, port) = master_address(target, master).await?;
        target.client(&host, port, database, true)?
      }
      None => target.client(&target.host, target.port, database, true)?,
    };
    let mut connection =
      client.get_connection_manager_with_config(config).await.map_err(describe_connect_error)?;
    redis::cmd("PING")
      .query_async::<String>(&mut connection)
      .await
      .map_err(describe_connect_error)?;
    Ok::<_, String>(RedisConnection::Single(connection))
  };
  match tokio::time::timeout(CONNECT_TIMEOUT + Duration::from_secs(2), attempt).await {
    Ok(result) => result,
    Err(_) => Err(format!("{REDIS_UNREACHABLE}: {}s", CONNECT_TIMEOUT.as_secs())),
  }
}

/// 集群：从填的那个节点读出槽位分布，连上全部主节点（`PING` 发给每一个）。响应时限给到
/// 查询超时的上限（一小时），与单机一样由每条命令外面那层时限管
async fn open_cluster(target: &RedisTarget) -> Result<RedisConnection, String> {
  let seed = target.connection_info(&target.host, target.port, 0, true)?;
  let mut builder = redis::cluster::ClusterClientBuilder::new(vec![seed])
    .connection_timeout(CONNECT_TIMEOUT)
    .response_timeout(Duration::from_secs(3_600))
    .retries(1);
  if !target.username.is_empty() {
    builder = builder.username(&target.username);
  }
  if !target.password.is_empty() {
    builder = builder.password(&target.password);
  }
  if let Some(certificates) = target.root_certificate()? {
    builder = builder.certs(certificates);
  }
  let client = builder.build().map_err(describe_connect_error)?;
  let attempt = async {
    let mut connection = client.get_async_connection().await.map_err(describe_connect_error)?;
    redis::cmd("PING")
      .query_async::<String>(&mut connection)
      .await
      .map_err(describe_connect_error)?;
    Ok::<_, String>(RedisConnection::Cluster(connection))
  };
  match tokio::time::timeout(CONNECT_TIMEOUT + Duration::from_secs(2), attempt).await {
    Ok(result) => result,
    Err(_) => Err(format!("{REDIS_UNREACHABLE}: {}s", CONNECT_TIMEOUT.as_secs())),
  }
}

/// 连接阶段的错误：网络层的一律是「连不上」，认证与服务端的照常分
fn describe_connect_error(error: redis::RedisError) -> String {
  if error.is_io_error() || error.is_timeout() || error.is_connection_refusal() {
    return format!("{REDIS_UNREACHABLE}: {error}");
  }
  describe_error(error)
}

/// 驱动的错误 → 带码的一句话。认不出的原样给
pub fn describe_error(error: redis::RedisError) -> String {
  let detail = error.detail().unwrap_or_default().to_string();
  match error.code() {
    Some("NOAUTH") => return format!("{REDIS_AUTH_REQUIRED}: {detail}"),
    Some("WRONGPASS") => return format!("{REDIS_AUTH_FAILED}: {detail}"),
    Some("NOPERM") => return format!("{REDIS_NO_PERMISSION}: {detail}"),
    _ => {}
  }
  match error.kind() {
    redis::ErrorKind::AuthenticationFailed => format!("{REDIS_AUTH_FAILED}: {error}"),
    redis::ErrorKind::Server(_) | redis::ErrorKind::Extension => {
      format!("{REDIS_SERVER_ERROR}: {}: {detail}", error.code().unwrap_or("ERR"))
    }
    _ if error.is_timeout() => format!("{REDIS_TIMEOUT}: {error}"),
    _ => error.to_string(),
  }
}

async fn with_deadline<T>(
  timeout: Duration,
  work: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
  match tokio::time::timeout(timeout, work).await {
    Ok(result) => result,
    Err(_) => Err(format!("{REDIS_TIMEOUT}: {}ms", timeout.as_millis())),
  }
}

/// 一段字节串：`raw` 是原样的 base64，拿去定位；`text` 给人看——是 UTF-8 就原样，
/// 不是就按 redis-cli 的写法转义（`\xff`），并且 `binary` 为真，界面据此标出来。
/// 带控制字符（换行、回车、制表之外）的 UTF-8 也算：`\0` 印出来看不见，
/// 当文字编辑还会被输入框吞掉
#[derive(Debug, Serialize, PartialEq)]
pub struct RedisBytes {
  pub raw: String,
  pub text: String,
  pub binary: bool,
}

impl RedisBytes {
  fn new(bytes: Vec<u8>) -> Self {
    let raw = BASE64.encode(&bytes);
    match String::from_utf8(bytes) {
      Ok(text) if !text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) => {
        Self { raw, text, binary: false }
      }
      Ok(text) => Self { raw, text: escape_bytes(text.as_bytes()), binary: true },
      Err(error) => Self { raw, text: escape_bytes(error.as_bytes()), binary: true },
    }
  }
}

/// redis-cli 的写法：可打印的 ASCII 原样（反斜杠与双引号加转义），其余 `\xHH`
fn escape_bytes(bytes: &[u8]) -> String {
  let mut text = String::with_capacity(bytes.len());
  for &byte in bytes {
    match byte {
      b'\\' => text.push_str("\\\\"),
      b'"' => text.push_str("\\\""),
      0x20..=0x7e => text.push(char::from(byte)),
      _ => text.push_str(&format!("\\x{byte:02x}")),
    }
  }
  text
}

/// 前端传回来的键
pub fn decode_key(raw: &str) -> Result<Vec<u8>, String> {
  BASE64.decode(raw).map_err(|_| REDIS_KEY_INVALID.to_string())
}

/// 对象树的一行：一个逻辑库。形状与关系库的对象目录一致，`object_schema` 空着
#[derive(Debug, Serialize, PartialEq)]
pub struct KeyspaceEntry {
  pub object_schema: Option<String>,
  pub object_name: String,
  pub object_kind: &'static str,
  pub object_id: String,
  pub keys: u64,
}

/// 有键的库，加上配置里那个库（哪怕是空的）。数的是 `INFO keyspace`——它只列有键的库，
/// 也不用 `CONFIG GET databases`：那条在 ACL 里属于危险命令，只读用户多半没有
pub async fn list_keyspaces(pool: &RedisPool) -> Result<Vec<KeyspaceEntry>, String> {
  let mut connection = pool.connection(pool.default_database()).await?;
  // 集群只有 0 号库；`DBSIZE` 由驱动发给每个主节点再加起来
  if pool.target.cluster {
    let keys: u64 =
      redis::cmd("DBSIZE").query_async(&mut connection).await.map_err(describe_error)?;
    return Ok(vec![KeyspaceEntry {
      object_schema: None,
      object_name: "db0".to_string(),
      object_kind: "keyspace",
      object_id: "db0".to_string(),
      keys,
    }]);
  }
  let info: String = redis::cmd("INFO")
    .arg("keyspace")
    .query_async(&mut connection)
    .await
    .map_err(describe_error)?;
  let mut counts = parse_keyspace(&info);
  counts.entry(pool.default_database()).or_insert(0);
  let mut indexes: Vec<_> = counts.into_iter().collect();
  indexes.sort_unstable();
  Ok(
    indexes
      .into_iter()
      .map(|(index, keys)| KeyspaceEntry {
        object_schema: None,
        object_name: format!("db{index}"),
        object_kind: "keyspace",
        object_id: format!("db{index}"),
        keys,
      })
      .collect(),
  )
}

/// `db0:keys=12,expires=0,avg_ttl=0` 这样的行 → 库号与键数
fn parse_keyspace(info: &str) -> HashMap<i64, u64> {
  info
    .lines()
    .filter_map(|line| {
      let (name, fields) = line.trim().split_once(':')?;
      let index = name.strip_prefix("db")?.parse::<i64>().ok()?;
      let keys = fields.split(',').find_map(|field| field.strip_prefix("keys="))?.parse().ok()?;
      Some((index, keys))
    })
    .collect()
}

pub struct ScanRequest {
  pub database: i64,
  pub pattern: String,
  /// 上一页给的游标，第一页是 "0"
  pub cursor: String,
  /// 只要这一种类型（`SCAN … TYPE`）
  pub kind: Option<String>,
  /// 凑够这么多个就停；一次 `SCAN` 多给的不丢——丢了下一页就再也看不到它们
  pub page: usize,
  pub timeout: Duration,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyRow {
  pub key: RedisBytes,
  /// `TYPE` 的回答：string / hash / list / set / zset / stream，或模块类型的名字
  pub kind: String,
  /// 毫秒；-1 是不过期，-2 是这一刻已经没了
  pub ttl_ms: i64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ScanPage {
  pub keys: Vec<RedisKeyRow>,
  /// 下一页的游标；`None` 是扫完了。游标可以大过 2⁵³，所以是字符串
  pub cursor: Option<String>,
}

/// 按模式翻一页键，顺带每个键的类型与剩余时间
pub async fn scan(pool: &RedisPool, request: ScanRequest) -> Result<ScanPage, String> {
  let mut connection = pool.connection(request.database).await?;
  let started = Instant::now();
  let work = async {
    let mut keys: Vec<Vec<u8>> = Vec::new();
    let mut scanned: u64 = 0;
    let cursor = if pool.target.cluster {
      scan_cluster(&mut connection, &request, started, &mut keys, &mut scanned).await?
    } else {
      let cursor = request.cursor.parse().unwrap_or(0);
      let next =
        scan_node(&mut connection, None, cursor, &request, started, &mut keys, &mut scanned)
          .await?;
      (next != 0).then(|| next.to_string())
    };
    let rows = describe_keys(&connection, keys).await?;
    Ok(ScanPage { keys: rows, cursor })
  };
  with_deadline(request.timeout, work).await
}

/// 在一个节点上从 `cursor` 接着扫，凑够一页、这个节点扫完或过了一半时限就停，返回这个节点上的
/// 下一个游标（0 是扫完了）。`node` 是集群里那个主节点的地址；单机是 `None`
async fn scan_node(
  connection: &mut RedisConnection,
  node: Option<&(String, u16)>,
  mut cursor: u64,
  request: &ScanRequest,
  started: Instant,
  keys: &mut Vec<Vec<u8>>,
  scanned: &mut u64,
) -> Result<u64, String> {
  loop {
    let count = next_scan_count(request.page, keys.len(), *scanned);
    *scanned += count;
    let mut command = redis::cmd("SCAN");
    command.arg(cursor).arg("MATCH").arg(&request.pattern).arg("COUNT").arg(count);
    if let Some(kind) = &request.kind {
      command.arg("TYPE").arg(kind);
    }
    let (next, batch): (u64, Vec<Vec<u8>>) = match (&mut *connection, node) {
      // 集群上的 `SCAN` 没有键可以定槽位，驱动不知道发给谁：按地址点名
      (RedisConnection::Cluster(cluster), Some((host, port))) => {
        let routing = RoutingInfo::SingleNode(SingleNodeRoutingInfo::ByAddress {
          host: host.clone(),
          port: *port,
        });
        let reply = cluster.route_command(command, routing).await.map_err(describe_error)?;
        redis::from_redis_value(reply).map_err(|error| error.to_string())?
      }
      (connection, _) => command.query_async(connection).await.map_err(describe_error)?,
    };
    keys.extend(batch);
    cursor = next;
    // 大库里匹配得很稀时一页可能要扫很多轮；过了一半时限就先交出已有的，
    // 带着游标让人接着翻，而不是整页超时
    if cursor == 0 || keys.len() >= request.page || started.elapsed() > request.timeout / 2 {
      return Ok(cursor);
    }
  }
}

/// 集群：主节点按地址排好，一个扫完接下一个。游标是「第几个主节点:那个节点上的游标」。
/// 两页之间增删了主节点（扩缩容、切换）时序号会错开，那一轮可能漏或重——翻页本来就不是快照
async fn scan_cluster(
  connection: &mut RedisConnection,
  request: &ScanRequest,
  started: Instant,
  keys: &mut Vec<Vec<u8>>,
  scanned: &mut u64,
) -> Result<Option<String>, String> {
  let nodes: String = redis::cmd("CLUSTER")
    .arg("NODES")
    .query_async(&mut *connection)
    .await
    .map_err(describe_error)?;
  let masters = cluster_masters(&nodes);
  let (mut node, mut cursor) = split_cluster_cursor(&request.cursor);
  while let Some(address) = masters.get(node) {
    cursor = scan_node(connection, Some(address), cursor, request, started, keys, scanned).await?;
    if cursor == 0 {
      node += 1;
    }
    if keys.len() >= request.page || started.elapsed() > request.timeout / 2 {
      break;
    }
  }
  Ok((node < masters.len()).then(|| format!("{node}:{cursor}")))
}

/// `CLUSTER NODES` 里能用的主节点，按地址排序（翻页时序号要稳定）。每行是
/// `id ip:port@cport[,hostname] flags …`；副本、失联、还在握手的不算
fn cluster_masters(nodes: &str) -> Vec<(String, u16)> {
  let mut masters: Vec<(String, u16)> = nodes
    .lines()
    .filter_map(|line| {
      let mut fields = line.split_whitespace();
      let address = fields.nth(1)?;
      let flags: Vec<&str> = fields.next()?.split(',').collect();
      let usable = flags.contains(&"master")
        && !flags.iter().any(|flag| matches!(*flag, "fail" | "fail?" | "noaddr" | "handshake"));
      if !usable {
        return None;
      }
      let (host, port) = address.split('@').next()?.rsplit_once(':')?;
      let host = host.trim_start_matches('[').trim_end_matches(']');
      Some((host.to_string(), port.parse().ok()?))
    })
    .collect();
  masters.sort();
  masters
}

/// 第一页的 "0" 与认不出的都从第一个主节点的开头扫
fn split_cluster_cursor(cursor: &str) -> (usize, u64) {
  cursor
    .split_once(':')
    .and_then(|(node, cursor)| Some((node.parse().ok()?, cursor.parse().ok()?)))
    .unwrap_or((0, 0))
}

/// 每个键的类型与剩余时间。集群上一条流水线只能落在一个槽里，所以一个键一条（`TYPE` 加
/// `PTTL`），一起发出去——连接是多路复用的，单机上也还是差不多一个来回
async fn describe_keys(
  connection: &RedisConnection,
  keys: Vec<Vec<u8>>,
) -> Result<Vec<RedisKeyRow>, String> {
  let replies = futures_util::future::try_join_all(keys.iter().map(|key| {
    let mut connection = connection.clone();
    async move {
      redis::pipe()
        .cmd("TYPE")
        .arg(key)
        .cmd("PTTL")
        .arg(key)
        .query_async::<Vec<Value>>(&mut connection)
        .await
    }
  }))
  .await
  .map_err(describe_error)?;
  let mut replies = replies.into_iter().flatten();
  Ok(
    keys
      .into_iter()
      .map(|key| {
        let kind = match replies.next() {
          Some(Value::SimpleString(kind)) => kind,
          Some(Value::BulkString(kind)) => String::from_utf8_lossy(&kind).into_owned(),
          _ => "none".to_string(),
        };
        let ttl_ms = match replies.next() {
          Some(Value::Int(ttl)) => ttl,
          _ => -2,
        };
        RedisKeyRow { key: RedisBytes::new(key), kind, ttl_ms }
      })
      .collect(),
  )
}

pub struct RedisKeyspaceAnalysisRequest {
  pub database: i64,
  pub pattern: String,
  pub sample_limit: usize,
  pub timeout: Duration,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RedisSampledKey {
  pub key: RedisBytes,
  pub kind: String,
  pub ttl_ms: i64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub memory_bytes: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub length: Option<u64>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyspaceAnalysis {
  pub sampled_count: usize,
  pub partial: bool,
  pub cancelled: bool,
  pub memory_usage_available: bool,
  pub elapsed_ms: u64,
  pub keys: Vec<RedisSampledKey>,
}

/// 非阻塞抽样体检（AN3）：复用游标 `SCAN`（单机 / Cluster 均适用，严禁 `KEYS *`），
/// 遵循 AN0-E2 实测拍板：首键单独探测 1 次 `MEMORY USAGE` 成功才纳入 pipeline，
/// 禁用时立即平滑降级至元素数 / 字符串长度估算口径；全程支持协作取消与时间硬上限。
pub async fn analyze_keyspace(
  pool: &RedisPool,
  request: RedisKeyspaceAnalysisRequest,
  cancel: Arc<AtomicBool>,
) -> Result<RedisKeyspaceAnalysis, String> {
  let mut connection = pool.connection(request.database).await?;
  let sample_limit = request.sample_limit.clamp(1, MAX_REDIS_ANALYSIS_SAMPLE);
  let pattern =
    if request.pattern.trim().is_empty() { "*".to_string() } else { request.pattern.clone() };
  let started = Instant::now();

  let masters: Option<Vec<(String, u16)>> = if pool.target.cluster {
    let nodes: String = redis::cmd("CLUSTER")
      .arg("NODES")
      .query_async(&mut connection)
      .await
      .map_err(describe_error)?;
    Some(cluster_masters(&nodes))
  } else {
    None
  };

  let mut node_index = 0usize;
  let mut cursor: u64 = 0;
  let mut seen: HashSet<Vec<u8>> = HashSet::new();
  let mut sampled: Vec<RedisSampledKey> = Vec::new();
  let mut memory_usage_available: Option<bool> = None;
  let mut partial = false;
  let mut cancelled = false;

  loop {
    if cancel.load(Ordering::SeqCst) {
      cancelled = true;
      partial = true;
      break;
    }
    if started.elapsed() >= request.timeout {
      partial = true;
      break;
    }

    let current_node = match &masters {
      Some(list) => match list.get(node_index) {
        Some(addr) => Some(addr),
        None => break,
      },
      None => None,
    };

    let remaining_budget = request.timeout.saturating_sub(started.elapsed());
    if remaining_budget.is_zero() {
      partial = true;
      break;
    }

    let remaining_keys = sample_limit.saturating_sub(sampled.len());
    if remaining_keys == 0 {
      partial = true;
      break;
    }
    let want = remaining_keys.min(ANALYSIS_BATCH_SIZE);

    let mut command = redis::cmd("SCAN");
    command.arg(cursor).arg("MATCH").arg(&pattern).arg("COUNT").arg(want);

    let scan_step = async {
      let reply: (u64, Vec<Vec<u8>>) = match (&mut connection, current_node) {
        (RedisConnection::Cluster(cluster), Some((host, port))) => {
          let routing = RoutingInfo::SingleNode(SingleNodeRoutingInfo::ByAddress {
            host: host.clone(),
            port: *port,
          });
          let raw = cluster.route_command(command, routing).await.map_err(describe_error)?;
          redis::from_redis_value(raw).map_err(|error| error.to_string())?
        }
        (conn, _) => command.query_async(conn).await.map_err(describe_error)?,
      };
      Ok::<(u64, Vec<Vec<u8>>), String>(reply)
    };

    let (next_cursor, batch) = match tokio::time::timeout(remaining_budget, scan_step).await {
      Ok(Ok(res)) => res,
      Ok(Err(err)) => return Err(err),
      Err(_) => {
        partial = true;
        break;
      }
    };

    let mut fresh: Vec<Vec<u8>> = Vec::with_capacity(batch.len());
    for key in batch {
      if sampled.len() + fresh.len() >= sample_limit {
        partial = true;
        break;
      }
      if seen.insert(key.clone()) {
        fresh.push(key);
      }
    }

    if !fresh.is_empty() {
      // AN0-E2：首键单独探测一次 MEMORY USAGE，避免 ACL NOPERM 导致整批 pipeline 丢弃 TYPE/PTTL
      if memory_usage_available.is_none() {
        if let Some(first_key) = fresh.first() {
          let probe_rem = request.timeout.saturating_sub(started.elapsed());
          let mut probe_cmd = redis::cmd("MEMORY");
          probe_cmd.arg("USAGE").arg(first_key).arg("SAMPLES").arg(5);
          let probe_fut = probe_cmd.query_async::<Option<u64>>(&mut connection);
          memory_usage_available = match tokio::time::timeout(probe_rem, probe_fut).await {
            Ok(Ok(_)) => Some(true),
            Ok(Err(_)) => Some(false),
            Err(_) => {
              partial = true;
              break;
            }
          };
        }
      }

      let use_memory = memory_usage_available.unwrap_or(false);
      let inspect_rem = request.timeout.saturating_sub(started.elapsed());
      if inspect_rem.is_zero() {
        partial = true;
        break;
      }
      match tokio::time::timeout(inspect_rem, inspect_sampled_keys(&connection, fresh, use_memory))
        .await
      {
        Ok(Ok(rows)) => sampled.extend(rows),
        Ok(Err(err)) => return Err(err),
        Err(_) => {
          partial = true;
          break;
        }
      }
    }

    cursor = next_cursor;
    if cursor == 0 {
      if masters.is_some() {
        node_index += 1;
      } else {
        break;
      }
    }
    if sampled.len() >= sample_limit {
      partial = true;
      break;
    }
  }

  Ok(RedisKeyspaceAnalysis {
    sampled_count: sampled.len(),
    partial,
    cancelled,
    memory_usage_available: memory_usage_available.unwrap_or(false),
    elapsed_ms: started.elapsed().as_millis() as u64,
    keys: sampled,
  })
}

fn length_command_for_kind(kind: &str) -> Option<&'static str> {
  match kind {
    "string" => Some("STRLEN"),
    "hash" => Some("HLEN"),
    "list" => Some("LLEN"),
    "set" => Some("SCARD"),
    "zset" => Some("ZCARD"),
    "stream" => Some("XLEN"),
    _ => None,
  }
}

async fn inspect_sampled_keys(
  connection: &RedisConnection,
  keys: Vec<Vec<u8>>,
  include_memory: bool,
) -> Result<Vec<RedisSampledKey>, String> {
  let pass1 = futures_util::future::try_join_all(keys.iter().map(|key| {
    let mut connection = connection.clone();
    async move {
      let mut pipe = redis::pipe();
      pipe.cmd("TYPE").arg(key).cmd("PTTL").arg(key);
      if include_memory {
        pipe.cmd("MEMORY").arg("USAGE").arg(key).arg("SAMPLES").arg(5);
      }
      pipe.query_async::<Vec<Value>>(&mut connection).await
    }
  }))
  .await
  .map_err(describe_error)?;

  let mut base: Vec<(Vec<u8>, String, i64, Option<u64>)> = Vec::with_capacity(keys.len());
  for (key, reply_vec) in keys.into_iter().zip(pass1) {
    let kind = match reply_vec.first() {
      Some(Value::SimpleString(kind)) => kind.clone(),
      Some(Value::BulkString(kind)) => String::from_utf8_lossy(kind).into_owned(),
      _ => "none".to_string(),
    };
    let ttl_ms = match reply_vec.get(1) {
      Some(Value::Int(ttl)) => *ttl,
      _ => -2,
    };
    let memory_bytes = if include_memory {
      match reply_vec.get(2) {
        Some(Value::Int(bytes)) if *bytes >= 0 => Some(*bytes as u64),
        _ => None,
      }
    } else {
      None
    };
    base.push((key, kind, ttl_ms, memory_bytes));
  }

  let lengths = futures_util::future::try_join_all(base.iter().map(|(key, kind, _, _)| {
    let mut connection = connection.clone();
    let cmd_name = length_command_for_kind(kind);
    async move {
      let Some(cmd_name) = cmd_name else {
        return Ok::<Option<u64>, redis::RedisError>(None);
      };
      let reply: Value = redis::cmd(cmd_name).arg(key).query_async(&mut connection).await?;
      Ok(match reply {
        Value::Int(len) if len >= 0 => Some(len as u64),
        _ => None,
      })
    }
  }))
  .await
  .map_err(describe_error)?;

  Ok(
    base
      .into_iter()
      .zip(lengths)
      .map(|((key, kind, ttl_ms, memory_bytes), length)| RedisSampledKey {
        key: RedisBytes::new(key),
        kind,
        ttl_ms,
        memory_bytes,
        length,
      })
      .collect(),
  )
}

/// 一个键的值，按类型各一种形状；集合类的都是一页，`next` 是下一页从哪接（`None` 是完了）。
/// 翻页的位置对前端是不透明的字符串：散列与集合是 `*SCAN` 的游标，列表与有序集合是
/// 下标，流是上一页最后一条的 ID
#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RedisValue {
  String {
    size: u64,
    value: RedisBytes,
    truncated: bool,
  },
  Hash {
    length: u64,
    entries: Vec<(RedisBytes, RedisBytes)>,
    next: Option<String>,
  },
  List {
    length: u64,
    offset: u64,
    items: Vec<RedisBytes>,
    next: Option<String>,
  },
  Set {
    length: u64,
    members: Vec<RedisBytes>,
    next: Option<String>,
  },
  Zset {
    length: u64,
    offset: u64,
    entries: Vec<(RedisBytes, String)>,
    next: Option<String>,
  },
  Stream {
    length: u64,
    entries: Vec<StreamEntry>,
    next: Option<String>,
  },
  /// 模块类型（RedisJSON、布隆过滤器……）：说出类型名，不假装看得懂
  #[serde(rename_all = "camelCase")]
  Unsupported {
    redis_type: String,
  },
}

/// `XRANGE` 的一条：ID 与 `[f1, v1, …]`
type RawStreamEntry = (String, Vec<Vec<u8>>);

#[derive(Debug, Serialize, PartialEq)]
pub struct StreamEntry {
  pub id: String,
  pub fields: Vec<(RedisBytes, RedisBytes)>,
}

pub struct ValueRequest {
  pub database: i64,
  pub key: Vec<u8>,
  /// 上一页给的 `next`；第一页是 `None`
  pub position: Option<String>,
  pub page: u64,
  pub timeout: Duration,
}

pub async fn read_value(pool: &RedisPool, request: ValueRequest) -> Result<RedisValue, String> {
  let mut connection = pool.connection(request.database).await?;
  let work = async {
    let kind: String = redis::cmd("TYPE")
      .arg(&request.key)
      .query_async(&mut connection)
      .await
      .map_err(describe_error)?;
    let key = &request.key;
    let page = request.page.max(1);
    let position = request.position.as_deref();
    let offset: u64 = position.and_then(|position| position.parse().ok()).unwrap_or(0);
    let cursor = position.unwrap_or("0");
    let value = match kind.as_str() {
      "none" => return Err(REDIS_KEY_GONE.to_string()),
      "string" => {
        let (size, bytes): (u64, Vec<u8>) = redis::pipe()
          .cmd("STRLEN")
          .arg(key)
          .cmd("GETRANGE")
          .arg(key)
          .arg(0)
          .arg(STRING_PREVIEW_BYTES - 1)
          .query_async(&mut connection)
          .await
          .map_err(describe_error)?;
        let truncated = size > bytes.len() as u64;
        RedisValue::String { size, value: preview_bytes(bytes, truncated), truncated }
      }
      "hash" => {
        let (length, (next, flat)): (u64, (u64, Vec<Vec<u8>>)) = redis::pipe()
          .cmd("HLEN")
          .arg(key)
          .cmd("HSCAN")
          .arg(key)
          .arg(cursor)
          .arg("COUNT")
          .arg(page)
          .query_async(&mut connection)
          .await
          .map_err(describe_error)?;
        RedisValue::Hash {
          length,
          entries: pairs(flat),
          next: (next != 0).then(|| next.to_string()),
        }
      }
      "list" => {
        let end = offset + page - 1;
        let (length, items): (u64, Vec<Vec<u8>>) = redis::pipe()
          .cmd("LLEN")
          .arg(key)
          .cmd("LRANGE")
          .arg(key)
          .arg(offset)
          .arg(end)
          .query_async(&mut connection)
          .await
          .map_err(describe_error)?;
        RedisValue::List {
          length,
          offset,
          items: items.into_iter().map(RedisBytes::new).collect(),
          next: (end + 1 < length).then(|| (end + 1).to_string()),
        }
      }
      "set" => {
        let (length, (next, members)): (u64, (u64, Vec<Vec<u8>>)) = redis::pipe()
          .cmd("SCARD")
          .arg(key)
          .cmd("SSCAN")
          .arg(key)
          .arg(cursor)
          .arg("COUNT")
          .arg(page)
          .query_async(&mut connection)
          .await
          .map_err(describe_error)?;
        RedisValue::Set {
          length,
          members: members.into_iter().map(RedisBytes::new).collect(),
          next: (next != 0).then(|| next.to_string()),
        }
      }
      "zset" => {
        let end = offset + page - 1;
        let (length, flat): (u64, Vec<Vec<u8>>) = redis::pipe()
          .cmd("ZCARD")
          .arg(key)
          .cmd("ZRANGE")
          .arg(key)
          .arg(offset)
          .arg(end)
          .arg("WITHSCORES")
          .query_async(&mut connection)
          .await
          .map_err(describe_error)?;
        let entries = pairs(flat).into_iter().map(|(member, score)| (member, score.text)).collect();
        RedisValue::Zset {
          length,
          offset,
          entries,
          next: (end + 1 < length).then(|| (end + 1).to_string()),
        }
      }
      "stream" => {
        // 接着上一页最后一条往后读：`(` 是不含它自己（6.2 起）
        let start = position.map_or_else(|| "-".to_string(), |id| format!("({id}"));
        let (length, entries): (u64, Vec<RawStreamEntry>) = redis::pipe()
          .cmd("XLEN")
          .arg(key)
          .cmd("XRANGE")
          .arg(key)
          .arg(start)
          .arg("+")
          .arg("COUNT")
          .arg(page)
          .query_async(&mut connection)
          .await
          .map_err(describe_error)?;
        let next = (entries.len() as u64 == page)
          .then(|| entries.last().map(|(id, _)| id.clone()))
          .flatten();
        RedisValue::Stream {
          length,
          entries: entries
            .into_iter()
            .map(|(id, flat)| StreamEntry { id, fields: pairs(flat) })
            .collect(),
          next,
        }
      }
      other => RedisValue::Unsupported { redis_type: other.to_string() },
    };
    Ok(value)
  };
  with_deadline(request.timeout, work).await
}

/// 截断了的字符串值：截在一个多字节字符中间时，把那半个字符去掉再判断是不是 UTF-8——
/// 不然一段中文的前 512 KiB 会被当成二进制
fn preview_bytes(mut bytes: Vec<u8>, truncated: bool) -> RedisBytes {
  if truncated {
    if let Err(error) = std::str::from_utf8(&bytes) {
      if error.error_len().is_none() {
        bytes.truncate(error.valid_up_to());
      }
    }
  }
  RedisBytes::new(bytes)
}

/// `[f1, v1, f2, v2, …]` → `[(f1, v1), …]`
fn pairs(flat: Vec<Vec<u8>>) -> Vec<(RedisBytes, RedisBytes)> {
  let mut items = flat.into_iter();
  let mut pairs = Vec::new();
  while let (Some(first), Some(second)) = (items.next(), items.next()) {
    pairs.push((RedisBytes::new(first), RedisBytes::new(second)));
  }
  pairs
}

/// 改名的目标已经有了：不覆盖（`RENAMENX`）
pub const REDIS_KEY_EXISTS: &str = "DATAOMNI_REDIS_KEY_EXISTS";
/// 要改的值在打开之后被别处改过；不覆盖别人的改动
pub const REDIS_VALUE_CHANGED: &str = "DATAOMNI_REDIS_VALUE_CHANGED";

/// 界面上对一个键做的事。都是单个命令或一段先比对再写的脚本，发出去就生效
pub enum KeyChange {
  Delete,
  Rename {
    to: Vec<u8>,
  },
  /// `None` 是去掉过期（`PERSIST`）
  Expire {
    ttl_ms: Option<u64>,
  },
  /// 改字符串：服务端那份还是 `expected` 才写，剩余时间保留（`KEEPTTL`）
  SetString {
    expected: Vec<u8>,
    value: Vec<u8>,
  },
}

/// 比对后再写。多路复用的连接上没法 `WATCH`（它是整条连接的状态），而一段脚本在服务端
/// 是原子执行的：读、比、写之间插不进别的命令
const SET_STRING_IF_UNCHANGED: &str = r"
local current = redis.call('GET', KEYS[1])
if current == false then return -2 end
if current ~= ARGV[1] then return -1 end
redis.call('SET', KEYS[1], ARGV[2], 'KEEPTTL')
return 1
";

async fn execute_change_key(
  connection: &mut RedisConnection,
  key: &[u8],
  change: &KeyChange,
) -> Result<(), String> {
  let outcome: i64 = match change {
    KeyChange::Delete => {
      redis::cmd("UNLINK").arg(key).query_async(connection).await.map_err(describe_error)?
    }
    KeyChange::Rename { to } => {
      let renamed =
        redis::cmd("RENAMENX").arg(key).arg(to.as_slice()).query_async::<i64>(connection).await;
      match renamed {
        Ok(0) => return Err(format!("{REDIS_KEY_EXISTS}: {}", RedisBytes::new(to.clone()).text)),
        Ok(_) => 1,
        // 源键不在时服务端报 `ERR no such key`
        Err(error) if error.detail().is_some_and(|detail| detail.contains("no such key")) => 0,
        Err(error) => return Err(describe_error(error)),
      }
    }
    KeyChange::Expire { ttl_ms: Some(ttl) } => redis::cmd("PEXPIRE")
      .arg(key)
      .arg(*ttl)
      .query_async(connection)
      .await
      .map_err(describe_error)?,
    KeyChange::Expire { ttl_ms: None } => {
      // `PERSIST` 对「本来就不过期」也回 0，和「键不在」分不开，所以另问一次在不在
      let (_, exists): (i64, i64) = redis::pipe()
        .cmd("PERSIST")
        .arg(key)
        .cmd("EXISTS")
        .arg(key)
        .query_async(connection)
        .await
        .map_err(describe_error)?;
      exists
    }
    KeyChange::SetString { expected, value } => {
      let outcome: i64 = redis::Script::new(SET_STRING_IF_UNCHANGED)
        .key(key)
        .arg(expected.as_slice())
        .arg(value.as_slice())
        .invoke_async(connection)
        .await
        .map_err(describe_error)?;
      match outcome {
        -1 => return Err(REDIS_VALUE_CHANGED.to_string()),
        -2 => 0,
        _ => 1,
      }
    }
  };
  if outcome == 0 {
    return Err(REDIS_KEY_GONE.to_string());
  }
  Ok(())
}

pub async fn change_key(
  pool: &RedisPool,
  database: i64,
  key: Vec<u8>,
  change: KeyChange,
  timeout: Duration,
) -> Result<(), String> {
  let work = async {
    let mut connection = pool.connection(database).await?;
    match execute_change_key(&mut connection, &key, &change).await {
      Err(error) if pool.is_sentinel() && is_readonly_error_message(&error) => {
        let mut refreshed = pool.refresh_sentinel_connection(database).await?;
        execute_change_key(&mut refreshed, &key, &change).await
      }
      result => result,
    }
  };
  with_deadline(timeout, work).await
}

/// 要加的元素已经在了（hash 字段、set 成员、zset 成员）：不覆盖
pub const REDIS_ELEMENT_EXISTS: &str = "DATAOMNI_REDIS_ELEMENT_EXISTS";
/// 要改或删的元素已经不在了
pub const REDIS_ELEMENT_GONE: &str = "DATAOMNI_REDIS_ELEMENT_GONE";

/// 改一个值里面的一个元素。`expected` 是打开时看到的那份：服务端那份变了就不写。
/// 字节串都是原样的（hash 字段、成员可以是二进制）
pub enum ElementChange {
  /// `expected` 为 `None` 是新加一个字段（已经有了就不覆盖）
  HashSet {
    field: Vec<u8>,
    expected: Option<Vec<u8>>,
    value: Vec<u8>,
  },
  HashDelete {
    field: Vec<u8>,
  },
  ListSet {
    index: i64,
    expected: Vec<u8>,
    value: Vec<u8>,
  },
  ListDelete {
    index: i64,
    expected: Vec<u8>,
  },
  ListPush {
    value: Vec<u8>,
    head: bool,
  },
  SetAdd {
    member: Vec<u8>,
  },
  SetDelete {
    member: Vec<u8>,
  },
  /// `expected` 为 `None` 是新加一个成员；否则是改分数，服务端的分数得还是 `expected`
  ZsetSet {
    member: Vec<u8>,
    expected: Option<String>,
    score: String,
  },
  ZsetDelete {
    member: Vec<u8>,
  },
  /// 加一条，ID 由服务端按时间生成（`*`）；字段按给的次序
  StreamAdd {
    fields: Vec<(Vec<u8>, Vec<u8>)>,
  },
  StreamDelete {
    id: String,
  },
}

/// 每段脚本的回答：1 写了；-1 值变了；-2 键没了；-3 元素已经在；-4 元素没了。
/// 都先看键在不在：`HSET`、`SADD`、`ZADD` 对一个不在的键会建出一个新的——而人以为
/// 自己在改的那个键已经过期或被删了
const HASH_SET: &str = r"
if redis.call('EXISTS', KEYS[1]) == 0 then return -2 end
local current = redis.call('HGET', KEYS[1], ARGV[1])
if ARGV[3] == 'new' then
  if current then return -3 end
else
  if current == false then return -4 end
  if current ~= ARGV[4] then return -1 end
end
redis.call('HSET', KEYS[1], ARGV[1], ARGV[2])
return 1
";

/// `HDEL` / `SREM` / `ZREM` / `XDEL` 共用：命令名在 ARGV[1]
const MEMBER_DELETE: &str = r"
if redis.call('EXISTS', KEYS[1]) == 0 then return -2 end
if redis.call(ARGV[1], KEYS[1], ARGV[2]) == 0 then return -4 end
return 1
";

const LIST_SET: &str = r"
if redis.call('EXISTS', KEYS[1]) == 0 then return -2 end
local current = redis.call('LINDEX', KEYS[1], ARGV[1])
if current == false then return -4 end
if current ~= ARGV[2] then return -1 end
redis.call('LSET', KEYS[1], ARGV[1], ARGV[3])
return 1
";

/// 按下标删：Redis 没有这条命令。先比对，再把那一格换成一个一次性的记号，删掉这个记号——
/// 记号每次现造（ARGV[3]），不会和列表里真有的值撞上
const LIST_DELETE: &str = r"
if redis.call('EXISTS', KEYS[1]) == 0 then return -2 end
local current = redis.call('LINDEX', KEYS[1], ARGV[1])
if current == false then return -4 end
if current ~= ARGV[2] then return -1 end
redis.call('LSET', KEYS[1], ARGV[1], ARGV[3])
redis.call('LREM', KEYS[1], 1, ARGV[3])
return 1
";

const SET_ADD: &str = r"
if redis.call('EXISTS', KEYS[1]) == 0 then return -2 end
if redis.call('SADD', KEYS[1], ARGV[1]) == 0 then return -3 end
return 1
";

/// 字段与值依次在 ARGV 里。`XADD` 对不在的键同样会建出一个新的
const STREAM_ADD: &str = r"
if redis.call('EXISTS', KEYS[1]) == 0 then return -2 end
redis.call('XADD', KEYS[1], '*', unpack(ARGV))
return 1
";

const ZSET_SET: &str = r"
if redis.call('EXISTS', KEYS[1]) == 0 then return -2 end
local current = redis.call('ZSCORE', KEYS[1], ARGV[1])
if ARGV[3] == 'new' then
  if current then return -3 end
else
  if current == false then return -4 end
  if current ~= ARGV[4] then return -1 end
end
redis.call('ZADD', KEYS[1], ARGV[2], ARGV[1])
return 1
";

async fn execute_list_push(
  connection: &mut RedisConnection,
  command: &str,
  key: &[u8],
  value: &[u8],
) -> Result<(), String> {
  let length: i64 = redis::cmd(command)
    .arg(key)
    .arg(value)
    .query_async(connection)
    .await
    .map_err(describe_error)?;
  if length == 0 {
    return Err(REDIS_KEY_GONE.to_string());
  }
  Ok(())
}

async fn execute_element_script(
  connection: &mut RedisConnection,
  script: &str,
  key: &[u8],
  arguments: &[Vec<u8>],
) -> Result<(), String> {
  let script = redis::Script::new(script);
  let mut invocation = script.prepare_invoke();
  invocation.key(key);
  for argument in arguments {
    invocation.arg(argument.as_slice());
  }
  let outcome: i64 = invocation.invoke_async(connection).await.map_err(describe_error)?;
  script_outcome(outcome)
}

pub async fn change_element(
  pool: &RedisPool,
  database: i64,
  key: Vec<u8>,
  change: ElementChange,
  timeout: Duration,
) -> Result<(), String> {
  let mut connection = pool.connection(database).await?;
  let (script, arguments): (&str, Vec<Vec<u8>>) = match change {
    ElementChange::HashSet { field, expected: None, value } => {
      (HASH_SET, vec![field, value, b"new".to_vec()])
    }
    ElementChange::HashSet { field, expected: Some(expected), value } => {
      (HASH_SET, vec![field, value, b"edit".to_vec(), expected])
    }
    ElementChange::HashDelete { field } => (MEMBER_DELETE, vec![b"HDEL".to_vec(), field]),
    ElementChange::ListSet { index, expected, value } => {
      (LIST_SET, vec![index.to_string().into_bytes(), expected, value])
    }
    ElementChange::ListDelete { index, expected } => {
      let marker = format!("__dataomni_removed_{}__", uuid::Uuid::new_v4()).into_bytes();
      (LIST_DELETE, vec![index.to_string().into_bytes(), expected, marker])
    }
    ElementChange::ListPush { value, head } => {
      // `*PUSHX` 只往已经在的列表里推，列表没了回 0
      let command = if head { "LPUSHX" } else { "RPUSHX" };
      let work = async {
        match execute_list_push(&mut connection, command, &key, &value).await {
          Err(error) if pool.is_sentinel() && is_readonly_error_message(&error) => {
            let mut refreshed = pool.refresh_sentinel_connection(database).await?;
            execute_list_push(&mut refreshed, command, &key, &value).await
          }
          result => result,
        }
      };
      return with_deadline(timeout, work).await;
    }
    ElementChange::SetAdd { member } => (SET_ADD, vec![member]),
    ElementChange::SetDelete { member } => (MEMBER_DELETE, vec![b"SREM".to_vec(), member]),
    ElementChange::ZsetSet { member, expected: None, score } => {
      (ZSET_SET, vec![member, score.into_bytes(), b"new".to_vec()])
    }
    ElementChange::ZsetSet { member, expected: Some(expected), score } => {
      (ZSET_SET, vec![member, score.into_bytes(), b"edit".to_vec(), expected.into_bytes()])
    }
    ElementChange::ZsetDelete { member } => (MEMBER_DELETE, vec![b"ZREM".to_vec(), member]),
    ElementChange::StreamAdd { fields } => {
      (STREAM_ADD, fields.into_iter().flat_map(|(field, value)| [field, value]).collect())
    }
    ElementChange::StreamDelete { id } => (MEMBER_DELETE, vec![b"XDEL".to_vec(), id.into_bytes()]),
  };
  let work = async {
    match execute_element_script(&mut connection, script, &key, &arguments).await {
      Err(error) if pool.is_sentinel() && is_readonly_error_message(&error) => {
        let mut refreshed = pool.refresh_sentinel_connection(database).await?;
        execute_element_script(&mut refreshed, script, &key, &arguments).await
      }
      result => result,
    }
  };
  with_deadline(timeout, work).await
}

fn script_outcome(outcome: i64) -> Result<(), String> {
  match outcome {
    -1 => Err(REDIS_VALUE_CHANGED.to_string()),
    -2 => Err(REDIS_KEY_GONE.to_string()),
    -3 => Err(REDIS_ELEMENT_EXISTS.to_string()),
    -4 => Err(REDIS_ELEMENT_GONE.to_string()),
    _ => Ok(()),
  }
}

/// 新建一个键：给类型与第一个元素（Redis 没有空的 hash / list / set）。键名已经有了就不建
pub struct NewKey {
  pub kind: String,
  /// string 的值；hash 的字段；list / set 的元素；zset 的成员；stream 的字段
  pub first: Vec<u8>,
  /// hash 与 stream 的值；zset 的分数；其余不用
  pub second: Vec<u8>,
  pub ttl_ms: Option<u64>,
}

const CREATE_KEY: &str = r"
if redis.call('EXISTS', KEYS[1]) == 1 then return -3 end
local kind = ARGV[1]
if kind == 'string' then redis.call('SET', KEYS[1], ARGV[2])
elseif kind == 'hash' then redis.call('HSET', KEYS[1], ARGV[2], ARGV[3])
elseif kind == 'list' then redis.call('RPUSH', KEYS[1], ARGV[2])
elseif kind == 'set' then redis.call('SADD', KEYS[1], ARGV[2])
elseif kind == 'zset' then redis.call('ZADD', KEYS[1], ARGV[3], ARGV[2])
elseif kind == 'stream' then redis.call('XADD', KEYS[1], '*', ARGV[2], ARGV[3])
else return redis.error_reply('ERR unknown type ' .. kind) end
if ARGV[4] ~= '' then redis.call('PEXPIRE', KEYS[1], ARGV[4]) end
return 1
";

async fn execute_create_key(
  connection: &mut RedisConnection,
  key: &[u8],
  new_key: &NewKey,
  ttl_text: &str,
) -> Result<(), String> {
  let outcome: i64 = redis::Script::new(CREATE_KEY)
    .key(key)
    .arg(new_key.kind.as_str())
    .arg(new_key.first.as_slice())
    .arg(new_key.second.as_slice())
    .arg(ttl_text)
    .invoke_async(connection)
    .await
    .map_err(describe_error)?;
  if outcome == -3 {
    return Err(format!("{REDIS_KEY_EXISTS}: {}", RedisBytes::new(key.to_vec()).text));
  }
  Ok(())
}

pub async fn create_key(
  pool: &RedisPool,
  database: i64,
  key: Vec<u8>,
  new_key: NewKey,
  timeout: Duration,
) -> Result<(), String> {
  let ttl_text = new_key.ttl_ms.map(|ttl| ttl.to_string()).unwrap_or_default();
  let work = async {
    let mut connection = pool.connection(database).await?;
    match execute_create_key(&mut connection, &key, &new_key, &ttl_text).await {
      Err(error) if pool.is_sentinel() && is_readonly_error_message(&error) => {
        let mut refreshed = pool.refresh_sentinel_connection(database).await?;
        execute_create_key(&mut refreshed, &key, &new_key, &ttl_text).await
      }
      result => result,
    }
  };
  with_deadline(timeout, work).await
}

/// 命令行里不许跑：这条会阻塞，共用的连接上别的请求都得排在它后面。冒号后面是命令名
pub const REDIS_COMMAND_BLOCKING: &str = "DATAOMNI_REDIS_COMMAND_BLOCKING";
/// 命令行里不许跑：这条会改共用连接的状态（切库、事务、换身份）。冒号后面是命令名
pub const REDIS_COMMAND_CONNECTION_STATE: &str = "DATAOMNI_REDIS_COMMAND_CONNECTION_STATE";
/// 命令行里什么都没写
pub const REDIS_COMMAND_EMPTY: &str = "DATAOMNI_REDIS_COMMAND_EMPTY";

/// 为什么这条不能在这里跑。
///
/// 这里的连接是多路复用、各处共用的一条：会阻塞的命令（`BLPOP`、`MONITOR`、订阅）
/// 在它返回之前，对象树、键列表、值面板发出的每一条都排在它后面；改连接状态的
/// （`SELECT`、`MULTI`、`AUTH`、`CLIENT REPLY`）会让同一条连接上别处的命令落进
/// 另一个库、进了事务队列、或者换了身份。切库用对象树
fn refusal(name: &str, arguments: &[Vec<u8>]) -> Option<&'static str> {
  const BLOCKING: &[&str] = &[
    "BLPOP",
    "BRPOP",
    "BRPOPLPUSH",
    "BLMOVE",
    "BLMPOP",
    "BZPOPMIN",
    "BZPOPMAX",
    "BZMPOP",
    "WAIT",
    "WAITAOF",
    "MONITOR",
    "SUBSCRIBE",
    "PSUBSCRIBE",
    "SSUBSCRIBE",
    "UNSUBSCRIBE",
    "PUNSUBSCRIBE",
    "SUNSUBSCRIBE",
    "SYNC",
    "PSYNC",
  ];
  const CONNECTION_STATE: &[&str] =
    &["SELECT", "MULTI", "EXEC", "DISCARD", "WATCH", "UNWATCH", "AUTH", "HELLO", "RESET", "QUIT"];
  if BLOCKING.contains(&name) {
    return Some(REDIS_COMMAND_BLOCKING);
  }
  if CONNECTION_STATE.contains(&name) {
    return Some(REDIS_COMMAND_CONNECTION_STATE);
  }
  let has =
    |word: &str| arguments.iter().any(|argument| argument.eq_ignore_ascii_case(word.as_bytes()));
  // `XREAD` 本身不阻塞，带了 `BLOCK` 才阻塞
  if matches!(name, "XREAD" | "XREADGROUP") && has("BLOCK") {
    return Some(REDIS_COMMAND_BLOCKING);
  }
  let first = arguments.first().map(|argument| argument.to_ascii_uppercase());
  if name == "CLIENT"
    && matches!(
      first.as_deref(),
      Some(b"REPLY" | b"TRACKING" | b"CACHING" | b"NO-EVICT" | b"NO-TOUCH")
    )
  {
    return Some(REDIS_COMMAND_CONNECTION_STATE);
  }
  None
}

/// 一条回答，形状照服务端给的（RESP2 与 RESP3 的都认）。前端按 redis-cli 的样子画
#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RedisReply {
  Nil,
  /// 十进制文字：JSON 的数到了 JS 是双精度，超过 2^53 的会变
  Integer {
    value: String,
  },
  Bulk {
    value: RedisBytes,
  },
  Status {
    value: String,
  },
  Error {
    message: String,
  },
  Array {
    items: Vec<RedisReply>,
  },
  Map {
    entries: Vec<(RedisReply, RedisReply)>,
  },
  Double {
    value: f64,
  },
  Boolean {
    value: bool,
  },
  /// 超出 64 位的整数，原样的十进制文字
  BigNumber {
    value: String,
  },
}

impl RedisReply {
  fn from_value(value: Value) -> Self {
    match value {
      Value::Nil => Self::Nil,
      Value::Int(value) => Self::Integer { value: value.to_string() },
      Value::BulkString(bytes) => Self::Bulk { value: RedisBytes::new(bytes) },
      Value::SimpleString(value) => Self::Status { value },
      Value::Okay => Self::Status { value: "OK".to_string() },
      Value::Array(items) | Value::Set(items) | Value::Push { data: items, .. } => {
        Self::Array { items: items.into_iter().map(Self::from_value).collect() }
      }
      Value::Map(entries) => Self::Map {
        entries: entries
          .into_iter()
          .map(|(key, value)| (Self::from_value(key), Self::from_value(value)))
          .collect(),
      },
      Value::Attribute { data, .. } => Self::from_value(*data),
      Value::Double(value) => Self::Double { value },
      Value::Boolean(value) => Self::Boolean { value },
      Value::VerbatimString { text, .. } => {
        Self::Bulk { value: RedisBytes::new(text.into_bytes()) }
      }
      Value::BigNumber(digits) => {
        Self::BigNumber { value: String::from_utf8_lossy(&digits).into_owned() }
      }
      Value::ServerError(error) => Self::Error {
        message: format!("{} {}", error.code(), error.details().unwrap_or_default())
          .trim()
          .to_string(),
      },
      // `Value` 标了 non_exhaustive：驱动将来多一种形状时照原样说出来，不假装认识
      other => Self::Status { value: format!("{other:?}") },
    }
  }
}

/// 命令行：照原样发一条命令。参数是字节串（前端按 redis-cli 的规矩拆好、编成 base64），
/// 服务端的错误回答算作一条正常的回答（`(error) …`），不当作失败——那正是人要看的。
/// 网络断了、超时这类才是失败
pub async fn execute(
  pool: &RedisPool,
  database: i64,
  arguments: Vec<Vec<u8>>,
  timeout: Duration,
) -> Result<RedisReply, String> {
  let Some((name, rest)) = arguments.split_first() else {
    return Err(REDIS_COMMAND_EMPTY.to_string());
  };
  let name = String::from_utf8_lossy(name).to_ascii_uppercase();
  if let Some(reason) = refusal(&name, rest) {
    return Err(format!("{reason}: {name}"));
  }
  let mut command = redis::cmd(&name);
  for argument in rest {
    command.arg(argument);
  }
  let work = async {
    use redis::aio::ConnectionLike;
    let mut connection = pool.connection(database).await?;
    let outcome = match connection.req_packed_command(&command).await {
      Err(error) if pool.is_sentinel() && is_readonly_redis_error(&error) => {
        let mut refreshed = pool.refresh_sentinel_connection(database).await?;
        refreshed.req_packed_command(&command).await
      }
      Ok(ref value) if pool.is_sentinel() && is_readonly_value(value) => {
        let mut refreshed = pool.refresh_sentinel_connection(database).await?;
        refreshed.req_packed_command(&command).await
      }
      result => result,
    };
    match outcome {
      Ok(value) => Ok(RedisReply::from_value(value)),
      Err(error)
        if matches!(error.kind(), redis::ErrorKind::Server(_) | redis::ErrorKind::Extension) =>
      {
        Ok(RedisReply::Error {
          message: format!(
            "{} {}",
            error.code().unwrap_or("ERR"),
            error.detail().unwrap_or_default()
          ),
        })
      }
      Err(error) => Err(describe_error(error)),
    }
  };
  with_deadline(timeout, work).await
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::{
    REDIS_CLUSTER, REDIS_MODE_OPTION, REDIS_SENTINEL, REDIS_SENTINEL_MASTER_OPTION,
    REDIS_SENTINEL_NODES_OPTION,
  };

  fn cluster_profile(database: &str) -> ConnectionProfile {
    let mut profile = sentinel_profile(None);
    profile.options.insert(REDIS_MODE_OPTION.to_string(), REDIS_CLUSTER.to_string());
    profile.database = Some(database.to_string());
    profile
  }

  #[test]
  fn a_cluster_connection_has_its_own_key_and_only_database_zero() {
    let profile = cluster_profile("0");
    assert_eq!(
      profile.db_type.to_connection_string(&profile),
      "redis://@sentinel.example.com:26379/0?cluster"
    );
    assert!(RedisTarget::from_profile(&profile).expect("target").cluster);
    let error = RedisTarget::from_profile(&cluster_profile("3")).err().expect("db 3");
    assert_eq!(error, REDIS_CLUSTER_DATABASE);
  }

  #[test]
  fn a_cluster_connection_cannot_go_through_an_ssh_tunnel() {
    let mut profile = cluster_profile("0");
    profile.ssh_tunnel = Some(crate::models::SshTunnelConfig {
      host: "bastion".to_string(),
      port: 22,
      username: "ops".to_string(),
      private_key_path: String::new(),
      auth: Default::default(),
      secret: String::new(),
      secret_ref: None,
      remote_host: None,
      remote_port: None,
    });
    assert_eq!(
      RedisTarget::from_profile(&profile).err().as_deref(),
      Some(REDIS_DISCOVERY_VIA_TUNNEL)
    );
  }

  /// 只要能用的主节点：副本、失联的、还在握手的都不扫（副本上的键与主节点重复）
  #[test]
  fn the_masters_are_read_from_cluster_nodes_in_a_stable_order() {
    let nodes = "\
b 127.0.0.1:37002@47002 master - 0 1 2 connected 5461-10922
a 127.0.0.1:37001@47001,host-a myself,master - 0 0 1 connected 0-5460
c 127.0.0.1:37004@47004 slave b 0 1 2 connected
d 127.0.0.1:37005@47005 master,fail - 0 1 4 disconnected
e [::1]:37003@47003 master - 0 1 3 connected 10923-16383
";
    assert_eq!(
      cluster_masters(nodes),
      vec![
        ("127.0.0.1".to_string(), 37001),
        ("127.0.0.1".to_string(), 37002),
        ("::1".to_string(), 37003),
      ]
    );
  }

  /// 集群的游标是「第几个主节点:那个节点上的游标」；第一页照旧是 "0"
  #[test]
  fn a_cluster_cursor_names_the_node_and_its_own_cursor() {
    assert_eq!(split_cluster_cursor("0"), (0, 0));
    assert_eq!(split_cluster_cursor("2:1536"), (2, 1536));
    assert_eq!(split_cluster_cursor("x"), (0, 0));
  }

  fn sentinel_profile(master: Option<&str>) -> ConnectionProfile {
    let mut options = HashMap::new();
    if let Some(master) = master {
      options.insert(REDIS_MODE_OPTION.to_string(), REDIS_SENTINEL.to_string());
      options.insert(REDIS_SENTINEL_MASTER_OPTION.to_string(), master.to_string());
    }
    ConnectionProfile {
      db_type: crate::models::DatabaseType::Redis,
      host: "sentinel.example.com".to_string(),
      port: 26379,
      database: Some("2".to_string()),
      options,
      ..ConnectionProfile::default()
    }
  }

  /// 哨兵的地址与主节点的地址可以是同一个 host:port（单机上起的一套），也可以同一个哨兵
  /// 管着几组：主节点名不同就是不同的库，不共用池子
  #[test]
  fn a_sentinel_connection_is_keyed_by_its_master_name() {
    let key = |profile: ConnectionProfile| profile.db_type.to_connection_string(&profile);
    assert_eq!(key(sentinel_profile(None)), "redis://@sentinel.example.com:26379/2");
    assert_eq!(
      key(sentinel_profile(Some("mymaster"))),
      "redis://@sentinel.example.com:26379/2?sentinel=mymaster"
    );
    assert_eq!(
      key(sentinel_profile(Some("a b/c"))),
      "redis://@sentinel.example.com:26379/2?sentinel=a%20b%2Fc"
    );
  }

  #[test]
  fn a_sentinel_connection_cannot_go_through_an_ssh_tunnel() {
    let tunnel = crate::models::SshTunnelConfig {
      host: "bastion".to_string(),
      port: 22,
      username: "ops".to_string(),
      private_key_path: String::new(),
      auth: Default::default(),
      secret: String::new(),
      secret_ref: None,
      remote_host: None,
      remote_port: None,
    };
    let tunnelled =
      |master| ConnectionProfile { ssh_tunnel: Some(tunnel.clone()), ..sentinel_profile(master) };
    let error = RedisTarget::from_profile(&tunnelled(Some("mymaster"))).err().expect("refused");
    assert_eq!(error, REDIS_DISCOVERY_VIA_TUNNEL);
    assert!(RedisTarget::from_profile(&tunnelled(None)).is_ok(), "单机照常可以走隧道");
  }

  #[test]
  fn a_sentinel_profile_names_its_master_and_a_blank_name_is_refused() {
    let target = RedisTarget::from_profile(&sentinel_profile(Some(" mymaster "))).expect("target");
    assert_eq!(target.sentinel_master.as_deref(), Some("mymaster"));
    assert!(RedisTarget::from_profile(&sentinel_profile(None))
      .expect("plain")
      .sentinel_master
      .is_none());
    let error =
      RedisTarget::from_profile(&sentinel_profile(Some("  "))).err().expect("blank master");
    assert!(error.starts_with(REDIS_SENTINEL_MASTER_MISSING), "{error}");
  }

  #[test]
  fn the_scan_count_follows_how_densely_the_pattern_matches() {
    // 第一轮当作全都匹配：只要一页
    assert_eq!(next_scan_count(200, 0, 0), 200);
    // 全都匹配：还差多少要多少
    assert_eq!(next_scan_count(200, 150, 150), 50);
    // 四个里中三个：还差 50 个，看 66 个
    assert_eq!(next_scan_count(200, 150, 200), 66);
    // 十个里中一个：还差 190 个，看 1900 个槽位，封顶 1000
    assert_eq!(next_scan_count(200, 10, 200), 1_000);
    // 五个里中一个：还差 160 个，看 800 个
    assert_eq!(next_scan_count(200, 40, 200), 800);
    // 一个没中：直接上限
    assert_eq!(next_scan_count(200, 0, 200), 1_000);
    // 已经够了（循环会先停下）也至少给 1，不发 COUNT 0
    assert_eq!(next_scan_count(200, 200, 200), 1);
    // 页比上限还大：不压到页以下
    assert_eq!(next_scan_count(5_000, 0, 0), 5_000);
  }

  #[test]
  fn bytes_that_are_not_utf8_are_escaped_and_flagged() {
    let text = RedisBytes::new("user:中文".as_bytes().to_vec());
    assert_eq!((text.text.as_str(), text.binary), ("user:中文", false));
    let binary = RedisBytes::new(vec![b'k', 0xff, b'\\', b'"', b'\n']);
    assert_eq!((binary.text.as_str(), binary.binary), ("k\\xff\\\\\\\"\\x0a", true));
    assert_eq!(decode_key(&binary.raw), Ok(vec![b'k', 0xff, b'\\', b'"', b'\n']));
  }

  #[test]
  fn text_with_control_characters_is_escaped_too() {
    let nul = RedisBytes::new(b"a b\0c".to_vec());
    assert_eq!((nul.text.as_str(), nul.binary), ("a b\\x00c", true));
    // 多行的 JSON 很常见：换行、回车、制表仍是文字，由显示的那一侧转义
    let lines = RedisBytes::new(b"{\n\t\"a\": 1\r\n}".to_vec());
    assert_eq!((lines.text.as_str(), lines.binary), ("{\n\t\"a\": 1\r\n}", false));
  }

  #[test]
  fn a_string_cut_inside_a_character_is_still_text() {
    let mut bytes = "中文".as_bytes().to_vec();
    bytes.pop();
    let preview = preview_bytes(bytes.clone(), true);
    assert_eq!((preview.text.as_str(), preview.binary), ("中", false));
    // 反向：没截断时那半个字符是真的坏字节
    assert!(preview_bytes(bytes, false).binary);
  }

  #[test]
  fn the_keyspace_section_is_read_into_counts() {
    let info =
      "# Keyspace\r\ndb0:keys=12,expires=1,avg_ttl=0\r\ndb3:keys=5,expires=0,avg_ttl=0\r\n";
    let counts = parse_keyspace(info);
    assert_eq!(counts.get(&0), Some(&12));
    assert_eq!(counts.get(&3), Some(&5));
    assert_eq!(counts.len(), 2);
  }

  #[test]
  fn commands_that_block_or_change_the_shared_connection_are_refused() {
    assert_eq!(refusal("BLPOP", &[]), Some(REDIS_COMMAND_BLOCKING));
    assert_eq!(refusal("SELECT", &[b"3".to_vec()]), Some(REDIS_COMMAND_CONNECTION_STATE));
    assert_eq!(refusal("XREAD", &[b"block".to_vec(), b"0".to_vec()]), Some(REDIS_COMMAND_BLOCKING));
    assert_eq!(
      refusal("CLIENT", &[b"reply".to_vec(), b"off".to_vec()]),
      Some(REDIS_COMMAND_CONNECTION_STATE)
    );
    // 反向：不阻塞的同名兄弟照常跑
    assert_eq!(refusal("XREAD", &[b"COUNT".to_vec(), b"1".to_vec()]), None);
    assert_eq!(refusal("CLIENT", &[b"LIST".to_vec()]), None);
    assert_eq!(refusal("GET", &[b"k".to_vec()]), None);
  }

  #[test]
  fn the_database_field_is_a_non_negative_index() {
    assert_eq!(database_index(None), Ok(0));
    assert_eq!(database_index(Some(" ")), Ok(0));
    assert_eq!(database_index(Some("3")), Ok(3));
    assert!(database_index(Some("-1")).is_err());
    assert!(database_index(Some("abc")).is_err());
  }

  #[test]
  fn integers_reach_the_page_as_exact_digits() {
    // JSON 的数到了 JS 是双精度：超过 2^53 的 `INCR` 结果会差一（…993 变成 …992）
    let json = |value: i64| serde_json::to_value(RedisReply::from_value(Value::Int(value))).ok();
    assert_eq!(
      json(9_007_199_254_740_993),
      Some(serde_json::json!({ "kind": "integer", "value": "9007199254740993" }))
    );
    assert_eq!(
      json(i64::MIN),
      Some(serde_json::json!({ "kind": "integer", "value": "-9223372036854775808" }))
    );
  }

  fn read_mock_resp(reader: &mut std::io::BufReader<std::net::TcpStream>) -> Option<Vec<String>> {
    use std::io::{BufRead, Read};
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
      return None;
    }
    let count: usize = line.trim().strip_prefix('*')?.parse().ok()?;
    let mut args = Vec::with_capacity(count);
    for _ in 0..count {
      line.clear();
      if reader.read_line(&mut line).ok()? == 0 {
        return None;
      }
      let len: usize = line.trim().strip_prefix('$')?.parse().ok()?;
      let mut buf = vec![0u8; len + 2];
      reader.read_exact(&mut buf).ok()?;
      args.push(String::from_utf8_lossy(&buf[..len]).into_owned());
    }
    Some(args)
  }

  fn spawn_mock_redis_for_analysis(
    total_keys: usize,
    forbid_memory: bool,
  ) -> (u16, Arc<std::sync::atomic::AtomicUsize>, Arc<AtomicBool>) {
    use std::io::{BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let mem_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let mem_counter = Arc::clone(&mem_calls);
    let stop_flag = Arc::clone(&stop);

    std::thread::spawn(move || {
      while !stop_flag.load(Ordering::Relaxed) {
        let Ok((stream, _)) = listener.accept() else { break };
        let mem_c = Arc::clone(&mem_counter);
        let st = Arc::clone(&stop_flag);
        std::thread::spawn(move || {
          let mut writer = stream.try_clone().expect("clone");
          let mut reader = BufReader::new(stream);
          while !st.load(Ordering::Relaxed) {
            let Some(cmd) = read_mock_resp(&mut reader) else { break };
            let op = cmd.first().map(|s| s.to_ascii_uppercase()).unwrap_or_default();
            match op.as_str() {
              "PING" => {
                let _ = writer.write_all(b"+PONG\r\n");
              }
              "SCAN" => {
                let cursor: usize = cmd.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                let end = (cursor + 5).min(total_keys);
                let next = if end >= total_keys { 0 } else { end };
                let next_s = next.to_string();
                let mut out =
                  format!("*2\r\n${}\r\n{next_s}\r\n*{}\r\n", next_s.len(), end - cursor);
                for idx in cursor..end {
                  let k = if idx % 2 == 0 {
                    format!("session:u:{idx}")
                  } else {
                    format!("cache:item:{idx}")
                  };
                  out.push_str(&format!("${}\r\n{k}\r\n", k.len()));
                }
                let _ = writer.write_all(out.as_bytes());
              }
              "TYPE" => {
                let key = cmd.get(1).map(String::as_str).unwrap_or("");
                if key.starts_with("cache:") {
                  let _ = writer.write_all(b"+hash\r\n");
                } else {
                  let _ = writer.write_all(b"+string\r\n");
                }
              }
              "PTTL" => {
                let key = cmd.get(1).map(String::as_str).unwrap_or("");
                if key.starts_with("session:") {
                  let _ = writer.write_all(b":-1\r\n");
                } else {
                  let _ = writer.write_all(b":600000\r\n");
                }
              }
              "MEMORY" => {
                mem_c.fetch_add(1, Ordering::Relaxed);
                if forbid_memory {
                  let _ = writer.write_all(b"-NOPERM memory|usage disabled\r\n");
                } else {
                  let _ = writer.write_all(b":512\r\n");
                }
              }
              "STRLEN" => {
                let _ = writer.write_all(b":120\r\n");
              }
              "HLEN" => {
                let _ = writer.write_all(b":42\r\n");
              }
              _ => {
                let _ = writer.write_all(b"+OK\r\n");
              }
            }
          }
        });
      }
    });
    (port, mem_calls, stop)
  }

  #[tokio::test]
  async fn analyze_keyspace_handles_memory_usage_noperm_degradation_and_cancellation() {
    for forbid in [false, true] {
      let (port, mem_calls, stop) = spawn_mock_redis_for_analysis(12, forbid);
      let mut profile = sentinel_profile(None);
      profile.host = "127.0.0.1".to_string();
      profile.port = port;
      let target = RedisTarget::from_profile(&profile).expect("target");
      let pool = connect(target).await.expect("connect");

      let res = analyze_keyspace(
        &pool,
        RedisKeyspaceAnalysisRequest {
          database: 0,
          pattern: "*".to_string(),
          sample_limit: 50,
          timeout: Duration::from_secs(5),
        },
        Arc::new(AtomicBool::new(false)),
      )
      .await
      .expect("analyze");

      assert_eq!(res.sampled_count, 12);
      assert!(!res.partial);
      assert!(!res.cancelled);
      assert_eq!(res.memory_usage_available, !forbid);
      if forbid {
        assert_eq!(mem_calls.load(Ordering::Relaxed), 1, "must probe MEMORY USAGE only once");
        assert!(res.keys.iter().all(|k| k.memory_bytes.is_none()));
      } else {
        assert!(res.keys.iter().all(|k| k.memory_bytes == Some(512)));
      }
      assert!(res
        .keys
        .iter()
        .any(|k| k.kind == "string" && k.length == Some(120) && k.ttl_ms == -1));
      assert!(res
        .keys
        .iter()
        .any(|k| k.kind == "hash" && k.length == Some(42) && k.ttl_ms == 600_000));

      // Cooperative cancel before first batch returns immediately with cancelled=true
      let cancelled_flag = Arc::new(AtomicBool::new(true));
      let aborted = analyze_keyspace(
        &pool,
        RedisKeyspaceAnalysisRequest {
          database: 0,
          pattern: "*".to_string(),
          sample_limit: 50,
          timeout: Duration::from_secs(5),
        },
        cancelled_flag,
      )
      .await
      .expect("cancelled analyze");
      assert!(aborted.cancelled);
      assert!(aborted.partial);
      assert_eq!(aborted.sampled_count, 0);

      stop.store(true, Ordering::Relaxed);
    }
  }

  fn spawn_mock_sentinel(
    target_master_port: Arc<std::sync::atomic::AtomicU16>,
    expected_master_name: &'static str,
  ) -> (u16, Arc<AtomicBool>) {
    use std::io::{BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind sentinel");
    let port = listener.local_addr().expect("sentinel addr").port();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);

    std::thread::spawn(move || {
      while !stop_flag.load(Ordering::Relaxed) {
        let Ok((stream, _)) = listener.accept() else { break };
        let master_port = Arc::clone(&target_master_port);
        let st = Arc::clone(&stop_flag);
        std::thread::spawn(move || {
          let mut writer = stream.try_clone().expect("clone");
          let mut reader = BufReader::new(stream);
          while !st.load(Ordering::Relaxed) {
            let Some(cmd) = read_mock_resp(&mut reader) else { break };
            let op = cmd.first().map(|s| s.to_ascii_uppercase()).unwrap_or_default();
            match op.as_str() {
              "SENTINEL" => {
                let name = cmd.get(2).map(String::as_str).unwrap_or("");
                if name == expected_master_name {
                  let p = master_port.load(Ordering::SeqCst).to_string();
                  let reply = format!("*2\r\n$9\r\n127.0.0.1\r\n${}\r\n{p}\r\n", p.len());
                  let _ = writer.write_all(reply.as_bytes());
                } else {
                  let _ = writer.write_all(b"*-1\r\n");
                }
              }
              "PING" => {
                let _ = writer.write_all(b"+PONG\r\n");
              }
              _ => {
                let _ = writer.write_all(b"+OK\r\n");
              }
            }
          }
        });
      }
    });
    (port, stop)
  }

  fn spawn_mock_data_node(
    readonly: Arc<AtomicBool>,
  ) -> (u16, Arc<std::sync::atomic::AtomicUsize>, Arc<AtomicBool>) {
    use std::io::{BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind data node");
    let port = listener.local_addr().expect("data addr").port();
    let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let writes_counter = Arc::clone(&writes);
    let stop_flag = Arc::clone(&stop);

    std::thread::spawn(move || {
      while !stop_flag.load(Ordering::Relaxed) {
        let Ok((stream, _)) = listener.accept() else { break };
        let ro = Arc::clone(&readonly);
        let wc = Arc::clone(&writes_counter);
        let st = Arc::clone(&stop_flag);
        std::thread::spawn(move || {
          let mut writer = stream.try_clone().expect("clone");
          let mut reader = BufReader::new(stream);
          while !st.load(Ordering::Relaxed) {
            let Some(cmd) = read_mock_resp(&mut reader) else { break };
            let op = cmd.first().map(|s| s.to_ascii_uppercase()).unwrap_or_default();
            match op.as_str() {
              "PING" => {
                let _ = writer.write_all(b"+PONG\r\n");
              }
              "SET" | "UNLINK" => {
                wc.fetch_add(1, Ordering::SeqCst);
                if ro.load(Ordering::SeqCst) {
                  let _ =
                    writer.write_all(b"-READONLY You can't write against a read only replica.\r\n");
                } else if op == "UNLINK" {
                  let _ = writer.write_all(b":1\r\n");
                } else {
                  let _ = writer.write_all(b"+OK\r\n");
                }
              }
              _ => {
                let _ = writer.write_all(b"+OK\r\n");
              }
            }
          }
        });
      }
    });
    (port, writes, stop)
  }

  #[test]
  fn parse_sentinel_nodes_supports_lists_ipv6_defaults_dedup_and_rejects_invalid() {
    let nodes = parse_sentinel_nodes(
      "sentinel1.local",
      26379,
      Some("sentinel2.local:26380, sentinel3.local\n[::1]:26381;SENTINEL1.LOCAL:26379"),
    )
    .expect("parse nodes");
    assert_eq!(
      nodes,
      vec![
        ("sentinel1.local".to_string(), 26379),
        ("sentinel2.local".to_string(), 26380),
        ("sentinel3.local".to_string(), 26379),
        ("::1".to_string(), 26381),
      ]
    );

    assert!(parse_sentinel_nodes("127.0.0.1", 26379, Some("bad:99999"))
      .expect_err("bad port")
      .starts_with(REDIS_SENTINEL_NODES_INVALID));
    assert!(parse_sentinel_nodes("127.0.0.1", 26379, Some("bad:0"))
      .expect_err("zero port")
      .starts_with(REDIS_SENTINEL_NODES_INVALID));
    assert!(parse_sentinel_nodes("127.0.0.1", 26379, Some(":26379"))
      .expect_err("missing host")
      .starts_with(REDIS_SENTINEL_NODES_INVALID));
  }

  #[tokio::test]
  async fn sentinel_falls_back_to_secondary_node_when_primary_unreachable() {
    // 抢占并立即释放一个端口，确保主哨兵端口确定不可达（ECONNREFUSED）
    let dead_port = {
      let tmp = std::net::TcpListener::bind("127.0.0.1:0").expect("bind dead");
      tmp.local_addr().expect("addr").port()
    };
    let (data_port, _writes, stop_data) = spawn_mock_data_node(Arc::new(AtomicBool::new(false)));
    let master_port = Arc::new(std::sync::atomic::AtomicU16::new(data_port));
    let (backup_sentinel_port, stop_sentinel) = spawn_mock_sentinel(master_port, "mymaster");

    let mut profile = sentinel_profile(Some("mymaster"));
    profile.host = "127.0.0.1".to_string();
    profile.port = dead_port;
    profile
      .options
      .insert(REDIS_SENTINEL_NODES_OPTION.to_string(), format!("127.0.0.1:{backup_sentinel_port}"));

    let target = RedisTarget::from_profile(&profile).expect("target");
    let pool = connect(target).await.expect("fallback to secondary sentinel");
    let reply = execute(&pool, 0, vec![b"PING".to_vec()], Duration::from_secs(3))
      .await
      .expect("ping through master");
    assert!(matches!(reply, RedisReply::Status { value } if value == "PONG"));

    stop_sentinel.store(true, Ordering::Relaxed);
    stop_data.store(true, Ordering::Relaxed);
    let _ = std::net::TcpStream::connect(("127.0.0.1", backup_sentinel_port));
    let _ = std::net::TcpStream::connect(("127.0.0.1", data_port));
  }

  #[tokio::test]
  async fn sentinel_auto_refreshes_master_on_readonly_failover() {
    let old_readonly = Arc::new(AtomicBool::new(false));
    let (old_port, old_writes, stop_old) = spawn_mock_data_node(Arc::clone(&old_readonly));
    let (new_port, new_writes, stop_new) = spawn_mock_data_node(Arc::new(AtomicBool::new(false)));

    let master_port = Arc::new(std::sync::atomic::AtomicU16::new(old_port));
    let (sentinel_port, stop_sentinel) = spawn_mock_sentinel(Arc::clone(&master_port), "mymaster");

    let mut profile = sentinel_profile(Some("mymaster"));
    profile.host = "127.0.0.1".to_string();
    profile.port = sentinel_port;
    profile.database = Some("0".to_string());
    let target = RedisTarget::from_profile(&profile).expect("target");
    let pool = connect(target).await.expect("connect to old master");

    // 模拟主从切换：旧主降级为只读从库，哨兵切向新主
    old_readonly.store(true, Ordering::SeqCst);
    master_port.store(new_port, Ordering::SeqCst);

    // 1) `execute` 写命令撞上 READONLY 后自动切主并单次重试成功
    let reply = execute(
      &pool,
      0,
      vec![b"SET".to_vec(), b"k".to_vec(), b"v".to_vec()],
      Duration::from_secs(5),
    )
    .await
    .expect("execute after failover");
    assert!(matches!(reply, RedisReply::Status { value } if value == "OK"));
    assert_eq!(old_writes.load(Ordering::SeqCst), 1);
    assert_eq!(new_writes.load(Ordering::SeqCst), 1);

    // 2) 再次模拟切回另一轮 READONLY，验证 `change_key` 写路径同样自动切主重试
    let (third_port, third_writes, stop_third) =
      spawn_mock_data_node(Arc::new(AtomicBool::new(false)));
    let new_ro = Arc::new(AtomicBool::new(true));
    let (ro_port, ro_writes, stop_ro) = spawn_mock_data_node(new_ro);
    master_port.store(ro_port, Ordering::SeqCst);
    let _ = pool.refresh_sentinel_connection(0).await.expect("switch to ro node");
    master_port.store(third_port, Ordering::SeqCst);

    change_key(&pool, 0, b"k".to_vec(), KeyChange::Delete, Duration::from_secs(5))
      .await
      .expect("change_key after failover");
    assert_eq!(ro_writes.load(Ordering::SeqCst), 1);
    assert_eq!(third_writes.load(Ordering::SeqCst), 1);

    stop_sentinel.store(true, Ordering::Relaxed);
    stop_old.store(true, Ordering::Relaxed);
    stop_new.store(true, Ordering::Relaxed);
    stop_third.store(true, Ordering::Relaxed);
    stop_ro.store(true, Ordering::Relaxed);
    for p in [sentinel_port, old_port, new_port, third_port, ro_port] {
      let _ = std::net::TcpStream::connect(("127.0.0.1", p));
    }
  }

  #[tokio::test]
  async fn sentinel_readonly_retries_at_most_once_without_infinite_loop() {
    let always_ro = Arc::new(AtomicBool::new(true));
    let (ro_port, writes, stop_ro) = spawn_mock_data_node(always_ro);
    let master_port = Arc::new(std::sync::atomic::AtomicU16::new(ro_port));
    let (sentinel_port, stop_sentinel) = spawn_mock_sentinel(master_port, "mymaster");

    let mut profile = sentinel_profile(Some("mymaster"));
    profile.host = "127.0.0.1".to_string();
    profile.port = sentinel_port;
    profile.database = Some("0".to_string());
    let pool =
      connect(RedisTarget::from_profile(&profile).expect("target")).await.expect("connect");

    let reply = execute(
      &pool,
      0,
      vec![b"SET".to_vec(), b"k".to_vec(), b"v".to_vec()],
      Duration::from_secs(5),
    )
    .await
    .expect("server error reply");
    assert!(
      matches!(reply, RedisReply::Error { ref message } if message.contains("READONLY")),
      "expected READONLY reply, got {reply:?}"
    );
    assert_eq!(writes.load(Ordering::SeqCst), 2, "must attempt initial + at most 1 failover retry");

    stop_sentinel.store(true, Ordering::Relaxed);
    stop_ro.store(true, Ordering::Relaxed);
    let _ = std::net::TcpStream::connect(("127.0.0.1", sentinel_port));
    let _ = std::net::TcpStream::connect(("127.0.0.1", ro_port));
  }
}
