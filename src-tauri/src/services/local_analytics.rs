//! 内置内存 DuckDB 统一分析沙盒与列级数据画像（`rfcs/duckdb-analytics.md`）。
//!
//! 不依赖 `tauri::AppHandle`：GUI（`analytics_commands`）与后续 CLI / MCP（`CM2`）
//! 共享同一份装载、沙盒锁定与列画像实现，保证字段口径、分桶与截断语义一致。
//!
//! 关键约束（由 DA0 实验 E1–E5 拍板）：
//! - 默认上限 `MAX_ANALYTICS_ROWS`（100,000 行），单会话 `memory_limit = '512MB'`、`threads = 4`。
//! - 最多同时存活 `MAX_ANALYTICS_SESSIONS`（8 个）会话，超限时按 LRU 淘汰最久未用会话。
//! - 建表装载完成后立即执行 `SET enable_external_access = false; SET lock_configuration = true;`，
//!   物理阻断 `read_text` / `read_csv` / `COPY TO` / `ATTACH` 等本机文件系统逃逸。
//! - MongoDB / Elasticsearch 异构 JSON 文档走纯内存单遍扫键 + `Appender`，不落临时文件，
//!   同时精确统计 `present_count`（字段存在次数）与 `explicit_null_count`（字段存在但值为 `null`）。

use crate::services::query_executor::number_duplicate_columns;
use crate::services::QueryError;
use duckdb::types::Value;
use duckdb::{appender_params_from_iter, Connection, InterruptHandle};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

/// 内存分析单会话默认最大装载行数（DA0 E1：10 万行 × 20 列 DuckDB 内存 54.5 MiB）
pub const MAX_ANALYTICS_ROWS: usize = 100_000;
/// 允许同时存活的分析会话上限；超出时按 LRU 淘汰最久未使用的非活跃会话
pub const MAX_ANALYTICS_SESSIONS: usize = 8;
/// 单会话 DuckDB 内存硬上限
pub const ANALYTICS_MEMORY_LIMIT: &str = "512MB";
/// 单会话 DuckDB 并行线程上限
pub const ANALYTICS_THREADS: usize = 4;
/// 数值/时间列等宽直方图默认桶数
pub const HISTOGRAM_BIN_COUNT: usize = 10;
/// 分类/文本列 Top-K 默认条数
pub const TOP_VALUES_LIMIT: usize = 5;

/// 会话不存在或已被淘汰/关闭
pub const ANALYTICS_NOT_FOUND_CODE: &str = "DATAOMNI_ANALYTICS_NOT_FOUND";
/// 分析查询已被用户取消
pub const ANALYTICS_CANCELLED_CODE: &str = "DATAOMNI_ANALYTICS_CANCELLED";

