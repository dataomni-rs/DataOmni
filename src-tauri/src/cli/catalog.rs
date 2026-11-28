//! CLI / MCP 元数据目录子命令：dictionary / schema / ddl / diff（H17 / P2-2）。

use super::*;

pub(super) fn dictionary(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<String, CliError> {
  let [connection] = rest else {
    return Err(CliError::usage(USAGE_DICTIONARY));
  };
  let service = open_service(config_dir)?;
  let profile = find_open(&service, connection)?;
  let started = Instant::now();
  let outcome = run_dictionary(&service, &profile, config_dir);
  audit::record(log_dir, "dictionary", &profile.name, "", &outcome, started.elapsed());
  outcome
}

pub(super) fn run_dictionary(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  config_dir: Option<&Path>,
) -> Result<String, CliError> {
  let queries = crate::services::er_diagram_queries(&profile.db_type)
    .filter(|_| session::supports(&profile.db_type))
    .ok_or_else(|| unsupported(&profile.db_type))?;
  let database = profile.database.clone().unwrap_or_default();
  // 同 `list_objects`：没有库名时 MySQL 的目录条件恒不匹配，字典会是空的，像是库里没有表
  if queries.parameter_count > 0 && database.is_empty() {
    return Err(CliError::usage("this connection has no database name; set one in DataOmni"));
  }
  let resolved = resolve_verified(service, profile)?;
  let params = vec![Value::String(database); usize::from(queries.parameter_count)];
  let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECONDS);
  let (columns, foreign_keys) = block_on_within(timeout, "the catalog", async {
    let opened = session::open(&resolved, config_dir).await.map_err(session_error)?;
    let columns = opened.select(queries.columns, params.clone()).await;
    let foreign_keys = match columns {
      Ok(_) => opened.select(queries.foreign_keys, params).await,
      Err(_) => Ok(Vec::new()),
    };
    opened.close().await;
    Ok((columns.map_err(session_error)?, foreign_keys.map_err(session_error)?))
  })?;
  let date = chrono::Local::now().format("%Y-%m-%d").to_string();
  let tables = dictionary::tables_from_rows(&columns);
  let links = dictionary::links_from_rows(&foreign_keys);
  let semantics = config_dir.and_then(|dir| {
    crate::services::connection_semantics::read_semantics_for_agent(dir, &profile.id)
  });
  Ok(dictionary::render_with_semantics(
    &profile.name,
    dictionary::dialect(profile),
    &date,
    &tables,
    &links,
    semantics.as_deref(),
  ))
}

pub(super) struct SchemaArguments {
  connection: String,
  table: Option<String>,
  schema: Option<String>,
}

pub(super) fn parse_schema_arguments(rest: &[String]) -> Result<SchemaArguments, CliError> {
  let mut positional = Vec::new();
  let mut schema = None;
  let mut iter = rest.iter();
  while let Some(argument) = iter.next() {
    match argument.as_str() {
      "--schema" => {
        schema = Some(iter.next().cloned().ok_or_else(|| CliError::usage("--schema takes a name"))?)
      }
      _ => positional.push(argument.clone()),
    }
  }
  let usage = || CliError::usage(USAGE_SCHEMA);
  let mut positional = positional.into_iter();
  let connection = positional.next().ok_or_else(usage)?;
  let table = positional.next();
  if positional.next().is_some() || (schema.is_some() && table.is_none()) {
    return Err(usage());
  }
  Ok(SchemaArguments { connection, table, schema })
}

/// 表或视图的定义原文，同界面结构页的「对象定义」：**数据库自己给的**（`SHOW CREATE TABLE`、
/// `sqlite_master`、`DBMS_METADATA`……），不从目录重建——重建出来的看着权威、照着建却不等价
pub(super) fn ddl(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let arguments = parse_schema_arguments(rest)?;
  let Some(table) = arguments.table.clone() else {
    return Err(CliError::usage(USAGE_DDL));
  };
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &arguments.connection)?;
  let started = Instant::now();
  let outcome = run_ddl(&service, &profile, &table, arguments.schema.as_deref(), config_dir);
  audit::record(log_dir, "ddl", &profile.name, &table, &outcome, started.elapsed());
  let statements = outcome?;
  let ddl: Vec<String> = statements
    .iter()
    .map(
      |statement| {
        if statement.ends_with(';') {
          statement.clone()
        } else {
          format!("{statement};")
        }
      },
    )
    .collect();
  Ok(
    json!({ "schema": OUTPUT_SCHEMA, "connection": profile.name, "table": table, "ddl": ddl.join("\n\n") }),
  )
}

