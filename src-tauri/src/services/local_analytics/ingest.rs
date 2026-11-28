//! 结果集、异构 JSON 与本地文件的内存 DuckDB 装载与类型推断（H17 / P2-2）。

use super::*;

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
    file_kind: None,
    row_count: admitted_rows.len(),
    truncated,
    columns: plans,
    attached_tables: Mutex::new(Vec::new()),
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
    file_kind: None,
    row_count: normalized_docs.len(),
    truncated,
    columns: plans,
    attached_tables: Mutex::new(Vec::new()),
  })
}

/// 识别受支持的本地只读探索数据文件扩展名
pub fn detect_local_file_kind(path: &Path) -> Option<LocalFileKind> {
  let ext = path.extension()?.to_str()?.to_ascii_lowercase();
  match ext.as_str() {
    "parquet" | "pq" => Some(LocalFileKind::Parquet),
    "csv" => Some(LocalFileKind::Csv),
    "tsv" => Some(LocalFileKind::Tsv),
    "json" => Some(LocalFileKind::Json),
    "ndjson" | "jsonl" => Some(LocalFileKind::Ndjson),
    _ => None,
  }
}

/// 打开本地 `.parquet` / `.csv` / `.tsv` / `.json` / `.ndjson` 文件物化为内存表 `result_set` 并立即锁死沙盒
pub fn open_local_file_session(
  path: &Path,
  max_rows: usize,
) -> Result<AnalyticsSession, QueryError> {
  let Some(file_kind) = detect_local_file_kind(path) else {
    return Err(QueryError::with_code(
      ANALYTICS_UNSUPPORTED_FILE_CODE,
      format!("{ANALYTICS_UNSUPPORTED_FILE_CODE}: {}", path.display()),
    ));
  };
  let metadata = std::fs::metadata(path).map_err(|error| {
    QueryError::message(format!("Failed to access local file {}: {error}", path.display()))
  })?;
  if !metadata.is_file() {
    return Err(QueryError::with_code(
      ANALYTICS_UNSUPPORTED_FILE_CODE,
      format!("{ANALYTICS_UNSUPPORTED_FILE_CODE}: {}", path.display()),
    ));
  }

  let path_str = path.to_str().ok_or_else(|| {
    QueryError::with_code(
      ANALYTICS_UNSUPPORTED_FILE_CODE,
      format!("{ANALYTICS_UNSUPPORTED_FILE_CODE}: non-UTF-8 path"),
    )
  })?;
  let escaped = path_str.replace('\'', "''");
  let reader_expr = match file_kind {
    LocalFileKind::Parquet => format!("read_parquet('{escaped}')"),
    LocalFileKind::Csv => format!("read_csv_auto('{escaped}')"),
    LocalFileKind::Tsv => format!("read_csv_auto('{escaped}', delim='\t')"),
    LocalFileKind::Json => format!("read_json_auto('{escaped}')"),
    LocalFileKind::Ndjson => format!("read_json_auto('{escaped}', format='newline_delimited')"),
  };

  let conn = open_memory_connection()?;
  let probe_limit = max_rows.saturating_add(1);
  conn
    .execute_batch(&format!(
      "CREATE TABLE __raw_import AS SELECT * FROM {reader_expr} LIMIT {probe_limit}"
    ))
    .map_err(duck_error)?;

  let raw_count: i64 =
    conn.query_row("SELECT COUNT(*) FROM __raw_import", [], |r| r.get(0)).map_err(duck_error)?;
  let raw_count_u = raw_count.max(0) as usize;
  let truncated = raw_count_u > max_rows;
  let row_count = if truncated {
    conn
      .execute_batch(&format!(
        "CREATE TABLE result_set AS SELECT * FROM __raw_import LIMIT {max_rows}; DROP TABLE __raw_import;"
      ))
      .map_err(duck_error)?;
    max_rows
  } else {
    conn.execute_batch("ALTER TABLE __raw_import RENAME TO result_set;").map_err(duck_error)?;
    raw_count_u
  };

  // 物化完成后立即锁死外部访问与配置，后续用户 SQL 与画像无法逃逸到磁盘
  lock_sandbox(&conn)?;

  let plans = introspect_result_table(&conn)?;
  let interrupt = conn.interrupt_handle();
  Ok(AnalyticsSession {
    connection: Mutex::new(conn),
    interrupt,
    cancelled: AtomicBool::new(false),
    busy: AtomicBool::new(false),
    source_kind: AnalyticsSourceKind::LocalFile,
    file_kind: Some(file_kind),
    row_count,
    truncated,
    columns: plans,
    attached_tables: Mutex::new(Vec::new()),
  })
}