const LOCK_POISONED: &str = "Analytics session lock poisoned";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalyticsSourceKind {
  ResultSet,
  JsonDocuments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileColumnType {
  Integer,
  Numeric,
  Text,
  Boolean,
  Date,
  Time,
  Datetime,
  Binary,
  Json,
  Unknown,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsColumnInput {
  pub name: String,
  #[serde(default)]
  pub logical_type: Option<String>,
  #[serde(default)]
  pub database_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsColumnMeta {
  pub name: String,
  pub duckdb_type: String,
  pub inferred_type: ProfileColumnType,
  pub precision_degraded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSessionSummary {
  pub analytics_id: String,
  pub source_kind: AnalyticsSourceKind,
  pub row_count: usize,
  pub column_count: usize,
  pub truncated: bool,
  pub columns: Vec<AnalyticsColumnMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistogramBin {
  pub lower: f64,
  pub upper: f64,
  pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopValueEntry {
  pub value: String,
  pub count: u64,
  pub ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnProfile {
  pub name: String,
  pub inferred_type: ProfileColumnType,
  pub duckdb_type: String,
  pub precision_degraded: bool,
  pub total_count: u64,
  pub null_count: u64,
  pub null_ratio: f64,
  pub distinct_count: u64,
  pub distinct_ratio: f64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub present_count: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub present_ratio: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub explicit_null_count: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub min: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub mean: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stddev: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub p25: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub p50: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub p75: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub p95: Option<f64>,
  pub bins: Vec<HistogramBin>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub min_length: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max_length: Option<u64>,
  pub top_values: Vec<TopValueEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResult {
  pub analytics_id: String,
  pub row_count: u64,
  pub column_count: usize,
  pub elapsed_ms: u64,
  pub columns: Vec<ColumnProfile>,
}

/// 单个文档型列的存在性统计（仅 `JsonDocuments` 模式填充，用于区分「缺字段」与「值为 null」）
#[derive(Debug, Clone, Copy)]
struct FieldPresence {
  present_count: u64,
  explicit_null_count: u64,
}

/// 内部列定义：承载 DuckDB 列类型、语义类型、精度降级标记与文档存在率
#[derive(Debug, Clone)]
struct ColumnPlan {
  name: String,
  storage: ColumnStorage,
  inferred_type: ProfileColumnType,
  precision_degraded: bool,
  presence: Option<FieldPresence>,
}

impl ColumnPlan {
  fn duckdb_type(&self) -> String {
    self.storage.sql_type()
  }

  fn meta(&self) -> AnalyticsColumnMeta {
    AnalyticsColumnMeta {
      name: self.name.clone(),
      duckdb_type: self.duckdb_type(),
      inferred_type: self.inferred_type,
      precision_degraded: self.precision_degraded,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnStorage {
  Boolean,
  BigInt,
  HugeInt,
  Double,
  Decimal { scale: u8 },
  Date,
  Time,
  Timestamp,
  Blob,
  Varchar,
}

impl ColumnStorage {
  fn sql_type(self) -> String {
    match self {
      Self::Boolean => "BOOLEAN".to_string(),
      Self::BigInt => "BIGINT".to_string(),
      Self::HugeInt => "HUGEINT".to_string(),
      Self::Double => "DOUBLE".to_string(),
      Self::Decimal { scale } => format!("DECIMAL(38,{scale})"),
      Self::Date => "DATE".to_string(),
      Self::Time => "TIME".to_string(),
      Self::Timestamp => "TIMESTAMP".to_string(),
      Self::Blob => "BLOB".to_string(),
      Self::Varchar => "VARCHAR".to_string(),
    }
  }
}

/// 一个隔离的内存 DuckDB 分析会话
pub struct AnalyticsSession {
  connection: Mutex<Connection>,
  interrupt: Arc<InterruptHandle>,
  cancelled: AtomicBool,
  busy: AtomicBool,
  source_kind: AnalyticsSourceKind,
  row_count: usize,
  truncated: bool,
  columns: Vec<ColumnPlan>,
}

struct RegistryEntry {
  session: Arc<AnalyticsSession>,
  last_used_seq: u64,
}

struct RegistryInner {
  sessions: HashMap<String, RegistryEntry>,
  seq: u64,
  max_sessions: usize,
}

/// 托管在 Tauri `State` 中的分析会话注册表，带会话数上限与 LRU 淘汰
pub struct LocalAnalyticsRegistry {
  inner: Mutex<RegistryInner>,
}

impl Default for LocalAnalyticsRegistry {
  fn default() -> Self {
    Self::with_capacity(MAX_ANALYTICS_SESSIONS)
  }
}

impl LocalAnalyticsRegistry {
  pub fn with_capacity(max_sessions: usize) -> Self {
    Self {
      inner: Mutex::new(RegistryInner {
        sessions: HashMap::new(),
        seq: 0,
        max_sessions: max_sessions.max(1),
      }),
    }
  }

  /// 装载 SQL / 表格查询结果集进隔离内存沙盒
  pub async fn load_result(
    &self,
    columns: Vec<AnalyticsColumnInput>,
    rows: Vec<Vec<JsonValue>>,
  ) -> Result<AnalyticsSessionSummary, QueryError> {
    let session =
      blocking(move || create_result_session(&columns, &rows, MAX_ANALYTICS_ROWS)).await?;
    self.insert(session)
  }

  /// 装载 MongoDB / Elasticsearch 异构 JSON 文档进隔离内存沙盒（零临时文件）
  pub async fn load_json_docs(
    &self,
    documents: Vec<JsonValue>,
  ) -> Result<AnalyticsSessionSummary, QueryError> {
    let session =
      blocking(move || create_json_docs_session(&documents, MAX_ANALYTICS_ROWS)).await?;
    self.insert(session)
  }

  /// 对指定会话执行列级数据画像（异步在阻塞线程池跑，可被 `cancel` 中止）
  pub async fn profile(&self, analytics_id: &str) -> Result<ProfileResult, QueryError> {
    let session = self.get(analytics_id)?;
    let id = analytics_id.to_string();
    blocking(move || profile_session(&id, &session)).await
  }

  /// 中止正在该会话上运行的画像或分析查询；会话中止后保持可复用
  pub fn cancel(&self, analytics_id: &str) -> Result<bool, QueryError> {
    let inner = self.inner.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
    let Some(entry) = inner.sessions.get(analytics_id) else {
      return Ok(false);
    };
    entry.session.cancelled.store(true, Ordering::SeqCst);
    entry.session.interrupt.interrupt();
    Ok(true)
  }

  /// 关闭并释放指定会话的内存
  pub fn close(&self, analytics_id: &str) -> Result<bool, QueryError> {
    let mut inner = self.inner.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
    let removed = inner.sessions.remove(analytics_id);
    if let Some(entry) = removed {
      entry.session.cancelled.store(true, Ordering::SeqCst);
      entry.session.interrupt.interrupt();
      return Ok(true);
    }
    Ok(false)
  }

  #[cfg(test)]
  fn len(&self) -> usize {
    self.inner.lock().map(|g| g.sessions.len()).unwrap_or(0)
  }

  fn insert(&self, session: AnalyticsSession) -> Result<AnalyticsSessionSummary, QueryError> {
    let analytics_id = Uuid::new_v4().to_string();
    let summary = AnalyticsSessionSummary {
      analytics_id: analytics_id.clone(),
      source_kind: session.source_kind,
      row_count: session.row_count,
      column_count: session.columns.len(),
      truncated: session.truncated,
      columns: session.columns.iter().map(ColumnPlan::meta).collect(),
    };

    let mut inner = self.inner.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
    inner.seq += 1;
    let seq = inner.seq;

    while inner.sessions.len() >= inner.max_sessions {
      // 优先淘汰最久未用的非活跃会话；若全部正忙，淘汰最久未用的一条并打断
      let victim = inner
        .sessions
        .iter()
        .filter(|(_, entry)| !entry.session.busy.load(Ordering::Acquire))
        .min_by_key(|(_, entry)| entry.last_used_seq)
        .or_else(|| inner.sessions.iter().min_by_key(|(_, entry)| entry.last_used_seq))
        .map(|(key, _)| key.clone());
      let Some(key) = victim else {
        break;
      };
      if let Some(removed) = inner.sessions.remove(&key) {
        removed.session.cancelled.store(true, Ordering::SeqCst);
        removed.session.interrupt.interrupt();
      }
    }

    inner
      .sessions
      .insert(analytics_id, RegistryEntry { session: Arc::new(session), last_used_seq: seq });
    Ok(summary)
  }

  fn get(&self, analytics_id: &str) -> Result<Arc<AnalyticsSession>, QueryError> {
    let mut inner = self.inner.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
    inner.seq += 1;
    let seq = inner.seq;
    let Some(entry) = inner.sessions.get_mut(analytics_id) else {
      return Err(QueryError::with_code(
        ANALYTICS_NOT_FOUND_CODE,
        format!("{ANALYTICS_NOT_FOUND_CODE}: {analytics_id}"),
      ));
    };
    entry.last_used_seq = seq;
    Ok(Arc::clone(&entry.session))
  }
}

async fn blocking<T: Send + 'static>(
  work: impl FnOnce() -> Result<T, QueryError> + Send + 'static,
) -> Result<T, QueryError> {
  tokio::task::spawn_blocking(work)
    .await
    .map_err(|error| QueryError::message(format!("Analytics worker failed: {error}")))?
}

/// 打开并配置一条基础内存 DuckDB 连接（尚未锁死外部访问，供建表装载使用）
fn open_memory_connection() -> Result<Connection, QueryError> {
  let conn = Connection::open_in_memory().map_err(duck_error)?;
  conn
    .execute_batch(&format!(
      "SET memory_limit = '{ANALYTICS_MEMORY_LIMIT}'; SET threads = {ANALYTICS_THREADS};"
    ))
    .map_err(duck_error)?;
  Ok(conn)
}

/// 锁死外部访问与配置：此后任何 `read_text`/`COPY TO`/`ATTACH`/`SET` 均被 DuckDB 拒绝
fn lock_sandbox(conn: &Connection) -> Result<(), QueryError> {
  conn
    .execute_batch("SET enable_external_access = false; SET lock_configuration = true;")
    .map_err(duck_error)
}

/// 从 `ResultColumn` + `SerializedResultValue` 行数组构建隔离的内存分析会话（CLI / GUI 共用）
pub fn create_result_session(
  columns: &[AnalyticsColumnInput],
  rows: &[Vec<JsonValue>],
  max_rows: usize,
) -> Result<AnalyticsSession, QueryError> {
  let truncated = rows.len() > max_rows;
  let admitted_rows = if truncated { &rows[..max_rows] } else { rows };

  let raw_names: Vec<String> = columns
    .iter()
    .enumerate()
    .map(|(index, col)| {
      let trimmed = col.name.trim();
      if trimmed.is_empty() {
        format!("col_{}", index + 1)
      } else {
        trimmed.to_string()
      }
    })
    .collect();
  let unique_names = number_duplicate_columns(raw_names.iter().map(String::as_str));

  let plans: Vec<ColumnPlan> = unique_names
    .into_iter()
    .enumerate()
    .map(|(col_idx, name)| {
      let logical_hint = columns.get(col_idx).and_then(|c| c.logical_type.as_deref());
      infer_result_column_plan(name, logical_hint, admitted_rows, col_idx)
    })
    .collect();

  let conn = open_memory_connection()?;
  create_result_table(&conn, &plans)?;

  if !plans.is_empty() && !admitted_rows.is_empty() {
    let mut appender = conn.appender("result_set").map_err(duck_error)?;
    let mut buffer: Vec<Value> = Vec::with_capacity(plans.len());
    for row in admitted_rows {
      buffer.clear();
      for (col_idx, plan) in plans.iter().enumerate() {
        let cell = row.get(col_idx).unwrap_or(&JsonValue::Null);
        buffer.push(encode_result_cell(cell, plan.storage));
      }
      appender.append_row(appender_params_from_iter(buffer.iter())).map_err(duck_error)?;
    }
    appender.flush().map_err(duck_error)?;
  }

  lock_sandbox(&conn)?;
  let interrupt = conn.interrupt_handle();
  Ok(AnalyticsSession {
    connection: Mutex::new(conn),
    interrupt,
    cancelled: AtomicBool::new(false),
    busy: AtomicBool::new(false),
    source_kind: AnalyticsSourceKind::ResultSet,
    row_count: admitted_rows.len(),
    truncated,
    columns: plans,
  })
}

/// 从异构 JSON 文档列表构建隔离的内存分析会话（MongoDB / Elasticsearch 复用，零临时文件）
pub fn create_json_docs_session(
  documents: &[JsonValue],
  max_rows: usize,
) -> Result<AnalyticsSession, QueryError> {
  let truncated = documents.len() > max_rows;
  let admitted = if truncated { &documents[..max_rows] } else { documents };

  // 将非对象文档包进单一 `value` 字段，对象文档保留顶层键首次出现顺序
  let mut col_names: Vec<String> = Vec::new();
  let mut col_index: HashMap<String, usize> = HashMap::new();
  let mut inferred: Vec<JsonFieldAccumulator> = Vec::new();

  let mut normalized_docs: Vec<Map<String, JsonValue>> = Vec::with_capacity(admitted.len());
  for doc in admitted {
    let map = match doc {
      JsonValue::Object(obj) => obj.clone(),
      other => {
        let mut wrapped = Map::new();
        wrapped.insert("value".to_string(), other.clone());
        wrapped
      }
    };
    for (key, value) in &map {
      let idx = match col_index.get(key) {
        Some(&existing) => existing,
        None => {
          let next = col_names.len();
          col_names.push(key.clone());
          col_index.insert(key.clone(), next);
          inferred.push(JsonFieldAccumulator::default());
          next
        }
      };
      inferred[idx].observe(value);
    }
    normalized_docs.push(map);
  }

  let plans: Vec<ColumnPlan> =
    col_names.iter().zip(&inferred).map(|(name, acc)| acc.to_plan(name.clone())).collect();

  let conn = open_memory_connection()?;
  create_result_table(&conn, &plans)?;

  if !plans.is_empty() && !normalized_docs.is_empty() {
    let mut appender = conn.appender("result_set").map_err(duck_error)?;
    let mut buffer: Vec<Value> = Vec::with_capacity(plans.len());
    for doc in &normalized_docs {
      buffer.clear();
      for (name, plan) in col_names.iter().zip(&plans) {
        let val = match doc.get(name) {
          None | Some(JsonValue::Null) => Value::Null,
          Some(raw) => encode_json_doc_value(raw, plan.storage),
        };
        buffer.push(val);
      }
      appender.append_row(appender_params_from_iter(buffer.iter())).map_err(duck_error)?;
    }
    appender.flush().map_err(duck_error)?;
  }

  lock_sandbox(&conn)?;
  let interrupt = conn.interrupt_handle();
  Ok(AnalyticsSession {
    connection: Mutex::new(conn),
    interrupt,
    cancelled: AtomicBool::new(false),
    busy: AtomicBool::new(false),
    source_kind: AnalyticsSourceKind::JsonDocuments,
    row_count: normalized_docs.len(),
    truncated,
    columns: plans,
  })
}

/// 对已装载的分析会话执行全列画像（GUI / CLI / MCP 共用同一实现）
pub fn profile_session(
  analytics_id: &str,
  session: &AnalyticsSession,
) -> Result<ProfileResult, QueryError> {
  let started = Instant::now();
  session.cancelled.store(false, Ordering::SeqCst);
  session.busy.store(true, Ordering::Release);
  let _busy_guard = ResetBusy(&session.busy);

  let conn = session.connection.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;

  let mut profiles = Vec::with_capacity(session.columns.len());
  for plan in &session.columns {
    if session.cancelled.load(Ordering::SeqCst) {
      return Err(QueryError::with_code(ANALYTICS_CANCELLED_CODE, ANALYTICS_CANCELLED_CODE));
    }
    let profile = profile_single_column(&conn, plan, session.row_count as u64)
      .map_err(|err| map_interrupt_error(err, &session.cancelled))?;
    profiles.push(profile);
  }

  Ok(ProfileResult {
    analytics_id: analytics_id.to_string(),
    row_count: session.row_count as u64,
    column_count: profiles.len(),
    elapsed_ms: started.elapsed().as_millis() as u64,
    columns: profiles,
  })
}

struct ResetBusy<'a>(&'a AtomicBool);
impl Drop for ResetBusy<'_> {
  fn drop(&mut self) {
    self.0.store(false, Ordering::Release);
  }
}

fn create_result_table(conn: &Connection, plans: &[ColumnPlan]) -> Result<(), QueryError> {
  if plans.is_empty() {
    // 空列结果集建占位单列零行表，避免 DuckDB 拒绝零列 CREATE TABLE
    conn
      .execute_batch("CREATE TABLE result_set (__empty_placeholder VARCHAR)")
      .map_err(duck_error)?;
    return Ok(());
  }
  let cols_sql: Vec<String> = plans
    .iter()
    .map(|plan| format!("{} {}", quote_ident(&plan.name), plan.duckdb_type()))
    .collect();
  conn
    .execute_batch(&format!("CREATE TABLE result_set ({})", cols_sql.join(", ")))
    .map_err(duck_error)
}

fn quote_ident(name: &str) -> String {
  format!("\"{}\"", name.replace('"', "\"\""))
}

// ---------------------------------------------------------------------------
// 列类型推断与值编码（SQL ResultSet）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellKind {
  Bool,
  Int64,
  Int128,
  BigIntOverflow,
  Float,
  Decimal { int_digits: u8, scale: u8 },
  DecimalOverflow,
  Date,
  Time,
  Datetime,
  Binary,
  Json,
  Text,
}

fn classify_result_cell(cell: &JsonValue) -> Option<CellKind> {
  match cell {
    JsonValue::Null => None,
    JsonValue::Bool(_) => Some(CellKind::Bool),
    JsonValue::Number(num) => {
      if num.is_i64() {
        Some(CellKind::Int64)
      } else if let Some(u) = num.as_u64() {
        if i64::try_from(u).is_ok() {
          Some(CellKind::Int64)
        } else {
          Some(CellKind::Int128)
        }
      } else {
        Some(CellKind::Float)
      }
    }
    JsonValue::String(_) => Some(CellKind::Text),
    JsonValue::Object(map) => {
      let tag = map.get("type").and_then(JsonValue::as_str);
      let val = map.get("value").and_then(JsonValue::as_str);
      match (tag, val) {
        (Some("bigint"), Some(text)) => {
          let trimmed = text.trim();
          if trimmed.parse::<i64>().is_ok() {
            Some(CellKind::Int64)
          } else if trimmed.parse::<i128>().is_ok() {
            Some(CellKind::Int128)
          } else {
            Some(CellKind::BigIntOverflow)
          }
        }
        (Some("decimal"), Some(text)) => match parse_decimal_shape(text) {
          Some((int_digits, scale)) => Some(CellKind::Decimal { int_digits, scale }),
          None => Some(CellKind::DecimalOverflow),
        },
        (Some("date"), Some(text)) => {
          if is_plain_date(text) {
            Some(CellKind::Date)
          } else {
            Some(CellKind::Text)
          }
        }
        (Some("time"), Some(text)) => {
          if is_plain_time(text) {
            Some(CellKind::Time)
          } else {
            Some(CellKind::Text)
          }
        }
        (Some("datetime"), Some(text)) => {
          if normalize_timestamp(text).is_some() {
            Some(CellKind::Datetime)
          } else {
            Some(CellKind::Text)
          }
        }
        (Some("binary"), Some(text)) => {
          if decode_hex(text).is_some() {
            Some(CellKind::Binary)
          } else {
            Some(CellKind::Text)
          }
        }
        (Some("json"), Some(_)) => Some(CellKind::Json),
        _ => Some(CellKind::Text),
      }
    }
    JsonValue::Array(_) => Some(CellKind::Json),
  }
}

fn infer_result_column_plan(
  name: String,
  logical_hint: Option<&str>,
  rows: &[Vec<JsonValue>],
  col_idx: usize,
) -> ColumnPlan {
  let mut saw_bool = false;
  let mut saw_int64 = false;
  let mut saw_int128 = false;
  let mut saw_bigint_overflow = false;
  let mut saw_float = false;
  let mut saw_decimal = false;
  let mut saw_decimal_overflow = false;
  let mut max_int_digits: u8 = 0;
  let mut max_scale: u8 = 0;
  let mut saw_date = false;
  let mut saw_time = false;
  let mut saw_datetime = false;
  let mut saw_binary = false;
  let mut saw_json = false;
  let mut saw_text = false;

  for row in rows {
    let cell = row.get(col_idx).unwrap_or(&JsonValue::Null);
    let Some(kind) = classify_result_cell(cell) else {
      continue;
    };
    match kind {
      CellKind::Bool => saw_bool = true,
      CellKind::Int64 => saw_int64 = true,
      CellKind::Int128 => saw_int128 = true,
      CellKind::BigIntOverflow => saw_bigint_overflow = true,
      CellKind::Float => saw_float = true,
      CellKind::Decimal { int_digits, scale } => {
        saw_decimal = true;
        max_int_digits = max_int_digits.max(int_digits);
        max_scale = max_scale.max(scale);
      }
      CellKind::DecimalOverflow => saw_decimal_overflow = true,
      CellKind::Date => saw_date = true,
      CellKind::Time => saw_time = true,
      CellKind::Datetime => saw_datetime = true,
      CellKind::Binary => saw_binary = true,
      CellKind::Json => saw_json = true,
      CellKind::Text => saw_text = true,
    }
  }

  let any_observed = saw_bool
    || saw_int64
    || saw_int128
    || saw_bigint_overflow
    || saw_float
    || saw_decimal
    || saw_decimal_overflow
    || saw_date
    || saw_time
    || saw_datetime
    || saw_binary
    || saw_json
    || saw_text;

  if !any_observed {
    let (storage, inferred_type) = match logical_hint {
      Some("boolean") => (ColumnStorage::Boolean, ProfileColumnType::Boolean),
      Some("integer") => (ColumnStorage::BigInt, ProfileColumnType::Integer),
      Some("decimal") => (ColumnStorage::Double, ProfileColumnType::Numeric),
      Some("date") => (ColumnStorage::Date, ProfileColumnType::Date),
      Some("time") => (ColumnStorage::Time, ProfileColumnType::Time),
      Some("datetime") => (ColumnStorage::Timestamp, ProfileColumnType::Datetime),
      Some("binary") => (ColumnStorage::Blob, ProfileColumnType::Binary),
      Some("json") => (ColumnStorage::Varchar, ProfileColumnType::Json),
      Some("text") => (ColumnStorage::Varchar, ProfileColumnType::Text),
      _ => (ColumnStorage::Varchar, ProfileColumnType::Unknown),
    };
    return ColumnPlan { name, storage, inferred_type, precision_degraded: false, presence: None };
  }

  let non_numeric =
    saw_bool || saw_date || saw_time || saw_datetime || saw_binary || saw_json || saw_text;

  if !non_numeric {
    if saw_bigint_overflow || saw_decimal_overflow {
      return ColumnPlan {
        name,
        storage: ColumnStorage::Varchar,
        inferred_type: ProfileColumnType::Text,
        precision_degraded: true,
        presence: None,
      };
    }
    if saw_decimal && !saw_float {
      // 若混合整数与小数，确认总位数不超 38
      let int_part = if saw_int128 {
        38
      } else if saw_int64 {
        max_int_digits.max(19)
      } else {
        max_int_digits
      };
      if u16::from(int_part) + u16::from(max_scale) <= 38 && max_scale <= 38 {
        return ColumnPlan {
          name,
          storage: ColumnStorage::Decimal { scale: max_scale },
          inferred_type: ProfileColumnType::Numeric,
          precision_degraded: false,
          presence: None,
        };
      }
      return ColumnPlan {
        name,
        storage: ColumnStorage::Varchar,
        inferred_type: ProfileColumnType::Text,
        precision_degraded: true,
        presence: None,
      };
    }
    if saw_float || saw_decimal {
      return ColumnPlan {
        name,
        storage: ColumnStorage::Double,
        inferred_type: ProfileColumnType::Numeric,
        precision_degraded: false,
        presence: None,
      };
    }
    if saw_int128 {
      return ColumnPlan {
        name,
        storage: ColumnStorage::HugeInt,
        inferred_type: ProfileColumnType::Integer,
        precision_degraded: false,
        presence: None,
      };
    }
    if saw_int64 {
      return ColumnPlan {
        name,
        storage: ColumnStorage::BigInt,
        inferred_type: ProfileColumnType::Integer,
        precision_degraded: false,
        presence: None,
      };
    }
  }

  let any_numeric = saw_int64
    || saw_int128
    || saw_bigint_overflow
    || saw_float
    || saw_decimal
    || saw_decimal_overflow;
  if !any_numeric {
    if saw_bool && !(saw_date || saw_time || saw_datetime || saw_binary || saw_json || saw_text) {
      return ColumnPlan {
        name,
        storage: ColumnStorage::Boolean,
        inferred_type: ProfileColumnType::Boolean,
        precision_degraded: false,
        presence: None,
      };
    }
    if saw_date && !(saw_bool || saw_time || saw_datetime || saw_binary || saw_json || saw_text) {
      return ColumnPlan {
        name,
        storage: ColumnStorage::Date,
        inferred_type: ProfileColumnType::Date,
        precision_degraded: false,
        presence: None,
      };
    }
    if saw_time && !(saw_bool || saw_date || saw_datetime || saw_binary || saw_json || saw_text) {
      return ColumnPlan {
        name,
        storage: ColumnStorage::Time,
        inferred_type: ProfileColumnType::Time,
        precision_degraded: false,
        presence: None,
      };
    }
    if saw_datetime && !(saw_bool || saw_date || saw_time || saw_binary || saw_json || saw_text) {
      return ColumnPlan {
        name,
        storage: ColumnStorage::Timestamp,
        inferred_type: ProfileColumnType::Datetime,
        precision_degraded: false,
        presence: None,
      };
    }
    if saw_binary && !(saw_bool || saw_date || saw_time || saw_datetime || saw_json || saw_text) {
      return ColumnPlan {
        name,
        storage: ColumnStorage::Blob,
        inferred_type: ProfileColumnType::Binary,
        precision_degraded: false,
        presence: None,
      };
    }
    if saw_json && !(saw_bool || saw_date || saw_time || saw_datetime || saw_binary || saw_text) {
      return ColumnPlan {
        name,
        storage: ColumnStorage::Varchar,
        inferred_type: ProfileColumnType::Json,
        precision_degraded: false,
        presence: None,
      };
    }
  }

  ColumnPlan {
    name,
    storage: ColumnStorage::Varchar,
    inferred_type: ProfileColumnType::Text,
    precision_degraded: false,
    presence: None,
  }
}

fn encode_result_cell(cell: &JsonValue, storage: ColumnStorage) -> Value {
  if cell.is_null() {
    return Value::Null;
  }
  match storage {
    ColumnStorage::Boolean => cell.as_bool().map_or(Value::Null, Value::Boolean),
    ColumnStorage::BigInt => extract_i64(cell).map_or(Value::Null, Value::BigInt),
    ColumnStorage::HugeInt => extract_i128(cell).map_or(Value::Null, Value::HugeInt),
    ColumnStorage::Double => extract_f64(cell).map_or(Value::Null, Value::Double),
    ColumnStorage::Decimal { .. } => Value::Text(extract_scalar_text(cell)),
    ColumnStorage::Date => Value::Text(extract_scalar_text(cell).trim().to_string()),
    ColumnStorage::Time => Value::Text(extract_scalar_text(cell).trim().to_string()),
    ColumnStorage::Timestamp => {
      let text = extract_scalar_text(cell);
      normalize_timestamp(&text).map_or(Value::Null, Value::Text)
    }
    ColumnStorage::Blob => {
      let hex_text = extract_scalar_text(cell);
      decode_hex(&hex_text).map_or(Value::Null, Value::Blob)
    }
    ColumnStorage::Varchar => Value::Text(extract_scalar_text(cell)),
  }
}

fn extract_scalar_text(cell: &JsonValue) -> String {
  match cell {
    JsonValue::Null => String::new(),
    JsonValue::String(s) => s.clone(),
    JsonValue::Bool(b) => b.to_string(),
    JsonValue::Number(n) => n.to_string(),
    JsonValue::Object(map) => {
      if map.contains_key("type") {
        if let Some(val) = map.get("value").and_then(JsonValue::as_str) {
          return val.to_string();
        }
      }
      cell.to_string()
    }
    JsonValue::Array(_) => cell.to_string(),
  }
}

fn extract_i64(cell: &JsonValue) -> Option<i64> {
  match cell {
    JsonValue::Number(n) => n.as_i64(),
    JsonValue::Object(map) if map.get("type").and_then(JsonValue::as_str) == Some("bigint") => {
      map.get("value").and_then(JsonValue::as_str)?.trim().parse::<i64>().ok()
    }
    _ => None,
  }
}

fn extract_i128(cell: &JsonValue) -> Option<i128> {
  match cell {
    JsonValue::Number(n) => n.as_i64().map(i128::from).or_else(|| n.as_u64().map(i128::from)),
    JsonValue::Object(map) if map.get("type").and_then(JsonValue::as_str) == Some("bigint") => {
      map.get("value").and_then(JsonValue::as_str)?.trim().parse::<i128>().ok()
    }
    _ => None,
  }
}

fn extract_f64(cell: &JsonValue) -> Option<f64> {
  match cell {
    JsonValue::Number(n) => n.as_f64(),
    JsonValue::Object(map) => {
      let tag = map.get("type").and_then(JsonValue::as_str)?;
      if tag == "decimal" || tag == "bigint" {
        map.get("value").and_then(JsonValue::as_str)?.trim().parse::<f64>().ok()
      } else {
        None
      }
    }
    _ => None,
  }
}

/// 校验十进制字符串能否无损放入 DuckDB `DECIMAL(38, scale)`；返回 `(整数位数, 小数位数)`
fn parse_decimal_shape(raw: &str) -> Option<(u8, u8)> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return None;
  }
  let body = trimmed.strip_prefix('-').or_else(|| trimmed.strip_prefix('+')).unwrap_or(trimmed);
  if body.is_empty() {
    return None;
  }
  let (int_part, frac_part) = match body.split_once('.') {
    Some((i, f)) => (i, f),
    None => (body, ""),
  };
  if (int_part.is_empty() && frac_part.is_empty())
    || !int_part.bytes().all(|b| b.is_ascii_digit())
    || !frac_part.bytes().all(|b| b.is_ascii_digit())
  {
    return None;
  }
  let significant_int = int_part.trim_start_matches('0').len().max(1);
  let scale = frac_part.len();
  if significant_int + scale > 38 || scale > 38 {
    return None;
  }
  Some((significant_int as u8, scale as u8))
}

fn is_plain_date(raw: &str) -> bool {
  let s = raw.trim();
  let bytes = s.as_bytes();
  bytes.len() == 10
    && bytes[4] == b'-'
    && bytes[7] == b'-'
    && bytes[0..4].iter().all(u8::is_ascii_digit)
    && bytes[5..7].iter().all(u8::is_ascii_digit)
    && bytes[8..10].iter().all(u8::is_ascii_digit)
}

fn is_plain_time(raw: &str) -> bool {
  let s = raw.trim();
  let bytes = s.as_bytes();
  if bytes.len() < 8 || bytes[2] != b':' || bytes[5] != b':' {
    return false;
  }
  let hh_mm_ss = bytes[0..2].iter().all(u8::is_ascii_digit)
    && bytes[3..5].iter().all(u8::is_ascii_digit)
    && bytes[6..8].iter().all(u8::is_ascii_digit);
  if !hh_mm_ss {
    return false;
  }
  if bytes.len() == 8 {
    return true;
  }
  bytes[8] == b'.' && bytes.len() > 9 && bytes[9..].iter().all(u8::is_ascii_digit)
}

fn normalize_timestamp(raw: &str) -> Option<String> {
  let s = raw.trim();
  if s.len() < 19 {
    return None;
  }
  let bytes = s.as_bytes();
  if !is_plain_date(&s[..10]) || (bytes[10] != b' ' && bytes[10] != b'T') {
    return None;
  }
  let time_part = &s[11..];
  // 去掉末尾时区（Z / +08 / +08:00 / -05:00）以便直接入 DuckDB TIMESTAMP（未编 ICU）
  let stripped =
    if let Some(body) = time_part.strip_suffix('Z').or_else(|| time_part.strip_suffix('z')) {
      body
    } else if let Some(tz_pos) = time_part.rfind(['+', '-']) {
      if tz_pos >= 8 {
        &time_part[..tz_pos]
      } else {
        time_part
      }
    } else {
      time_part
    };
  if !is_plain_time(stripped) {
    return None;
  }
  Some(format!("{} {stripped}", &s[..10]))
}

fn decode_hex(raw: &str) -> Option<Vec<u8>> {
  let trimmed = raw.trim();
  let hex = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")).unwrap_or(trimmed);
  if !hex.len().is_multiple_of(2) {
    return None;
  }
  let mut out = Vec::with_capacity(hex.len() / 2);
  let bytes = hex.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    let hi = hex_nibble(bytes[i])?;
    let lo = hex_nibble(bytes[i + 1])?;
    out.push((hi << 4) | lo);
    i += 2;
  }
  Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
  match byte {
    b'0'..=b'9' => Some(byte - b'0'),
    b'a'..=b'f' => Some(byte - b'a' + 10),
    b'A'..=b'F' => Some(byte - b'A' + 10),
    _ => None,
  }
}

// ---------------------------------------------------------------------------
// 异构 JSON 文档扫描与编码（MongoDB / Elasticsearch）
// ---------------------------------------------------------------------------

#[derive(Default)]
struct JsonFieldAccumulator {
  present_count: u64,
  explicit_null_count: u64,
  saw_bool: bool,
  saw_int: bool,
  saw_float: bool,
  saw_string: bool,
  saw_nested: bool,
}

impl JsonFieldAccumulator {
  fn observe(&mut self, value: &JsonValue) {
    self.present_count += 1;
    match value {
      JsonValue::Null => self.explicit_null_count += 1,
      JsonValue::Bool(_) => self.saw_bool = true,
      JsonValue::Number(num) => {
        if num.is_i64() {
          self.saw_int = true;
        } else {
          self.saw_float = true;
        }
      }
      JsonValue::String(_) => self.saw_string = true,
      JsonValue::Array(_) | JsonValue::Object(_) => self.saw_nested = true,
    }
  }

  fn to_plan(&self, name: String) -> ColumnPlan {
    let (storage, inferred_type) =
      match (self.saw_bool, self.saw_int, self.saw_float, self.saw_string, self.saw_nested) {
        (false, false, false, false, false) => (ColumnStorage::Varchar, ProfileColumnType::Unknown),
        (true, false, false, false, false) => (ColumnStorage::Boolean, ProfileColumnType::Boolean),
        (false, true, false, false, false) => (ColumnStorage::BigInt, ProfileColumnType::Integer),
        (false, _, true, false, false) => (ColumnStorage::Double, ProfileColumnType::Numeric),
        (false, false, false, false, true) => (ColumnStorage::Varchar, ProfileColumnType::Json),
        // 类型漂移（如数字与字符串或嵌套对象混杂）一律稳定降级为 VARCHAR
        _ => (ColumnStorage::Varchar, ProfileColumnType::Text),
      };
    ColumnPlan {
      name,
      storage,
      inferred_type,
      precision_degraded: false,
      presence: Some(FieldPresence {
        present_count: self.present_count,
        explicit_null_count: self.explicit_null_count,
      }),
    }
  }
}

fn encode_json_doc_value(value: &JsonValue, storage: ColumnStorage) -> Value {
  match storage {
    ColumnStorage::Boolean => value.as_bool().map_or(Value::Null, Value::Boolean),
    ColumnStorage::BigInt => value.as_i64().map_or(Value::Null, Value::BigInt),
    ColumnStorage::Double => value.as_f64().map_or(Value::Null, Value::Double),
    _ => match value {
      JsonValue::Null => Value::Null,
      JsonValue::String(s) => Value::Text(s.clone()),
      other => Value::Text(other.to_string()),
    },
  }
}

// ---------------------------------------------------------------------------
// 列级数据画像 SQL 执行
// ---------------------------------------------------------------------------

fn profile_single_column(
  conn: &Connection,
  plan: &ColumnPlan,
  total_rows: u64,
) -> Result<ColumnProfile, QueryError> {
  let col = quote_ident(&plan.name);
  let (present_count, present_ratio, explicit_null_count) = match plan.presence {
    Some(p) => (
      Some(p.present_count),
      Some(safe_ratio(p.present_count, total_rows)),
      Some(p.explicit_null_count),
    ),
    None => (None, None, None),
  };

  if total_rows == 0 {
    return Ok(ColumnProfile {
      name: plan.name.clone(),
      inferred_type: plan.inferred_type,
      duckdb_type: plan.duckdb_type(),
      precision_degraded: plan.precision_degraded,
      total_count: 0,
      null_count: 0,
      null_ratio: 0.0,
      distinct_count: 0,
      distinct_ratio: 0.0,
      present_count,
      present_ratio,
      explicit_null_count,
      min: None,
      max: None,
      mean: None,
      stddev: None,
      p25: None,
      p50: None,
      p75: None,
      p95: None,
      bins: Vec::new(),
      min_length: None,
      max_length: None,
      top_values: Vec::new(),
    });
  }

  match plan.inferred_type {
    ProfileColumnType::Integer | ProfileColumnType::Numeric => profile_numeric_column(
      conn,
      plan,
      &col,
      total_rows,
      present_count,
      present_ratio,
      explicit_null_count,
    ),
    ProfileColumnType::Binary => profile_binary_column(
      conn,
      plan,
      &col,
      total_rows,
      present_count,
      present_ratio,
      explicit_null_count,
    ),
    ProfileColumnType::Boolean => profile_boolean_column(
      conn,
      plan,
      &col,
      total_rows,
      present_count,
      present_ratio,
      explicit_null_count,
    ),
    ProfileColumnType::Date | ProfileColumnType::Time | ProfileColumnType::Datetime => {
      profile_temporal_column(
        conn,
        plan,
        &col,
        total_rows,
        present_count,
        present_ratio,
        explicit_null_count,
      )
    }
    ProfileColumnType::Text | ProfileColumnType::Json | ProfileColumnType::Unknown => {
      profile_text_column(
        conn,
        plan,
        &col,
        total_rows,
        present_count,
        present_ratio,
        explicit_null_count,
      )
    }
  }
}

#[allow(clippy::too_many_arguments)]
fn profile_numeric_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!(
    "SELECT \
       COUNT(*) - COUNT({col}) AS null_cnt, \
       COUNT(DISTINCT {col}) AS distinct_cnt, \
       MIN({col})::VARCHAR AS min_v, \
       MAX({col})::VARCHAR AS max_v, \
       AVG(CAST({col} AS DOUBLE)) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS mean_v, \
       STDDEV_SAMP(CAST({col} AS DOUBLE)) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS stddev_v, \
       QUANTILE_CONT(CAST({col} AS DOUBLE), 0.25) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS p25, \
       QUANTILE_CONT(CAST({col} AS DOUBLE), 0.50) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS p50, \
       QUANTILE_CONT(CAST({col} AS DOUBLE), 0.75) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS p75, \
       QUANTILE_CONT(CAST({col} AS DOUBLE), 0.95) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS p95, \
       MIN(CAST({col} AS DOUBLE)) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS lo, \
       MAX(CAST({col} AS DOUBLE)) FILTER (WHERE isfinite(CAST({col} AS DOUBLE))) AS hi, \
       COUNT(*) FILTER (WHERE {col} IS NOT NULL AND isfinite(CAST({col} AS DOUBLE))) AS finite_cnt \
     FROM result_set"
  );

  struct NumericAggregateRow {
    null_count: i64,
    distinct_count: i64,
    min: Option<String>,
    max: Option<String>,
    mean: Option<f64>,
    stddev: Option<f64>,
    p25: Option<f64>,
    p50: Option<f64>,
    p75: Option<f64>,
    p95: Option<f64>,
    lo: Option<f64>,
    hi: Option<f64>,
    finite_cnt: i64,
  }

  let agg = conn
    .query_row(&sql, [], |row| {
      Ok(NumericAggregateRow {
        null_count: row.get(0)?,
        distinct_count: row.get(1)?,
        min: row.get(2)?,
        max: row.get(3)?,
        mean: row.get(4)?,
        stddev: row.get(5)?,
        p25: row.get(6)?,
        p50: row.get(7)?,
        p75: row.get(8)?,
        p95: row.get(9)?,
        lo: row.get(10)?,
        hi: row.get(11)?,
        finite_cnt: row.get(12)?,
      })
    })
    .map_err(duck_error)?;

  let NumericAggregateRow {
    null_count,
    distinct_count,
    min,
    max,
    mean,
    stddev,
    p25,
    p50,
    p75,
    p95,
    lo,
    hi,
    finite_cnt,
  } = agg;

  let null_count = null_count.max(0) as u64;
  let distinct_count = distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);

  let bins = match (lo, hi) {
    (Some(lo), Some(hi)) if lo.is_finite() && hi.is_finite() && finite_cnt > 0 => {
      if (hi - lo).abs() <= f64::EPSILON {
        vec![HistogramBin { lower: lo, upper: hi, count: finite_cnt as u64 }]
      } else {
        query_numeric_histogram(conn, col, lo, hi, HISTOGRAM_BIN_COUNT)?
      }
    }
    _ => Vec::new(),
  };

  // 低基数整数列（如状态码/枚举 ID）一并附带 Top-5 分布，日常排障更有用
  let top_values =
    if plan.inferred_type == ProfileColumnType::Integer && distinct_count <= 20 && non_null > 0 {
      query_top_values(conn, col, non_null)?
    } else {
      Vec::new()
    };

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min,
    max,
    mean: finite_opt(mean),
    stddev: finite_opt(stddev),
    p25: finite_opt(p25),
    p50: finite_opt(p50),
    p75: finite_opt(p75),
    p95: finite_opt(p95),
    bins,
    min_length: None,
    max_length: None,
    top_values,
  })
}

