//! 调一次大模型：只有两种协议——Anthropic Messages 与 OpenAI 兼容的 Chat Completions。
//!
//! 国内常用的模型服务（DeepSeek、通义千问、智谱、Kimi）与本地的 Ollama 都给 OpenAI 兼容接口，
//! 所以两种就覆盖了（rfcs/editions-and-branching.md §6）。整个模块在 Cargo feature `ai` 后面：
//! 不开这个 feature 的构建里没有任何发往模型服务的代码。
//!
//! 这里只管「把一段系统提示和一段用户输入发出去、拿回文本」。发什么由前端的
//! `utils/aiDesign.ts` 决定，并原样展示给人看。

use crate::services::http_endpoint::error_chain;
use crate::services::QueryError;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::time::Duration;

pub const AI_REQUEST_FAILED: &str = "DATAOMNI_AI_REQUEST_FAILED";
pub const AI_BAD_RESPONSE: &str = "DATAOMNI_AI_BAD_RESPONSE";
pub const AI_CANCELLED: &str = "DATAOMNI_AI_CANCELLED";

/// 一份设计十来张表、上百列，输出要几千个 token；A0 实验里单次中位数 4.6 秒，
/// 慢的模型再翻几倍。超过两分钟多半是服务端卡住了
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Anthropic 要求显式给上限。一份设计远用不到这么多，给足是为了不在半截 JSON 处被截断
const MAX_OUTPUT_TOKENS: u32 = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProtocol {
  Anthropic,
  Openai,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequest {
  pub protocol: AiProtocol,
  /// Anthropic 写到域名（`https://api.anthropic.com`）；OpenAI 兼容写到 `/chat/completions`
  /// 之前那一段（`https://api.deepseek.com`、`http://localhost:11434/v1`），各家文档给的就是这一段
  pub base_url: String,
  pub model: String,
  pub system: String,
  pub user: String,
  /// 可选请求 ID，用于 `ai_cancel(requestId)` 中途取消正在进行的请求
  #[serde(default)]
  pub request_id: Option<String>,
  /// 可选配置档案 ID（省略或 `"default"` 读 `#ai`，其余如 `"local"` 读 `#ai:local`）
  #[serde(default)]
  pub profile_id: Option<String>,
  /// 可选输出上限（如连通性探测传 16）
  #[serde(default)]
  pub max_tokens: Option<u32>,
}

/// 判断端点是否指向本机回环地址（Ollama / LM Studio 等免 Key 本地模型服务）。
/// 严格按 URL Host 判定，拒绝 `localhost.evil.com` 等伪装域名。
pub fn is_local_ai_endpoint(base_url: &str) -> bool {
  let Ok(parsed) = url::Url::parse(base_url.trim()) else {
    return false;
  };
  if !matches!(parsed.scheme(), "http" | "https") {
    return false;
  }
  match parsed.host() {
    Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
    Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
    Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
    None => false,
  }
}

/// 规范化端点 Origin（`scheme://host[:port]`），用于将钥匙串中的 API Key 绑定至保存时的目标主机（H15 / S8）。
pub fn canonical_origin(base_url: &str) -> Result<String, String> {
  let parsed = validate_ai_endpoint_url(base_url)?;
  let scheme = parsed.scheme().to_ascii_lowercase();
  let host = match parsed.host() {
    Some(url::Host::Domain(domain)) => domain.to_ascii_lowercase(),
    Some(url::Host::Ipv4(ip)) => ip.to_string(),
    Some(url::Host::Ipv6(ip)) => format!("[{ip}]"),
    None => return Err(format!("{AI_REQUEST_FAILED}: AI endpoint URL has no host")),
  };
  match parsed.port() {
    Some(port) => Ok(format!("{scheme}://{host}:{port}")),
    None => Ok(format!("{scheme}://{host}")),
  }
}

/// 校验 AI 端点 URL：非本机回环地址强制要求 `https://`，并禁止 URL 内嵌明文凭据（H15 / S8）。
pub fn validate_ai_endpoint_url(base_url: &str) -> Result<url::Url, String> {
  let parsed = url::Url::parse(base_url.trim())
    .map_err(|error| format!("{AI_REQUEST_FAILED}: invalid AI endpoint URL: {error}"))?;
  if !matches!(parsed.scheme(), "http" | "https") {
    return Err(format!("{AI_REQUEST_FAILED}: AI endpoint must use https://"));
  }
  if parsed.host().is_none() {
    return Err(format!("{AI_REQUEST_FAILED}: AI endpoint URL has no host"));
  }
  if !parsed.username().is_empty() || parsed.password().is_some() {
    return Err(format!(
      "{AI_REQUEST_FAILED}: AI endpoint URL must not embed username or password"
    ));
  }
  if parsed.scheme() != "https" && !is_local_ai_endpoint(base_url) {
    return Err(format!(
      "{AI_REQUEST_FAILED}: non-local AI endpoint must use https:// to protect API keys"
    ));
  }
  Ok(parsed)
}

const BOUND_KEY_PREFIX: &str = "v2:";

/// 将 API Key 与其保存时的端点 Origin 编码存储到钥匙串，防止渲染层改写 `base_url` 将钥匙串 Key 外泄到任意主机（H15 / S8）。
pub fn encode_bound_api_key(key: &str, base_url: Option<&str>) -> Result<String, String> {
  let trimmed = key.trim();
  let Some(url) = base_url.map(str::trim).filter(|url| !url.is_empty()) else {
    return Ok(trimmed.to_string());
  };
  let origin = canonical_origin(url)?;
  Ok(format!("{BOUND_KEY_PREFIX}{origin}\n{trimmed}"))
}

/// 解包钥匙串中的 API Key 并核验是否与当前 `base_url` 的 Origin 一致；无损兼容旧版未绑定格式的 Key。
pub fn decode_bound_api_key(stored: &str, base_url: &str) -> Result<String, String> {
  let request_origin = canonical_origin(base_url)?;
  if let Some(rest) = stored.strip_prefix(BOUND_KEY_PREFIX) {
    let Some((bound_origin, secret)) = rest.split_once('\n') else {
      return Err(format!("{AI_REQUEST_FAILED}: stored API key binding is malformed"));
    };
    if bound_origin != request_origin {
      return Err(format!(
        "{AI_REQUEST_FAILED}: saved API key is bound to {bound_origin} and cannot be sent to {request_origin}"
      ));
    }
    return Ok(secret.to_string());
  }
  Ok(stored.to_string())
}

pub async fn complete(request: &AiRequest, api_key: &str) -> Result<String, QueryError> {
  validate_ai_endpoint_url(&request.base_url).map_err(QueryError::message)?;
  // 生产环境不关代理：模型服务在公网上，很多人正是靠代理才连得上；
  // 但指向本机回环地址（Ollama / LM Studio）时绕过系统代理，避免被代理黑洞；
  // 显式禁用 HTTP 重定向，防止 302 将 Authorization / x-api-key 头转发到其他主机（H15 / S8）。
  let mut builder =
    reqwest::Client::builder().timeout(REQUEST_TIMEOUT).redirect(reqwest::redirect::Policy::none());
  if is_local_ai_endpoint(&request.base_url) {
    builder = builder.no_proxy();
  }
  let client = builder.build().map_err(|error| {
    QueryError::message(format!("{AI_REQUEST_FAILED}: {}", error_chain(&error)))
  })?;
  complete_with(&client, request, api_key).await
}

pub async fn complete_with(
  client: &reqwest::Client,
  request: &AiRequest,
  api_key: &str,
) -> Result<String, QueryError> {
  validate_ai_endpoint_url(&request.base_url).map_err(QueryError::message)?;
  let base = request.base_url.trim().trim_end_matches('/');
  let key = api_key.trim();
  let (builder, body) = match request.protocol {
    AiProtocol::Anthropic => {
      let mut req =
        client.post(format!("{base}/v1/messages")).header("anthropic-version", "2023-06-01");
      if !key.is_empty() {
        req = req.header("x-api-key", key);
      }
      (
        req,
        json!({
          "model": request.model,
          "max_tokens": request.max_tokens.unwrap_or(MAX_OUTPUT_TOKENS),
          "system": request.system,
          "messages": [{ "role": "user", "content": request.user }],
        }),
      )
    }
    // 不发 `response_format`：OpenAI 兼容的各家不是都认，不认的直接 400。
    // 提示里已经要求只输出 JSON，读的那一侧也容忍一层代码块
    AiProtocol::Openai => {
      let mut req = client.post(format!("{base}/chat/completions"));
      if !key.is_empty() {
        req = req.bearer_auth(key);
      }
      let mut payload = json!({
        "model": request.model,
        "messages": [
          { "role": "system", "content": request.system },
          { "role": "user", "content": request.user },
        ],
      });
      if let Some(max_tokens) = request.max_tokens {
        payload["max_tokens"] = json!(max_tokens);
      }
      (req, payload)
    }
  };

  // reqwest 没开 `json` feature（别处用不上），请求体手写序列化
  let response = builder
    .header("content-type", "application/json")
    .body(body.to_string())
    .send()
    .await
    .map_err(|error| {
      QueryError::message(format!("{AI_REQUEST_FAILED}: {}", error_chain(&error)))
    })?;
  let status = response.status();
  let body = response.text().await.map_err(|error| {
    QueryError::message(format!("{AI_REQUEST_FAILED}: {}", error_chain(&error)))
  })?;
  if !status.is_success() {
    // 服务端的原话最有用（Key 错、模型名错、余额不足），截一段带上
    return Err(QueryError::message(format!(
      "{AI_REQUEST_FAILED}: HTTP {} · {}",
      status.as_u16(),
      body.chars().take(500).collect::<String>()
    )));
  }
  let value: JsonValue = serde_json::from_str(&body)
    .map_err(|error| QueryError::message(format!("{AI_BAD_RESPONSE}: {error}")))?;
  response_text(request.protocol, &value).ok_or_else(|| {
    QueryError::message(format!(
      "{AI_BAD_RESPONSE}: {}",
      body.chars().take(500).collect::<String>()
    ))
  })
}

/// 两家回复里放文本的位置。Anthropic 的 `content` 是分块的数组，只取 text 块拼起来
fn response_text(protocol: AiProtocol, value: &JsonValue) -> Option<String> {
  match protocol {
    AiProtocol::Anthropic => {
      let blocks = value.get("content")?.as_array()?;
      let text = blocks
        .iter()
        .filter(|block| block.get("type").and_then(JsonValue::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(JsonValue::as_str))
        .collect::<String>();
      (!text.is_empty()).then_some(text)
    }
    AiProtocol::Openai => {
      let text = value.get("choices")?.get(0)?.get("message")?.get("content")?.as_str()?;
      (!text.trim().is_empty()).then(|| text.to_string())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::{Read, Write};
  use std::net::TcpListener;

  /// 起一个只回一次的 HTTP 服务，返回地址和「收到的请求原文」
  fn serve_once(status: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = format!("http://{}", listener.local_addr().expect("addr"));
    let response =
      format!("HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    let handle = std::thread::spawn(move || {
      let (mut stream, _) = listener.accept().expect("accept");
      let mut received = Vec::new();
      let mut buffer = [0u8; 4096];
      // 读到请求体收齐为止：头里有 content-length
      loop {
        let read = stream.read(&mut buffer).expect("read");
        received.extend_from_slice(&buffer[..read]);
        let text = String::from_utf8_lossy(&received).to_string();
        if let Some((head, rest)) = text.split_once("\r\n\r\n") {
          let length = head
            .lines()
            .find_map(|line| {
              line.to_ascii_lowercase().strip_prefix("content-length: ").map(str::to_string)
            })
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
          if rest.len() >= length {
            break;
          }
        }
        if read == 0 {
          break;
        }
      }
      stream.write_all(response.as_bytes()).expect("write");
      String::from_utf8_lossy(&received).to_string()
    });
    (address, handle)
  }

  fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().expect("test client")
  }

  fn request(protocol: AiProtocol, base_url: String) -> AiRequest {
    AiRequest {
      protocol,
      base_url,
      model: "m-1".into(),
      system: "只输出 JSON".into(),
      user: "博客".into(),
      request_id: None,
      profile_id: None,
      max_tokens: None,
    }
  }

  #[test]
  fn detects_only_true_loopback_endpoints_as_local() {
    assert!(is_local_ai_endpoint("http://localhost:11434/v1"));
    assert!(is_local_ai_endpoint("http://127.0.0.1:1234/v1"));
    assert!(is_local_ai_endpoint("http://[::1]:11434/v1"));
    assert!(!is_local_ai_endpoint("https://api.openai.com/v1"));
    assert!(!is_local_ai_endpoint("http://localhost.evil.com/v1"));
    assert!(!is_local_ai_endpoint("http://192.168.1.50:11434/v1"));
    assert!(!is_local_ai_endpoint("not a url"));
  }

  #[tokio::test]
  async fn keyless_local_request_omits_auth_header_and_forwards_max_tokens() {
    let (base, server) =
      serve_once("200 OK", r#"{"choices":[{"message":{"role":"assistant","content":"OK"}}]}"#);
    let mut req = request(AiProtocol::Openai, format!("{base}/v1"));
    req.max_tokens = Some(16);
    let text = complete_with(&client(), &req, "").await.expect("complete keyless local");
    assert_eq!(text, "OK");
    let received = server.join().expect("server").to_ascii_lowercase();
    assert!(!received.contains("authorization:"), "{received}");
    assert!(received.contains(r#""max_tokens":16"#), "{received}");
  }

  /// reqwest 的 Display 只有「error sending request」，看不出是拒连、证书还是 DNS
  #[tokio::test]
  async fn unreachable_service_names_the_cause() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    drop(listener);
    let error = complete_with(&client(), &request(AiProtocol::Openai, base), "sk-test")
      .await
      .expect_err("nothing listens there");
    assert!(error.message.contains(AI_REQUEST_FAILED), "{}", error.message);
    // Windows 的系统报错跟着界面语言走（中文是「积极拒绝」），只有错误码不变
    let message = error.message.to_lowercase();
    assert!(message.contains("refused") || message.contains("os error 10061"), "{}", error.message);
  }

  #[tokio::test]
  async fn openai_compatible_sends_bearer_and_reads_the_first_choice() {
    let (base, server) = serve_once(
      "200 OK",
      r#"{"choices":[{"message":{"role":"assistant","content":"{\"tables\":[]}"}}]}"#,
    );
    let text =
      complete_with(&client(), &request(AiProtocol::Openai, format!("{base}/v1/")), "sk-test")
        .await
        .expect("complete");
    assert_eq!(text, r#"{"tables":[]}"#);
    let received = server.join().expect("server");
    assert!(received.starts_with("POST /v1/chat/completions "), "{received}");
    assert!(received.to_ascii_lowercase().contains("authorization: bearer sk-test"));
    assert!(received.contains(r#""role":"system","content":"只输出 JSON""#), "{received}");
    assert!(!received.contains("response_format"), "不是每家都认 response_format");
  }

  #[tokio::test]
  async fn anthropic_sends_its_headers_and_joins_the_text_blocks() {
    let (base, server) = serve_once(
      "200 OK",
      r#"{"content":[{"type":"text","text":"{\"tables\":"},{"type":"thinking","thinking":"x"},{"type":"text","text":"[]}"}]}"#,
    );
    let text = complete_with(&client(), &request(AiProtocol::Anthropic, base), "sk-ant")
      .await
      .expect("complete");
    assert_eq!(text, r#"{"tables":[]}"#);
    let received = server.join().expect("server").to_ascii_lowercase();
    assert!(received.starts_with("post /v1/messages "), "{received}");
    assert!(received.contains("x-api-key: sk-ant"));
    assert!(received.contains("anthropic-version: 2023-06-01"));
    assert!(received.contains(r#""max_tokens":8192"#));
  }

  #[tokio::test]
  async fn a_server_error_carries_the_status_and_what_the_server_said() {
    let (base, server) =
      serve_once("401 Unauthorized", r#"{"error":{"message":"Invalid API key"}}"#);
    let error = complete_with(&client(), &request(AiProtocol::Openai, base), "bad")
      .await
      .expect_err("should fail");
    server.join().expect("server");
    assert!(error.message.starts_with(AI_REQUEST_FAILED), "{}", error.message);
    assert!(error.message.contains("HTTP 401"));
    assert!(error.message.contains("Invalid API key"));
  }

  #[tokio::test]
  async fn a_reply_without_text_is_a_bad_response_not_an_empty_design() {
    let (base, server) = serve_once("200 OK", r#"{"choices":[]}"#);
    let error = complete_with(&client(), &request(AiProtocol::Openai, base), "k")
      .await
      .expect_err("should fail");
    server.join().expect("server");
    assert!(error.message.starts_with(AI_BAD_RESPONSE), "{}", error.message);

    // 思考/推理模型耗尽 max_tokens 返回空 content 时同样不应静默当成成功
    let (base_empty, server_empty) = serve_once(
      "200 OK",
      r#"{"choices":[{"finish_reason":"length","message":{"role":"assistant","content":""}}]}"#,
    );
    let empty_err = complete_with(&client(), &request(AiProtocol::Openai, base_empty), "k")
      .await
      .expect_err("empty content must fail");
    server_empty.join().expect("server");
    assert!(empty_err.message.starts_with(AI_BAD_RESPONSE), "{}", empty_err.message);
    assert!(empty_err.message.contains("finish_reason"), "{}", empty_err.message);
  }

  /// H15（S8）：非本机回环端点必须强制 `https://`，禁止内嵌凭据；已保存 Key 严格绑定保存时的 Origin
  #[test]
  fn validates_https_for_non_local_endpoints_and_enforces_origin_bound_keys() {
    for ok in [
      "https://api.openai.com/v1",
      "https://api.anthropic.com",
      "http://localhost:11434/v1",
      "http://127.0.0.1:11434/v1",
      "http://[::1]:11434/v1",
    ] {
      assert!(validate_ai_endpoint_url(ok).is_ok(), "should allow {ok}");
    }

    for bad in [
      "http://api.openai.com/v1",
      "http://169.254.169.254/latest/meta-data",
      "http://10.0.0.1:8080/v1",
      "http://localhost.evil.com/v1",
      "file:///etc/passwd",
      "https://user:pass@api.openai.com/v1",
    ] {
      assert!(validate_ai_endpoint_url(bad).is_err(), "should reject {bad}");
    }

    let encoded =
      encode_bound_api_key("sk-secret-1", Some("https://api.openai.com/v1")).expect("encode");
    assert_eq!(
      decode_bound_api_key(&encoded, "https://API.OPENAI.COM/v1/chat").expect("same origin"),
      "sk-secret-1"
    );
    assert!(
      decode_bound_api_key(&encoded, "https://evil.example.com/v1").is_err(),
      "different host must be refused"
    );
    assert!(
      decode_bound_api_key(&encoded, "https://api.openai.com:8443/v1").is_err(),
      "different port must be refused"
    );
    // 兼容旧版未绑定 Key
    assert_eq!(
      decode_bound_api_key("sk-legacy", "https://api.openai.com/v1").expect("legacy"),
      "sk-legacy"
    );
  }
}
