//! 达梦 DM8 连接与查询执行（经可选 `odbc` Cargo feature 对接官方 `libdodbc`/`dodbc` 驱动，DC5）。
//!
//! 为什么采用 ODBC 而不是逆向纯 Rust 协议驱动：见 `rfcs/driver-and-gap-closure.md` §2.2。
//! 实测 `dm-pure` 存在非行内 CLOB 静默为空、含 LOB 参数查询丢尾列、`EXPLAIN` 挂死与无 TLS 四项硬伤；
//! 本模块不随包分发任何闭源驱动库，仅通过标准 ODBC 接口加载用户本机安装的官方达梦 ODBC 驱动。

use crate::models::{ConnectionProfile, TlsMode};
use crate::services::query_error::QueryError;
use serde_json::Value as JsonValue;

/// 前端据此走 `dameng_select` / `close_dameng`，后端据此进 [`DamengRegistry`]。
pub const DAMENG_SCHEME: &str = "dameng://";

/// 找不到用户配置的 ODBC 驱动或系统未安装 unixODBC / 驱动管理器
pub const DAMENG_DRIVER_MISSING: &str = "DATAOMNI_DAMENG_DRIVER_MISSING";
/// 达梦 ODBC 连接失败（保留驱动原始报错详情）
pub const DAMENG_CONNECT_FAILED: &str = "DATAOMNI_DAMENG_CONNECT_FAILED";
/// 需要证书校验或未提供达梦 SSL 证书目录
pub const DAMENG_TLS_UNSUPPORTED: &str = "DATAOMNI_DAMENG_TLS_UNSUPPORTED";

/// 环境变量默认驱动路径/名称（常用于自动化冒烟测试或统一部署）
const DAMENG_ODBC_DRIVER_ENV: &str = "DATAOMNI_DM8_ODBC_DRIVER";
/// Windows 达梦官方安装包默认注册的 ODBC 驱动名
pub const DEFAULT_DAMENG_ODBC_DRIVER: &str = "DM8 ODBC DRIVER";

pub type DamengRegistry = crate::services::pool_registry::PoolRegistry<DamengPool>;

/// 达梦连接目标参数（所有平台/特性下均可用，便于单测验证连接串转义与校验）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamengTarget {
  pub host: String,
  pub port: u16,
  pub username: String,
  pub password: String,
  pub schema: Option<String>,
  pub driver: String,
  pub ssl_path: Option<String>,
}

impl DamengTarget {
  pub fn from_profile(profile: &ConnectionProfile) -> Result<Self, String> {
    let tls_mode = profile.effective_tls_mode();
    let ssl_path = profile
      .ca_certificate_path
      .as_deref()
      .map(str::trim)
      .filter(|value| !value.is_empty())
      .map(str::to_string);

    if matches!(tls_mode, TlsMode::Required | TlsMode::VerifyCa | TlsMode::VerifyFull)
      && ssl_path.is_none()
    {
      return Err(DAMENG_TLS_UNSUPPORTED.to_string());
    }

    let driver = profile
      .dameng_odbc_driver()
      .map(str::to_string)
      .or_else(|| {
        std::env::var(DAMENG_ODBC_DRIVER_ENV)
          .ok()
          .map(|v| v.trim().to_string())
          .filter(|v| !v.is_empty())
      })
      .unwrap_or_else(|| DEFAULT_DAMENG_ODBC_DRIVER.to_string());

    let schema = profile
      .database
      .as_deref()
      .map(str::trim)
      .filter(|value| !value.is_empty())
      .map(str::to_string);

    Ok(Self {
      host: profile.host.trim().to_string(),
      port: profile.port,
      username: profile.username.clone(),
      password: profile.password.clone(),
      schema,
      driver,
      ssl_path,
    })
  }

  /// 生成 ODBC 连接串。含 `;`、`{`、`}` 或首尾空白的值按 ODBC 规范包进 `{...}` 且 `}` 转义为 `}}`。
  pub fn odbc_connection_string(&self) -> String {
    let mut parts = vec![
      format!("DRIVER={}", odbc_value(&self.driver)),
      format!("SERVER={}", odbc_value(&self.host)),
      format!("TCP_PORT={}", self.port),
      format!("UID={}", odbc_value(&self.username)),
      format!("PWD={}", odbc_value(&self.password)),
    ];
    if let Some(schema) = &self.schema {
      parts.push(format!("SCHEMA={}", odbc_value(schema)));
    }
    if let Some(ssl_path) = &self.ssl_path {
      parts.push(format!("SSL_PATH={}", odbc_value(ssl_path)));
    }
    parts.join(";")
  }
}

fn odbc_value(raw: &str) -> String {
  let needs_braces = raw.contains(';')
    || raw.contains('{')
    || raw.contains('}')
    || raw.starts_with(char::is_whitespace)
    || raw.ends_with(char::is_whitespace);
  if needs_braces {
    format!("{{{}}}", raw.replace('}', "}}"))
  } else {
    raw.to_string()
  }
}

/// 这些语句自己控制事务边界（同 Oracle 规则）
pub fn controls_transaction(sql: &str) -> bool {
  let (first, second) = crate::services::transaction_state::leading_keywords(sql);
  matches!(first.as_str(), "COMMIT" | "ROLLBACK" | "SAVEPOINT")
    || (first == "SET" && second == "TRANSACTION")
}

/// 剥掉单条普通 SQL 末尾的分号，保留 `BEGIN ... END;` / `CREATE PROCEDURE ...` 等 PL/SQL 块尾部分号
pub fn statement_text(sql: &str) -> String {
  let trimmed = sql.trim();
  if is_plsql(trimmed) {
    return trimmed.to_string();
  }
  trimmed.trim_end_matches(|ch: char| ch == ';' || ch.is_whitespace()).to_string()
}

