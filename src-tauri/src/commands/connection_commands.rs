use crate::models::{ConnectionProfile, DatabaseType};
use crate::services::mongodb::MongoTarget;
use crate::services::{
  sql_server, ssh_tunnel, ConnectionService, EngineRegistries, SqlServerPool, SqlServerTarget,
  TunnelRegistry,
};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

// 全局连接服务状态（Arc 包装以支持把钥匙串与磁盘 I/O 移入 spawn_blocking，B5）
pub type ConnectionServiceState = Arc<Mutex<Option<ConnectionService>>>;

/// 拿不到服务状态的锁——上一次持有它的线程 panic 了。用户无从下手，
/// 但也不该看见一句中文
pub const SERVICE_STATE_UNAVAILABLE: &str = "DATAOMNI_SERVICE_STATE_UNAVAILABLE";
/// 连接服务建不起来：读不到配置目录、配置文件坏了之类
pub const SERVICE_INIT_FAILED: &str = "DATAOMNI_SERVICE_INIT_FAILED";

/// 取连接服务，没初始化就地初始化，并在 `spawn_blocking` 线程池中执行，
/// 避免系统钥匙串 IPC（macOS `/usr/bin/security` / Keychain）与配置文件磁盘 I/O 卡住 Tokio 工作线程（B5）。
pub(crate) async fn with_service_blocking<T, F>(
  service_state: &ConnectionServiceState,
  app_handle: &AppHandle,
  action: F,
) -> Result<T, String>
where
  T: Send + 'static,
  F: FnOnce(&mut ConnectionService) -> Result<T, String> + Send + 'static,
{
  let state = Arc::clone(service_state);
  let app = app_handle.clone();
  tokio::task::spawn_blocking(move || {
    let mut guard =
      state.lock().map_err(|error| format!("{SERVICE_STATE_UNAVAILABLE}: {error}"))?;

    let service = match &mut *guard {
      Some(service) => service,
      slot => slot.insert(
        ConnectionService::new(&app).map_err(|error| format!("{SERVICE_INIT_FAILED}: {error}"))?,
      ),
    };

    action(service)
  })
  .await
  .map_err(|error| format!("{SERVICE_STATE_UNAVAILABLE}: {error}"))?
}

#[tauri::command]
pub async fn create_connection(
  config: ConnectionProfile,
  app_handle: AppHandle,
  service_state: State<'_, ConnectionServiceState>,
) -> Result<String, String> {
  log::info!("创建连接: {}", config.name);

  with_service_blocking(&service_state, &app_handle, move |service| {
    service.create_connection(config)
  })
  .await
}

#[tauri::command]
pub async fn update_connection(
  id: String,
  config: ConnectionProfile,
  app_handle: AppHandle,
  service_state: State<'_, ConnectionServiceState>,
) -> Result<(), String> {
  log::info!("更新连接: {} ({id})", config.name);

  with_service_blocking(&service_state, &app_handle, move |service| {
    service.update_connection(&id, config)
  })
  .await
}

#[tauri::command]
pub async fn delete_connection(
  id: String,
  app_handle: AppHandle,
  service_state: State<'_, ConnectionServiceState>,
  tunnels: State<'_, TunnelRegistry>,
) -> Result<(), String> {
  log::info!("删除连接: {id}");

  let target_id = id.clone();
  with_service_blocking(&service_state, &app_handle, move |service| {
    service.delete_connection(&target_id)
  })
  .await?;
  // 删除配置后同步拆掉该连接残留的活动 SSH 隧道（B4）
  tunnels.close(&id).await;
  Ok(())
}

#[tauri::command]
pub async fn get_connections(
  app_handle: AppHandle,
  service_state: State<'_, ConnectionServiceState>,
) -> Result<Vec<ConnectionProfile>, String> {
  with_service_blocking(&service_state, &app_handle, |service| Ok(service.get_connections())).await
}

