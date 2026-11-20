use crate::services::local_analytics::{
  AnalyticsColumnInput, AnalyticsSessionSummary, LocalAnalyticsRegistry, ProfileResult,
};
use crate::services::QueryError;
use serde_json::Value as JsonValue;
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

/// 计算指定分析会话的全列数据画像
#[tauri::command]
pub async fn analytics_profile(
  analytics_id: String,
  registry: State<'_, LocalAnalyticsRegistry>,
) -> Result<ProfileResult, QueryError> {
  registry.profile(&analytics_id).await
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
