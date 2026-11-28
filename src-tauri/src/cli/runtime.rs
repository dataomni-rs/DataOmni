//! CLI / MCP 运行时调度、共享隧道与钥匙串鉴权看门狗（H17 / P2-2）。

use super::*;

pub(super) struct McpRuntime {
  runtime: tokio::runtime::Runtime,
  tunnels: Arc<TunnelRegistry>,
}

thread_local! {
  static ACTIVE_MCP: std::cell::RefCell<Option<Arc<McpRuntime>>> =
    const { std::cell::RefCell::new(None) };
}

/// `mcp::serve` 期间持有的常驻 Tokio 运行时与 SSH 隧道复用池。
/// 离开作用域时优雅断开全部活跃隧道再释放运行时。
pub(super) struct McpSessionGuard {
  runtime: Arc<McpRuntime>,
}

impl McpSessionGuard {
  pub(super) fn enter() -> Result<Self, CliError> {
    let runtime =
      Arc::new(McpRuntime { runtime: runtime()?, tunnels: Arc::new(TunnelRegistry::default()) });
    ACTIVE_MCP.with(|slot| {
      *slot.borrow_mut() = Some(Arc::clone(&runtime));
    });
    Ok(Self { runtime })
  }
}

impl Drop for McpSessionGuard {
  fn drop(&mut self) {
    ACTIVE_MCP.with(|slot| {
      *slot.borrow_mut() = None;
    });
    self.runtime.runtime.block_on(self.runtime.tunnels.close_all());
  }
}

pub(super) fn shared_mcp_tunnels() -> Option<Arc<TunnelRegistry>> {
  ACTIVE_MCP.with(|slot| slot.borrow().as_ref().map(|mcp| Arc::clone(&mcp.tunnels)))
}

/// 在常驻 MCP 运行时或新运行时里跑完 `work`，到点没完就报超时。`what` 是报错里的主语
pub(super) fn block_on_within<T>(
  timeout: Duration,
  what: &str,
  work: impl std::future::Future<Output = Result<T, CliError>>,
) -> Result<T, CliError> {
  let shared = ACTIVE_MCP.with(|slot| slot.borrow().clone());
  let timed = match shared {
    Some(mcp) => mcp.runtime.block_on(async { tokio::time::timeout(timeout, work).await }),
    None => runtime()?.block_on(async { tokio::time::timeout(timeout, work).await }),
  };
  timed.map_err(|_| CliError {
    exit: Exit::Database,
    kind: "timeout",
    message: format!("{what} did not finish within {} s", timeout.as_secs()),
    diagnosis: None,
  })?
}

pub(super) fn runtime() -> Result<tokio::runtime::Runtime, CliError> {
  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .map_err(|error| CliError::unavailable("runtime", error.to_string()))
}

pub(super) fn session_error(failure: session::Failure) -> CliError {
  match failure {
    session::Failure::Unsupported(db_type) => unsupported(&db_type),
    session::Failure::Connect(message) => CliError::unavailable("connect", message),
    session::Failure::Database(error) => CliError {
      exit: Exit::Database,
      kind: "database",
      message: error.to_string(),
      diagnosis: None,
    },
  }
}

pub(super) fn unsupported(db_type: &crate::models::DatabaseType) -> CliError {
  match db_type {
    DatabaseType::MongoDB => {
      return CliError::refused("MongoDB connections take `dataomni mongo`, not SQL")
    }
    DatabaseType::Redis => {
      return CliError::refused("Redis connections take `dataomni redis`, not SQL")
    }
    DatabaseType::Neo4j => {
      return CliError::refused("Neo4j connections take `dataomni neo4j`, not SQL")
    }
    DatabaseType::Elasticsearch => {
      return CliError::refused("Elasticsearch connections take `dataomni es`, not SQL")
    }
    _ => {}
  }
  let name =
    serde_json::to_value(db_type).ok().and_then(|value| value.as_str().map(str::to_string));
  CliError::refused(format!(
    "{} connections are not supported on the command line yet",
    name.unwrap_or_else(|| format!("{db_type:?}"))
  ))
}

/// 钥匙串授权等到点时是否退出进程。命令行退出；MCP 服务（`serve_mcp`）关掉它，只在 stderr
/// 提一句、接着等
pub(super) static KEYCHAIN_DEADLINE_EXITS: AtomicBool = AtomicBool::new(true);