/// 读取连接级本地业务语义备注（`semantics/<id>.md`，CM4）
#[tauri::command]
pub async fn get_connection_semantics(id: String, app_handle: AppHandle) -> Result<String, String> {
  use tauri::Manager;
  let config_dir = app_handle
    .path()
    .app_config_dir()
    .map_err(|error| format!("DATAOMNI_CONFIG_DIR_UNAVAILABLE: {error}"))?;
  tokio::task::spawn_blocking(move || {
    Ok(crate::services::connection_semantics::read_semantics(&config_dir, &id)?.unwrap_or_default())
  })
  .await
  .map_err(|error| format!("{SERVICE_STATE_UNAVAILABLE}: {error}"))?
}

/// 保存连接级本地业务语义备注（上限 32 KB，不修改 `connections.json` 与 `<id>#agent` 指纹，CM4）
#[tauri::command]
pub async fn save_connection_semantics(
  id: String,
  content: String,
  app_handle: AppHandle,
  service_state: State<'_, ConnectionServiceState>,
) -> Result<(), String> {
  use tauri::Manager;
  let check_id = id.clone();
  with_service_blocking(&service_state, &app_handle, move |service| {
    if service.get_connection(&check_id).is_none() {
      return Err(crate::services::connection_service::CONNECTION_NOT_FOUND.to_string());
    }
    Ok(())
  })
  .await?;
  let config_dir = app_handle
    .path()
    .app_config_dir()
    .map_err(|error| format!("DATAOMNI_CONFIG_DIR_UNAVAILABLE: {error}"))?;
  tokio::task::spawn_blocking(move || {
    crate::services::connection_semantics::save_semantics(&config_dir, &id, &content)
  })
  .await
  .map_err(|error| format!("{SERVICE_STATE_UNAVAILABLE}: {error}"))?
}

/// 把所有连接导出到 `path`（不含口令，见 `ConnectionService::export_document`），返回导出了几个
#[tauri::command]
pub async fn export_connections(
  path: String,
  app_handle: AppHandle,
  service_state: State<'_, ConnectionServiceState>,
) -> Result<usize, String> {
  let target = std::path::PathBuf::from(&path);
  crate::services::path_guard::refuse_reserved_path_with_app(&target, Some(&app_handle))?;
  let (document, count) = with_service_blocking(&service_state, &app_handle, |service| {
    Ok((service.export_document()?, service.get_connections().len()))
  })
  .await?;
  tokio::task::spawn_blocking(move || {
    // 和连接配置、导出、备份一样先写 .part 再改名：覆盖一份旧的导出时，写到一半不会把它弄坏
    let part = target.with_extension("json.part");
    std::fs::write(&part, document).map_err(|error| format!("{}: {error}", part.display()))?;
    std::fs::rename(&part, &target).map_err(|error| format!("{path}: {error}"))?;
    Ok(count)
  })
  .await
  .map_err(|error| format!("{SERVICE_STATE_UNAVAILABLE}: {error}"))?
}

#[tauri::command]
pub async fn import_connections(
  path: String,
  app_handle: AppHandle,
  service_state: State<'_, ConnectionServiceState>,
) -> Result<crate::services::connection_service::ConnectionImport, String> {
  let target = std::path::PathBuf::from(&path);
  crate::services::path_guard::refuse_reserved_path_with_app(&target, Some(&app_handle))?;
  let content = tokio::task::spawn_blocking(move || {
    std::fs::read_to_string(&target).map_err(|error| format!("{path}: {error}"))
  })
  .await
  .map_err(|error| format!("{SERVICE_STATE_UNAVAILABLE}: {error}"))??;
  with_service_blocking(&service_state, &app_handle, move |service| {
    service.import_document(&content)
  })
  .await
}

#[tauri::command]
pub async fn import_connection_profiles(
  profiles: Vec<ConnectionProfile>,
  app_handle: AppHandle,
  service_state: State<'_, ConnectionServiceState>,
) -> Result<crate::services::connection_service::ConnectionImport, String> {
  with_service_blocking(&service_state, &app_handle, move |service| {
    service.import_profiles(profiles)
  })
  .await
}