fn is_plsql(sql: &str) -> bool {
  let (first, second) = crate::services::transaction_state::leading_keywords(sql);
  match first.as_str() {
    "BEGIN" | "DECLARE" => true,
    "CREATE" => {
      let words: Vec<String> =
        sql.split_whitespace().take(6).map(|word| word.to_ascii_uppercase()).collect();
      let kind_at = if second == "OR" { 3 } else { 1 };
      words.get(kind_at).is_some_and(|kind| {
        matches!(kind.as_str(), "PROCEDURE" | "FUNCTION" | "PACKAGE" | "TRIGGER" | "TYPE")
      })
    }
    _ => false,
  }
}

/// 生成达梦 `dm_svc.conf` 内容，按当前本地时区偏移写入并钉死 `CHAR_CODE=(PG_UTF8)`。
#[cfg(any(feature = "odbc", test))]
fn dm_svc_conf_contents(utc_offset_minutes: i32) -> String {
  format!("TIME_ZONE=({utc_offset_minutes})\nLANGUAGE=(cn)\nCHAR_CODE=(PG_UTF8)\n")
}

/// 在用户私有配置目录下原子写入 `dm8/dm_svc.conf`，避免使用公共 `/tmp` 目录产生抢占或符号链接风险。
#[cfg(any(feature = "odbc", test))]
fn write_dm_svc_conf_in(base_dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
  let dir = base_dir.join("dm8");
  std::fs::create_dir_all(&dir)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
  }
  let offset_minutes = chrono::Local::now().offset().local_minus_utc() / 60;
  let contents = dm_svc_conf_contents(offset_minutes);
  let conf = dir.join("dm_svc.conf");
  let part = dir.join("dm_svc.conf.part");
  std::fs::write(&part, contents)?;
  std::fs::rename(&part, &conf)?;
  Ok(dir)
}

/// 在进程仍处于单线程的启动阶段（`main()` 开头）初始化达梦官方驱动的 `DM_SVC_PATH`。
///
/// 若外部环境已显式设置 `DM_SVC_PATH`，完全尊重不覆盖；否则写入用户私有配置目录
/// `<app_config_dir>/dm8/dm_svc.conf` 并设置环境变量。严禁在多线程工作线程中调用 `set_var`。
pub fn init_dm_svc_env() {
  #[cfg(feature = "odbc")]
  {
    if std::env::var_os("DM_SVC_PATH").is_some() {
      return;
    }
    if let Some(config_dir) = crate::services::connection_service::app_config_dir() {
      if let Ok(dir) = write_dm_svc_conf_in(&config_dir) {
        std::env::set_var("DM_SVC_PATH", &dir);
      }
    }
  }
}

/// 判断是否属于达梦原生「只读事务内禁止写」等连接或驱动错误
#[cfg(feature = "odbc")]
fn format_odbc_error(error: &odbc_api::Error) -> QueryError {
  let message = error.to_string();
  let lowered = message.to_ascii_lowercase();
  if lowered.contains("can't open lib")
    || lowered.contains("data source name not found")
    || lowered.contains("file not found")
    || lowered.contains("im002")
  {
    return QueryError::message(format!("{DAMENG_DRIVER_MISSING}: {message}"));
  }
  QueryError::message(message)
}

#[cfg(not(feature = "odbc"))]
mod disabled {
  use super::*;
  use crate::services::query_executor::{
    QueryColumnMetadata, QueryExecutionSummary, QueryResultBatch, QueryRow, StreamOptions,
  };
  use crate::services::transaction_state::TransactionState;
  use crate::services::write_batch::{WriteBatchError, WriteStatement};
  use std::sync::Arc;

  /// 未编入 `odbc` feature 时不可构造的池类型：类型系统保证登记表恒空、零运行期分支
  #[derive(Debug)]
  pub enum DamengPool {}

  #[derive(Debug)]
  pub enum DamengConnection {}

  impl DamengPool {
    pub async fn acquire_for_session(&self) -> Result<DamengConnection, QueryError> {
      match *self {}
    }

    pub async fn select(
      self: &Arc<Self>,
      _sql: &str,
      _params: &[JsonValue],
    ) -> Result<Vec<QueryRow>, QueryError> {
      match **self {}
    }

    pub async fn write_batch(
      self: &Arc<Self>,
      _statements: &[WriteStatement],
    ) -> Result<Vec<u64>, WriteBatchError> {
      match **self {}
    }
  }

  impl DamengConnection {
    pub async fn ping(&mut self, _limit: std::time::Duration) -> bool {
      match *self {}
    }

    pub async fn execute_streaming(
      &mut self,
      _sql: &str,
      _options: StreamOptions,
      _sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
    ) -> Result<QueryExecutionSummary, QueryError> {
      match *self {}
    }

    pub async fn execute_streaming_with_params(
      &mut self,
      _sql: &str,
      _params: &[JsonValue],
      _options: StreamOptions,
      _sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
    ) -> Result<QueryExecutionSummary, QueryError> {
      match *self {}
    }

    pub fn set_autocommit(&mut self, _autocommit: bool) {
      match *self {}
    }

    pub fn transaction(&self) -> TransactionState {
      match *self {}
    }

    pub async fn execute_with_params(
      &mut self,
      _sql: &str,
      _params: &[Option<String>],
    ) -> Result<u64, QueryError> {
      match *self {}
    }

    pub async fn describe_columns(
      &mut self,
      _sql: &str,
    ) -> Result<Vec<QueryColumnMetadata>, QueryError> {
      match *self {}
    }

    pub async fn execute_batch(&mut self, _sql: &str) -> Result<u64, QueryError> {
      match *self {}
    }
  }
}

#[cfg(not(feature = "odbc"))]
pub use disabled::{DamengConnection, DamengPool};

