//! 内存分析沙盒的 Parquet 原子导出（H17 / P2-2）。

use super::*;

/// 将内存表或指定只读 SQL 的结果导出为 Parquet 文件（用户 SQL 仅在沙盒内跑，经独立瞬态连接原子落盘）
pub fn export_parquet_from_session(
  session: &AnalyticsSession,
  sql: Option<&str>,
  output_path: &Path,
) -> Result<ParquetExportSummary, QueryError> {
  let query_sql = match sql.map(str::trim) {
    Some(s) if !s.is_empty() => s,
    _ => "SELECT * FROM result_set",
  };
  if let Err(refusal) = read_only_gate::check(query_sql) {
    return Err(QueryError::with_code(
      ANALYTICS_READ_ONLY_REFUSED_CODE,
      format!("{ANALYTICS_READ_ONLY_REFUSED_CODE}: {refusal}"),
    ));
  }

  let parent = output_path.parent().ok_or_else(|| {
    QueryError::message(format!("Output path has no parent directory: {}", output_path.display()))
  })?;
  if !parent.as_os_str().is_empty() && !parent.is_dir() {
    return Err(QueryError::message(format!(
      "Output directory does not exist: {}",
      parent.display()
    )));
  }

  session.cancelled.store(false, Ordering::SeqCst);
  session.busy.store(true, Ordering::Release);
  let _busy_guard = ResetBusy(&session.busy);

  // 1) 在沙盒连接内安全执行用户只读 SQL，取出列类型与值
  let (col_defs, exported_rows): (Vec<(String, String)>, Vec<Vec<Value>>) = {
    let conn = session.connection.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
    let mut prepared = conn
      .prepare(query_sql)
      .map_err(|err| map_interrupt_error(duck_error(err), &session.cancelled))?;
    drop(
      prepared
        .stream_arrow([])
        .map_err(|err| map_interrupt_error(duck_error(err), &session.cancelled))?,
    );
    let (_metadata, names) = duckdb_column_header(&prepared);
    let mut defs = Vec::with_capacity(names.len());
    let mut type_ids = Vec::with_capacity(names.len());
    for (idx, name) in names.iter().enumerate() {
      let logical = prepared.column_logical_type(idx);
      let id = logical.id();
      type_ids.push(id);
      defs.push((name.clone(), parquet_export_sql_type(&logical)));
    }

    let mut rows_iter = prepared.raw_query();
    let mut collected = Vec::new();
    while let Some(row) =
      rows_iter.next().map_err(|err| map_interrupt_error(duck_error(err), &session.cancelled))?
    {
      if collected.len() >= MAX_ANALYTICS_ROWS {
        break;
      }
      let mut values = Vec::with_capacity(defs.len());
      for (idx, type_id) in type_ids.iter().enumerate() {
        let raw: Value = if *type_id == LogicalTypeId::SqlNull {
          Value::Null
        } else {
          row.get(idx).map_err(duck_error)?
        };
        values.push(normalize_export_value(raw, *type_id));
      }
      collected.push(values);
    }
    (defs, collected)
  };

  // 2) 在独立瞬态内存连接中写入临时 Parquet 并原子替换目标文件（不执行任何用户 SQL）
  let row_count = exported_rows.len() as u64;
  let export_conn = open_memory_connection()?;
  if col_defs.is_empty() {
    export_conn.execute_batch("CREATE TABLE __export (__empty VARCHAR)").map_err(duck_error)?;
  } else {
    let ddl_cols: Vec<String> =
      col_defs.iter().map(|(name, sql_ty)| format!("{} {sql_ty}", quote_ident(name))).collect();
    export_conn
      .execute_batch(&format!("CREATE TABLE __export ({})", ddl_cols.join(", ")))
      .map_err(duck_error)?;
    if !exported_rows.is_empty() {
      let mut appender = export_conn.appender("__export").map_err(duck_error)?;
      for row in &exported_rows {
        appender.append_row(appender_params_from_iter(row.iter())).map_err(duck_error)?;
      }
      appender.flush().map_err(duck_error)?;
    }
  }

  let file_name = output_path.file_name().and_then(|n| n.to_str()).unwrap_or("export.parquet");
  let tmp_path = parent.join(format!(".{file_name}.tmp-{}", Uuid::new_v4()));
  let tmp_str = tmp_path
    .to_str()
    .ok_or_else(|| QueryError::message("Temporary export path is not valid UTF-8"))?;
  let escaped_tmp = tmp_str.replace('\'', "''");

  let copy_res = export_conn
    .execute_batch(&format!("COPY __export TO '{escaped_tmp}' (FORMAT PARQUET)"))
    .map_err(duck_error);
  if let Err(err) = copy_res {
    let _ = std::fs::remove_file(&tmp_path);
    return Err(err);
  }

  if let Err(err) = std::fs::rename(&tmp_path, output_path) {
    let _ = std::fs::remove_file(&tmp_path);
    return Err(QueryError::message(format!(
      "Failed to finalize Parquet export {}: {err}",
      output_path.display()
    )));
  }

  let bytes = std::fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);
  Ok(ParquetExportSummary { row_count, bytes })
}

