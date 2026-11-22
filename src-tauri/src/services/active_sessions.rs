//! 关系库活跃会话与锁等待诊断查询及安全终止执行（DC7）。
//!
//! 仅对服务端关系库（MySQL / PostgreSQL / SQL Server / Oracle / ClickHouse）开放；
//! SQLite 与 DuckDB 是嵌入式文件库，没有服务器多会话概念。
//!
//! 读列表沿用 `session_target` / `object_catalog` 的同一模式：后端按 `DatabaseType`
//! 提供统一 10 列投影的只读 SQL（必要时附一条低版本/兼容库回退 SQL），前端走现有
//! `DatabaseHandle.select` 执行。终止动作走独立窄命令 `execute_session_action`，在后端
//! 严格白名单校验会话 ID 并直接借池中非事务连接执行，不接受前端任意 SQL，也不被编辑器的
//! 未提交事务阻挡。

use crate::models::DatabaseType;
use crate::services::query_error::QueryError;
use crate::services::query_executor::{PoolRef, StreamOptions};
use crate::services::write_batch::WriteStatement;
use crate::services::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::Executor;

pub const ACTIVE_SESSIONS_UNSUPPORTED: &str = "DATAOMNI_ACTIVE_SESSIONS_UNSUPPORTED";
pub const INVALID_SESSION_ID: &str = "DATAOMNI_INVALID_SESSION_ID";
pub const SESSION_ACTION_UNSUPPORTED: &str = "DATAOMNI_SESSION_ACTION_UNSUPPORTED";