#[cfg(feature = "odbc")]
mod enabled {
  use super::*;
  use crate::services::query_error::{CONNECTION_LOST, CONNECTION_LOST_CODE};
  use crate::services::query_executor::{
    admit_row_bytes, binary_json, flush_remaining_batch, number_duplicate_columns,
    refuse_non_query, refuse_use_statement, tagged_value, NonQueryHandling, QueryColumnMetadata,
    QueryExecutionSummary, QueryResultBatch, QueryRow, QueryTruncationReason, StreamOptions,
  };
  use crate::services::transaction_state::{TransactionState, TransactionStatus};
  use crate::services::write_batch::{
    WriteBatchError, WriteStatement, ROW_COUNT_MISMATCH, ROW_COUNT_MISMATCH_CODE,
    UNSUPPORTED_PARAMETER_TYPE,
  };
  use odbc_api::parameter::InputParameter;
  use odbc_api::{
    ColumnDescription, Connection, ConnectionOptions, Cursor, DataType, Environment, IntoParameter,
    ResultSetMetadata,
  };
  use serde_json::Map;
  use std::sync::{Arc, Mutex, OnceLock};
  use std::time::Duration;

  const MAX_IDLE: usize = 4;
  const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;
  const CONNECT_TIMEOUT_SECS: u32 = 15;

  type SharedOdbcConn = Arc<Mutex<Connection<'static>>>;

  static ODBC_ENV: OnceLock<Result<&'static Environment, String>> = OnceLock::new();

  fn environment() -> Result<&'static Environment, QueryError> {
    match ODBC_ENV.get_or_init(|| {
      Environment::new()
        .map(|env| Box::leak(Box::new(env)) as &'static Environment)
        .map_err(|err| format!("{DAMENG_DRIVER_MISSING}: {err}"))
    }) {
      Ok(env) => Ok(*env),
      Err(message) => Err(QueryError::message(message.clone())),
    }
  }

  async fn blocking<T, F>(work: F) -> Result<T, QueryError>
  where
    T: Send + 'static,
    F: FnOnce() -> Result<T, QueryError> + Send + 'static,
  {
    tokio::task::spawn_blocking(work).await.map_err(|error| {
      QueryError::with_code(CONNECTION_LOST_CODE, format!("{CONNECTION_LOST}: {error}"))
    })?
  }

  pub async fn connect(target: &DamengTarget) -> Result<SharedOdbcConn, QueryError> {
    let target = target.clone();
    blocking(move || {
      let env = environment()?;
      let conn_str = target.odbc_connection_string();
      let conn = env
        .connect_with_connection_string(
          &conn_str,
          ConnectionOptions { login_timeout_sec: Some(CONNECT_TIMEOUT_SECS), ..Default::default() },
        )
        .map_err(|err| {
          let mapped = format_odbc_error(&err);
          if mapped.message.starts_with(DAMENG_DRIVER_MISSING) {
            mapped
          } else {
            QueryError::message(format!("{DAMENG_CONNECT_FAILED}: {}", mapped.message))
          }
        })?;
      // 探活一次确保链路与权限完整可用
      if let Some(mut cursor) =
        conn.execute("SELECT 1 FROM DUAL", (), None).map_err(|err| format_odbc_error(&err))?
      {
        let _ = cursor.next_row();
      }
      Ok(Arc::new(Mutex::new(conn)))
    })
    .await
  }

  fn release_in_background(connections: Vec<SharedOdbcConn>) {
    if !connections.is_empty() {
      std::thread::spawn(move || drop(connections));
    }
  }

  pub struct DamengPool {
    target: DamengTarget,
    idle: crate::services::pool_registry::IdleConnections<SharedOdbcConn>,
  }

  impl DamengPool {
    pub fn new(target: DamengTarget, first: SharedOdbcConn) -> Arc<Self> {
      Arc::new(Self {
        target,
        idle: crate::services::pool_registry::IdleConnections::new(first, MAX_IDLE),
      })
    }

    pub async fn acquire_for_session(&self) -> Result<DamengConnection, QueryError> {
      Ok(DamengConnection {
        connection: Some(connect(&self.target).await?),
        target: self.target.clone(),
        autocommit: true,
        transaction: TransactionState::default(),
      })
    }

    pub async fn select(
      self: &Arc<Self>,
      sql: &str,
      params: &[JsonValue],
    ) -> Result<Vec<QueryRow>, QueryError> {
      let (idle, stale) = self.idle.take();
      release_in_background(stale);
      let Some(conn) = idle else {
        return self.select_on(connect(&self.target).await?, sql, params).await;
      };
      let result = self.select_on(conn, sql, params).await;
      if keeps_connection(&result) {
        return result;
      }
      release_in_background(self.idle.drain());
      self.select_on(connect(&self.target).await?, sql, params).await
    }

    async fn select_on(
      self: &Arc<Self>,
      connection: SharedOdbcConn,
      sql: &str,
      params: &[JsonValue],
    ) -> Result<Vec<QueryRow>, QueryError> {
      let sql = statement_text(sql);
      let params = params.to_vec();
      let worker = Arc::clone(&connection);
      let result = blocking(move || select_rows(&worker, &sql, &params)).await;
      if keeps_connection(&result) {
        self.idle.put(connection);
      }
      result
    }

