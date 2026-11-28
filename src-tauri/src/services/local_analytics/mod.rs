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

use crate::services::duckdb::{column_header as duckdb_column_header, decode as duckdb_decode};
use crate::services::query_executor::{
  number_duplicate_columns, QueryColumnMetadata, QueryTruncationReason, DEFAULT_QUERY_BYTE_LIMIT,
};
use crate::services::{read_only_gate, QueryError};
use duckdb::core::LogicalTypeId;
use duckdb::types::Value;
use duckdb::{appender_params_from_iter, Connection, InterruptHandle};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// 内存分析单会话默认最大装载行数（DA0 E1：10 万行 × 20 列 DuckDB 内存 54.5 MiB）
pub const MAX_ANALYTICS_ROWS: usize = 100_000;
/// 二次 SQL 查询默认返回行数上限
pub const DEFAULT_ANALYTICS_QUERY_LIMIT: usize = 10_000;
/// 二次 SQL 查询默认超时（秒）
pub const DEFAULT_ANALYTICS_TIMEOUT_SECS: u64 = 30;
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
/// 单会话最多附加的对账参考表数量（共享 512MB 内存上限）
pub const MAX_ATTACHED_TABLES: usize = 4;

/// 会话不存在或已被淘汰/关闭
pub const ANALYTICS_NOT_FOUND_CODE: &str = "DATAOMNI_ANALYTICS_NOT_FOUND";
/// 分析查询已被用户取消
pub const ANALYTICS_CANCELLED_CODE: &str = "DATAOMNI_ANALYTICS_CANCELLED";
/// 二次 SQL 违反只读白名单（写关键字、多语句或副作用函数）
pub const ANALYTICS_READ_ONLY_REFUSED_CODE: &str = "DATAOMNI_ANALYTICS_READ_ONLY_REFUSED";
/// 二次 SQL 执行超时
pub const ANALYTICS_TIMEOUT_CODE: &str = "DATAOMNI_ANALYTICS_TIMEOUT";
/// 本地数据文件格式不受支持或无法打开
pub const ANALYTICS_UNSUPPORTED_FILE_CODE: &str = "DATAOMNI_ANALYTICS_UNSUPPORTED_FILE";
/// 单会话附加表数量已达上限
pub const ANALYTICS_ATTACH_LIMIT_CODE: &str = "DATAOMNI_ANALYTICS_ATTACH_LIMIT";
/// 附加表名非法或与主表/既有附加表冲突
pub const ANALYTICS_INVALID_TABLE_NAME_CODE: &str = "DATAOMNI_ANALYTICS_INVALID_TABLE_NAME";