#[derive(Debug, Clone, Serialize)]
pub struct ActiveSessionsQuery {
  /// 统一投影 10 列：
  /// `session_id`, `username`, `database_name`, `client_addr`, `state`,
  /// `duration_seconds`, `wait_event`, `blocking_pid`, `sql_text`, `is_current`
  pub sql: &'static str,
  /// 当主查询因兼容库/旧版本缺少锁或等待函数（例如 openGauss 无 `pg_blocking_pids`、
  /// 旧 ClickHouse 无 `queryID()`）报错时，前端自动回退一次的基础查询。
  pub fallback_sql: Option<&'static str>,
  /// 是否支持仅取消当前查询而不踢断连接
  pub supports_cancel: bool,
  /// 是否支持强制断开会话连接
  pub supports_terminate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionAction {
  Cancel,
  Terminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminateSessionResult {
  /// 服务端是否确认受理了终止/取消请求（PostgreSQL 会显式返回 true/false；
  /// 其余方言在语句无报错返回时记为 true）
  pub applied: bool,
  /// 实际发往服务端的管理语句（供审计日志与 UI 展示）
  pub statement: String,
}

/// 按数据库类型返回只读活跃会话查询与可用终止动作。
pub fn active_sessions_query(db_type: &DatabaseType) -> Option<ActiveSessionsQuery> {
  match db_type {
    DatabaseType::MySQL => Some(ActiveSessionsQuery {
      sql: MYSQL_SESSIONS,
      fallback_sql: None,
      supports_cancel: true,
      supports_terminate: true,
    }),
    DatabaseType::PostgreSQL => Some(ActiveSessionsQuery {
      sql: POSTGRES_SESSIONS,
      fallback_sql: Some(POSTGRES_SESSIONS_FALLBACK),
      supports_cancel: true,
      supports_terminate: true,
    }),
    DatabaseType::SqlServer => Some(ActiveSessionsQuery {
      sql: SQL_SERVER_SESSIONS,
      fallback_sql: None,
      supports_cancel: false,
      supports_terminate: true,
    }),
    DatabaseType::Oracle => Some(ActiveSessionsQuery {
      sql: ORACLE_SESSIONS,
      fallback_sql: None,
      supports_cancel: false,
      supports_terminate: true,
    }),
    DatabaseType::Dameng => Some(ActiveSessionsQuery {
      sql: DAMENG_SESSIONS,
      fallback_sql: None,
      supports_cancel: false,
      supports_terminate: true,
    }),
    DatabaseType::ClickHouse => Some(ActiveSessionsQuery {
      sql: CLICKHOUSE_SESSIONS,
      fallback_sql: Some(CLICKHOUSE_SESSIONS_FALLBACK),
      supports_cancel: true,
      supports_terminate: false,
    }),
    _ => None,
  }
}

/// MySQL / MariaDB / TiDB / OceanBase（MySQL 模式）：`information_schema.PROCESSLIST`
/// 全系通用且不依赖高权限 `performance_schema`。
const MYSQL_SESSIONS: &str = r#"
SELECT
  CAST(ID AS CHAR) AS session_id,
  COALESCE(CAST(USER AS CHAR), '') AS username,
  COALESCE(CAST(DB AS CHAR), '') AS database_name,
  COALESCE(CAST(HOST AS CHAR), '') AS client_addr,
  COALESCE(CAST(COMMAND AS CHAR), '') AS state,
  COALESCE(TIME, 0) AS duration_seconds,
  NULLIF(COALESCE(CAST(STATE AS CHAR), ''), '') AS wait_event,
  CAST(NULL AS CHAR) AS blocking_pid,
  LEFT(COALESCE(CAST(INFO AS CHAR), ''), 2000) AS sql_text,
  (ID = CONNECTION_ID()) AS is_current
FROM information_schema.PROCESSLIST
ORDER BY
  (COMMAND <> 'Sleep') DESC,
  TIME DESC
LIMIT 200
"#;

/// PostgreSQL 10+ / KingbaseES V9：`pg_stat_activity` + `pg_blocking_pids(pid)`。
/// 排除后台进程（仅保留 `client backend`），优先展示被阻塞与活跃会话。
const POSTGRES_SESSIONS: &str = r#"
SELECT
  pid::text AS session_id,
  COALESCE(usename, '') AS username,
  COALESCE(datname, '') AS database_name,
  COALESCE(host(client_addr), '') AS client_addr,
  COALESCE(state, '') AS state,
  CASE
    WHEN state = 'active' AND query_start IS NOT NULL
      THEN GREATEST(0, EXTRACT(EPOCH FROM (clock_timestamp() - query_start))::bigint)
    WHEN state_change IS NOT NULL
      THEN GREATEST(0, EXTRACT(EPOCH FROM (clock_timestamp() - state_change))::bigint)
    ELSE 0
  END AS duration_seconds,
  CASE
    WHEN wait_event_type IS NOT NULL AND wait_event IS NOT NULL
      THEN wait_event_type || ':' || wait_event
    WHEN wait_event IS NOT NULL
      THEN wait_event
    ELSE NULL
  END AS wait_event,
  NULLIF(array_to_string(pg_blocking_pids(pid), ','), '') AS blocking_pid,
  LEFT(COALESCE(query, ''), 2000) AS sql_text,
  (pid = pg_backend_pid()) AS is_current
FROM pg_stat_activity
WHERE COALESCE(backend_type, 'client backend') = 'client backend'
ORDER BY
  (cardinality(pg_blocking_pids(pid)) > 0) DESC,
  (state = 'active') DESC,
  duration_seconds DESC
LIMIT 200
"#;

/// openGauss / CockroachDB / 老版本 PostgreSQL 兼容回退：
/// 不依赖 `backend_type`、`wait_event_type` 与 `pg_blocking_pids`。
const POSTGRES_SESSIONS_FALLBACK: &str = r#"
SELECT
  pid::text AS session_id,
  COALESCE(usename, '') AS username,
  COALESCE(datname, '') AS database_name,
  COALESCE(client_addr::text, '') AS client_addr,
  COALESCE(state, '') AS state,
  CASE
    WHEN query_start IS NOT NULL
      THEN GREATEST(0, EXTRACT(EPOCH FROM (clock_timestamp() - query_start))::bigint)
    ELSE 0
  END AS duration_seconds,
  CAST(NULL AS text) AS wait_event,
  CAST(NULL AS text) AS blocking_pid,
  LEFT(COALESCE(query, ''), 2000) AS sql_text,
  (pid = pg_backend_pid()) AS is_current
FROM pg_stat_activity
ORDER BY
  (state = 'active') DESC,
  duration_seconds DESC
LIMIT 200
"#;

/// SQL Server：`sys.dm_exec_sessions` + `sys.dm_exec_requests` + `sys.dm_exec_sql_text`。
/// 仅查用户进程（`is_user_process = 1`），`blocking_session_id > 0` 即为阻塞源。
const SQL_SERVER_SESSIONS: &str = r#"
SELECT TOP 200
  CAST(s.session_id AS NVARCHAR(32)) AS session_id,
  COALESCE(s.login_name, N'') AS username,
  COALESCE(DB_NAME(COALESCE(r.database_id, s.database_id)), N'') AS database_name,
  COALESCE(s.host_name, N'') AS client_addr,
  COALESCE(r.status, s.status, N'') AS state,
  CASE
    WHEN r.start_time IS NOT NULL
      THEN CASE WHEN DATEDIFF(SECOND, r.start_time, GETDATE()) > 0 THEN DATEDIFF(SECOND, r.start_time, GETDATE()) ELSE 0 END
    WHEN s.last_request_start_time IS NOT NULL
      THEN CASE WHEN DATEDIFF(SECOND, s.last_request_start_time, GETDATE()) > 0 THEN DATEDIFF(SECOND, s.last_request_start_time, GETDATE()) ELSE 0 END
    ELSE 0
  END AS duration_seconds,
  NULLIF(COALESCE(r.wait_type, N''), N'') AS wait_event,
  CASE
    WHEN r.blocking_session_id IS NOT NULL AND r.blocking_session_id > 0
      THEN CAST(r.blocking_session_id AS NVARCHAR(32))
    ELSE NULL
  END AS blocking_pid,
  LEFT(COALESCE(st.text, N''), 2000) AS sql_text,
  CAST(CASE WHEN s.session_id = @@SPID THEN 1 ELSE 0 END AS BIT) AS is_current
FROM sys.dm_exec_sessions s
LEFT JOIN sys.dm_exec_requests r ON s.session_id = r.session_id
OUTER APPLY sys.dm_exec_sql_text(r.sql_handle) st
WHERE s.is_user_process = 1
ORDER BY
  CASE WHEN r.blocking_session_id IS NOT NULL AND r.blocking_session_id > 0 THEN 0 ELSE 1 END,
  CASE WHEN r.session_id IS NOT NULL THEN 0 ELSE 1 END,
  duration_seconds DESC
"#;

/// Oracle：`V$SESSION` + `V$SQL`。别名全部加双引号避免被折成大写；
/// `session_id` 组合 `SID,SERIAL#`，与 `ALTER SYSTEM KILL SESSION` 格式一一对应。
const ORACLE_SESSIONS: &str = r#"
SELECT * FROM (
  SELECT
    TO_CHAR(s.SID) || ',' || TO_CHAR(s.SERIAL#) AS "session_id",
    NVL(s.USERNAME, '') AS "username",
    NVL(SYS_CONTEXT('USERENV', 'CON_NAME'), '') AS "database_name",
    NVL(s.MACHINE, '') AS "client_addr",
    NVL(s.STATUS, '') AS "state",
    NVL(s.LAST_CALL_ET, 0) AS "duration_seconds",
    CASE WHEN s.WAIT_CLASS <> 'Idle' THEN s.EVENT ELSE NULL END AS "wait_event",
    CASE WHEN s.BLOCKING_SESSION IS NOT NULL THEN TO_CHAR(s.BLOCKING_SESSION) ELSE NULL END AS "blocking_pid",
    SUBSTR(NVL(q.SQL_TEXT, ''), 1, 2000) AS "sql_text",
    CASE WHEN s.SID = SYS_CONTEXT('USERENV', 'SID') THEN 1 ELSE 0 END AS "is_current"
  FROM V$SESSION s
  LEFT JOIN V$SQL q ON s.SQL_ID = q.SQL_ID AND s.SQL_CHILD_NUMBER = q.CHILD_NUMBER
  WHERE s.TYPE = 'USER'
  ORDER BY
    CASE WHEN s.BLOCKING_SESSION IS NOT NULL THEN 0 ELSE 1 END,
    CASE WHEN s.STATUS = 'ACTIVE' THEN 0 ELSE 1 END,
    s.LAST_CALL_ET DESC
)
WHERE ROWNUM <= 200
"#;

/// 达梦 DM8：`V$SESSIONS`，使用 `SP_CLOSE_SESSION(sess_id)` 终止会话。
const DAMENG_SESSIONS: &str = r#"
SELECT
  TO_CHAR(s.SESS_ID) AS "session_id",
  NVL(s.USER_NAME, '') AS "username",
  NVL(s.CURR_SCH, '') AS "database_name",
  NVL(s.CLNT_IP, '') AS "client_addr",
  NVL(s.STATE, '') AS "state",
  GREATEST(0, CAST(DATEDIFF(SECOND, NVL(s.LAST_RECV_TIME, SYSDATE), SYSDATE) AS BIGINT)) AS "duration_seconds",
  CAST(NULL AS VARCHAR(1)) AS "wait_event",
  CAST(NULL AS VARCHAR(1)) AS "blocking_pid",
  SUBSTR(NVL(s.SQL_TEXT, ''), 1, 2000) AS "sql_text",
  CASE WHEN s.SESS_ID = SESSID() THEN 1 ELSE 0 END AS "is_current"
FROM V$SESSIONS s
ORDER BY
  CASE WHEN s.STATE = 'ACTIVE' THEN 0 ELSE 1 END,
  "duration_seconds" DESC
LIMIT 200
"#;

/// ClickHouse：`system.processes`（按 `query_id` 标识正在执行的查询）。
const CLICKHOUSE_SESSIONS: &str = r#"
SELECT
  query_id AS session_id,
  user AS username,
  current_database AS database_name,
  toString(address) AS client_addr,
  'active' AS state,
  toInt64(elapsed) AS duration_seconds,
  CAST(NULL AS Nullable(String)) AS wait_event,
  CAST(NULL AS Nullable(String)) AS blocking_pid,
  substringUTF8(query, 1, 2000) AS sql_text,
  toUInt8(query_id = queryID()) AS is_current
FROM system.processes
WHERE is_cancelled = 0
ORDER BY elapsed DESC
LIMIT 200
"#;

/// 旧版 ClickHouse（无 `queryID()` 函数）回退查询。
const CLICKHOUSE_SESSIONS_FALLBACK: &str = r#"
SELECT
  query_id AS session_id,
  user AS username,
  current_database AS database_name,
  toString(address) AS client_addr,
  'active' AS state,
  toInt64(elapsed) AS duration_seconds,
  CAST(NULL AS Nullable(String)) AS wait_event,
  CAST(NULL AS Nullable(String)) AS blocking_pid,
  substringUTF8(query, 1, 2000) AS sql_text,
  toUInt8(0) AS is_current
FROM system.processes
WHERE is_cancelled = 0
ORDER BY elapsed DESC
LIMIT 200
"#;

/// 解析并校验严格的正整数会话 ID（拒绝空白、负号、分号或任意非数字字符）。
fn parse_positive_u64(raw: &str) -> Result<u64, QueryError> {
  if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(QueryError::message(format!("{INVALID_SESSION_ID}: {raw}")));
  }
  let value =
    raw.parse::<u64>().map_err(|_| QueryError::message(format!("{INVALID_SESSION_ID}: {raw}")))?;
  if value == 0 {
    return Err(QueryError::message(format!("{INVALID_SESSION_ID}: {raw}")));
  }
  Ok(value)
}

fn parse_postgres_pid(raw: &str) -> Result<i32, QueryError> {
  let value = parse_positive_u64(raw)?;
  i32::try_from(value).map_err(|_| QueryError::message(format!("{INVALID_SESSION_ID}: {raw}")))
}

/// 解析 Oracle `sid,serial#`（`sid > 0`, `serial# >= 0`，两段均为纯数字）。
fn parse_oracle_session(raw: &str) -> Result<(u64, u64), QueryError> {
  let Some((sid_str, serial_str)) = raw.split_once(',') else {
    return Err(QueryError::message(format!("{INVALID_SESSION_ID}: {raw}")));
  };
  let sid = parse_positive_u64(sid_str)?;
  if serial_str.is_empty() || !serial_str.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(QueryError::message(format!("{INVALID_SESSION_ID}: {raw}")));
  }
  let serial = serial_str
    .parse::<u64>()
    .map_err(|_| QueryError::message(format!("{INVALID_SESSION_ID}: {raw}")))?;
  Ok((sid, serial))
}

/// 校验 ClickHouse `query_id`（拒绝引号、换行、控制字符与过长串）。
fn validate_clickhouse_query_id(raw: &str) -> Result<&str, QueryError> {
  if raw.is_empty()
    || raw.len() > 128
    || !raw.bytes().all(|byte| {
      byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'@' | b'/' | b'-')
    })
  {
    return Err(QueryError::message(format!("{INVALID_SESSION_ID}: {raw}")));
  }
  Ok(raw)
}