fn query_numeric_histogram(
  conn: &Connection,
  col: &str,
  lo: f64,
  hi: f64,
  bin_count: usize,
) -> Result<Vec<HistogramBin>, QueryError> {
  let max_bucket = (bin_count.saturating_sub(1)) as i64;
  let span = hi - lo;
  let width = span / bin_count as f64;
  let sql = format!(
    "SELECT \
       LEAST({max_bucket}, GREATEST(0, CAST(FLOOR((CAST({col} AS DOUBLE) - $1) / $2 * {bin_count}) AS BIGINT))) AS bucket, \
       COUNT(*) AS cnt \
     FROM result_set \
     WHERE {col} IS NOT NULL AND isfinite(CAST({col} AS DOUBLE)) \
     GROUP BY bucket \
     ORDER BY bucket"
  );
  let mut counts = vec![0u64; bin_count];
  let mut stmt = conn.prepare(&sql).map_err(duck_error)?;
  let mut rows = stmt.query([lo, span]).map_err(duck_error)?;
  while let Some(row) = rows.next().map_err(duck_error)? {
    let bucket: i64 = row.get(0).map_err(duck_error)?;
    let cnt: i64 = row.get(1).map_err(duck_error)?;
    if let Ok(idx) = usize::try_from(bucket) {
      if idx < bin_count {
        counts[idx] = cnt.max(0) as u64;
      }
    }
  }
  Ok(
    counts
      .into_iter()
      .enumerate()
      .map(|(i, count)| HistogramBin {
        lower: lo + width * i as f64,
        upper: if i + 1 == bin_count { hi } else { lo + width * (i + 1) as f64 },
        count,
      })
      .collect(),
  )
}

