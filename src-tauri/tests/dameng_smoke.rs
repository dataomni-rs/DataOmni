//! 达梦 DM8（ODBC 路线）真库冒烟测试（`--features odbc`）。
//!
//! 默认静默跳过；设置以下环境变量后在真实 DM8 实例上运行：
//! - `DATAOMNI_REQUIRE_NETWORK_DATABASE_TESTS=1`
//! - `DATAOMNI_DAMENG_TEST_URL=dameng://SYSDBA:<password>@127.0.0.1:5236/SYSDBA`
//! - `DATAOMNI_DM8_ODBC_DRIVER=/path/to/libdodbc.so`（或 Windows 驱动名 `DM8 ODBC DRIVER`）

use dataomni_lib::models::{ConnectionProfile, DatabaseType, TlsMode};
use dataomni_lib::services::dameng::{connect, DamengPool, DamengTarget};
use dataomni_lib::services::{
  active_sessions_query, completion_catalog_query, er_diagram_queries, execute_write_batch,
  export_query, object_catalog_queries, schema_metadata_queries, session_target_query,
  ExportFormat, ExportOptions, PoolRef, SessionConnection, StreamOptions, WriteStatement,
};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use url::Url;

const DAMENG_URL_ENV: &str = "DATAOMNI_DAMENG_TEST_URL";
const REQUIRE_ENV: &str = "DATAOMNI_REQUIRE_NETWORK_DATABASE_TESTS";

fn dameng_target() -> Option<DamengTarget> {
  let raw = match std::env::var(DAMENG_URL_ENV) {
    Ok(value) if !value.trim().is_empty() => value,
    _ => {
      if std::env::var(REQUIRE_ENV).ok().as_deref() == Some("1") {
        panic!("设置了 {REQUIRE_ENV}=1 但未设置 {DAMENG_URL_ENV}");
      }
      return None;
    }
  };
  let parsed = Url::parse(&raw).expect("valid dameng:// test URL");
  let host = parsed.host_str().expect("host").to_string();
  let port = parsed.port().unwrap_or(5236);
  let username = urlencoding::decode(parsed.username()).expect("username").into_owned();
  let password =
    urlencoding::decode(parsed.password().unwrap_or("")).expect("password").into_owned();
  let schema = parsed.path().trim_start_matches('/').to_string();
  let mut options = HashMap::new();
  if let Ok(driver) = std::env::var("DATAOMNI_DM8_ODBC_DRIVER") {
    if !driver.trim().is_empty() {
      options.insert("odbcDriver".to_string(), driver);
    }
  }
  let profile = ConnectionProfile {
    id: "dameng-smoke".to_string(),
    name: "DM8 Smoke".to_string(),
    db_type: DatabaseType::Dameng,
    host,
    port,
    database: (!schema.is_empty()).then_some(schema),
    username,
    password,
    ssl: false,
    tls_mode: Some(TlsMode::Disabled),
    ca_certificate_path: None,
    client_certificate_path: None,
    client_key_path: None,
    save_password: false,
    credential_ref: None,
    ssh_tunnel: None,
    options,
    tags: vec![],
    environment: Default::default(),
    agent_access: Default::default(),
    created_at: String::new(),
    updated_at: String::new(),
  };
  Some(DamengTarget::from_profile(&profile).expect("build DamengTarget"))
}