/// 连接失败之后查一遍断在哪一段。
///
/// 不经过 `ConnectionService`：这里只看主机、端口和文件路径，用不到凭据，
/// 也不该因为服务没初始化而查不成——而「连不上」的时候服务正好最可能没初始化。
#[tauri::command]
pub async fn diagnose_connection(
  config: ConnectionProfile,
) -> Result<crate::services::ConnectionDiagnosis, String> {
  log::info!("诊断连接: {}", config.name);
  Ok(crate::services::diagnose(&config).await)
}

/// 打开一个库文件前，按文件头认它是 SQLite 还是 DuckDB（见 `database_file_type`）
#[tauri::command]
pub fn database_file_type(path: String) -> Option<crate::models::DatabaseType> {
  crate::services::connection_probe::database_file_type(&path)
}

#[tauri::command]
pub async fn test_connection(
  config: ConnectionProfile,
  app_handle: AppHandle,
  service_state: State<'_, ConnectionServiceState>,
  tunnels: State<'_, TunnelRegistry>,
  engines: State<'_, EngineRegistries>,
) -> Result<String, String> {
  log::info!("测试连接: {}", config.name);

  // 校验与读钥匙串移入 spawn_blocking，不阻塞 Tokio 工作线程（B5）
  let resolved = with_service_blocking(&service_state, &app_handle, move |service| {
    service.resolve_for_connection(&config)
  })
  .await?;

  // 未保存草稿（表单「测试连接」）不向 TunnelRegistry 或任何持久 PoolRegistry 写入状态，
  // 测完即拆临时隧道与临时连接，避免孤儿隧道与池泄漏（B4）。
  let is_draft = resolved.id.trim().is_empty();
  let mut ephemeral_tunnel: Option<ssh_tunnel::ActiveTunnel> = None;

  let local_port = match resolved.ssh_tunnel.clone() {
    None => None,
    Some(tunnel) => {
      let Some(known_hosts) = ssh_tunnel::default_known_hosts() else {
        return Err(crate::services::connection_service::KNOWN_HOSTS_NO_HOME.to_string());
      };
      if is_draft {
        let opened = ssh_tunnel::open(&resolved, &tunnel, &known_hosts)
          .await
          .map_err(|error| error.to_string())?;
        let port = opened.local_port;
        ephemeral_tunnel = Some(opened);
        log::info!("草稿临时 SSH 隧道已就绪: 127.0.0.1:{port} → {}:{}", tunnel.host, tunnel.port);
        Some(port)
      } else {
        let port = tunnels
          .ensure(&resolved, &tunnel, &known_hosts)
          .await
          .map_err(|error| error.to_string())?;
        log::info!("SSH 隧道已就绪: 127.0.0.1:{port} → {}:{}", tunnel.host, tunnel.port);
        Some(port)
      }
    }
  };

  // 连接串必须和执行查询那条路算出来的**逐字节相同**（有隧道时指向本地转发
  // 端口）——两边都走 `connection_string_via`，就没有第二份实现可以走偏
  let connection_string = resolved.connection_string_via(local_port);

  let outcome: Result<(), String> = async {
    if resolved.db_type == DatabaseType::SqlServer {
      let reachable = match local_port {
        Some(port) => resolved.redirected_to("127.0.0.1", port),
        None => resolved,
      };
      let target = SqlServerTarget::from_profile(&reachable);
      let client = sql_server::connect(&target).await.map_err(|error| error.message)?;
      if !is_draft {
        let pool = SqlServerPool::new(target, client).await.map_err(|error| error.message)?;
        engines.sql_server.insert(connection_string.clone(), pool);
      }
    } else if resolved.db_type == DatabaseType::Oracle {
      #[cfg(feature = "oracle")]
      {
        let reachable = match local_port {
          Some(port) => resolved.redirected_to("127.0.0.1", port),
          None => resolved,
        };
        let target = crate::services::oracle::OracleTarget::from_profile(&reachable);
        let connection =
          crate::services::oracle::connect(&target).await.map_err(|error| error.message)?;
        let pool = crate::services::oracle::OraclePool::new(target, connection);
        if !is_draft {
          engines.oracle.insert(connection_string.clone(), pool);
        }
      }
      #[cfg(not(feature = "oracle"))]
      {
        unreachable!(
          "resolve_for_connection already rejected Oracle when oracle feature is disabled"
        );
      }
    } else if resolved.db_type == DatabaseType::Dameng {
      #[cfg(feature = "odbc")]
      {
        let reachable = match local_port {
          Some(port) => resolved.redirected_to("127.0.0.1", port),
          None => resolved,
        };
        let target = crate::services::dameng::DamengTarget::from_profile(&reachable)?;
        let connection =
          crate::services::dameng::connect(&target).await.map_err(|error| error.message)?;
        let pool = crate::services::dameng::DamengPool::new(target, connection);
        if !is_draft {
          engines.dameng.insert(connection_string.clone(), pool);
        }
      }
      #[cfg(not(feature = "odbc"))]
      {
        unreachable!(
          "resolve_for_connection already rejected Dameng when odbc feature is disabled"
        );
      }
    } else if resolved.db_type == DatabaseType::MongoDB {
      let reachable = match local_port {
        Some(port) => resolved.redirected_to("127.0.0.1", port),
        None => resolved,
      };
      let client =
        crate::services::mongodb::connect(&MongoTarget::from_profile(&reachable)).await?;
      if !is_draft {
        engines.mongo.insert(connection_string.clone(), std::sync::Arc::new(client));
      }
    } else if resolved.db_type == DatabaseType::Redis {
      let reachable = match local_port {
        Some(port) => resolved.redirected_to("127.0.0.1", port),
        None => resolved,
      };
      let target = crate::services::redis::RedisTarget::from_profile(&reachable)?;
      let pool = crate::services::redis::connect(target).await?;
      if !is_draft {
        engines.redis.insert(connection_string.clone(), std::sync::Arc::new(pool));
      }
    } else if resolved.db_type == DatabaseType::Neo4j {
      let reachable = match local_port {
        Some(port) => resolved.redirected_to("127.0.0.1", port),
        None => resolved,
      };
      let target = crate::services::neo4j::Neo4jTarget::from_profile(&reachable)?;
      let pool = crate::services::neo4j::connect(target).await?;
      if !is_draft {
        engines.neo4j.insert(connection_string.clone(), std::sync::Arc::new(pool));
      }
    } else if resolved.db_type == DatabaseType::DuckDB {
      // 已经开着就接着用：同一个文件在进程里只开一份，另开一份的话，两份各持一把锁、
      // 各有一份缓存，谁的写入先落盘说不清
      if engines.duckdb.get(&connection_string).is_none() {
        let path =
          duckdb_file_path(&app_handle, resolved.database.as_deref().unwrap_or(":memory:"))?;
        let pool = crate::services::duckdb::open(&path).await.map_err(|error| error.message)?;
        if !is_draft {
          engines.duckdb.insert(connection_string.clone(), pool);
        }
      }
    } else if resolved.db_type == DatabaseType::Elasticsearch {
      // 不换主机：经隧道的 HTTPS 仍按原来的主机名校验证书（见 `EsTarget::from_profile`）
      let target = crate::services::elasticsearch::EsTarget::from_profile(&resolved, local_port);
      let pool = crate::services::elasticsearch::connect(target).await?;
      if !is_draft {
        engines.es.insert(connection_string.clone(), std::sync::Arc::new(pool));
      }
    } else if resolved.db_type == DatabaseType::ClickHouse {
      // 同 Elasticsearch：不换主机，经隧道的 HTTPS 仍按原来的主机名校验证书
      let target =
        crate::services::clickhouse::ClickHouseTarget::from_profile(&resolved, local_port);
      let pool = crate::services::clickhouse::connect(target).await?;
      if !is_draft {
        engines.clickhouse.insert(connection_string.clone(), pool);
      }
    } else if resolved.db_type == DatabaseType::SQLite {
      // 驱动打开时不读文件头，DuckDB 的库、CSV 都能「连上」。绝对路径先核验魔数
      if let Some(path) =
        resolved.database.as_deref().filter(|path| std::path::Path::new(path).is_absolute())
      {
        if crate::services::connection_probe::is_non_sqlite_file(path) {
          return Err(format!("{}: {path}", crate::services::connection_probe::NOT_SQLITE_FILE));
        }
      }
      let connect_url = match connection_string.strip_prefix("sqlite:") {
        Some(rest) if rest != ":memory:" && !std::path::Path::new(rest).is_absolute() => {
          use tauri::Manager;
          let base = app_handle.path().app_config_dir().map_err(|error| error.to_string())?;
          crate::services::sqlite_path::sqlite_connection_string(&base.join(rest).to_string_lossy())
        }
        _ => connection_string.clone(),
      };
      let pool = crate::services::sqlx_pool::open(&connect_url).await?;
      if is_draft {
        pool.close().await;
      } else {
        crate::services::sqlx_pool::register(&engines.sqlx, connection_string.clone(), pool);
      }
    } else if matches!(resolved.db_type, DatabaseType::MySQL | DatabaseType::PostgreSQL) {
      // 真实含口令 URL 留在 Rust 后端内部建连，池子按无口令的 `connection_string` 登记（H7 / S2）
      let connect_url = resolved.sqlx_connect_url_via(local_port);
      let pool = crate::services::sqlx_pool::open(&connect_url).await?;
      if is_draft {
        pool.close().await;
      } else {
        crate::services::sqlx_pool::register(&engines.sqlx, connection_string.clone(), pool);
      }
    }
    Ok(())
  }
  .await;

  if let Some(tunnel) = ephemeral_tunnel {
    tunnel.disconnect().await;
  }

  outcome?;
  Ok(connection_string)
}