#[allow(clippy::too_many_arguments)]
fn profile_binary_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!(
    "SELECT \
       COUNT(*) - COUNT({col}) AS null_cnt, \
       COUNT(DISTINCT {col}) AS distinct_cnt, \
       MIN(octet_length({col})) AS min_len, \
       MAX(octet_length({col})) AS max_len \
     FROM result_set"
  );
  let (null_count, distinct_count, min_len, max_len): (i64, i64, Option<i64>, Option<i64>) = conn
    .query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .map_err(duck_error)?;
  let null_count = null_count.max(0) as u64;
  let distinct_count = distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min: None,
    max: None,
    mean: None,
    stddev: None,
    p25: None,
    p50: None,
    p75: None,
    p95: None,
    bins: Vec::new(),
    min_length: min_len.map(|v| v.max(0) as u64),
    max_length: max_len.map(|v| v.max(0) as u64),
    top_values: Vec::new(),
  })
}

#[allow(clippy::too_many_arguments)]
fn profile_boolean_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!("SELECT COUNT(*) - COUNT({col}), COUNT(DISTINCT {col}) FROM result_set");
  let (null_count, distinct_count): (i64, i64) =
    conn.query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?))).map_err(duck_error)?;
  let null_count = null_count.max(0) as u64;
  let distinct_count = distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);
  let top_values = if non_null > 0 { query_top_values(conn, col, non_null)? } else { Vec::new() };

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min: None,
    max: None,
    mean: None,
    stddev: None,
    p25: None,
    p50: None,
    p75: None,
    p95: None,
    bins: Vec::new(),
    min_length: None,
    max_length: None,
    top_values,
  })
}