const LOCK_POISONED: &str = "Analytics session lock poisoned";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalyticsSourceKind {
  ResultSet,
  JsonDocuments,
  LocalFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalFileKind {
  Parquet,
  Csv,
  Tsv,
  Json,
  Ndjson,
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
  #[serde(skip_serializing_if = "Option::is_none")]
  pub file_kind: Option<LocalFileKind>,
  pub row_count: usize,
  pub column_count: usize,
  pub truncated: bool,
  pub columns: Vec<AnalyticsColumnMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachedTableSummary {
  pub table_name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub file_kind: Option<LocalFileKind>,
  pub row_count: usize,
  pub column_count: usize,
  pub truncated: bool,
  pub columns: Vec<AnalyticsColumnMeta>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsQueryResult {
  pub columns: Vec<String>,
  pub column_metadata: Vec<QueryColumnMetadata>,
  pub rows: Vec<Vec<JsonValue>>,
  pub row_count: usize,
  pub truncated: bool,
  pub truncation_reason: Option<QueryTruncationReason>,
  pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParquetExportSummary {
  pub row_count: u64,
  pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistogramBin {
  pub lower: f64,
  pub upper: f64,
  pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimeHistogramBin {
  pub label: String,
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
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub time_bins: Vec<TimeHistogramBin>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub min_length: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max_length: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub empty_string_count: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub padded_whitespace_count: Option<u64>,
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
  file_kind: Option<LocalFileKind>,
  row_count: usize,
  truncated: bool,
  columns: Vec<ColumnPlan>,
  attached_tables: Mutex<Vec<String>>,
}

impl AnalyticsSession {
  pub fn is_truncated(&self) -> bool {
    self.truncated
  }

  pub fn file_kind(&self) -> Option<LocalFileKind> {
    self.file_kind
  }
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

  /// 打开本地 `.parquet` / `.csv` / `.tsv` / `.json` / `.ndjson` 文件物化到内存表后锁死沙盒
  pub async fn open_file(&self, path: PathBuf) -> Result<AnalyticsSessionSummary, QueryError> {
    let session = blocking(move || open_local_file_session(&path, MAX_ANALYTICS_ROWS)).await?;
    self.insert(session)
  }

  /// 向既有已锁沙盒会话附加一张内存结果表（如 `ref_1`）用于跨结果 LEFT JOIN / EXCEPT 对账
  pub async fn attach_result(
    &self,
    analytics_id: &str,
    table_name: String,
    columns: Vec<AnalyticsColumnInput>,
    rows: Vec<Vec<JsonValue>>,
  ) -> Result<AttachedTableSummary, QueryError> {
    let session = self.get(analytics_id)?;
    blocking(move || {
      attach_result_to_session(&session, &table_name, &columns, &rows, MAX_ANALYTICS_ROWS, None)
    })
    .await
  }

  /// 通过瞬态只读连接物化本地文件再写入既有已锁沙盒会话（用户 SQL 所在连接全程永不解锁）
  pub async fn attach_file(
    &self,
    analytics_id: &str,
    table_name: String,
    path: PathBuf,
  ) -> Result<AttachedTableSummary, QueryError> {
    let session = self.get(analytics_id)?;
    blocking(move || attach_file_to_session(&session, &table_name, &path, MAX_ANALYTICS_ROWS)).await
  }

  /// 对指定会话执行列级数据画像（异步在阻塞线程池跑，可被 `cancel` 中止）
  pub async fn profile(&self, analytics_id: &str) -> Result<ProfileResult, QueryError> {
    let session = self.get(analytics_id)?;
    let id = analytics_id.to_string();
    blocking(move || profile_session(&id, &session)).await
  }

  /// 对内存表 `result_set` 执行单条只读 SQL（`read_only_gate` + 沙盒双保险 + 超时中止）
  pub async fn query(
    &self,
    analytics_id: &str,
    sql: String,
    limit: Option<usize>,
    timeout_seconds: Option<u64>,
  ) -> Result<AnalyticsQueryResult, QueryError> {
    let session = self.get(analytics_id)?;
    let row_limit = limit.unwrap_or(DEFAULT_ANALYTICS_QUERY_LIMIT).clamp(1, MAX_ANALYTICS_ROWS);
    let timeout_secs = timeout_seconds.unwrap_or(DEFAULT_ANALYTICS_TIMEOUT_SECS).clamp(1, 300);
    let interrupt = Arc::clone(&session.interrupt);
    let session_for_work = Arc::clone(&session);

    let fut =
      blocking(move || query_session(&session_for_work, &sql, row_limit, DEFAULT_QUERY_BYTE_LIMIT));
    match tokio::time::timeout(Duration::from_secs(timeout_secs), fut).await {
      Ok(result) => result,
      Err(_) => {
        interrupt.interrupt();
        Err(QueryError::with_code(
          ANALYTICS_TIMEOUT_CODE,
          format!("{ANALYTICS_TIMEOUT_CODE}: {timeout_secs}"),
        ))
      }
    }
  }

  /// 将内存表或指定只读查询结果导出为 Parquet（用户 SQL 仅在沙盒内执行，经瞬态连接原子写盘）
  pub async fn export_parquet(
    &self,
    analytics_id: &str,
    sql: Option<String>,
    output_path: PathBuf,
  ) -> Result<ParquetExportSummary, QueryError> {
    let session = self.get(analytics_id)?;
    blocking(move || export_parquet_from_session(&session, sql.as_deref(), &output_path)).await
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

  /// 关闭并释放指定会话的内存（在注册表锁外释放 DuckDB 连接，H16 / P2-3）
  pub fn close(&self, analytics_id: &str) -> Result<bool, QueryError> {
    let removed = {
      let mut inner = self.inner.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
      inner.sessions.remove(analytics_id)
    };
    if let Some(entry) = removed {
      entry.session.cancelled.store(true, Ordering::SeqCst);
      entry.session.interrupt.interrupt();
      drop(entry);
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
      file_kind: session.file_kind,
      row_count: session.row_count,
      column_count: session.columns.len(),
      truncated: session.truncated,
      columns: session.columns.iter().map(ColumnPlan::meta).collect(),
    };

    // 在锁内仅摘除被淘汰会话，把 DuckDB C++ 析构移到锁外（H16 / P2-3）
    let mut evicted = Vec::new();
    {
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
          evicted.push(removed);
        }
      }

      inner
        .sessions
        .insert(analytics_id, RegistryEntry { session: Arc::new(session), last_used_seq: seq });
    }
    drop(evicted);
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

mod export;
mod ingest;
mod profile;

pub use export::export_parquet_from_session;
pub use ingest::{
  attach_file_to_session, attach_result_to_session, create_json_docs_session,
  create_result_session, detect_local_file_kind, open_local_file_session,
};
pub use profile::profile_session;

fn introspect_result_table(conn: &Connection) -> Result<Vec<ColumnPlan>, QueryError> {
  let mut stmt = conn.prepare("SELECT * FROM result_set LIMIT 0").map_err(duck_error)?;
  drop(stmt.stream_arrow([]).map_err(duck_error)?);
  let (metadata, _names) = duckdb_column_header(&stmt);
  let mut plans = Vec::with_capacity(metadata.len());
  for (idx, meta) in metadata.into_iter().enumerate() {
    let logical = stmt.column_logical_type(idx);
    let (storage, inferred_type) = map_duckdb_logical_type(&logical);
    plans.push(ColumnPlan {
      name: meta.name,
      storage,
      inferred_type,
      precision_degraded: false,
      presence: None,
    });
  }
  Ok(plans)
}

fn map_duckdb_logical_type(
  logical: &duckdb::core::LogicalTypeHandle,
) -> (ColumnStorage, ProfileColumnType) {
  match logical.id() {
    LogicalTypeId::Boolean => (ColumnStorage::Boolean, ProfileColumnType::Boolean),
    LogicalTypeId::Tinyint
    | LogicalTypeId::Smallint
    | LogicalTypeId::Integer
    | LogicalTypeId::Bigint
    | LogicalTypeId::UTinyint
    | LogicalTypeId::USmallint
    | LogicalTypeId::UInteger
    | LogicalTypeId::UBigint => (ColumnStorage::BigInt, ProfileColumnType::Integer),
    LogicalTypeId::Hugeint | LogicalTypeId::UHugeint => {
      (ColumnStorage::HugeInt, ProfileColumnType::Integer)
    }
    LogicalTypeId::Decimal => {
      (ColumnStorage::Decimal { scale: logical.decimal_scale() }, ProfileColumnType::Numeric)
    }
    LogicalTypeId::Float | LogicalTypeId::Double => {
      (ColumnStorage::Double, ProfileColumnType::Numeric)
    }
    LogicalTypeId::Date => (ColumnStorage::Date, ProfileColumnType::Date),
    LogicalTypeId::Time | LogicalTypeId::TimeTZ | LogicalTypeId::TimeNs => {
      (ColumnStorage::Time, ProfileColumnType::Time)
    }
    LogicalTypeId::Timestamp
    | LogicalTypeId::TimestampS
    | LogicalTypeId::TimestampMs
    | LogicalTypeId::TimestampNs
    | LogicalTypeId::TimestampTZ => (ColumnStorage::Timestamp, ProfileColumnType::Datetime),
    LogicalTypeId::Blob | LogicalTypeId::Geometry => {
      (ColumnStorage::Blob, ProfileColumnType::Binary)
    }
    LogicalTypeId::List | LogicalTypeId::Array | LogicalTypeId::Struct => {
      (ColumnStorage::Varchar, ProfileColumnType::Json)
    }
    _ => (ColumnStorage::Varchar, ProfileColumnType::Text),
  }
}

/// 在沙盒内执行单条只读 SQL（`read_only_gate` + 沙盒锁双层防线，受行数与字节预算保护）
pub fn query_session(
  session: &AnalyticsSession,
  sql: &str,
  row_limit: usize,
  byte_limit: usize,
) -> Result<AnalyticsQueryResult, QueryError> {
  if let Err(refusal) = read_only_gate::check(sql) {
    return Err(QueryError::with_code(
      ANALYTICS_READ_ONLY_REFUSED_CODE,
      format!("{ANALYTICS_READ_ONLY_REFUSED_CODE}: {refusal}"),
    ));
  }

  let started = Instant::now();
  session.cancelled.store(false, Ordering::SeqCst);
  session.busy.store(true, Ordering::Release);
  let _busy_guard = ResetBusy(&session.busy);

  let conn = session.connection.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
  let mut prepared =
    conn.prepare(sql).map_err(|err| map_interrupt_error(duck_error(err), &session.cancelled))?;
  drop(
    prepared
      .stream_arrow([])
      .map_err(|err| map_interrupt_error(duck_error(err), &session.cancelled))?,
  );
  let (column_metadata, columns) = duckdb_column_header(&prepared);
  let type_ids: Vec<LogicalTypeId> =
    (0..columns.len()).map(|idx| prepared.column_logical_type(idx).id()).collect();

  let mut raw_rows = prepared.raw_query();
  let mut rows: Vec<Vec<JsonValue>> = Vec::new();
  let mut bytes_read: usize = 0;
  let mut truncation_reason: Option<QueryTruncationReason> = None;

  while let Some(row) =
    raw_rows.next().map_err(|err| map_interrupt_error(duck_error(err), &session.cancelled))?
  {
    if rows.len() >= row_limit {
      truncation_reason = Some(QueryTruncationReason::RowLimit);
      break;
    }
    let mut values = Vec::with_capacity(columns.len());
    for (idx, type_id) in type_ids.iter().enumerate() {
      let value: Value = if *type_id == LogicalTypeId::SqlNull {
        Value::Null
      } else {
        row.get(idx).map_err(duck_error)?
      };
      values.push(duckdb_decode(&value, *type_id));
    }
    let row_bytes = serde_json::to_vec(&values).map(|encoded| encoded.len()).unwrap_or(0);
    if !rows.is_empty() && bytes_read.saturating_add(row_bytes) > byte_limit {
      truncation_reason = Some(QueryTruncationReason::ByteLimit);
      break;
    }
    bytes_read = bytes_read.saturating_add(row_bytes);
    rows.push(values);
  }

  let row_count = rows.len();
  Ok(AnalyticsQueryResult {
    columns,
    column_metadata,
    rows,
    row_count,
    truncated: truncation_reason.is_some(),
    truncation_reason,
    elapsed_ms: started.elapsed().as_millis() as u64,
  })
}

struct ResetBusy<'a>(&'a AtomicBool);
impl Drop for ResetBusy<'_> {
  fn drop(&mut self) {
    self.0.store(false, Ordering::Release);
  }
}

fn quote_ident(name: &str) -> String {
  format!("\"{}\"", name.replace('"', "\"\""))
}

// ---------------------------------------------------------------------------
// 列类型推断与值编码（SQL ResultSet）
// ---------------------------------------------------------------------------

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
  #[allow(unused_imports)]
  use super::ingest::*;
  #[allow(unused_imports)]
  use super::profile::*;
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

  #[test]
  fn secondary_query_supports_group_by_pivot_and_refuses_writes() {
    let columns = vec![col("region", "text"), col("quarter", "text"), col("amount", "decimal")];
    let rows = vec![
      vec![json!("North"), json!("Q1"), json!(100.5)],
      vec![json!("North"), json!("Q2"), json!(200.0)],
      vec![json!("South"), json!("Q1"), json!(50.25)],
      vec![json!("South"), json!("Q2"), json!(75.75)],
    ];
    let session = create_result_session(&columns, &rows, MAX_ANALYTICS_ROWS).expect("load");

    // GROUP BY
    let grouped = query_session(
      &session,
      "SELECT region, SUM(amount) AS total FROM result_set GROUP BY region ORDER BY region",
      100,
      DEFAULT_QUERY_BYTE_LIMIT,
    )
    .expect("group by");
    assert_eq!(grouped.columns, vec!["region", "total"]);
    assert_eq!(grouped.row_count, 2);
    assert_eq!(grouped.rows[0][0], json!("North"));
    assert_eq!(grouped.rows[0][1], json!(300.5));

    // DuckDB 原生 PIVOT
    let pivoted = query_session(
      &session,
      "PIVOT result_set ON quarter USING SUM(amount) GROUP BY region ORDER BY region",
      100,
      DEFAULT_QUERY_BYTE_LIMIT,
    )
    .expect("pivot");
    assert_eq!(pivoted.row_count, 2);
    assert!(pivoted.columns.contains(&"Q1".to_string()));
    assert!(pivoted.columns.contains(&"Q2".to_string()));

    // 行数截断门
    let limited = query_session(&session, "SELECT * FROM result_set", 2, DEFAULT_QUERY_BYTE_LIMIT)
      .expect("limited");
    assert_eq!(limited.row_count, 2);
    assert!(limited.truncated);
    assert_eq!(limited.truncation_reason, Some(QueryTruncationReason::RowLimit));

    // 写语句与多语句必须被 read_only_gate 拒绝
    for bad in [
      "DELETE FROM result_set",
      "DROP TABLE result_set",
      "SELECT 1; SELECT 2",
      "SELECT * FROM read_text('/etc/hosts')",
    ] {
      let err = query_session(&session, bad, 100, DEFAULT_QUERY_BYTE_LIMIT)
        .expect_err(&format!("must refuse {bad}"));
      assert_eq!(err.code.as_deref(), Some(ANALYTICS_READ_ONLY_REFUSED_CODE));
    }
  }

  #[test]
  fn export_parquet_and_open_local_files_roundtrip_and_lock_sandbox() {
    let dir = std::env::temp_dir().join(format!("dataomni-da2-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("mkdir");

    let columns = vec![
      col("id", "integer"),
      col("name", "text"),
      col("price", "decimal"),
      col("created", "date"),
      col("active", "boolean"),
      col("raw", "binary"),
    ];
    let rows = vec![
      vec![
        json!(1),
        json!("apple"),
        json!({"type": "decimal", "value": "12.50"}),
        json!({"type": "date", "value": "2026-11-20"}),
        json!(true),
        json!({"type": "binary", "value": "cafe"}),
      ],
      vec![
        json!(2),
        json!("banana"),
        json!({"type": "decimal", "value": "8.25"}),
        json!({"type": "date", "value": "2026-11-21"}),
        json!(false),
        JsonValue::Null,
      ],
      vec![
        json!(3),
        json!("cherry"),
        json!({"type": "decimal", "value": "19.99"}),
        json!({"type": "date", "value": "2026-11-22"}),
        json!(true),
        json!({"type": "binary", "value": "0102"}),
      ],
    ];
    let src = create_result_session(&columns, &rows, MAX_ANALYTICS_ROWS).expect("load src");

    // 1) 导出带过滤 SQL 的 Parquet
    let parquet_path = dir.join("filtered.parquet");
    let exported = export_parquet_from_session(
      &src,
      Some("SELECT id, name, price, created, active, raw FROM result_set WHERE active = true ORDER BY id"),
      &parquet_path,
    )
    .expect("export parquet");
    assert_eq!(exported.row_count, 2);
    assert!(exported.bytes > 0);

    // 写语句在导出 Parquet 里同样被拒
    let bad_export =
      export_parquet_from_session(&src, Some("DROP TABLE result_set"), &parquet_path)
        .expect_err("refuse write in parquet export");
    assert_eq!(bad_export.code.as_deref(), Some(ANALYTICS_READ_ONLY_REFUSED_CODE));

    // 2) 用 open_local_file_session 重新打开刚导出的 Parquet 并验证沙盒已锁死
    let reopened =
      open_local_file_session(&parquet_path, MAX_ANALYTICS_ROWS).expect("open parquet");
    assert_eq!(reopened.file_kind, Some(LocalFileKind::Parquet));
    assert_eq!(reopened.row_count, 2);
    assert!(!reopened.truncated);
    let q = query_session(
      &reopened,
      "SELECT id, name FROM result_set ORDER BY id",
      10,
      DEFAULT_QUERY_BYTE_LIMIT,
    )
    .expect("query reopened");
    assert_eq!(q.rows[0], vec![json!(1), json!("apple")]);
    assert_eq!(q.rows[1], vec![json!(3), json!("cherry")]);
    let prof = profile_session("pq-1", &reopened).expect("profile parquet");
    assert_eq!(prof.column_count, 6);
    {
      let guard = reopened.connection.lock().expect("lock");
      assert!(guard.execute_batch("SELECT * FROM read_text('/etc/hosts')").is_err());
      assert!(guard.execute_batch("SET enable_external_access = true").is_err());
    }

    // 3) 打开 CSV 并验证行数上限截断（max_rows = 2 时 3 行文件被截断并标记 truncated = true）
    let csv_path = dir.join("sample.csv");
    std::fs::write(&csv_path, "k,v\na,1\nb,2\nc,3\n").expect("write csv");
    let csv_session = open_local_file_session(&csv_path, 2).expect("open csv");
    assert_eq!(csv_session.file_kind, Some(LocalFileKind::Csv));
    assert_eq!(csv_session.row_count, 2);
    assert!(csv_session.truncated);

    // 4) 打开 NDJSON
    let ndjson_path = dir.join("events.ndjson");
    std::fs::write(&ndjson_path, "{\"id\":1,\"ok\":true}\n{\"id\":2,\"ok\":false}\n")
      .expect("write ndjson");
    let ndjson_session = open_local_file_session(&ndjson_path, 100).expect("open ndjson");
    assert_eq!(ndjson_session.file_kind, Some(LocalFileKind::Ndjson));
    assert_eq!(ndjson_session.row_count, 2);

    // 5) 不支持的扩展名被拒
    let bad_path = dir.join("notes.txt");
    std::fs::write(&bad_path, "hello").expect("write txt");
    let err = open_local_file_session(&bad_path, 100).err().expect("refuse .txt");
    assert_eq!(err.code.as_deref(), Some(ANALYTICS_UNSUPPORTED_FILE_CODE));

    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn non_ascii_timestamp_values_fall_back_to_text_without_panicking() {
    assert_eq!(normalize_timestamp("2026年11月20日 12:00:00"), None);
    assert_eq!(normalize_timestamp("2026-11-20T12:34:56Z").as_deref(), Some("2026-11-20 12:34:56"));

    let columns = vec![AnalyticsColumnInput {
      name: "created_at".to_string(),
      logical_type: Some("datetime".to_string()),
      database_type: Some("DATETIME".to_string()),
    }];
    let rows = vec![vec![JsonValue::String("2026年11月20日 12:00:00".to_string())]];
    let session = create_result_session(&columns, &rows, MAX_ANALYTICS_ROWS).expect("load session");
    let profile = profile_session("utf8-ts", &session).expect("profile");
    assert_eq!(profile.row_count, 1);
    assert_eq!(profile.columns[0].duckdb_type, "VARCHAR");
  }

  #[test]
  fn temporal_columns_emit_time_bins_for_multi_point_spans() {
    let columns = vec![
      AnalyticsColumnInput {
        name: "created_at".to_string(),
        logical_type: Some("datetime".to_string()),
        database_type: Some("TIMESTAMP".to_string()),
      },
      AnalyticsColumnInput {
        name: "same_day".to_string(),
        logical_type: Some("date".to_string()),
        database_type: Some("DATE".to_string()),
      },
    ];
    let rows = vec![
      vec![
        json!({ "type": "datetime", "value": "2026-11-01 10:00:00" }),
        json!({ "type": "date", "value": "2026-11-01" }),
      ],
      vec![
        json!({ "type": "datetime", "value": "2026-11-05 12:00:00" }),
        json!({ "type": "date", "value": "2026-11-01" }),
      ],
      vec![
        json!({ "type": "datetime", "value": "2026-11-10 15:00:00" }),
        json!({ "type": "date", "value": "2026-11-01" }),
      ],
      vec![
        json!({ "type": "datetime", "value": "2026-11-10 18:00:00" }),
        json!({ "type": "date", "value": "2026-11-01" }),
      ],
    ];
    let session = create_result_session(&columns, &rows, MAX_ANALYTICS_ROWS).expect("load session");
    let profile = profile_session("time-bins", &session).expect("profile");

    let created = &profile.columns[0];
    assert_eq!(created.time_bins.len(), 12);
    let total_binned: u64 = created.time_bins.iter().map(|b| b.count).sum();
    assert_eq!(total_binned, 4);
    assert!(created.time_bins[0].label.starts_with("2026-11-01"));

    // Constant date column must not emit redundant time bins
    let same_day = &profile.columns[1];
    assert!(same_day.time_bins.is_empty());
  }

  #[test]
  fn attach_result_and_local_file_support_reconciliation_while_keeping_sandbox_locked() {
    let dir = std::env::temp_dir().join(format!("dataomni-attach-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let csv_path = dir.join("ops_list.csv");
    std::fs::write(&csv_path, "order_id,flag\n2,vip\n3,refund\n4,missing_in_db\n")
      .expect("write csv");

    let primary_cols = vec![
      AnalyticsColumnInput {
        name: "order_id".to_string(),
        logical_type: Some("integer".to_string()),
        database_type: Some("INT".to_string()),
      },
      AnalyticsColumnInput {
        name: "amount".to_string(),
        logical_type: Some("numeric".to_string()),
        database_type: Some("DOUBLE".to_string()),
      },
    ];
    let primary_rows =
      vec![vec![json!(1), json!(10.0)], vec![json!(2), json!(20.0)], vec![json!(3), json!(30.0)]];
    let session =
      create_result_session(&primary_cols, &primary_rows, MAX_ANALYTICS_ROWS).expect("primary");

    // 1) Attach a second in-memory result set
    let ref1 = attach_result_to_session(
      &session,
      "ref_1",
      &[AnalyticsColumnInput {
        name: "order_id".to_string(),
        logical_type: Some("integer".to_string()),
        database_type: Some("INT".to_string()),
      }],
      &[vec![json!(1)], vec![json!(2)]],
      MAX_ANALYTICS_ROWS,
      None,
    )
    .expect("attach ref_1");
    assert_eq!(ref1.table_name, "ref_1");
    assert_eq!(ref1.row_count, 2);

    // 2) Attach a local CSV via dual-connection isolation
    let ref2 = attach_file_to_session(&session, "ref_2", &csv_path, MAX_ANALYTICS_ROWS)
      .expect("attach ref_2");
    assert_eq!(ref2.table_name, "ref_2");
    assert_eq!(ref2.file_kind, Some(LocalFileKind::Csv));
    assert_eq!(ref2.row_count, 3);

    // 3) Cross-table LEFT JOIN and EXCEPT work cleanly
    let joined = query_session(
      &session,
      "SELECT a.order_id, b.flag FROM result_set a LEFT JOIN ref_2 b ON a.order_id = b.order_id ORDER BY a.order_id",
      100,
      DEFAULT_QUERY_BYTE_LIMIT,
    )
    .expect("left join");
    assert_eq!(joined.row_count, 3);
    assert_eq!(joined.rows[0], vec![json!(1), json!(null)]);
    assert_eq!(joined.rows[1], vec![json!(2), json!("vip")]);
    assert_eq!(joined.rows[2], vec![json!(3), json!("refund")]);

    let diff = query_session(
      &session,
      "SELECT order_id FROM result_set EXCEPT SELECT order_id FROM ref_1 ORDER BY order_id",
      100,
      DEFAULT_QUERY_BYTE_LIMIT,
    )
    .expect("except");
    assert_eq!(diff.rows, vec![vec![json!(3)]]);

    // 4) Invalid names, collisions, and max limit are rejected
    let dup = attach_result_to_session(&session, "ref_1", &primary_cols, &[], 10, None)
      .expect_err("reject duplicate");
    assert_eq!(dup.code.as_deref(), Some(ANALYTICS_INVALID_TABLE_NAME_CODE));
    let reserved = attach_result_to_session(&session, "result_set", &primary_cols, &[], 10, None)
      .expect_err("reject result_set");
    assert_eq!(reserved.code.as_deref(), Some(ANALYTICS_INVALID_TABLE_NAME_CODE));
    let bad_ident =
      attach_result_to_session(&session, "bad-name;drop", &primary_cols, &[], 10, None)
        .expect_err("reject bad ident");
    assert_eq!(bad_ident.code.as_deref(), Some(ANALYTICS_INVALID_TABLE_NAME_CODE));

    attach_result_to_session(&session, "ref_3", &primary_cols, &[], 10, None).expect("ref_3");
    attach_result_to_session(&session, "ref_4", &primary_cols, &[], 10, None).expect("ref_4");
    let over_limit = attach_result_to_session(&session, "ref_5", &primary_cols, &[], 10, None)
      .expect_err("reject 5th attach");
    assert_eq!(over_limit.code.as_deref(), Some(ANALYTICS_ATTACH_LIMIT_CODE));

    // 5) Forward + reverse sandbox security gate after attach_file_to_session
    {
      let guard = session.connection.lock().expect("lock");
      for escape in [
        format!("SELECT * FROM read_text('{}')", csv_path.display()),
        format!("COPY result_set TO '{}/out.csv'", dir.display()),
        format!("ATTACH '{}/ext.db' AS ext", dir.display()),
        "SET enable_external_access = true".to_string(),
      ] {
        assert!(guard.execute_batch(&escape).is_err(), "must reject escape: {escape}");
      }
    }
    let unlocked = open_memory_connection().expect("unlocked");
    let ok: String = unlocked
      .query_row(&format!("SELECT content FROM read_text('{}')", csv_path.display()), [], |r| {
        r.get(0)
      })
      .expect("unlocked read_text succeeds");
    assert!(ok.starts_with("order_id,flag"));

    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn plain_string_timestamps_and_dates_infer_temporal_types_with_bins() {
    let cols = vec![
      AnalyticsColumnInput {
        name: "created_at".to_string(),
        logical_type: None,
        database_type: Some("TIMESTAMP".to_string()),
      },
      AnalyticsColumnInput {
        name: "order_date".to_string(),
        logical_type: None,
        database_type: Some("DATE".to_string()),
      },
      AnalyticsColumnInput {
        name: "bad_date".to_string(),
        logical_type: None,
        database_type: Some("TEXT".to_string()),
      },
    ];
    let rows = vec![
      vec![json!("2026-01-05 08:10:00"), json!("2026-01-05"), json!("2026-02-28")],
      vec![json!("2026-06-18 14:20:00"), json!("2026-06-18"), json!("2026-02-30")],
      vec![json!("2026-11-11 23:59:59"), json!("2026-11-11"), json!("2026-03-01")],
    ];

    let session =
      create_result_session(&cols, &rows, 100).expect("load sqlite-like string timestamps");
    assert_eq!(session.columns[0].inferred_type, ProfileColumnType::Datetime);
    assert_eq!(session.columns[1].inferred_type, ProfileColumnType::Date);
    assert_eq!(session.columns[2].inferred_type, ProfileColumnType::Text);

    let profile = profile_session("sqlite-ts", &session).expect("profile");
    assert!(!profile.columns[0].time_bins.is_empty(), "created_at should emit time_bins");
    assert!(!profile.columns[1].time_bins.is_empty(), "order_date should emit time_bins");
    assert!(profile.columns[2].time_bins.is_empty(), "invalid date column stays text");
  }

  #[test]
  fn text_profile_counts_empty_strings_and_padded_whitespace() {
    let cols = vec![
      AnalyticsColumnInput {
        name: "status".to_string(),
        logical_type: None,
        database_type: Some("VARCHAR".to_string()),
      },
      AnalyticsColumnInput {
        name: "amount".to_string(),
        logical_type: None,
        database_type: Some("DOUBLE".to_string()),
      },
    ];
    let rows = vec![
      vec![json!("paid"), json!(10.0)],
      vec![json!(""), json!(20.0)],
      vec![json!(""), json!(30.0)],
      vec![json!(" paid "), json!(40.0)],
      vec![json!("   "), json!(50.0)],
      vec![json!(null), json!(60.0)],
    ];

    let session = create_result_session(&cols, &rows, 100).expect("load text whitespace rows");
    let profile = profile_session("ws-check", &session).expect("profile");
    let status = &profile.columns[0];
    assert_eq!(status.null_count, 1);
    assert_eq!(status.empty_string_count, Some(2));
    assert_eq!(status.padded_whitespace_count, Some(2));

    let amount = &profile.columns[1];
    assert_eq!(amount.empty_string_count, None);
    assert_eq!(amount.padded_whitespace_count, None);
  }
}