/// MySQL / PostgreSQL / SQLite 的目录查询（取出池子副本后立即释放注册表锁）
#[tauri::command]
pub async fn sqlx_select(
  connection_string: String,
  sql: String,
  params: Vec<serde_json::Value>,
  engines: State<'_, EngineRegistries>,
) -> Result<Vec<indexmap::IndexMap<String, serde_json::Value>>, crate::services::QueryError> {
  let pool =
    crate::services::sqlx_pool::shared(&engines.sqlx, &connection_string).ok_or_else(|| {
      crate::services::QueryError::message(
        crate::commands::database_commands::DB_SESSION_NOT_CONNECTED,
      )
    })?;
  crate::services::sqlx_pool::select(&pool, &sql, params).await
}

/// 摘掉并在后台异步关闭 sqlx 池子，避免借出的挂起连接阻塞断开操作
#[tauri::command]
pub async fn close_sqlx_pool(
  connection_string: String,
  engines: State<'_, EngineRegistries>,
) -> Result<bool, String> {
  Ok(crate::services::sqlx_pool::close(&engines.sqlx, &connection_string))
}

/// SQL Server 连接上的目录查询：前端对另外三家走插件的 `select`，对这一家走这里。
/// 返回的形状与插件一致——一行一个对象，值不带类型标签。
#[tauri::command]
pub async fn sql_server_select(
  connection_string: String,
  sql: String,
  params: Vec<serde_json::Value>,
  engines: State<'_, EngineRegistries>,
) -> Result<Vec<crate::services::QueryRow>, crate::services::QueryError> {
  let pool = engines.sql_server.get(&connection_string).ok_or_else(|| {
    crate::services::QueryError::message(
      crate::commands::database_commands::DB_SESSION_NOT_CONNECTED,
    )
  })?;
  pool.select(&sql, &params).await
}