#[allow(clippy::too_many_arguments)]
fn profile_temporal_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!(
    "SELECT \
       COUNT(*) - COUNT({col}), \
       COUNT(DISTINCT {col}), \
       MIN({col})::VARCHAR, \
       MAX({col})::VARCHAR \
     FROM result_set"
  );
  let (null_count, distinct_count, min, max): (i64, i64, Option<String>, Option<String>) = conn
    .query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .map_err(duck_error)?;
  let null_count = null_count.max(0) as u64;
  let distinct_count = distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);
  let top_values = if non_null > 0 { query_top_values(conn, col, non_null)? } else { Vec::new() };

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min,
    max,
    mean: None,
    stddev: None,
    p25: None,
    p50: None,
    p75: None,
    p95: None,
    bins: Vec::new(),
    min_length: None,
    max_length: None,
    top_values,
  })
}

#[allow(clippy::too_many_arguments)]
fn profile_text_column(
  conn: &Connection,
  plan: &ColumnPlan,
  col: &str,
  total_rows: u64,
  present_count: Option<u64>,
  present_ratio: Option<f64>,
  explicit_null_count: Option<u64>,
) -> Result<ColumnProfile, QueryError> {
  let sql = format!(
    "SELECT \
       COUNT(*) - COUNT({col}), \
       COUNT(DISTINCT {col}), \
       MIN({col}), \
       MAX({col}), \
       MIN(length({col})), \
       MAX(length({col})) \
     FROM result_set"
  );
  let (null_count, distinct_count, min, max, min_len, max_len): (
    i64,
    i64,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
  ) = conn
    .query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
    .map_err(duck_error)?;
  let null_count = null_count.max(0) as u64;
  let distinct_count = distinct_count.max(0) as u64;
  let non_null = total_rows.saturating_sub(null_count);
  let top_values = if non_null > 0 { query_top_values(conn, col, non_null)? } else { Vec::new() };

  Ok(ColumnProfile {
    name: plan.name.clone(),
    inferred_type: plan.inferred_type,
    duckdb_type: plan.duckdb_type(),
    precision_degraded: plan.precision_degraded,
    total_count: total_rows,
    null_count,
    null_ratio: safe_ratio(null_count, total_rows),
    distinct_count,
    distinct_ratio: safe_ratio(distinct_count, non_null),
    present_count,
    present_ratio,
    explicit_null_count,
    min,
    max,
    mean: None,
    stddev: None,
    p25: None,
    p50: None,
    p75: None,
    p95: None,
    bins: Vec::new(),
    min_length: min_len.map(|v| v.max(0) as u64),
    max_length: max_len.map(|v| v.max(0) as u64),
    top_values,
  })
}