#[tokio::test]
async fn dameng_catalog_metadata_lob_params_and_write_batch() {
  let Some(target) = dameng_target() else {
    return;
  };
  let first = connect(&target).await.expect("connect to Dameng DM8");
  let pool = DamengPool::new(target, first);
  let mut session =
    SessionConnection::acquire(PoolRef::Dameng(&pool)).await.expect("acquire Dameng session");

  // 1. 清理残留并建主从两张测试表（含主键、唯一索引、外键、检查约束、列注释）
  for sql in ["DROP TABLE DATAOMNI_DM_CHILD", "DROP TABLE DATAOMNI_DM_PARENT"] {
    let _ = session.execute(sql, 10).await;
  }
  session
    .execute(
      "CREATE TABLE DATAOMNI_DM_PARENT (
         ID INT PRIMARY KEY,
         CODE VARCHAR(40) NOT NULL UNIQUE
       )",
      10,
    )
    .await
    .expect("create parent");
  session
    .execute(
      "CREATE TABLE DATAOMNI_DM_CHILD (
         ID INT PRIMARY KEY,
         PARENT_ID INT NOT NULL REFERENCES DATAOMNI_DM_PARENT(ID),
         AMT DECIMAL(18, 4),
         TXT VARCHAR(200),
         C CLOB,
         B BLOB,
         DT DATE,
         TS TIMESTAMP(6),
         TAIL VARCHAR(50),
         CONSTRAINT CK_DM_CHILD_AMT CHECK (AMT >= 0)
       )",
      10,
    )
    .await
    .expect("create child");
  session
    .execute("COMMENT ON COLUMN DATAOMNI_DM_CHILD.TXT IS '中文备注'", 10)
    .await
    .expect("comment");

  // 2. 插入 12,000 字符 off-row CLOB + BLOB + UTF-8 中文 + DECIMAL
  session
    .execute("INSERT INTO DATAOMNI_DM_PARENT (ID, CODE) VALUES (1, 'P-01')", 10)
    .await
    .expect("insert parent");
  session
    .execute(
      "INSERT INTO DATAOMNI_DM_CHILD VALUES (
         1,
         1,
         12345678901234.5678,
         '达梦中文测试',
         TO_CLOB(REPEAT('达梦', 2000)) || TO_CLOB(REPEAT('达梦', 2000)) || TO_CLOB(REPEAT('达梦', 2000)),
         0xDEADBEEF01020304,
         DATE '2026-11-22',
         TIMESTAMP '2026-11-22 14:30:45.123456',
         'tail-ok'
       )",
      10,
    )
    .await
    .expect("insert child");

  // 3. 含 LOB 列的参数化查询：必须返回全部 4 列，且 CLOB 完整 12000 字
  let mut rows = Vec::new();
  let summary = session
    .execute_streaming_with_params(
      "SELECT ID, C, B, TAIL FROM DATAOMNI_DM_CHILD WHERE ID = ?",
      &[json!(1)],
      StreamOptions::limited(100, 16 * 1024 * 1024, 100),
      &mut |batch| {
        rows.extend(batch.rows);
        Ok(())
      },
    )
    .await
    .expect("parameterized LOB select");
  assert_eq!(rows.len(), 1, "summary={summary:?}");
  let row = &rows[0];
  let clob = row.get("C").and_then(JsonValue::as_str).expect("CLOB string");
  assert_eq!(clob.chars().count(), 12_000, "off-row CLOB 必须完整 12000 字符");
  assert_eq!(row.get("TAIL"), Some(&json!("tail-ok")), "含 LOB 参数查询不可丢失尾列");
  assert!(row.get("B").is_some_and(|val| !val.is_null()), "BLOB 列不可为空");

  // 4. 验证全部 6 组只读目录与诊断查询
  let target_q = session_target_query(&DatabaseType::Dameng).expect("session target query");
  let target_rows = pool.select(target_q.sql, &[]).await.expect("session target");
  assert_eq!(target_rows.len(), 1);
  let schema =
    target_rows[0].get("schema_name").and_then(JsonValue::as_str).expect("schema_name").to_string();

  let obj_q = object_catalog_queries(&DatabaseType::Dameng).expect("object catalog");
  let obj_rows = pool.select(obj_q.objects, &[]).await.expect("object catalog select");
  assert!(
    obj_rows.iter().any(|r| r.get("object_name") == Some(&json!("DATAOMNI_DM_CHILD"))),
    "对象目录应包含新建表"
  );
  assert!(
    !obj_rows.iter().any(|r| r.get("object_schema") == Some(&json!("SYS"))),
    "对象目录应过滤内置系统模式 SYS"
  );

  let meta_q = schema_metadata_queries(&DatabaseType::Dameng).expect("schema metadata");
  let col_rows = pool
    .select(meta_q.columns, &[json!("DATAOMNI_DM_CHILD"), json!(schema.clone())])
    .await
    .expect("columns metadata");
  assert_eq!(col_rows.len(), 9);
  let fk_rows = pool
    .select(meta_q.foreign_keys, &[json!("DATAOMNI_DM_CHILD"), json!(schema.clone())])
    .await
    .expect("foreign keys");
  assert_eq!(fk_rows.len(), 1);
  let idx_rows = pool
    .select(meta_q.indexes, &[json!("DATAOMNI_DM_CHILD"), json!(schema.clone())])
    .await
    .expect("indexes");
  assert!(!idx_rows.is_empty());
  if let Some(dataomni_lib::services::DdlQuery::Bound { sql: ddl_sql }) = meta_q.ddl {
    let ddl_rows = pool
      .select(ddl_sql, &[json!("DATAOMNI_DM_CHILD"), json!(schema.clone())])
      .await
      .expect("table ddl");
    let ddl = ddl_rows[0].get("sql").and_then(JsonValue::as_str).unwrap_or("");
    assert!(ddl.contains("DATAOMNI_DM_CHILD"), "GET_DDL 应返回完整建表 DDL: {ddl}");
  }

  let comp_q = completion_catalog_query(&DatabaseType::Dameng).expect("completion catalog");
  let comp_rows = pool.select(comp_q.relations, &[]).await.expect("completion select");
  assert!(comp_rows.iter().any(|r| r.get("relation_name") == Some(&json!("DATAOMNI_DM_CHILD"))));

  let er_q = er_diagram_queries(&DatabaseType::Dameng).expect("er diagram");
  let er_fks = pool.select(er_q.foreign_keys, &[]).await.expect("er fks");
  assert!(er_fks.iter().any(|r| r.get("table_name") == Some(&json!("DATAOMNI_DM_CHILD"))));

  let sess_q = active_sessions_query(&DatabaseType::Dameng).expect("active sessions");
  let sess_rows = pool.select(sess_q.sql, &[]).await.expect("active sessions select");
  assert!(!sess_rows.is_empty(), "V$SESSIONS 应至少返回当前会话");

  // 5. write_batch 原子事务：expect_rows 不匹配必须整批回滚
  let rollback_err = execute_write_batch(
    PoolRef::Dameng(&pool),
    &[
      WriteStatement {
        sql: "INSERT INTO DATAOMNI_DM_PARENT (ID, CODE) VALUES (?, ?)".to_string(),
        params: vec![json!(2), json!("P-02")],
        expect_rows: Some(1),
      },
      WriteStatement {
        sql: "UPDATE DATAOMNI_DM_PARENT SET CODE = ? WHERE ID = ?".to_string(),
        params: vec![json!("NOPE"), json!(999)],
        expect_rows: Some(1),
      },
    ],
  )
  .await
  .expect_err("row count mismatch must roll back");
  assert_eq!(rollback_err.statement_index, 1);
  let check_rows = pool
    .select("SELECT COUNT(*) AS N FROM DATAOMNI_DM_PARENT WHERE ID = 2", &[])
    .await
    .expect("count after rollback");
  assert_eq!(check_rows[0].get("N"), Some(&json!(0)), "回滚后 ID=2 不应存在");

  // 6. 流式导出 CSV
  let export_path = std::env::temp_dir().join("dataomni-dm8-smoke-export.csv");
  let export_summary = export_query(
    PoolRef::Dameng(&pool),
    "SELECT ID, TXT, AMT, TAIL FROM DATAOMNI_DM_CHILD ORDER BY ID",
    &export_path,
    ExportOptions {
      format: ExportFormat::Csv,
      delimiter: ",".to_string(),
      include_header: true,
      null_text: String::new(),
      byte_order_mark: false,
      sql_table: String::new(),
      sql_dialect: None,
      sql_computed_columns: vec![],
      sql_identity_columns: vec![],
      sql_sequence_columns: vec![],
    },
    &mut |_| {},
    &mut || false,
  )
  .await
  .expect("export CSV");
  assert_eq!(export_summary.rows_written, 1);
  let csv_text = std::fs::read_to_string(&export_path).expect("read exported csv");
  assert!(csv_text.contains("达梦中文测试"));
  assert!(csv_text.contains("12345678901234.5678"));
  let _ = std::fs::remove_file(&export_path);

  // 清理测试表
  let _ = session.execute("DROP TABLE DATAOMNI_DM_CHILD", 10).await;
  let _ = session.execute("DROP TABLE DATAOMNI_DM_PARENT", 10).await;
}