    pub async fn write_batch(
      self: &Arc<Self>,
      statements: &[WriteStatement],
    ) -> Result<Vec<u64>, WriteBatchError> {
      if statements.is_empty() {
        return Ok(Vec::new());
      }
      let (idle, stale) = self.idle.take();
      release_in_background(stale);
      let connection = match idle {
        Some(conn) => conn,
        None => connect(&self.target).await.map_err(|error| WriteBatchError::at(0, error))?,
      };
      let batch: Vec<(String, Vec<JsonValue>, Option<u64>)> = statements
        .iter()
        .map(|stmt| (statement_text(&stmt.sql), stmt.params.clone(), stmt.expect_rows))
        .collect();
      let worker = Arc::clone(&connection);
      let outcome = blocking(move || Ok(write_in_transaction(&worker, &batch)))
        .await
        .map_err(|error| WriteBatchError::at(0, error))?;
      let reusable = match &outcome {
        Ok(_) => true,
        Err((_, error)) => error.code.as_deref() != Some(CONNECTION_LOST_CODE),
      };
      if reusable {
        self.idle.put(connection);
      } else {
        release_in_background(self.idle.drain());
      }
      outcome.map_err(|(index, error)| WriteBatchError::at(index, error))
    }
  }

  fn keeps_connection<T>(result: &Result<T, QueryError>) -> bool {
    match result {
      Ok(_) => true,
      Err(error) => error.code.as_deref() != Some(CONNECTION_LOST_CODE),
    }
  }