/// 在独立非事务连接上安全执行取消/终止会话命令。
pub async fn execute_session_action<'a>(
  pool: PoolRef<'a>,
  action: SessionAction,
  session_id: &str,
) -> Result<TerminateSessionResult, QueryError> {
  match pool {
    PoolRef::Sqlx(DbPool::MySql(pool)) => {
      let id = parse_positive_u64(session_id)?;
      let statement = match action {
        SessionAction::Cancel => format!("KILL QUERY {id}"),
        SessionAction::Terminate => format!("KILL CONNECTION {id}"),
      };
      pool.execute(sqlx::raw_sql(&statement)).await.map_err(QueryError::from)?;
      Ok(TerminateSessionResult { applied: true, statement })
    }
    PoolRef::Sqlx(DbPool::Postgres(pool)) => {
      let pid = parse_postgres_pid(session_id)?;
      let (query, statement) = match action {
        SessionAction::Cancel => {
          ("SELECT pg_cancel_backend($1)", format!("SELECT pg_cancel_backend({pid})"))
        }
        SessionAction::Terminate => {
          ("SELECT pg_terminate_backend($1)", format!("SELECT pg_terminate_backend({pid})"))
        }
      };
      let applied: bool =
        sqlx::query_scalar(query).bind(pid).fetch_one(pool).await.map_err(QueryError::from)?;
      Ok(TerminateSessionResult { applied, statement })
    }
    PoolRef::SqlServer(pool) => {
      if action != SessionAction::Terminate {
        return Err(QueryError::message(format!("{SESSION_ACTION_UNSUPPORTED}: SqlServer")));
      }
      let spid = parse_positive_u64(session_id)?;
      let statement = format!("KILL {spid}");
      let mut conn = pool.acquire_for_session().await?;
      conn
        .execute_streaming(&statement, StreamOptions::limited(1, 4096, 1), &mut |_| Ok(()))
        .await?;
      Ok(TerminateSessionResult { applied: true, statement })
    }
    PoolRef::Oracle(pool) => {
      if action != SessionAction::Terminate {
        return Err(QueryError::message(format!("{SESSION_ACTION_UNSUPPORTED}: Oracle")));
      }
      let (sid, serial) = parse_oracle_session(session_id)?;
      let statement = format!("ALTER SYSTEM KILL SESSION '{sid},{serial}' IMMEDIATE");
      let mut conn = pool.acquire_for_session().await?;
      conn.execute_with_params(&statement, &[]).await?;
      Ok(TerminateSessionResult { applied: true, statement })
    }
    PoolRef::Dameng(pool) => {
      if action != SessionAction::Terminate {
        return Err(QueryError::message(format!("{SESSION_ACTION_UNSUPPORTED}: Dameng")));
      }
      let sess_id = parse_positive_u64(session_id)?;
      let statement = format!("CALL SP_CLOSE_SESSION({sess_id})");
      let mut conn = pool.acquire_for_session().await?;
      conn.execute_batch(&statement).await?;
      Ok(TerminateSessionResult { applied: true, statement })
    }
    PoolRef::ClickHouse(pool) => {
      if action != SessionAction::Cancel {
        return Err(QueryError::message(format!("{SESSION_ACTION_UNSUPPORTED}: ClickHouse")));
      }
      let query_id = validate_clickhouse_query_id(session_id)?;
      let statement = format!("KILL QUERY WHERE query_id = '{query_id}' SYNC");
      pool
        .write_batch(&[WriteStatement {
          sql: "KILL QUERY WHERE query_id = {p1:String} SYNC".to_string(),
          params: vec![JsonValue::String(query_id.to_string())],
          expect_rows: None,
        }])
        .await
        .map_err(|batch_err| batch_err.error)?;
      Ok(TerminateSessionResult { applied: true, statement })
    }
    PoolRef::Sqlx(DbPool::Sqlite(_)) => {
      Err(QueryError::message(format!("{ACTIVE_SESSIONS_UNSUPPORTED}: SQLite")))
    }
    PoolRef::DuckDb(_) => {
      Err(QueryError::message(format!("{ACTIVE_SESSIONS_UNSUPPORTED}: DuckDB")))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const SUPPORTED: [DatabaseType; 5] = [
    DatabaseType::MySQL,
    DatabaseType::PostgreSQL,
    DatabaseType::SqlServer,
    DatabaseType::Oracle,
    DatabaseType::ClickHouse,
  ];

  const REQUIRED_ALIASES: [&str; 10] = [
    "session_id",
    "username",
    "database_name",
    "client_addr",
    "state",
    "duration_seconds",
    "wait_event",
    "blocking_pid",
    "sql_text",
    "is_current",
  ];

  fn assert_projects_all_columns(label: &str, sql: &str) {
    for alias in REQUIRED_ALIASES {
      assert!(
        sql.contains(&format!("AS {alias}")) || sql.contains(&format!("AS \"{alias}\"")),
        "{label} 缺少统一投影列 {alias}"
      );
    }
  }

  #[test]
  fn every_supported_dialect_projects_all_ten_columns_without_params() {
    for db_type in SUPPORTED {
      let query = active_sessions_query(&db_type).expect("supported dialect");
      assert_projects_all_columns(&format!("{db_type:?}"), query.sql);
      assert_eq!(query.sql.matches('?').count(), 0, "{db_type:?} 主查询不应需要参数");
      if let Some(fallback) = query.fallback_sql {
        assert_projects_all_columns(&format!("{db_type:?} fallback"), fallback);
        assert_eq!(fallback.matches('?').count(), 0, "{db_type:?} 回退查询不应需要参数");
      }
      assert!(query.supports_cancel || query.supports_terminate, "{db_type:?} 至少应提供一种动作");
    }
  }

  #[test]
  fn oracle_quotes_all_column_aliases_to_preserve_lowercase() {
    let query = active_sessions_query(&DatabaseType::Oracle).expect("oracle");
    for alias in REQUIRED_ALIASES {
      assert!(
        query.sql.contains(&format!("AS \"{alias}\"")),
        "Oracle 别名 {alias} 必须加双引号，否则会被折成全大写"
      );
    }
  }

  #[test]
  fn embedded_and_non_sql_databases_are_unsupported() {
    for db_type in [
      DatabaseType::SQLite,
      DatabaseType::DuckDB,
      DatabaseType::MongoDB,
      DatabaseType::Redis,
      DatabaseType::Neo4j,
      DatabaseType::Elasticsearch,
    ] {
      assert!(active_sessions_query(&db_type).is_none(), "{db_type:?} 不应暴露关系库会话查询");
    }
  }

  #[test]
  fn session_id_parsers_reject_injection_and_malformed_inputs() {
    assert_eq!(parse_positive_u64("42").expect("valid u64"), 42);
    for bad in ["", "0", "-1", "42; DROP TABLE t", "42 ", " 42", "abc", "1,2"] {
      assert!(parse_positive_u64(bad).is_err(), "应拒绝非法数字 ID: {bad:?}");
    }

    assert_eq!(parse_postgres_pid("12345").expect("valid pid"), 12345);
    assert!(parse_postgres_pid("99999999999999").is_err(), "超出 i32 的 PID 应被拒");

    assert_eq!(parse_oracle_session("128,4567").expect("valid oracle sid"), (128, 4567));
    assert_eq!(parse_oracle_session("1,0").expect("serial 0 allowed"), (1, 0));
    for bad in ["", "128", "0,10", "128,", "128,1'; DROP USER x--", "128, 1"] {
      assert!(parse_oracle_session(bad).is_err(), "应拒绝非法 Oracle SID: {bad:?}");
    }

    assert_eq!(
      validate_clickhouse_query_id("a1b2c3-d4e5_f6:7@node/1").expect("valid query_id"),
      "a1b2c3-d4e5_f6:7@node/1"
    );
    for bad in ["", "abc' OR '1'='1", "abc; DROP TABLE x", "abc\ndef", "abc def"] {
      assert!(
        validate_clickhouse_query_id(bad).is_err(),
        "应拒绝非法 ClickHouse query_id: {bad:?}"
      );
    }
  }
}