pub(super) fn run_ddl(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  table: &str,
  schema: Option<&str>,
  config_dir: Option<&Path>,
) -> Result<Vec<String>, CliError> {
  if !session::supports(&profile.db_type) {
    return Err(unsupported(&profile.db_type));
  }
  let resolved = resolve_verified(service, profile)?;
  let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECONDS);
  block_on_within(timeout, "the definition", async {
    let opened = session::open(&resolved, config_dir).await.map_err(session_error)?;
    let statements = ddl_statements(&opened, &resolved.db_type, table, schema).await;
    opened.close().await;
    statements
  })
}

/// 定义原文的每一条（SQLite 连着索引与触发器）。查不到就是没有这张表或视图；PostgreSQL 的表
/// 本来就没有原文（它没有 `SHOW CREATE TABLE`），单独说
pub(super) async fn ddl_statements(
  opened: &session::Opened,
  db_type: &DatabaseType,
  table: &str,
  schema: Option<&str>,
) -> Result<Vec<String>, CliError> {
  use crate::services::schema_metadata::{schema_metadata_queries, DdlQuery};
  let queries = schema_metadata_queries(db_type).ok_or_else(|| unsupported(db_type))?;
  let ddl = queries.ddl.ok_or_else(|| unsupported(db_type))?;
  let rows = match ddl {
    DdlQuery::Bound { sql } => {
      let params = if queries.parameter_count == 1 {
        vec![Value::from(table)]
      } else {
        vec![Value::from(table), schema.map_or(Value::Null, Value::from)]
      };
      opened.select(sql, params).await
    }
    // 只有 MySQL 走这一种：`SHOW CREATE TABLE` 不收占位符，表名作为引用过的标识符拼进去
    DdlQuery::Interpolated { sql } => {
      let quote = |name: &str| format!("`{}`", name.replace('`', "``"));
      let target = match schema {
        Some(schema) => format!("{}.{}", quote(schema), quote(table)),
        None => quote(table),
      };
      opened.select(&sql.replace("{table}", &target), Vec::new()).await
    }
  };
  // MySQL 对不存在的表报 1146，当作「没有」而不是数据库错误。sqlx 给的码是 SQLSTATE（42S02），不是错误号
  let rows = match rows {
    Ok(rows) => rows,
    Err(session::Failure::Database(error)) if error.code.as_deref() == Some("42S02") => Vec::new(),
    Err(failure) => return Err(session_error(failure)),
  };
  // 同 `schemaObjects.ts` 的 `extractDdlStatements`：几家把原文放在不同的列里
  let statements: Vec<String> = rows
    .iter()
    .filter_map(|row| {
      ["Create Table", "Create View", "sql"]
        .iter()
        .find_map(|column| row[*column].as_str().map(str::trim).filter(|text| !text.is_empty()))
        .map(str::to_string)
    })
    .collect();
  if statements.is_empty() {
    let message = if *db_type == DatabaseType::PostgreSQL {
      format!("no view named {table}: PostgreSQL keeps no CREATE TABLE text for tables; `schema <connection> {table}` gives the columns, indexes and foreign keys")
    } else {
      format!("no table or view named {table}")
    };
    return Err(CliError { exit: Exit::Usage, kind: "not_found", message, diagnosis: None });
  }
  Ok(statements)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DiffFormat {
  Json,
  Markdown,
}

pub(super) struct DiffArguments {
  source: String,
  target: String,
  format: DiffFormat,
  timeout: Duration,
}

pub(super) fn parse_diff_arguments(rest: &[String]) -> Result<DiffArguments, CliError> {
  let usage = USAGE_DIFF;
  let mut positional = Vec::new();
  let mut format = DiffFormat::Json;
  let mut timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
  let mut iter = rest.iter();
  while let Some(argument) = iter.next() {
    match argument.as_str() {
      "--format" => {
        let value =
          iter.next().ok_or_else(|| CliError::usage("--format takes json or markdown"))?;
        format = match value.as_str() {
          "json" => DiffFormat::Json,
          "markdown" => DiffFormat::Markdown,
          other => {
            return Err(CliError::usage(format!("--format is json or markdown, got {other}")))
          }
        };
      }
      "--timeout" => {
        timeout_seconds = number_after("--timeout", iter.next(), 1, MAX_TIMEOUT_SECONDS)?
      }
      flag if flag.starts_with("--") => {
        return Err(CliError::usage(format!("unknown option {flag}; {usage}")))
      }
      _ => positional.push(argument.clone()),
    }
  }
  let [source, target] = <[String; 2]>::try_from(positional).map_err(|_| CliError::usage(usage))?;
  Ok(DiffArguments { source, target, format, timeout: Duration::from_secs(timeout_seconds) })
}

/// CM3 只读跨库结构差异报告：双边连接均须开放且通过凭证校验（任一不可见一律报统一的 `not_found`，不泄露哪一侧受限）。
pub(super) fn diff_command(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Output, CliError> {
  let arguments = parse_diff_arguments(rest)?;
  let service = open_service(config_dir)?;
  let bilateral_not_found = || CliError {
    exit: Exit::Usage,
    kind: "not_found",
    message: "one or both connections are not open to the command line".to_string(),
    diagnosis: None,
  };
  let source_profile = find_open(&service, &arguments.source).map_err(|_| bilateral_not_found())?;
  let target_profile = find_open(&service, &arguments.target).map_err(|_| bilateral_not_found())?;
  let started = Instant::now();
  let outcome = run_diff(&service, &source_profile, &target_profile, arguments.timeout, config_dir);
  let label = format!("{} -> {}", source_profile.name, target_profile.name);
  audit::record(log_dir, "diff", &label, "", &outcome, started.elapsed());
  let report = outcome?;
  match arguments.format {
    DiffFormat::Markdown => Ok(Output::Markdown(diff::render_markdown(
      &source_profile.name,
      &target_profile.name,
      &report,
    ))),
    DiffFormat::Json => Ok(Output::Json(json!({
      "schema": OUTPUT_SCHEMA,
      "source": source_profile.name,
      "target": target_profile.name,
      "same_dialect": report.same_dialect,
      "ignore_schema": report.ignore_schema,
      "summary": report.summary,
      "tables_added": report.tables_added,
      "tables_removed": report.tables_removed,
      "tables_changed": report.tables_changed,
      "elapsed_ms": started.elapsed().as_millis() as u64,
    }))),
  }
}

pub(super) fn run_diff(
  service: &ConnectionService,
  source_profile: &ConnectionProfile,
  target_profile: &ConnectionProfile,
  timeout: Duration,
  config_dir: Option<&Path>,
) -> Result<diff::SchemaDiffReport, CliError> {
  if !diff::supports_diff(&source_profile.db_type) {
    return Err(unsupported(&source_profile.db_type));
  }
  if !diff::supports_diff(&target_profile.db_type) {
    return Err(unsupported(&target_profile.db_type));
  }
  let source_resolved = resolve_verified(service, source_profile)?;
  let target_resolved = resolve_verified(service, target_profile)?;
  block_on_within(timeout, "the schema diff", async {
    let source_tables = load_diff_snapshot(&source_resolved, config_dir).await?;
    let target_tables = load_diff_snapshot(&target_resolved, config_dir).await?;
    Ok(diff::compare_snapshots(
      &source_resolved.db_type,
      &source_tables,
      &target_resolved.db_type,
      &target_tables,
    ))
  })
}

pub(super) async fn load_diff_snapshot(
  profile: &ConnectionProfile,
  config_dir: Option<&Path>,
) -> Result<Vec<diff::DiffTable>, CliError> {
  let obj_queries = crate::services::object_catalog::object_catalog_queries(&profile.db_type)
    .ok_or_else(|| unsupported(&profile.db_type))?;
  let meta_queries = crate::services::schema_metadata::schema_metadata_queries(&profile.db_type)
    .ok_or_else(|| unsupported(&profile.db_type))?;
  let database = profile.database.clone().unwrap_or_default();
  if obj_queries.object_parameter_count > 0 && database.is_empty() {
    return Err(CliError::usage(format!(
      "{} has no database name; set one in DataOmni",
      profile.name
    )));
  }

  let opened = session::open(profile, config_dir).await.map_err(session_error)?;
  let outcome = async {
    let params =
      vec![Value::String(database.clone()); usize::from(obj_queries.object_parameter_count)];
    let rows = opened.select(obj_queries.objects, params).await.map_err(session_error)?;
    let mut table_list = Vec::new();
    for row in rows {
      let kind = row["object_kind"].as_str().unwrap_or_default().to_ascii_lowercase();
      if !kind.contains("table") {
        continue;
      }
      let name = row["object_name"].as_str().unwrap_or_default().trim().to_string();
      if name.is_empty() {
        continue;
      }
      let schema =
        row["object_schema"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
      table_list.push((schema, name));
    }

    // MySQL / PostgreSQL 走整库 3 条批量查询（常量往返），避免大库经 SSH 隧道做逐表 N+1 超时
    if let Some(bulk) = diff::bulk_diff_queries(&profile.db_type) {
      let bulk_params =
        if bulk.parameter_count == 0 { Vec::new() } else { vec![Value::String(database)] };
      let col_rows =
        opened.select(bulk.columns, bulk_params.clone()).await.map_err(session_error)?;
      let idx_rows =
        opened.select(bulk.indexes, bulk_params.clone()).await.map_err(session_error)?;
      let fk_rows = opened.select(bulk.foreign_keys, bulk_params).await.map_err(session_error)?;
      return Ok(diff::build_bulk_tables(table_list, col_rows, idx_rows, fk_rows, truthy));
    }

    let mut tables = Vec::with_capacity(table_list.len());
    for (schema, name) in table_list {
      let meta_params = if meta_queries.parameter_count == 1 {
        vec![Value::from(name.as_str())]
      } else {
        vec![Value::from(name.as_str()), schema.as_deref().map_or(Value::Null, Value::from)]
      };
      let col_rows =
        opened.select(meta_queries.columns, meta_params.clone()).await.map_err(session_error)?;
      let idx_rows =
        opened.select(meta_queries.indexes, meta_params.clone()).await.map_err(session_error)?;
      let fk_rows =
        opened.select(meta_queries.foreign_keys, meta_params).await.map_err(session_error)?;

      let (columns, primary_key) = diff::columns_and_pk_from_rows(&col_rows, truthy);
      let indexes = diff::indexes_from_rows(&idx_rows, truthy);
      let foreign_keys = diff::foreign_keys_from_rows(&fk_rows);
      tables.push(diff::DiffTable { schema, name, columns, primary_key, indexes, foreign_keys });
    }
    Ok(tables)
  }
  .await;
  opened.close().await;
  outcome
}

pub(super) fn schema(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let arguments = parse_schema_arguments(rest)?;
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &arguments.connection)?;
  let started = Instant::now();
  let outcome = run_schema(&service, &profile, &arguments, config_dir);
  let subject = arguments.table.as_deref().unwrap_or("");
  audit::record(log_dir, "schema", &profile.name, subject, &outcome, started.elapsed());
  outcome
}

pub(super) fn run_schema(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  arguments: &SchemaArguments,
  config_dir: Option<&Path>,
) -> Result<Value, CliError> {
  if !session::supports(&profile.db_type) {
    return Err(unsupported(&profile.db_type));
  }
  let resolved = resolve_verified(service, profile)?;
  let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECONDS);
  let body = block_on_within(timeout, "the catalog", async {
    let opened = session::open(&resolved, config_dir).await.map_err(session_error)?;
    let result = match &arguments.table {
      None => list_objects(&opened, &resolved).await,
      Some(table) => describe_table(&opened, &resolved, table, arguments.schema.as_deref()).await,
    };
    opened.close().await;
    result
  })?;
  let mut output = json!({ "schema": OUTPUT_SCHEMA, "connection": profile.name });
  if let (Value::Object(output), Value::Object(body)) = (&mut output, body) {
    output.extend(body);
  }
  if let Some(table) = arguments.table.as_deref() {
    if let Some(table_semantics) = config_dir
      .and_then(|dir| {
        crate::services::connection_semantics::read_semantics_for_agent(dir, &profile.id)
      })
      .and_then(|notes| {
        crate::services::connection_semantics::extract_table_section(
          &notes,
          table,
          arguments.schema.as_deref(),
        )
      })
    {
      output["semantics"] = Value::String(table_semantics);
    }
  }
  Ok(output)
}

pub(super) async fn list_objects(
  opened: &session::Opened,
  profile: &ConnectionProfile,
) -> Result<Value, CliError> {
  let queries = crate::services::object_catalog::object_catalog_queries(&profile.db_type)
    .ok_or_else(|| unsupported(&profile.db_type))?;
  let database = profile.database.clone().unwrap_or_default();
  // 没有库名时 MySQL 的目录条件恒不匹配，会安静地查出 0 行，像是库是空的（同界面）
  if queries.object_parameter_count > 0 && database.is_empty() {
    return Err(CliError::usage("this connection has no database name; set one in DataOmni"));
  }
  let params = vec![Value::String(database); usize::from(queries.object_parameter_count)];
  let rows = opened.select(queries.objects, params).await.map_err(session_error)?;
  let objects: Vec<Value> = rows
    .into_iter()
    .map(|row| json!({ "schema": row["object_schema"], "name": row["object_name"], "kind": row["object_kind"] }))
    .collect();
  Ok(json!({ "objects": objects }))
}

pub(super) async fn describe_table(
  opened: &session::Opened,
  profile: &ConnectionProfile,
  table: &str,
  schema: Option<&str>,
) -> Result<Value, CliError> {
  let queries = crate::services::schema_metadata::schema_metadata_queries(&profile.db_type)
    .ok_or_else(|| unsupported(&profile.db_type))?;
  // 同 `catalogQueries.ts` 的 `catalogQueryParams`：一个参数是表名，两个是表名与 schema
  let params = if queries.parameter_count == 1 {
    vec![Value::from(table)]
  } else {
    vec![Value::from(table), schema.map_or(Value::Null, Value::from)]
  };
  let columns = opened.select(queries.columns, params.clone()).await.map_err(session_error)?;
  if columns.is_empty() {
    return Err(CliError {
      exit: Exit::Usage,
      kind: "not_found",
      message: format!("no table or view named {table}"),
      diagnosis: None,
    });
  }
  let indexes = opened.select(queries.indexes, params.clone()).await.map_err(session_error)?;
  let foreign_keys = opened.select(queries.foreign_keys, params).await.map_err(session_error)?;
  let columns: Vec<Value> = columns
    .into_iter()
    .map(|row| {
      json!({
        "name": row["column_name"],
        "type": row["data_type"],
        "nullable": truthy(&row["is_nullable"]),
        "primary_key": truthy(&row["is_primary_key"]),
        "default": row["column_default"],
        "comment": row["comment"],
      })
    })
    .collect();
  Ok(
    json!({ "table": table, "columns": columns, "indexes": indexes, "foreign_keys": foreign_keys }),
  )
}

/// 目录里的布尔：PostgreSQL 给真布尔，MySQL 与 SQLite 给 1/0（同 `tableMetadata.ts`）
pub(super) fn truthy(value: &Value) -> bool {
  match value {
    Value::Bool(flag) => *flag,
    Value::Number(number) => number.as_f64().is_some_and(|number| number != 0.0),
    Value::String(text) => matches!(text.as_str(), "1" | "t" | "true" | "YES" | "Y"),
    _ => false,
  }
}

/// 界面用的带类型的值（`{"type": "decimal", "value": "1.50"}`）摊平成值本身：类型是给网格
/// 选编辑器用的，Agent 只要值。整数在 JSON 数字放得下的范围内给数字，超出的仍是字符串——
/// 界面打标签就是为了不丢精度；`json` 列给解析后的值
pub(super) fn plain(value: Option<Value>) -> Value {
  const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;
  let Some(value) = value else { return Value::Null };
  let Value::Object(object) = &value else { return value };
  let (Some(Value::String(kind)), Some(Value::String(text)), 2) =
    (object.get("type"), object.get("value"), object.len())
  else {
    return value;
  };
  match kind.as_str() {
    "bigint" => match text.parse::<i64>() {
      Ok(number) if number.abs() <= MAX_SAFE_INTEGER => Value::from(number),
      _ => Value::String(text.clone()),
    },
    "json" => serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone())),
    _ => Value::String(text.clone()),
  }
}
