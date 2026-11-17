//! 结构比对的往返验收（`scripts/schema-diff-verify.sh`）用的是应用里**同一份**目录查询：
//! 设了 `DATAOMNI_DUMP_CATALOG_QUERIES=<文件>` 时把各方言的 ER 图查询与单表结构查询写成 JSON，
//! 没设时什么也不做。查询只住在 Rust 这边（见 `catalogQueries.ts` 开头），脚本另抄一份就验不到真东西了

use dataomni_lib::models::DatabaseType;
use dataomni_lib::services::duckdb::{self, DuckDbPool};
use dataomni_lib::services::{
  er_diagram_queries, schema_metadata_queries, PoolRef, SessionConnection,
};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::Arc;

#[test]
fn dump_catalog_queries() {
  let Ok(path) = std::env::var("DATAOMNI_DUMP_CATALOG_QUERIES") else {
    return;
  };
  let mut dialects = serde_json::Map::new();
  for (name, db_type) in [
    ("postgresql", DatabaseType::PostgreSQL),
    ("mysql", DatabaseType::MySQL),
    ("sqlite", DatabaseType::SQLite),
    ("sqlserver", DatabaseType::SqlServer),
    ("duckdb", DatabaseType::DuckDB),
    ("oracle", DatabaseType::Oracle),
  ] {
    dialects.insert(
      name.to_string(),
      serde_json::json!({
        "er": er_diagram_queries(&db_type),
        "metadata": schema_metadata_queries(&db_type),
      }),
    );
  }
  let text = serde_json::to_string_pretty(&dialects).expect("queries serialize");
  std::fs::write(&path, text).expect("write the dumped queries");
}

#[derive(Deserialize)]
struct WorkerRequest {
  id: u64,
  path: String,
  op: String,
  sql: String,
  #[serde(default)]
  params: Vec<JsonValue>,
}

/// 供 `scripts/schema-diff-verify.ts` 调应用自己的 DuckDB 连接池与解码器做真库往返验证；
/// 未设 `DATAOMNI_DUCKDB_WORKER=1` 时立即返回，日常 `cargo test` 零开销。
#[tokio::test]
async fn duckdb_worker() {
  if std::env::var("DATAOMNI_DUCKDB_WORKER").as_deref() != Ok("1") {
    return;
  }
  let stdin = std::io::stdin();
  let mut stdout = std::io::stdout();
  let mut pools: HashMap<String, Arc<DuckDbPool>> = HashMap::new();

  for line in stdin.lock().lines() {
    let Ok(line) = line else { break };
    if line.trim().is_empty() {
      continue;
    }
    let req: WorkerRequest = serde_json::from_str(&line).expect("valid worker JSON");
    let pool = match pools.get(&req.path) {
      Some(existing) => existing.clone(),
      None => match duckdb::open(&req.path).await {
        Ok(opened) => {
          pools.insert(req.path.clone(), opened.clone());
          opened
        }
        Err(err) => {
          let reply = serde_json::json!({ "id": req.id, "error": err.message });
          writeln!(stdout, "DATAOMNI_RPC:{reply}").expect("write error");
          stdout.flush().expect("flush");
          continue;
        }
      },
    };

    let response = match req.op.as_str() {
      "select" => match pool.select(&req.sql, &req.params).await {
        Ok(rows) => serde_json::json!({ "id": req.id, "rows": rows }),
        Err(err) => serde_json::json!({ "id": req.id, "error": err.message }),
      },
      "exec" => match SessionConnection::acquire(PoolRef::DuckDb(&pool)).await {
        Ok(mut conn) => match conn.execute(&req.sql, 30).await {
          Ok(_) => serde_json::json!({ "id": req.id, "rows": [] }),
          Err(err) => serde_json::json!({ "id": req.id, "error": err.message }),
        },
        Err(err) => serde_json::json!({ "id": req.id, "error": err.message }),
      },
      other => serde_json::json!({ "id": req.id, "error": format!("unknown op: {other}") }),
    };

    writeln!(stdout, "DATAOMNI_RPC:{response}").expect("write reply");
    stdout.flush().expect("flush");
  }
}