fn query_top_values(
  conn: &Connection,
  col: &str,
  non_null_total: u64,
) -> Result<Vec<TopValueEntry>, QueryError> {
  let sql = format!(
    "SELECT CAST({col} AS VARCHAR) AS val, COUNT(*) AS cnt \
     FROM result_set \
     WHERE {col} IS NOT NULL \
     GROUP BY {col} \
     ORDER BY cnt DESC, val ASC \
     LIMIT {TOP_VALUES_LIMIT}"
  );
  let mut stmt = conn.prepare(&sql).map_err(duck_error)?;
  let mut rows = stmt.query([]).map_err(duck_error)?;
  let mut out = Vec::with_capacity(TOP_VALUES_LIMIT);
  while let Some(row) = rows.next().map_err(duck_error)? {
    let value: String = row.get(0).map_err(duck_error)?;
    let count: i64 = row.get(1).map_err(duck_error)?;
    let count_u = count.max(0) as u64;
    out.push(TopValueEntry { value, count: count_u, ratio: safe_ratio(count_u, non_null_total) });
  }
  Ok(out)
}

fn safe_ratio(numerator: u64, denominator: u64) -> f64 {
  if denominator == 0 {
    0.0
  } else {
    numerator as f64 / denominator as f64
  }
}

fn finite_opt(value: Option<f64>) -> Option<f64> {
  value.filter(|v| v.is_finite())
}