  fn lock_conn<'a>(
    conn: &'a SharedOdbcConn,
  ) -> Result<std::sync::MutexGuard<'a, Connection<'static>>, QueryError> {
    conn
      .lock()
      .map_err(|_| QueryError::with_code(CONNECTION_LOST_CODE, CONNECTION_LOST.to_string()))
  }

  /// 将 JSON 标量参数转为 ODBC 绑定参数列表
  fn build_odbc_params(params: &[JsonValue]) -> Result<Vec<Box<dyn InputParameter>>, QueryError> {
    let mut out: Vec<Box<dyn InputParameter>> = Vec::with_capacity(params.len());
    for param in params {
      match param {
        JsonValue::Null => out.push(Box::new(None::<String>.into_parameter())),
        JsonValue::Bool(value) => out.push(Box::new(i32::from(*value).into_parameter())),
        JsonValue::Number(number) => {
          if let Some(int_val) = number.as_i64() {
            out.push(Box::new(int_val.into_parameter()));
          } else if let Some(float_val) = number.as_f64() {
            out.push(Box::new(float_val.into_parameter()));
          } else {
            out.push(Box::new(number.to_string().into_parameter()));
          }
        }
        JsonValue::String(text) => out.push(Box::new(text.clone().into_parameter())),
        other => {
          return Err(QueryError::message(format!("{UNSUPPORTED_PARAMETER_TYPE}: {other}")));
        }
      }
    }
    Ok(out)
  }

  fn describe_cursor_columns(
    cursor: &mut impl Cursor,
  ) -> Result<Vec<(String, DataType, bool)>, QueryError> {
    let num_cols = cursor.num_result_cols().map_err(|err| format_odbc_error(&err))?;
    let count = u16::try_from(num_cols).unwrap_or(0);
    let mut columns = Vec::with_capacity(usize::from(count));
    for idx in 1..=count {
      let mut desc = ColumnDescription::default();
      cursor.describe_col(idx, &mut desc).map_err(|err| format_odbc_error(&err))?;
      let name = desc.name_to_string().unwrap_or_else(|_| format!("col_{idx}"));
      let nullable = !matches!(desc.nullability, odbc_api::Nullability::NoNulls);
      columns.push((name, desc.data_type, nullable));
    }
    Ok(columns)
  }

  fn logical_type(data_type: &DataType) -> &'static str {
    match data_type {
      DataType::BigInt | DataType::Integer | DataType::SmallInt | DataType::TinyInt => "integer",
      DataType::Decimal { scale: 0, .. } | DataType::Numeric { scale: 0, .. } => "integer",
      DataType::Decimal { .. }
      | DataType::Numeric { .. }
      | DataType::Float { .. }
      | DataType::Real
      | DataType::Double => "decimal",
      DataType::Date | DataType::Time { .. } | DataType::Timestamp { .. } => "datetime",
      DataType::Binary { .. } | DataType::Varbinary { .. } | DataType::LongVarbinary { .. } => {
        "binary"
      }
      DataType::Bit => "boolean",
      _ => "text",
    }
  }

  fn database_type_name(data_type: &DataType) -> String {
    match data_type {
      DataType::BigInt => "BIGINT".to_string(),
      DataType::Integer => "INTEGER".to_string(),
      DataType::SmallInt => "SMALLINT".to_string(),
      DataType::TinyInt => "TINYINT".to_string(),
      DataType::Decimal { precision, scale } => format!("DECIMAL({precision},{scale})"),
      DataType::Numeric { precision, scale } => format!("NUMERIC({precision},{scale})"),
      DataType::Float { .. } => "FLOAT".to_string(),
      DataType::Real => "REAL".to_string(),
      DataType::Double => "DOUBLE".to_string(),
      DataType::Bit => "BIT".to_string(),
      DataType::Date => "DATE".to_string(),
      DataType::Time { .. } => "TIME".to_string(),
      DataType::Timestamp { precision } => format!("TIMESTAMP({precision})"),
      DataType::Binary { .. } => "BINARY".to_string(),
      DataType::Varbinary { .. } => "VARBINARY".to_string(),
      DataType::LongVarbinary { .. } => "BLOB".to_string(),
      DataType::LongVarchar { .. } | DataType::WLongVarchar { .. } => "CLOB".to_string(),
      DataType::Char { .. } | DataType::WChar { .. } => "CHAR".to_string(),
      DataType::Varchar { .. } | DataType::WVarchar { .. } => "VARCHAR".to_string(),
      other => format!("{other:?}"),
    }
  }

  fn column_header(
    columns: &[(String, DataType, bool)],
  ) -> (Vec<QueryColumnMetadata>, Vec<String>) {
    let names = number_duplicate_columns(columns.iter().map(|(name, _, _)| name.as_str()));
    let metadata = columns
      .iter()
      .zip(&names)
      .enumerate()
      .map(|(ordinal, ((_, data_type, nullable), name))| QueryColumnMetadata {
        name: name.clone(),
        ordinal,
        database_type: database_type_name(data_type),
        logical_type: logical_type(data_type).to_string(),
        nullable: Some(*nullable),
      })
      .collect();
    (metadata, names)
  }

  fn leading_zero(text: &str) -> String {
    if let Some(rest) = text.strip_prefix("-.") {
      return format!("-0.{rest}");
    }
    if let Some(rest) = text.strip_prefix('.') {
      return format!("0.{rest}");
    }
    text.to_string()
  }

  /// 解码单个 ODBC 单元格：
  /// - `LongVarchar`/`LongWvarchar`（CLOB）使用 `get_wide_text` 彻底规避达梦驱动 indicator 报字符数引发的 panic；
  /// - `Binary`/`Varbinary`/`LongVarbinary` 使用 `get_binary`；
  /// - 普通字符列使用预分配 4096 字节的 `get_text` 保真完整 4 字节 UTF-8 Emoji。
  fn decode_cell(
    row: &mut odbc_api::CursorRow<'_>,
    col_index: u16,
    data_type: &DataType,
  ) -> Result<JsonValue, QueryError> {
    match data_type {
      DataType::Binary { .. } | DataType::Varbinary { .. } | DataType::LongVarbinary { .. } => {
        let mut buf = Vec::with_capacity(4096);
        let not_null =
          row.get_binary(col_index, &mut buf).map_err(|err| format_odbc_error(&err))?;
        if !not_null {
          return Ok(JsonValue::Null);
        }
        Ok(binary_json(&buf))
      }
      DataType::LongVarchar { .. } | DataType::WLongVarchar { .. } => {
        let mut wbuf = Vec::with_capacity(4096);
        let not_null =
          row.get_wide_text(col_index, &mut wbuf).map_err(|err| format_odbc_error(&err))?;
        if !not_null {
          return Ok(JsonValue::Null);
        }
        Ok(JsonValue::String(String::from_utf16_lossy(&wbuf)))
      }
      _ => {
        let mut buf = Vec::with_capacity(4096);
        let not_null = row.get_text(col_index, &mut buf).map_err(|err| format_odbc_error(&err))?;
        if !not_null {
          return Ok(JsonValue::Null);
        }
        let text = String::from_utf8_lossy(&buf).into_owned();
        Ok(classify_text_value(data_type, text))
      }
    }
  }

  fn classify_text_value(data_type: &DataType, text: String) -> JsonValue {
    match data_type {
      DataType::Bit => {
        let trimmed = text.trim();
        JsonValue::Bool(trimmed == "1" || trimmed.eq_ignore_ascii_case("true"))
      }
      DataType::TinyInt | DataType::SmallInt | DataType::Integer | DataType::BigInt => {
        let trimmed = text.trim();
        match trimmed.parse::<i64>() {
          Ok(num) if num.unsigned_abs() <= MAX_SAFE_INTEGER.unsigned_abs() => JsonValue::from(num),
          _ => tagged_value("bigint", trimmed.to_string()),
        }
      }
      DataType::Decimal { scale: 0, .. } | DataType::Numeric { scale: 0, .. } => {
        let trimmed = text.trim();
        match trimmed.parse::<i64>() {
          Ok(num) if num.unsigned_abs() <= MAX_SAFE_INTEGER.unsigned_abs() => JsonValue::from(num),
          _ => tagged_value("bigint", trimmed.to_string()),
        }
      }
      DataType::Decimal { .. } | DataType::Numeric { .. } => {
        tagged_value("decimal", leading_zero(text.trim()))
      }
      DataType::Float { .. } | DataType::Real | DataType::Double => {
        let trimmed = text.trim();
        match trimmed.parse::<f64>().ok().and_then(serde_json::Number::from_f64) {
          Some(num) => JsonValue::Number(num),
          None => JsonValue::String(trimmed.to_string()),
        }
      }
      DataType::Date => tagged_value("date", text.trim().to_string()),
      DataType::Time { .. } => tagged_value("time", text.trim().to_string()),
      DataType::Timestamp { .. } => tagged_value("datetime", text.trim().to_string()),
      _ => JsonValue::String(text),
    }
  }

  fn untagged(value: JsonValue) -> JsonValue {
    match value {
      JsonValue::Object(mut map) => map.remove("value").unwrap_or(JsonValue::Object(map)),
      other => other,
    }
  }

  fn select_rows(
    connection: &SharedOdbcConn,
    sql: &str,
    params: &[JsonValue],
  ) -> Result<Vec<QueryRow>, QueryError> {
    let conn = lock_conn(connection)?;
    let odbc_params = build_odbc_params(params)?;
    let cursor_opt =
      conn.execute(sql, &odbc_params[..], None).map_err(|err| format_odbc_error(&err))?;
    let Some(mut cursor) = cursor_opt else {
      return Ok(Vec::new());
    };
    let columns = describe_cursor_columns(&mut cursor)?;
    let (_, names) = column_header(&columns);
    let mut rows = Vec::new();
    while let Some(mut row) = cursor.next_row().map_err(|err| format_odbc_error(&err))? {
      let mut map = Map::new();
      for (idx, ((_, data_type, _), name)) in columns.iter().zip(&names).enumerate() {
        let cell = decode_cell(&mut row, u16::try_from(idx + 1).unwrap_or(1), data_type)?;
        map.insert(name.clone(), untagged(cell));
      }
      rows.push(map);
    }
    Ok(rows)
  }

  fn write_in_transaction(
    connection: &SharedOdbcConn,
    batch: &[(String, Vec<JsonValue>, Option<u64>)],
  ) -> Result<Vec<u64>, (usize, QueryError)> {
    let conn = lock_conn(connection).map_err(|err| (0, err))?;
    conn.set_autocommit(false).map_err(|err| (0, format_odbc_error(&err)))?;
    let mut affected = Vec::with_capacity(batch.len());

    for (index, (sql, params, expect_rows)) in batch.iter().enumerate() {
      let step = || -> Result<u64, QueryError> {
        let odbc_params = build_odbc_params(params)?;
        let mut stmt = conn.prepare(sql).map_err(|err| format_odbc_error(&err))?;
        let cursor_opt = stmt.execute(&odbc_params[..]).map_err(|err| format_odbc_error(&err))?;
        drop(cursor_opt);
        let rows = stmt.row_count().map_err(|err| format_odbc_error(&err))?.unwrap_or(0) as u64;
        if let Some(expected) = expect_rows {
          if rows != *expected {
            return Err(QueryError::with_code(
              ROW_COUNT_MISMATCH_CODE,
              format!("{ROW_COUNT_MISMATCH}: {expected} · {rows}"),
            ));
          }
        }
        Ok(rows)
      };

      match step() {
        Ok(rows) => affected.push(rows),
        Err(error) => {
          let _ = conn.rollback();
          let _ = conn.set_autocommit(true);
          return Err((index, error));
        }
      }
    }

    if let Err(err) = conn.commit() {
      let _ = conn.rollback();
      let _ = conn.set_autocommit(true);
      return Err((batch.len(), format_odbc_error(&err)));
    }
    let _ = conn.set_autocommit(true);
    Ok(affected)
  }

  pub struct DamengConnection {
    connection: Option<SharedOdbcConn>,
    target: DamengTarget,
    autocommit: bool,
    transaction: TransactionState,
  }

  impl DamengConnection {
    async fn take_connection(&mut self) -> Result<SharedOdbcConn, QueryError> {
      match self.connection.take() {
        Some(conn) => Ok(conn),
        None => connect(&self.target).await,
      }
    }

    pub async fn ping(&mut self, limit: Duration) -> bool {
      let Some(conn) = self.connection.take() else {
        return true;
      };
      let worker = Arc::clone(&conn);
      let ok = tokio::time::timeout(
        limit,
        blocking(move || {
          let guard = lock_conn(&worker)?;
          if let Some(mut cur) =
            guard.execute("SELECT 1 FROM DUAL", (), None).map_err(|err| format_odbc_error(&err))?
          {
            let _ = cur.next_row();
          }
          Ok(())
        }),
      )
      .await;
      let alive = matches!(ok, Ok(Ok(())));
      if alive {
        self.connection = Some(conn);
      } else {
        release_in_background(vec![conn]);
      }
      alive
    }

    pub fn set_autocommit(&mut self, autocommit: bool) {
      self.autocommit = autocommit;
    }

    pub fn transaction(&self) -> TransactionState {
      match self.connection {
        Some(_) => self.transaction.clone(),
        None => TransactionState::default(),
      }
    }

    pub async fn execute_streaming(
      &mut self,
      sql: &str,
      options: StreamOptions,
      sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
    ) -> Result<QueryExecutionSummary, QueryError> {
      self.execute_streaming_with_params(sql, &[], options, sink).await
    }

    pub async fn execute_streaming_with_params(
      &mut self,
      sql: &str,
      params: &[JsonValue],
      options: StreamOptions,
      sink: &mut (dyn FnMut(QueryResultBatch) -> Result<(), QueryError> + Send),
    ) -> Result<QueryExecutionSummary, QueryError> {
      refuse_use_statement(sql)?;
      let statement = statement_text(sql);
      let params = params.to_vec();
      let autocommit = self.autocommit;
      let was_in_tx = self.transaction.in_transaction();
      let connection = self.take_connection().await?;
      let worker = Arc::clone(&connection);

      let (first_kw, _) = crate::services::transaction_state::leading_keywords(&statement);
      let is_commit = first_kw == "COMMIT";
      let is_rollback = first_kw == "ROLLBACK";
      let is_begin = first_kw == "BEGIN" || (first_kw == "SET" && controls_transaction(&statement));

      let row_limit = options.row_limit;
      let byte_limit = options.byte_limit;
      let non_query = options.non_query;

      let outcome = blocking(move || {
        let guard = lock_conn(&worker)?;
        // 显式控制 ODBC 连接的 autocommit 状态
        let desired_auto = autocommit && !was_in_tx && !is_begin;
        if is_commit {
          guard.commit().map_err(|err| format_odbc_error(&err))?;
          let _ = guard.set_autocommit(autocommit);
          return Ok((StreamResult::Affected(0), false));
        }
        if is_rollback {
          guard.rollback().map_err(|err| format_odbc_error(&err))?;
          let _ = guard.set_autocommit(autocommit);
          return Ok((StreamResult::Affected(0), false));
        }
        guard.set_autocommit(desired_auto).map_err(|err| format_odbc_error(&err))?;
        let odbc_params = build_odbc_params(&params)?;
        let mut stmt = guard.prepare(&statement).map_err(|err| format_odbc_error(&err))?;
        let num_cols = stmt.num_result_cols().map_err(|err| format_odbc_error(&err))?;
        if num_cols == 0 && non_query == NonQueryHandling::Refuse {
          refuse_non_query(non_query)?;
        }
        let maybe_rows = {
          let cursor_opt = stmt.execute(&odbc_params[..]).map_err(|err| format_odbc_error(&err))?;
          if let Some(mut cursor) = cursor_opt {
            let columns = describe_cursor_columns(&mut cursor)?;
            let (column_metadata, names) = column_header(&columns);
            let mut rows = Vec::new();
            let mut bytes_read = 0usize;
            let mut truncation_reason = None;

            while let Some(mut row) = cursor.next_row().map_err(|err| format_odbc_error(&err))? {
              if rows.len() >= row_limit {
                truncation_reason = Some(QueryTruncationReason::RowLimit);
                break;
              }
              let mut map = Map::new();
              for (idx, ((_, data_type, _), name)) in columns.iter().zip(&names).enumerate() {
                let cell = decode_cell(&mut row, u16::try_from(idx + 1).unwrap_or(1), data_type)?;
                map.insert(name.clone(), cell);
              }
              if !admit_row_bytes(&map, byte_limit, &mut bytes_read)? && !rows.is_empty() {
                truncation_reason = Some(QueryTruncationReason::ByteLimit);
                break;
              }
              rows.push(map);
            }
            Some(StreamResult::Rows {
              columns: names,
              column_metadata,
              rows,
              bytes_read,
              truncation_reason,
            })
          } else {
            None
          }
        };

        let now_in_tx = !desired_auto;
        if let Some(rows_result) = maybe_rows {
          Ok((rows_result, now_in_tx))
        } else {
          let affected =
            stmt.row_count().map_err(|err| format_odbc_error(&err))?.unwrap_or(0) as u64;
          Ok((StreamResult::Affected(affected), now_in_tx))
        }
      })
      .await;

      match outcome {
        Ok((result, now_in_tx)) => {
          self.connection = Some(connection);
          if now_in_tx {
            let started_at =
              self.transaction.started_at.clone().or_else(|| Some(chrono::Utc::now().to_rfc3339()));
            self.transaction = TransactionState { status: TransactionStatus::Active, started_at };
          } else {
            self.transaction = TransactionState::default();
          }
          match result {
            StreamResult::Affected(rows_affected) => {
              Ok(QueryExecutionSummary::Affected { rows_affected })
            }
            StreamResult::Rows {
              columns,
              column_metadata,
              rows,
              bytes_read,
              truncation_reason,
            } => {
              let row_count = rows.len();
              let mut batch_count = 0usize;
              let mut emitted = 0usize;
              let mut pending = Vec::new();
              for row in rows {
                pending.push(row);
                if pending.len() >= options.batch_size {
                  let batch_rows = std::mem::take(&mut pending);
                  let offset = emitted;
                  emitted += batch_rows.len();
                  let index = batch_count;
                  batch_count += 1;
                  sink(QueryResultBatch { index, offset, rows: batch_rows })?;
                }
              }
              flush_remaining_batch(&mut pending, &mut batch_count, row_count, sink)?;
              Ok(QueryExecutionSummary::Rows {
                columns,
                column_metadata,
                row_count,
                batch_count,
                truncated: truncation_reason.is_some(),
                truncation_reason,
                row_limit: options.row_limit,
                byte_limit: options.byte_limit,
                bytes_read,
                omitted_result_sets: 0,
                extra_result_sets: Vec::new(),
              })
            }
          }
        }
        Err(error) => {
          if error.code.as_deref() != Some(CONNECTION_LOST_CODE) {
            self.connection = Some(connection);
          } else {
            release_in_background(vec![connection]);
            self.transaction = TransactionState::default();
          }
          Err(error)
        }
      }
    }

    pub async fn describe_columns(
      &mut self,
      sql: &str,
    ) -> Result<Vec<QueryColumnMetadata>, QueryError> {
      let statement = statement_text(sql);
      let connection = self.take_connection().await?;
      let worker = Arc::clone(&connection);
      let outcome = blocking(move || {
        let guard = lock_conn(&worker)?;
        let mut stmt = guard.prepare(&statement).map_err(|err| format_odbc_error(&err))?;
        let num_cols = stmt.num_result_cols().map_err(|err| format_odbc_error(&err))?;
        let count = u16::try_from(num_cols).unwrap_or(0);
        if count == 0 {
          return Ok(Vec::new());
        }
        let mut columns = Vec::with_capacity(usize::from(count));
        for idx in 1..=count {
          let mut desc = ColumnDescription::default();
          stmt.describe_col(idx, &mut desc).map_err(|err| format_odbc_error(&err))?;
          let name = desc.name_to_string().unwrap_or_else(|_| format!("col_{idx}"));
          let nullable = !matches!(desc.nullability, odbc_api::Nullability::NoNulls);
          columns.push((name, desc.data_type, nullable));
        }
        Ok(column_header(&columns).0)
      })
      .await;
      if keeps_connection(&outcome) {
        self.connection = Some(connection);
      } else {
        release_in_background(vec![connection]);
      }
      outcome
    }

    pub async fn execute_with_params(
      &mut self,
      sql: &str,
      params: &[Option<String>],
    ) -> Result<u64, QueryError> {
      let statement = statement_text(sql);
      let json_params: Vec<JsonValue> = params
        .iter()
        .map(|opt| match opt {
          Some(text) => JsonValue::String(text.clone()),
          None => JsonValue::Null,
        })
        .collect();
      let connection = self.take_connection().await?;
      let worker = Arc::clone(&connection);
      let outcome = blocking(move || {
        let guard = lock_conn(&worker)?;
        let odbc_params = build_odbc_params(&json_params)?;
        let mut stmt = guard.prepare(&statement).map_err(|err| format_odbc_error(&err))?;
        let cursor = stmt.execute(&odbc_params[..]).map_err(|err| format_odbc_error(&err))?;
        drop(cursor);
        let affected = stmt.row_count().map_err(|err| format_odbc_error(&err))?.unwrap_or(0) as u64;
        Ok(affected)
      })
      .await;
      if keeps_connection(&outcome) {
        self.connection = Some(connection);
      } else {
        release_in_background(vec![connection]);
      }
      outcome
    }

    pub async fn execute_batch(&mut self, sql: &str) -> Result<u64, QueryError> {
      let statement = statement_text(sql);
      let connection = self.take_connection().await?;
      let worker = Arc::clone(&connection);
      let (first_kw, _) = crate::services::transaction_state::leading_keywords(&statement);
      let outcome = blocking(move || {
        let guard = lock_conn(&worker)?;
        if first_kw == "COMMIT" {
          guard.commit().map_err(|err| format_odbc_error(&err))?;
          return Ok(0);
        }
        if first_kw == "ROLLBACK" && !statement.to_ascii_uppercase().contains("SAVEPOINT") {
          guard.rollback().map_err(|err| format_odbc_error(&err))?;
          return Ok(0);
        }
        if first_kw == "SET" && controls_transaction(&statement) {
          guard.set_autocommit(false).map_err(|err| format_odbc_error(&err))?;
          let cursor =
            guard.execute(&statement, (), None).map_err(|err| format_odbc_error(&err))?;
          drop(cursor);
          return Ok(0);
        }
        let cursor = guard.execute(&statement, (), None).map_err(|err| format_odbc_error(&err))?;
        drop(cursor);
        Ok(0)
      })
      .await;
      if keeps_connection(&outcome) {
        self.connection = Some(connection);
      } else {
        release_in_background(vec![connection]);
      }
      outcome
    }
  }

  enum StreamResult {
    Affected(u64),
    Rows {
      columns: Vec<String>,
      column_metadata: Vec<QueryColumnMetadata>,
      rows: Vec<QueryRow>,
      bytes_read: usize,
      truncation_reason: Option<QueryTruncationReason>,
    },
  }
}