/// 读口令可能卡在钥匙串的授权框上。到点还没回来就直接退出并说清楚，框留给人去点——
/// 卡着不动的命令会让 Agent 一直等下去。用 Condvar 在 `work` 返回时立刻唤醒看门狗线程，
/// 避免长生命周期 MCP 进程里每次调用都遗留一条空睡 10 秒的线程（H16 / P2-3）。
pub(super) fn with_keychain_deadline<T>(work: impl FnOnce() -> T) -> T {
  let pair = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
  let watched = Arc::clone(&pair);
  std::thread::spawn(move || {
    let (lock, cvar) = &*watched;
    let Ok(guard) = lock.lock() else {
      return;
    };
    let Ok((done, _)) = cvar.wait_timeout_while(guard, KEYCHAIN_DEADLINE, |done| !*done) else {
      return;
    };
    if !*done {
      let body = json!({
        "schema": OUTPUT_SCHEMA,
        "error": {
          "kind": "keychain",
          "message": "the system keychain is waiting for approval: choose \"Always Allow\" in the prompt, then run the command again",
        },
      });
      eprintln!("{body}");
      if KEYCHAIN_DEADLINE_EXITS.load(Ordering::SeqCst) {
        std::process::exit(Exit::Unavailable as i32);
      }
    }
  });
  let result = work();
  let (lock, cvar) = &*pair;
  if let Ok(mut done) = lock.lock() {
    *done = true;
    cvar.notify_one();
  }
  result
}

/// 按 id 或名字找文件里标着开放的连接。还没验凭证：那一步要读钥匙串，放在语句门之后
pub(super) fn find_open(
  service: &ConnectionService,
  name: &str,
) -> Result<ConnectionProfile, CliError> {
  let open: Vec<ConnectionProfile> =
    service.get_connections().into_iter().filter(ConnectionProfile::open_to_agents).collect();
  if let Some(profile) = open.iter().find(|profile| profile.id == name) {
    return Ok(profile.clone());
  }
  let mut named = open.into_iter().filter(|profile| profile.name == name);
  match (named.next(), named.next()) {
    (Some(profile), None) => Ok(profile),
    (Some(_), Some(_)) => {
      Err(CliError::usage(format!("several connections are named {name}; use the id instead")))
    }
    (None, _) => Err(CliError::not_found(name)),
  }
}

/// 验过开放凭证、补好凭据的连接。凭证对不上的和不存在的一样，报「找不到」
pub(super) fn resolve_verified(
  service: &ConnectionService,
  profile: &ConnectionProfile,
) -> Result<ConnectionProfile, CliError> {
  match with_keychain_deadline(|| service.resolve_for_agents(profile)) {
    Ok(Some(resolved)) => Ok(resolved),
    Ok(None) => Err(CliError::not_found(&profile.name)),
    Err(error) => Err(CliError::unavailable("credentials", error)),
  }
}

pub(super) fn no_arguments(command: &str, rest: &[String]) -> Result<(), CliError> {
  match rest.first() {
    None => Ok(()),
    Some(extra) => Err(CliError::usage(format!("{command} takes no arguments, got {extra}"))),
  }
}

pub(super) fn open_service(config_dir: Option<&Path>) -> Result<ConnectionService, CliError> {
  let config_dir = config_dir.ok_or_else(|| {
    CliError::unavailable("config", "cannot locate the DataOmni configuration directory")
  })?;
  #[cfg(not(test))]
  let service = ConnectionService::open_read_only(config_dir);
  #[cfg(test)]
  let service = ConnectionService::open_read_only_without_keychain(config_dir);
  service.map_err(|error| CliError::unavailable("config", error.to_string()))
}

/// 开放给命令行、并且凭证对得上的连接。不带口令、钥匙串引用、私钥和证书的路径：
/// Agent 要的只是「有哪些库、叫什么、是什么类型」。开放了却用不了的（比如口令只在会话里输）
/// 单独列在 `unavailable` 里，说明原因，而不是悄悄少一个
pub(super) fn list_open(service: &ConnectionService) -> Value {
  let mut open: Vec<ConnectionProfile> =
    service.get_connections().into_iter().filter(ConnectionProfile::open_to_agents).collect();
  open.sort_by(|left, right| left.name.cmp(&right.name));
  let mut usable = Vec::new();
  let mut unavailable = Vec::new();
  with_keychain_deadline(|| {
    for connection in open {
      match service.resolve_for_agents(&connection) {
        Ok(Some(_)) => usable.push(connection),
        Ok(None) => {}
        Err(error) => unavailable.push(json!({ "name": connection.name, "reason": error })),
      }
    }
  });
  let connections: Vec<Value> = usable
    .into_iter()
    .map(|connection| {
      json!({
        "id": connection.id,
        "name": connection.name,
        "type": connection.db_type,
        "environment": connection.environment,
        "host": connection.host,
        "port": connection.port,
        "database": connection.database,
        "ssh_tunnel": connection.ssh_tunnel.is_some(),
      })
    })
    .collect();
  json!({ "schema": OUTPUT_SCHEMA, "connections": connections, "unavailable": unavailable })
}