/// Oracle 连接上的目录查询。与 `sql_server_select` 同一个角色、同一种返回形状
#[tauri::command]
pub async fn oracle_select(
  connection_string: String,
  sql: String,
  params: Vec<serde_json::Value>,
  engines: State<'_, EngineRegistries>,
) -> Result<Vec<crate::services::QueryRow>, crate::services::QueryError> {
  let pool = engines.oracle.get(&connection_string).ok_or_else(|| {
    crate::services::QueryError::message(
      crate::commands::database_commands::DB_SESSION_NOT_CONNECTED,
    )
  })?;
  pool.select(&sql, &params).await
}

#[tauri::command]
pub fn close_oracle(connection_string: String, engines: State<'_, EngineRegistries>) -> bool {
  engines.oracle.remove(&connection_string)
}

/// 达梦 DM8 连接上的目录查询。与 `oracle_select` 同一个角色、同一种返回形状
#[tauri::command]
pub async fn dameng_select(
  connection_string: String,
  sql: String,
  params: Vec<serde_json::Value>,
  engines: State<'_, EngineRegistries>,
) -> Result<Vec<crate::services::QueryRow>, crate::services::QueryError> {
  let pool = engines.dameng.get(&connection_string).ok_or_else(|| {
    crate::services::QueryError::message(
      crate::commands::database_commands::DB_SESSION_NOT_CONNECTED,
    )
  })?;
  pool.select(&sql, &params).await
}

