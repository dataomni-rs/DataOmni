//! AI 设计用到的命令。
//!
//! 命令本身在所有构建里都注册，没开 `ai` feature 时一律返回 [`AI_NOT_IN_BUILD`]——
//! 发往模型服务的代码（`services::ai`）不在那种构建的二进制里。前端先问
//! [`ai_available`]，没有就不显示任何 AI 入口。

use crate::services::QueryError;
use std::collections::HashMap;
use std::sync::Mutex;

/// 这个构建没有编进 AI（`--no-default-features` 的内网构建）
#[cfg(not(feature = "ai"))]
pub const AI_NOT_IN_BUILD: &str = "DATAOMNI_AI_NOT_IN_BUILD";

/// 正在进行的 AI 请求句柄表，用于 `ai_cancel(requestId)` 中途取消，避免卡死等待 120 秒
#[derive(Default)]
pub struct AiRequestRegistry(Mutex<HashMap<String, tokio::task::AbortHandle>>);

impl AiRequestRegistry {
  #[cfg(feature = "ai")]
  fn register(&self, request_id: String, handle: tokio::task::AbortHandle) {
    let mut guard = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(previous) = guard.insert(request_id, handle) {
      previous.abort();
    }
  }

  #[cfg(feature = "ai")]
  fn remove(&self, request_id: &str) {
    let mut guard = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.remove(request_id);
  }

  pub fn cancel(&self, request_id: &str) -> bool {
    let mut guard = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match guard.remove(request_id) {
      Some(handle) => {
        handle.abort();
        true
      }
      None => false,
    }
  }
}

/// Key 在钥匙串里的默认账户名。同一个服务名下，`#` 开头不会和连接的 uuid 撞上（`#ssh` 同理）。
#[cfg(feature = "ai")]
const AI_KEY_ACCOUNT: &str = "#ai";

/// 按配置档案映射到钥匙串账户名：默认档案继续用 `#ai`（无损兼容既有 Key），额外档案用 `#ai:<id>`
#[cfg(feature = "ai")]
fn ai_key_account(profile_id: Option<&str>) -> Result<String, String> {
  let Some(raw) = profile_id.map(str::trim).filter(|s| !s.is_empty() && *s != "default") else {
    return Ok(AI_KEY_ACCOUNT.to_string());
  };
  if raw.len() > 64 || !raw.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')) {
    return Err(format!("DATAOMNI_AI_REQUEST_FAILED: invalid profile id {raw}"));
  }
  Ok(format!("{AI_KEY_ACCOUNT}:{raw}"))
}

/// 钥匙串里没有 Key：还没在设置里填。连接那句 `CREDENTIAL_MISSING` 说的是「这个连接的密码」，
/// 放在这里指错了地方
#[cfg(feature = "ai")]
pub const AI_KEY_MISSING: &str = "DATAOMNI_AI_KEY_MISSING";
/// Key 没存进钥匙串。和连接的密码不同：密码存不进去，这次还能照用；Key 存不进去，
/// AI 就用不了，也没有「不保存、每次输入」这条退路
#[cfg(feature = "ai")]
pub const AI_KEY_SAVE_FAILED: &str = "DATAOMNI_AI_KEY_SAVE_FAILED";

/// 读 Key 失败的说明：没有条目单说，其余（锁着、被拒）与连接的密码是同一回事
#[cfg(feature = "ai")]
fn describe_key_read_failure(error: &keyring::Error) -> String {
  match error {
    keyring::Error::NoEntry => AI_KEY_MISSING.to_string(),
    other => crate::services::connection_service::describe_credential_read_failure(other),
  }
}

#[tauri::command]
pub fn ai_available() -> bool {
  cfg!(feature = "ai")
}

#[cfg(feature = "ai")]
async fn run_cancellable_ai_task<F>(
  registry: &AiRequestRegistry,
  request_id: Option<String>,
  future: F,
) -> Result<String, QueryError>
where
  F: std::future::Future<Output = Result<String, QueryError>> + Send + 'static,
{
  let request_id = request_id.map(|id| id.trim().to_string()).filter(|id| !id.is_empty());
  let Some(id) = request_id else {
    return future.await;
  };
  let handle = tokio::spawn(future);
  registry.register(id.clone(), handle.abort_handle());
  let joined = handle.await;
  registry.remove(&id);
  match joined {
    Ok(result) => result,
    Err(join_error) if join_error.is_cancelled() => {
      Err(QueryError::message(crate::services::ai::AI_CANCELLED))
    }
    Err(join_error) => {
      Err(QueryError::message(format!("{}: {join_error}", crate::services::ai::AI_REQUEST_FAILED)))
    }
  }
}

#[cfg(feature = "ai")]
#[tauri::command]
pub async fn ai_complete(
  request: crate::services::ai::AiRequest,
  registry: tauri::State<'_, AiRequestRegistry>,
) -> Result<String, QueryError> {
  use crate::services::connection_service::credential_entry;
  let account = ai_key_account(request.profile_id.as_deref()).map_err(QueryError::message)?;
  let entry = credential_entry(&account).map_err(QueryError::message)?;
  let key = match entry.get_password() {
    Ok(secret) => secret,
    Err(keyring::Error::NoEntry)
      if crate::services::ai::is_local_ai_endpoint(&request.base_url) =>
    {
      // Ollama / LM Studio 等本机端点允许免 Key 调用
      String::new()
    }
    Err(error) => return Err(QueryError::message(describe_key_read_failure(&error))),
  };
  let request_id = request.request_id.clone();
  run_cancellable_ai_task(&registry, request_id, async move {
    crate::services::ai::complete(&request, &key).await
  })
  .await
}