fn duck_error(error: duckdb::Error) -> QueryError {
  QueryError::message(error.to_string())
}

fn map_interrupt_error(error: QueryError, cancelled: &AtomicBool) -> QueryError {
  if cancelled.load(Ordering::SeqCst) || error.message.contains("INTERRUPT Error") {
    QueryError::with_code(ANALYTICS_CANCELLED_CODE, ANALYTICS_CANCELLED_CODE)
  } else {
    error
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;
  use std::time::Duration;

  fn col(name: &str, logical: &str) -> AnalyticsColumnInput {
    AnalyticsColumnInput {
      name: name.to_string(),
      logical_type: Some(logical.to_string()),
      database_type: None,
    }
  }

  #[test]
  fn precision_and_type_boundaries_profile_accurately() {
    let columns = vec![
      col("big_i64", "integer"),
      col("big_huge", "integer"),
      col("big_overflow", "integer"),
      col("dec_38", "decimal"),
      col("dec_overflow", "decimal"),
      col("bin", "binary"),
      col("dt", "date"),
      col("tm", "time"),
      col("ts", "datetime"),
      col("flag", "boolean"),
      col("txt", "text"),
    ];
    let rows = vec![
      vec![
        json!({"type": "bigint", "value": "9007199254740993"}),
        json!({"type": "bigint", "value": "170141183460469231731687303715884105727"}),
        json!({"type": "bigint", "value": "999999999999999999999999999999999999999"}),
        json!({"type": "decimal", "value": "123456789012345678901234567890.12345678"}),
        json!({"type": "decimal", "value": "1234567890123456789012345678901234567890.12345"}),
        json!({"type": "binary", "value": "aa01ff00"}),
        json!({"type": "date", "value": "2026-11-20"}),
        json!({"type": "time", "value": "09:15:30.123456"}),
        json!({"type": "datetime", "value": "2026-11-20T09:15:30+08:00"}),
        json!(true),
        json!(""),
      ],
      vec![
        json!({"type": "bigint", "value": "-9223372036854775808"}),
        json!({"type": "bigint", "value": "-170141183460469231731687303715884105727"}),
        JsonValue::Null,
        json!({"type": "decimal", "value": "-99999.50"}),
        json!({"type": "decimal", "value": "0.00000001"}),
        json!({"type": "binary", "value": "00"}),
        json!({"type": "date", "value": "2026-01-01"}),
        json!({"type": "time", "value": "00:00:00"}),
        json!({"type": "datetime", "value": "2026-01-01 00:00:00"}),
        JsonValue::Null,
        json!("hello"),
      ],
    ];

    let session = create_result_session(&columns, &rows, MAX_ANALYTICS_ROWS).expect("load");
    let result = profile_session("t1", &session).expect("profile");
    assert_eq!(result.row_count, 2);
    assert_eq!(result.column_count, 11);

    let by_name = |name: &str| {
      result
        .columns
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("missing column {name}"))
    };

    let big_i64 = by_name("big_i64");
    assert_eq!(big_i64.duckdb_type, "BIGINT");
    assert_eq!(big_i64.min.as_deref(), Some("-9223372036854775808"));
    assert_eq!(big_i64.max.as_deref(), Some("9007199254740993"));
    assert!(!big_i64.precision_degraded);

    let big_huge = by_name("big_huge");
    assert_eq!(big_huge.duckdb_type, "HUGEINT");
    assert_eq!(big_huge.max.as_deref(), Some("170141183460469231731687303715884105727"));

    let big_ov = by_name("big_overflow");
    assert_eq!(big_ov.duckdb_type, "VARCHAR");
    assert!(big_ov.precision_degraded);
    assert_eq!(big_ov.null_count, 1);

    let dec38 = by_name("dec_38");
    assert_eq!(dec38.duckdb_type, "DECIMAL(38,8)");
    assert!(!dec38.precision_degraded);
    assert_eq!(dec38.max.as_deref(), Some("123456789012345678901234567890.12345678"));

    let dec_ov = by_name("dec_overflow");
    assert_eq!(dec_ov.duckdb_type, "VARCHAR");
    assert!(dec_ov.precision_degraded);

    let bin = by_name("bin");
    assert_eq!(bin.duckdb_type, "BLOB");
    assert_eq!(bin.min_length, Some(1));
    assert_eq!(bin.max_length, Some(4));
    assert!(bin.min.is_none());

    let dt = by_name("dt");
    assert_eq!(dt.duckdb_type, "DATE");
    assert_eq!(dt.min.as_deref(), Some("2026-01-01"));
    assert_eq!(dt.max.as_deref(), Some("2026-11-20"));

    let ts = by_name("ts");
    assert_eq!(ts.duckdb_type, "TIMESTAMP");
    assert_eq!(ts.max.as_deref(), Some("2026-11-20 09:15:30"));

    let flag = by_name("flag");
    assert_eq!(flag.duckdb_type, "BOOLEAN");
    assert_eq!(flag.null_count, 1);
    assert_eq!(flag.top_values.len(), 1);
    assert_eq!(flag.top_values[0].value, "true");

    let txt = by_name("txt");
    assert_eq!(txt.min_length, Some(0));
    assert_eq!(txt.max_length, Some(5));
  }

  #[test]
  fn sandbox_blocks_filesystem_escape_and_reverse_verifies_unprotected() {
    let columns = vec![col("id", "integer")];
    let rows = vec![vec![json!(1)], vec![json!(2)]];
    let session = create_result_session(&columns, &rows, MAX_ANALYTICS_ROWS).expect("load");
    let conn = session.connection.lock().expect("lock");

    let escape_csv =
      std::env::temp_dir().join(format!("dataomni-analytics-escape-{}.csv", std::process::id()));
    let _ = std::fs::remove_file(&escape_csv);
    let escaped_path = escape_csv.to_string_lossy().replace('\'', "''");

    assert!(conn.execute_batch("SELECT * FROM read_text('/etc/hosts')").is_err());
    assert!(conn.execute_batch(&format!("COPY result_set TO '{escaped_path}'")).is_err());
    assert!(conn.execute_batch("ATTACH '/tmp/dataomni-should-fail.duckdb' AS ext").is_err());
    assert!(conn.execute_batch("SET enable_external_access = true").is_err());
    assert!(!escape_csv.exists());

    // 反向验证：未调 lock_sandbox 时 read_text 可成功读到 /etc/hosts，确认上面的断言确实由沙盒拦下
    let unprotected = open_memory_connection().expect("unprotected");
    let content: String = unprotected
      .query_row("SELECT content FROM read_text('/etc/hosts') LIMIT 1", [], |r| r.get(0))
      .expect("unprotected read_text must succeed");
    assert!(!content.is_empty());
  }

  #[test]
  fn json_docs_distinguish_missing_fields_from_explicit_nulls_and_handle_type_drift() {
    let docs = vec![
      json!({"_id": "1", "email": null, "score": 100, "meta": {"a": 1}}),
      json!({"_id": "2", "email": "b@example.com", "score": "N/A"}),
      json!({"_id": "3", "score": 95.5}),
    ];
    let session = create_json_docs_session(&docs, MAX_ANALYTICS_ROWS).expect("load docs");
    let result = profile_session("nosql-1", &session).expect("profile docs");
    assert_eq!(result.row_count, 3);

    let email = result.columns.iter().find(|c| c.name == "email").expect("email col");
    assert_eq!(email.present_count, Some(2));
    assert_eq!(email.explicit_null_count, Some(1));
    assert_eq!(email.null_count, 2); // 1 缺失 + 1 显式 null
    assert!((email.present_ratio.unwrap_or(0.0) - (2.0 / 3.0)).abs() < 1e-6);

    let score = result.columns.iter().find(|c| c.name == "score").expect("score col");
    assert_eq!(score.duckdb_type, "VARCHAR");
    assert_eq!(score.present_count, Some(3));
    assert_eq!(score.explicit_null_count, Some(0));

    let meta = result.columns.iter().find(|c| c.name == "meta").expect("meta col");
    assert_eq!(meta.inferred_type, ProfileColumnType::Json);
    assert_eq!(meta.present_count, Some(1));
  }

  #[test]
  fn numeric_histogram_handles_constant_and_nan_values_safely() {
    let columns = vec![col("same", "integer"), col("spread", "decimal")];
    let rows: Vec<Vec<JsonValue>> = (0..100)
      .map(|i| vec![json!(42), if i == 0 { JsonValue::Null } else { json!(i as f64) }])
      .collect();
    let session = create_result_session(&columns, &rows, MAX_ANALYTICS_ROWS).expect("load");
    let result = profile_session("hist", &session).expect("profile");

    let same = &result.columns[0];
    assert_eq!(same.bins.len(), 1);
    assert_eq!(same.bins[0].count, 100);
    assert_eq!(same.bins[0].lower, 42.0);
    assert_eq!(same.bins[0].upper, 42.0);

    let spread = &result.columns[1];
    assert_eq!(spread.bins.len(), HISTOGRAM_BIN_COUNT);
    let total_binned: u64 = spread.bins.iter().map(|b| b.count).sum();
    assert_eq!(total_binned, 99);
    assert_eq!(spread.null_count, 1);
    assert!(spread.p50.is_some());
    assert!(spread.p95.is_some());
  }

  #[tokio::test]
  async fn registry_handles_cancel_recovery_close_and_lru_eviction() {
    let registry = LocalAnalyticsRegistry::with_capacity(2);
    let s1 = registry
      .load_result(vec![col("id", "integer")], vec![vec![json!(1)]])
      .await
      .expect("load s1");
    let s2 = registry
      .load_result(vec![col("id", "integer")], vec![vec![json!(2)]])
      .await
      .expect("load s2");
    assert_eq!(registry.len(), 2);

    // 访问 s1 刷新其 LRU 顺序，接着装载 s3 应淘汰 s2
    let _ = registry.profile(&s1.analytics_id).await.expect("profile s1");
    let s3 = registry
      .load_result(vec![col("id", "integer")], vec![vec![json!(3)]])
      .await
      .expect("load s3");
    assert_eq!(registry.len(), 2);

    let err = registry.profile(&s2.analytics_id).await.expect_err("s2 evicted");
    assert_eq!(err.code.as_deref(), Some(ANALYTICS_NOT_FOUND_CODE));

    assert!(registry.close(&s3.analytics_id).expect("close s3"));
    assert_eq!(registry.len(), 1);
    assert!(!registry.close(&s3.analytics_id).expect("close again"));

    // 验证 interrupt 取消慢查询且会话可复用
    let heavy_rows: Vec<Vec<JsonValue>> = (0..15_000).map(|i| vec![json!(i)]).collect();
    let heavy =
      registry.load_result(vec![col("id", "integer")], heavy_rows).await.expect("load heavy");
    let session = registry.get(&heavy.analytics_id).expect("get heavy");
    let session_clone = Arc::clone(&session);
    let handle = std::thread::spawn(move || {
      let guard = session_clone.connection.lock().expect("lock");
      guard.query_row::<i64, _, _>(
        "SELECT COUNT(*) FROM result_set a, result_set b WHERE hash(a.id, b.id) % 97 = 0",
        [],
        |r| r.get(0),
      )
    });
    std::thread::sleep(Duration::from_millis(80));
    let start = Instant::now();
    assert!(registry.cancel(&heavy.analytics_id).expect("cancel"));
    let res = handle.join().expect("join");
    assert!(res.is_err());
    assert!(start.elapsed() < Duration::from_secs(1));

    // 中止后同一会话仍可正常画像
    let recovered = registry.profile(&heavy.analytics_id).await.expect("recover profile");
    assert_eq!(recovered.row_count, 15_000);
  }

  #[test]
  fn handles_empty_result_set_and_duplicate_column_names() {
    let columns = vec![col("id", "integer"), col("id", "text"), col("", "text")];
    let rows: Vec<Vec<JsonValue>> = Vec::new();
    let session = create_result_session(&columns, &rows, MAX_ANALYTICS_ROWS).expect("load empty");
    let profile = profile_session("empty", &session).expect("profile empty");
    assert_eq!(profile.row_count, 0);
    assert_eq!(profile.column_count, 3);
    assert_eq!(profile.columns[0].name, "id");
    assert_eq!(profile.columns[1].name, "id 2");
    assert_eq!(profile.columns[2].name, "col_3");
    assert_eq!(profile.columns[0].null_ratio, 0.0);
  }
}