#[tauri::command]
pub fn close_dameng(connection_string: String, engines: State<'_, EngineRegistries>) -> bool {
  engines.dameng.remove(&connection_string)
}

/// 相对路径落在应用的配置目录下，和插件对 SQLite 的处理一样——同一格表单，两种库
/// 不该一个落在这里、一个落在进程的当前目录（从 Finder 打开时是 `/`）
fn duckdb_file_path(app_handle: &AppHandle, path: &str) -> Result<String, String> {
  if path == ":memory:" || std::path::Path::new(path).is_absolute() {
    return Ok(path.to_string());
  }
  use tauri::Manager;
  let base = app_handle.path().app_config_dir().map_err(|error| error.to_string())?;
  std::fs::create_dir_all(&base).map_err(|error| error.to_string())?;
  Ok(base.join(path).to_string_lossy().into_owned())
}

/// DuckDB 连接上的目录查询。与 `sql_server_select` 同一个角色、同一种返回形状
#[tauri::command]
pub async fn duckdb_select(
  connection_string: String,
  sql: String,
  params: Vec<serde_json::Value>,
  engines: State<'_, EngineRegistries>,
) -> Result<Vec<crate::services::QueryRow>, crate::services::QueryError> {
  let pool = engines.duckdb.get(&connection_string).ok_or_else(|| {
    crate::services::QueryError::message(
      crate::commands::database_commands::DB_SESSION_NOT_CONNECTED,
    )
  })?;
  pool.select(&sql, &params).await
}

/// ClickHouse 连接上的目录查询。与 `sql_server_select` 同一个角色、同一种返回形状；
/// 参数按序号绑成服务端参数 `{p1:String}`、`{p2:…}`
#[tauri::command]
pub async fn clickhouse_select(
  connection_string: String,
  sql: String,
  params: Vec<serde_json::Value>,
  engines: State<'_, EngineRegistries>,
) -> Result<Vec<crate::services::QueryRow>, crate::services::QueryError> {
  let pool = engines.clickhouse.get(&connection_string).ok_or_else(|| {
    crate::services::QueryError::message(
      crate::commands::database_commands::DB_SESSION_NOT_CONNECTED,
    )
  })?;
  pool.select(&sql, &params).await
}

/// 断开时去掉登记的客户端。会话连接由 `release_database_session` 另行释放
#[tauri::command]
pub fn close_clickhouse(connection_string: String, engines: State<'_, EngineRegistries>) -> bool {
  engines.clickhouse.remove(&connection_string)
}

/// 断开：去掉登记的库。会话连接由 `release_database_session` 另行释放，两边都放掉之后
/// 库才真的关上、文件锁才还回去
#[tauri::command]
pub fn close_duckdb(connection_string: String, engines: State<'_, EngineRegistries>) -> bool {
  engines.duckdb.remove(&connection_string)
}

/// 断开时把登记的池子去掉。池子里的空闲连接随之关闭；会话连接由
/// `release_database_session` 另行释放。
#[tauri::command]
pub fn close_sql_server(connection_string: String, engines: State<'_, EngineRegistries>) -> bool {
  engines.sql_server.remove(&connection_string)
}

/// 断开连接时拆掉隧道。
///
/// 没有隧道也算成功——前端不知道某个连接有没有隧道，让它无条件调一次比
/// 让它先去问一遍简单，而多出来的代价只是一次空的 IPC。
#[tauri::command]
pub async fn close_ssh_tunnel(
  connection_id: String,
  tunnels: State<'_, TunnelRegistry>,
) -> Result<(), String> {
  tunnels.close(&connection_id).await;
  Ok(())
}
