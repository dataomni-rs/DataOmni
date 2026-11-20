use crate::services::local_analytics::{
  AnalyticsColumnInput, AnalyticsQueryResult, AnalyticsSessionSummary, LocalAnalyticsRegistry,
  ParquetExportSummary, ProfileResult,
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
  registry.open_file(PathBuf::from(path)).await
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
  registry.export_parquet(&analytics_id, sql, PathBuf::from(output_path)).await
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