pub(super) fn validate_attach_table_name(
  raw: &str,
  existing: &[String],
) -> Result<String, QueryError> {
  let trimmed = raw.trim();
  let valid = !trimmed.is_empty()
    && trimmed.len() <= 64
    && trimmed.bytes().enumerate().all(|(idx, byte)| {
      if idx == 0 {
        byte.is_ascii_alphabetic() || byte == b'_'
      } else {
        byte.is_ascii_alphanumeric() || byte == b'_'
      }
    });
  if !valid || trimmed.eq_ignore_ascii_case("result_set") {
    return Err(QueryError::with_code(
      ANALYTICS_INVALID_TABLE_NAME_CODE,
      format!("{ANALYTICS_INVALID_TABLE_NAME_CODE}: {trimmed}"),
    ));
  }
  if existing.iter().any(|name| name.eq_ignore_ascii_case(trimmed)) {
    return Err(QueryError::with_code(
      ANALYTICS_INVALID_TABLE_NAME_CODE,
      format!("{ANALYTICS_INVALID_TABLE_NAME_CODE}: duplicate table {trimmed}"),
    ));
  }
  if existing.len() >= MAX_ATTACHED_TABLES {
    return Err(QueryError::with_code(
      ANALYTICS_ATTACH_LIMIT_CODE,
      format!("{ANALYTICS_ATTACH_LIMIT_CODE}: max {MAX_ATTACHED_TABLES} attached tables"),
    ));
  }
  Ok(trimmed.to_string())
}

pub(super) fn create_named_table(
  conn: &Connection,
  table_name: &str,
  plans: &[ColumnPlan],
) -> Result<(), QueryError> {
  let table_ident = quote_ident(table_name);
  if plans.is_empty() {
    conn
      .execute_batch(&format!("CREATE TABLE {table_ident} (__empty_placeholder VARCHAR)"))
      .map_err(duck_error)?;
    return Ok(());
  }
  let cols_sql: Vec<String> = plans
    .iter()
    .map(|plan| format!("{} {}", quote_ident(&plan.name), plan.duckdb_type()))
    .collect();
  conn
    .execute_batch(&format!("CREATE TABLE {table_ident} ({})", cols_sql.join(", ")))
    .map_err(duck_error)
}