fn parquet_export_sql_type(logical: &duckdb::core::LogicalTypeHandle) -> String {
  match logical.id() {
    LogicalTypeId::Boolean => "BOOLEAN".to_string(),
    LogicalTypeId::Tinyint => "TINYINT".to_string(),
    LogicalTypeId::Smallint => "SMALLINT".to_string(),
    LogicalTypeId::Integer => "INTEGER".to_string(),
    LogicalTypeId::Bigint => "BIGINT".to_string(),
    LogicalTypeId::UTinyint => "UTINYINT".to_string(),
    LogicalTypeId::USmallint => "USMALLINT".to_string(),
    LogicalTypeId::UInteger => "UINTEGER".to_string(),
    LogicalTypeId::UBigint => "UBIGINT".to_string(),
    LogicalTypeId::Hugeint => "HUGEINT".to_string(),
    LogicalTypeId::UHugeint => "UHUGEINT".to_string(),
    LogicalTypeId::Float => "FLOAT".to_string(),
    LogicalTypeId::Double => "DOUBLE".to_string(),
    LogicalTypeId::Decimal => {
      format!("DECIMAL({},{})", logical.decimal_width(), logical.decimal_scale())
    }
    LogicalTypeId::Date => "DATE".to_string(),
    LogicalTypeId::Time => "TIME".to_string(),
    LogicalTypeId::Timestamp
    | LogicalTypeId::TimestampS
    | LogicalTypeId::TimestampMs
    | LogicalTypeId::TimestampNs => "TIMESTAMP".to_string(),
    LogicalTypeId::Blob | LogicalTypeId::Geometry => "BLOB".to_string(),
    _ => "VARCHAR".to_string(),
  }
}

fn normalize_export_value(value: Value, type_id: LogicalTypeId) -> Value {
  match value {
    Value::Null
    | Value::Boolean(_)
    | Value::TinyInt(_)
    | Value::SmallInt(_)
    | Value::Int(_)
    | Value::BigInt(_)
    | Value::UTinyInt(_)
    | Value::USmallInt(_)
    | Value::UInt(_)
    | Value::UBigInt(_)
    | Value::HugeInt(_)
    | Value::Float(_)
    | Value::Double(_)
    | Value::Text(_)
    | Value::Blob(_) => value,
    Value::Decimal(dec) => Value::Text(dec.to_string()),
    other => match duckdb_decode(&other, type_id) {
      JsonValue::Null => Value::Null,
      JsonValue::String(text) => Value::Text(text),
      JsonValue::Object(map) => {
        if let Some(val) = map.get("value").and_then(JsonValue::as_str) {
          Value::Text(val.to_string())
        } else {
          Value::Text(JsonValue::Object(map).to_string())
        }
      }
      json => Value::Text(json.to_string()),
    },
  }
}