#[cfg(not(feature = "ai"))]
#[tauri::command]
pub async fn ai_complete(
  request: serde_json::Value,
  registry: tauri::State<'_, AiRequestRegistry>,
) -> Result<String, QueryError> {
  let _ = (request, registry);
  Err(QueryError::message(AI_NOT_IN_BUILD))
}

/// 取消指定 `requestId` 对应的在途 AI 请求；返回是否找到并中止了任务
#[tauri::command]
pub fn ai_cancel(request_id: String, registry: tauri::State<'_, AiRequestRegistry>) -> bool {
  registry.cancel(request_id.trim())
}

/// 存 Key。空串等于删掉：设置里清空那一格再保存，就是不想留着它
#[tauri::command]
pub fn ai_save_key(key: String, profile_id: Option<String>) -> Result<(), String> {
  #[cfg(feature = "ai")]
  {
    use crate::services::connection_service::{credential_entry, CREDENTIAL_DELETE_FAILED};
    let account = ai_key_account(profile_id.as_deref())?;
    let entry = credential_entry(&account)?;
    let key = key.trim();
    if key.is_empty() {
      return match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("{CREDENTIAL_DELETE_FAILED}: {error}")),
      };
    }
    entry.set_password(key).map_err(|error| format!("{AI_KEY_SAVE_FAILED}: {error}"))
  }
  #[cfg(not(feature = "ai"))]
  {
    let _ = (key, profile_id);
    Err(AI_NOT_IN_BUILD.to_string())
  }
}

/// 钥匙串里有没有 Key。只回答有没有，Key 本身不回到界面上
#[tauri::command]
pub fn ai_has_key(profile_id: Option<String>) -> Result<bool, String> {
  #[cfg(feature = "ai")]
  {
    use crate::services::connection_service::{credential_entry, describe_credential_read_failure};
    let account = ai_key_account(profile_id.as_deref())?;
    match credential_entry(&account)?.get_password() {
      Ok(_) => Ok(true),
      Err(keyring::Error::NoEntry) => Ok(false),
      Err(error) => Err(describe_credential_read_failure(&error)),
    }
  }
  #[cfg(not(feature = "ai"))]
  {
    let _ = profile_id;
    Ok(false)
  }
}

#[cfg(all(test, feature = "ai"))]
mod tests {
  use super::*;
  use std::net::TcpListener;
  use std::sync::Arc;
  use std::time::{Duration, Instant};

  #[test]
  fn a_missing_key_says_so_instead_of_blaming_a_connection_password() {
    assert_eq!(describe_key_read_failure(&keyring::Error::NoEntry), AI_KEY_MISSING);
  }

  #[test]
  fn maps_default_and_named_profile_ids_to_safe_keychain_accounts() {
    assert_eq!(ai_key_account(None).expect("none"), "#ai");
    assert_eq!(ai_key_account(Some("default")).expect("default"), "#ai");
    assert_eq!(ai_key_account(Some("local")).expect("local"), "#ai:local");
    assert!(ai_key_account(Some("../bad")).is_err());
  }

  /// AI0 取消验收门：连接本地 127.0.0.1 慢响应服务，调 `cancel` 后 500ms 内中止返回 `DATAOMNI_AI_CANCELLED`
  #[tokio::test]
  async fn cancels_in_flight_slow_request_within_500ms() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = format!("http://{}", listener.local_addr().expect("addr"));
    let _server = std::thread::spawn(move || {
      if let Ok((_stream, _)) = listener.accept() {
        std::thread::sleep(Duration::from_secs(10));
      }
    });

    let registry = Arc::new(AiRequestRegistry::default());
    let cancel_registry = Arc::clone(&registry);
    let cancel_task = tokio::spawn(async move {
      tokio::time::sleep(Duration::from_millis(40)).await;
      cancel_registry.cancel("req-slow-1")
    });

    let client = reqwest::Client::builder().no_proxy().build().expect("test client");
    let req = crate::services::ai::AiRequest {
      protocol: crate::services::ai::AiProtocol::Openai,
      base_url: address,
      model: "slow-model".into(),
      system: "hi".into(),
      user: "ping".into(),
      request_id: Some("req-slow-1".into()),
      profile_id: None,
      max_tokens: Some(16),
    };
    let started = Instant::now();
    let error = run_cancellable_ai_task(&registry, Some("req-slow-1".into()), async move {
      crate::services::ai::complete_with(&client, &req, "").await
    })
    .await
    .expect_err("must be cancelled");
    let elapsed = started.elapsed();
    let cancelled = cancel_task.await.expect("cancel task");

    assert!(cancelled);
    assert_eq!(error.message, crate::services::ai::AI_CANCELLED);
    assert!(elapsed < Duration::from_millis(500), "cancel took too long: {elapsed:?}");
  }
}