/// 将结果集附加为已锁沙盒会话内的只读对账表（不解锁外部访问）
pub fn attach_result_to_session(
  session: &AnalyticsSession,
  table_name: &str,
  columns: &[AnalyticsColumnInput],
  rows: &[Vec<JsonValue>],
  max_rows: usize,
  file_kind: Option<LocalFileKind>,
) -> Result<AttachedTableSummary, QueryError> {
  let validated_name = {
    let attached =
      session.attached_tables.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
    validate_attach_table_name(table_name, &attached)?
  };

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

  {
    let mut attached =
      session.attached_tables.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
    let _ = validate_attach_table_name(&validated_name, &attached)?;
    let conn = session.connection.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
    create_named_table(&conn, &validated_name, &plans)?;
    if !plans.is_empty() && !admitted_rows.is_empty() {
      let mut appender = conn.appender(&validated_name).map_err(duck_error)?;
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
    attached.push(validated_name.clone());
  }

  Ok(AttachedTableSummary {
    table_name: validated_name,
    file_kind,
    row_count: admitted_rows.len(),
    column_count: plans.len(),
    truncated,
    columns: plans.iter().map(ColumnPlan::meta).collect(),
  })
}

/// 通过独立瞬态读连接物化本地文件后经 `Appender` 写入已锁沙盒会话（AN0-E5 双连接隔离方案）
pub fn attach_file_to_session(
  session: &AnalyticsSession,
  table_name: &str,
  path: &Path,
  max_rows: usize,
) -> Result<AttachedTableSummary, QueryError> {
  {
    let attached =
      session.attached_tables.lock().map_err(|_| QueryError::message(LOCK_POISONED))?;
    let _ = validate_attach_table_name(table_name, &attached)?;
  }

  let transient = open_local_file_session(path, max_rows)?;
  let file_kind = transient.file_kind;
  let file_truncated = transient.truncated;
  let fetched = query_session(&transient, "SELECT * FROM result_set", max_rows, usize::MAX)?;
  drop(transient);

  let columns: Vec<AnalyticsColumnInput> = fetched
    .column_metadata
    .into_iter()
    .map(|col| AnalyticsColumnInput {
      name: col.name,
      logical_type: Some(col.logical_type),
      database_type: Some(col.database_type),
    })
    .collect();
  let mut summary =
    attach_result_to_session(session, table_name, &columns, &fetched.rows, max_rows, file_kind)?;
  summary.truncated = summary.truncated || file_truncated || fetched.truncated;
  Ok(summary)
}

pub(super) fn create_result_table(
  conn: &Connection,
  plans: &[ColumnPlan],
) -> Result<(), QueryError> {
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
    JsonValue::String(text) => {
      if normalize_timestamp(text).is_some() {
        Some(CellKind::Datetime)
      } else if is_plain_date(text) {
        Some(CellKind::Date)
      } else {
        Some(CellKind::Text)
      }
    }
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

pub(super) fn infer_result_column_plan(
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

pub(super) fn encode_result_cell(cell: &JsonValue, storage: ColumnStorage) -> Value {
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

pub(super) fn extract_scalar_text(cell: &JsonValue) -> String {
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

pub(super) fn extract_i64(cell: &JsonValue) -> Option<i64> {
  match cell {
    JsonValue::Number(n) => n.as_i64(),
    JsonValue::Object(map) if map.get("type").and_then(JsonValue::as_str) == Some("bigint") => {
      map.get("value").and_then(JsonValue::as_str)?.trim().parse::<i64>().ok()
    }
    _ => None,
  }
}

pub(super) fn extract_i128(cell: &JsonValue) -> Option<i128> {
  match cell {
    JsonValue::Number(n) => n.as_i64().map(i128::from).or_else(|| n.as_u64().map(i128::from)),
    JsonValue::Object(map) if map.get("type").and_then(JsonValue::as_str) == Some("bigint") => {
      map.get("value").and_then(JsonValue::as_str)?.trim().parse::<i128>().ok()
    }
    _ => None,
  }
}

pub(super) fn extract_f64(cell: &JsonValue) -> Option<f64> {
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
pub(super) fn parse_decimal_shape(raw: &str) -> Option<(u8, u8)> {
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

pub(super) fn is_leap_year(year: u32) -> bool {
  (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

pub(super) fn days_in_month(year: u32, month: u32) -> u32 {
  match month {
    1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
    4 | 6 | 9 | 11 => 30,
    2 => {
      if is_leap_year(year) {
        29
      } else {
        28
      }
    }
    _ => 0,
  }
}

pub(super) fn is_plain_date(raw: &str) -> bool {
  let s = raw.trim();
  let bytes = s.as_bytes();
  if !(bytes.len() == 10
    && bytes[4] == b'-'
    && bytes[7] == b'-'
    && bytes[0..4].iter().all(u8::is_ascii_digit)
    && bytes[5..7].iter().all(u8::is_ascii_digit)
    && bytes[8..10].iter().all(u8::is_ascii_digit))
  {
    return false;
  }
  let Ok(year) = s[0..4].parse::<u32>() else {
    return false;
  };
  let Ok(month) = s[5..7].parse::<u32>() else {
    return false;
  };
  let Ok(day) = s[8..10].parse::<u32>() else {
    return false;
  };
  year >= 1 && (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)
}

pub(super) fn is_plain_time(raw: &str) -> bool {
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
  let Ok(hour) = s[0..2].parse::<u32>() else {
    return false;
  };
  let Ok(minute) = s[3..5].parse::<u32>() else {
    return false;
  };
  let Ok(second) = s[6..8].parse::<u32>() else {
    return false;
  };
  if hour > 23 || minute > 59 || second > 59 {
    return false;
  }
  if bytes.len() == 8 {
    return true;
  }
  bytes[8] == b'.' && bytes.len() > 9 && bytes[9..].iter().all(u8::is_ascii_digit)
}

pub(super) fn normalize_timestamp(raw: &str) -> Option<String> {
  let s = raw.trim();
  // 标准 SQL/ISO 时间戳全为 ASCII；先判定 is_ascii() 避免在多字节 UTF-8 文本上按字节下标切片 panic
  if !s.is_ascii() || s.len() < 19 {
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

pub(super) fn decode_hex(raw: &str) -> Option<Vec<u8>> {
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

pub(super) fn hex_nibble(byte: u8) -> Option<u8> {
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

pub(super) fn encode_json_doc_value(value: &JsonValue, storage: ColumnStorage) -> Value {
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