#[cfg(feature = "odbc")]
pub use enabled::{connect, DamengConnection, DamengPool};

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::DatabaseType;
  use std::collections::HashMap;

  fn sample_profile() -> ConnectionProfile {
    ConnectionProfile {
      id: "dm-1".to_string(),
      name: "DM8 Test".to_string(),
      db_type: DatabaseType::Dameng,
      host: "127.0.0.1".to_string(),
      port: 5236,
      database: Some("SYSDBA".to_string()),
      username: "SYSDBA".to_string(),
      password: "p@ss;with}braces".to_string(),
      ssl: false,
      tls_mode: Some(TlsMode::Disabled),
      ca_certificate_path: None,
      client_certificate_path: None,
      client_key_path: None,
      save_password: true,
      credential_ref: None,
      ssh_tunnel: None,
      options: HashMap::from([(
        "odbcDriver".to_string(),
        "/opt/dmdbms/bin/libdodbc.so".to_string(),
      )]),
      tags: vec![],
      environment: Default::default(),
      agent_access: Default::default(),
      created_at: "2026-11-22T00:00:00Z".to_string(),
      updated_at: "2026-11-22T00:00:00Z".to_string(),
    }
  }

  #[test]
  fn odbc_connection_string_escapes_special_characters() {
    let target = DamengTarget::from_profile(&sample_profile()).expect("valid target");
    let conn_str = target.odbc_connection_string();
    assert!(conn_str.contains("DRIVER=/opt/dmdbms/bin/libdodbc.so"));
    assert!(conn_str.contains("SERVER=127.0.0.1;TCP_PORT=5236"));
    assert!(conn_str.contains("UID=SYSDBA"));
    assert!(conn_str.contains("PWD={p@ss;with}}braces}"));
    assert!(conn_str.contains("SCHEMA=SYSDBA"));
  }

  #[test]
  fn requires_ssl_path_when_tls_is_required() {
    let mut profile = sample_profile();
    profile.tls_mode = Some(TlsMode::Required);
    let err = DamengTarget::from_profile(&profile).expect_err("missing ssl_path");
    assert_eq!(err, DAMENG_TLS_UNSUPPORTED);

    profile.ca_certificate_path = Some("/etc/dm8/ssl".to_string());
    let target = DamengTarget::from_profile(&profile).expect("with ssl_path");
    assert!(target.odbc_connection_string().contains("SSL_PATH=/etc/dm8/ssl"));
  }

  #[test]
  fn statement_text_strips_trailing_semicolon_except_for_plsql_blocks() {
    assert_eq!(statement_text("SELECT 1 FROM DUAL;  "), "SELECT 1 FROM DUAL");
    assert_eq!(statement_text("BEGIN NULL; END;"), "BEGIN NULL; END;");
    assert_eq!(
      statement_text("CREATE OR REPLACE PROCEDURE p AS BEGIN NULL; END;"),
      "CREATE OR REPLACE PROCEDURE p AS BEGIN NULL; END;"
    );
  }

  #[test]
  fn dm_svc_conf_writes_to_private_dir_with_utf8_and_local_offset() {
    assert_eq!(dm_svc_conf_contents(480), "TIME_ZONE=(480)\nLANGUAGE=(cn)\nCHAR_CODE=(PG_UTF8)\n");
    assert_eq!(
      dm_svc_conf_contents(-300),
      "TIME_ZONE=(-300)\nLANGUAGE=(cn)\nCHAR_CODE=(PG_UTF8)\n"
    );

    let root = std::env::temp_dir().join(format!("dataomni-dm8-test-{}", std::process::id()));
    let dir = write_dm_svc_conf_in(&root).expect("write dm_svc.conf");
    assert_eq!(dir, root.join("dm8"));
    let body = std::fs::read_to_string(dir.join("dm_svc.conf")).expect("read dm_svc.conf");
    assert!(body.contains("CHAR_CODE=(PG_UTF8)\n"), "{body}");
    assert!(body.starts_with("TIME_ZONE=("), "{body}");
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mode = std::fs::metadata(&dir).expect("meta").permissions().mode() & 0o777;
      assert_eq!(mode, 0o700, "dm8 config dir must be owner-only");
    }
    std::fs::remove_dir_all(&root).ok();
  }
}
