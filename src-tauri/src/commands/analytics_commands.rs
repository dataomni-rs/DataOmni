use crate::services::local_analytics::{
  AnalyticsColumnInput, AnalyticsQueryResult, AnalyticsSessionSummary, AttachedTableSummary,
  LocalAnalyticsRegistry, ParquetExportSummary, ProfileResult,
};
use crate::services::QueryError;
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use tauri::State;

/// 装载表格/查询结果集进内存 DuckDB 分析沙盒
#[tauri::command]
pub async fn analytics_load_result(
  columns: Vec<AnalyticsColumnInput>,
  rows: Vec<Vec<JsonValue>>,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<AnalyticsSessionSummary, QueryError> {
  registry.load_result(columns, rows).await
}

/// 装载 MongoDB / Elasticsearch 异构 JSON 文档进内存 DuckDB 分析沙盒
#[tauri::command]
pub async fn analytics_load_json_docs(
  documents: Vec<JsonValue>,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<AnalyticsSessionSummary, QueryError> {
  registry.load_json_docs(documents).await
}

/// 打开本地 Parquet / CSV / TSV / JSON / NDJSON 文件进只读内存分析沙盒
#[tauri::command]
pub async fn analytics_open_file(
  path: String,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<AnalyticsSessionSummary, QueryError> {
  let target = PathBuf::from(path);
  crate::services::path_guard::refuse_reserved_path(&target).map_err(QueryError::message)?;
  registry.open_file(target).await
}

/// 向既有分析沙盒会话附加一张内存结果表（用于跨查询 LEFT JOIN / EXCEPT 对账）
#[tauri::command]
pub async fn analytics_attach_result(
  analytics_id: String,
  table_name: String,
  columns: Vec<AnalyticsColumnInput>,
  rows: Vec<Vec<JsonValue>>,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<AttachedTableSummary, QueryError> {
  registry.attach_result(&analytics_id, table_name, columns, rows).await
}

/// 通过瞬态只读连接物化本地数据文件并附加到既有沙盒会话（用户 SQL 会话全程不解锁外部访问）
#[tauri::command]
pub async fn analytics_attach_file(
  analytics_id: String,
  table_name: String,
  path: String,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<AttachedTableSummary, QueryError> {
  let target = PathBuf::from(path);
  crate::services::path_guard::refuse_reserved_path(&target).map_err(QueryError::message)?;
  registry.attach_file(&analytics_id, table_name, target).await
}

/// 计算指定分析会话的全列数据画像
#[tauri::command]
pub async fn analytics_profile(
  analytics_id: String,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<ProfileResult, QueryError> {
  registry.profile(&analytics_id).await
}

/// 在沙盒内对内存表 `result_set` 执行单条只读 SQL（含 GROUP BY / PIVOT）
#[tauri::command]
pub async fn analytics_query(
  analytics_id: String,
  sql: String,
  limit: Option<usize>,
  timeout_seconds: Option<u64>,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<AnalyticsQueryResult, QueryError> {
  registry.query(&analytics_id, sql, limit, timeout_seconds).await
}

/// 将内存表或只读 SQL 查询结果导出为 Parquet 文件
#[tauri::command]
pub async fn analytics_export_parquet(
  analytics_id: String,
  sql: Option<String>,
  output_path: String,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<ParquetExportSummary, QueryError> {
  let target = PathBuf::from(output_path);
  crate::services::path_guard::refuse_reserved_path(&target).map_err(QueryError::message)?;
  registry.export_parquet(&analytics_id, sql, target).await
}

/// 中止指定分析会话上正在进行的画像或二次查询
#[tauri::command]
pub fn analytics_cancel(
  analytics_id: String,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<bool, QueryError> {
  registry.cancel(&analytics_id)
}

/// 主动销毁指定分析会话，释放内存
#[tauri::command]
pub fn analytics_close(
  analytics_id: String,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<bool, QueryError> {
  registry.close(&analytics_id)
}
