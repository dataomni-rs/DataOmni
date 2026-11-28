//! CLI / MCP 查询、执行计划、导出、画像、CSV 预览与备份子命令（H17 / P2-2）。

use super::*;

pub(super) struct QueryArguments {
  connection: String,
  sql: String,
  row_limit: usize,
  timeout: Duration,
  params: std::collections::HashMap<String, Value>,
}

/// `query` 与 `explain` 的参数。`usage` 是那条命令的用法；没有 `--limit` / `--param` 的命令传
/// `false`，给了就报错，而不是悄悄不理
pub(super) fn parse_statement_arguments(
  rest: &[String],
  usage: &str,
  accepts_limit: bool,
  accepts_params: bool,
) -> Result<QueryArguments, CliError> {
  let mut positional = Vec::new();
  let mut row_limit = DEFAULT_ROW_LIMIT;
  let mut timeout_seconds = DEFAULT_TIMEOUT_SECONDS;
  let mut params = std::collections::HashMap::new();
  let mut iter = rest.iter();
  while let Some(argument) = iter.next() {
    match argument.as_str() {
      "--limit" if accepts_limit => {
        row_limit = number_after("--limit", iter.next(), 1, MAX_ROW_LIMIT as u64)? as usize
      }
      "--timeout" => {
        timeout_seconds = number_after("--timeout", iter.next(), 1, MAX_TIMEOUT_SECONDS)?
      }
      "--param" if accepts_params => {
        let raw = iter.next().ok_or_else(|| CliError::usage("--param takes name=value"))?;
        let Some((key, value)) = raw.split_once('=') else {
          return Err(CliError::usage("--param takes name=value"));
        };
        if key.is_empty() {
          return Err(CliError::usage("--param name cannot be empty"));
        }
        params.insert(key.to_string(), Value::String(value.to_string()));
      }
      "--param-null" if accepts_params => {
        let key = iter
          .next()
          .filter(|name| !name.is_empty())
          .ok_or_else(|| CliError::usage("--param-null takes a parameter name"))?;
        params.insert(key.clone(), Value::Null);
      }
      "--params-json" if accepts_params => {
        let raw =
          iter.next().ok_or_else(|| CliError::usage("--params-json takes a JSON object"))?;
        let parsed: Value = serde_json::from_str(raw)
          .map_err(|error| CliError::usage(format!("--params-json is not valid JSON: {error}")))?;
        let Value::Object(map) = parsed else {
          return Err(CliError::usage("--params-json must be a JSON object"));
        };
        for (key, value) in map {
          if key.is_empty() {
            return Err(CliError::usage("--params-json parameter name cannot be empty"));
          }
          if !matches!(value, Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)) {
            return Err(CliError::usage(format!(
              "parameter {key} must be a string, number, boolean or null"
            )));
          }
          params.insert(key, value);
        }
      }
      _ => positional.push(argument.clone()),
    }
  }
  let [connection, sql] =
    <[String; 2]>::try_from(positional).map_err(|_| CliError::usage(usage))?;
  let sql = read_argument(&sql)?;
  Ok(QueryArguments {
    connection,
    sql,
    row_limit,
    timeout: Duration::from_secs(timeout_seconds),
    params,
  })
}

pub(super) fn query(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let arguments = parse_statement_arguments(rest, USAGE_QUERY, true, true)?;
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &arguments.connection)?;
  let started = Instant::now();
  let outcome = run_query(&service, &profile, &arguments, config_dir);
  // 审计只记参数化之前的原始 SQL 长度与 SHA-256，参数值绝不进日志（CM3 安全红线）
  audit::record(log_dir, "query", &profile.name, &arguments.sql, &outcome, started.elapsed());
  let result = outcome?;
  Ok(query_output(&profile, result, started.elapsed()))
}

pub(super) fn run_query(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  arguments: &QueryArguments,
  config_dir: Option<&Path>,
) -> Result<QueryExecutionResult, CliError> {
  // 先过语句门、再看类型与参数绑定，最后才读口令：被拒的语句不该换来一次钥匙串弹框
  read_only_gate::check(&arguments.sql)
    .map_err(|refusal| CliError::refused(refusal.to_string()))?;
  if !session::supports(&profile.db_type) {
    return Err(unsupported(&profile.db_type));
  }
  let rewritten = crate::services::named_parameters::rewrite_named_parameters(
    &arguments.sql,
    &profile.db_type,
    &arguments.params,
  )
  .map_err(|error| CliError::usage(error.to_string()))?;
  let resolved = resolve_verified(service, profile)?;
  let run = session::run_read_only_with_params(
    &resolved,
    &rewritten.sql,
    &rewritten.params,
    arguments.row_limit,
    config_dir,
  );
  block_on_within(arguments.timeout, "the statement", async { run.await.map_err(session_error) })
}

pub(super) fn explain(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let arguments = parse_statement_arguments(rest, USAGE_EXPLAIN, false, false)?;
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &arguments.connection)?;
  let started = Instant::now();
  let outcome = run_explain(&service, &profile, &arguments, config_dir);
  audit::record(log_dir, "explain", &profile.name, &arguments.sql, &outcome, started.elapsed());
  let plan = outcome?;
  Ok(json!({
    "schema": OUTPUT_SCHEMA,
    "connection": profile.name,
    "plan": plan,
    "elapsed_ms": started.elapsed().as_millis() as u64,
  }))
}

pub(super) fn run_explain(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  arguments: &QueryArguments,
  config_dir: Option<&Path>,
) -> Result<crate::services::explain::QueryPlan, CliError> {
  // 包进 EXPLAIN 之前先过语句门：门只放读语句，EXPLAIN 永远不带 ANALYZE
  read_only_gate::check(&arguments.sql)
    .map_err(|refusal| CliError::refused(refusal.to_string()))?;

  if !session::supports(&profile.db_type) {
    return Err(unsupported(&profile.db_type));
  }
  let resolved = resolve_verified(service, profile)?;
  block_on_within(arguments.timeout, "the plan", async {
    let opened = session::open(&resolved, config_dir).await.map_err(session_error)?;
    let plan = opened.explain(&resolved.db_type, &arguments.sql).await;
    opened.close().await;
    plan.map_err(session_error)
  })
}

pub(super) fn test(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let [connection] = rest else {
    return Err(CliError::usage(USAGE_TEST));
  };
  let service = open_service(config_dir)?;
  let profile = find_open(&service, connection)?;
  let started = Instant::now();
  let outcome = run_test(&service, &profile, config_dir);
  audit::record(log_dir, "test", &profile.name, "", &outcome, started.elapsed());
  outcome?;
  Ok(json!({
    "schema": OUTPUT_SCHEMA,
    "connection": profile.name,
    "ok": true,
    "elapsed_ms": started.elapsed().as_millis() as u64,
  }))
}

pub(super) fn run_test(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  config_dir: Option<&Path>,
) -> Result<(), CliError> {
  if !session::supports(&profile.db_type) {
    return Err(unsupported(&profile.db_type));
  }
  let resolved = resolve_verified(service, profile)?;
  let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECONDS);
  block_on_within(timeout, "the connection", async {
    match session::open(&resolved, config_dir).await {
      Ok(opened) => {
        opened.close().await;
        Ok(())
      }
      Err(failure) => {
        let mut error = session_error(failure);
        // 只探网络库的主机与端口：文件库的报错里已经有路径与系统给的原因；经隧道的连接
        // 探到的是隧道后面的地址，从这台机器本来就到不了，结论会是错的
        let networked = !matches!(profile.db_type, DatabaseType::SQLite | DatabaseType::DuckDB);
        if error.kind == "connect" && networked && profile.ssh_tunnel.is_none() {
          let steps = crate::services::diagnose(profile).await.steps;
          error.diagnosis = serde_json::to_value(steps).ok();
        }
        Err(error)
      }
    }
  })
}

/// 导出最多等多久：整表导出要几分钟，语句的默认 30 秒不够
pub(super) const DEFAULT_EXPORT_TIMEOUT_SECONDS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CliExportFormat {
  Csv,
  Json,
  Parquet,
}

impl CliExportFormat {
  const fn as_str(self) -> &'static str {
    match self {
      Self::Csv => "csv",
      Self::Json => "json",
      Self::Parquet => "parquet",
    }
  }
}

pub(super) struct ExportArguments {
  connection: String,
  sql: String,
  out: PathBuf,
  format: CliExportFormat,
  options: crate::services::export_writer::ExportOptions,
  timeout: Duration,
}

pub(super) struct CliExportResult {
  path: String,
  rows_written: u64,
  bytes_written: u64,
  truncated: bool,
}

pub(super) fn parse_export_arguments(rest: &[String]) -> Result<ExportArguments, CliError> {
  use crate::services::export_writer::{ExportFormat, ExportOptions};
  let usage = USAGE_EXPORT;
  let mut positional = Vec::new();
  let mut out = None;
  let mut format = CliExportFormat::Csv;
  let mut options = ExportOptions {
    format: ExportFormat::Csv,
    delimiter: ",".to_string(),
    include_header: true,
    null_text: String::new(),
    byte_order_mark: false,
    sql_table: String::new(),
    sql_dialect: None,
    sql_computed_columns: Vec::new(),
    sql_identity_columns: Vec::new(),
    sql_sequence_columns: Vec::new(),
  };
  let mut timeout_seconds = DEFAULT_EXPORT_TIMEOUT_SECONDS;
  let mut iter = rest.iter();
  while let Some(argument) = iter.next() {
    let mut value = |flag: &str| {
      iter.next().cloned().ok_or_else(|| CliError::usage(format!("{flag} takes a value")))
    };
    match argument.as_str() {
      "--out" => out = Some(PathBuf::from(value("--out")?)),
      // `sql` 要知道哪些是计算列、自增列与序列（界面另查表的元数据），这一版不做
      "--format" => match value("--format")?.as_str() {
        "csv" => {
          format = CliExportFormat::Csv;
          options.format = ExportFormat::Csv;
        }
        "json" => {
          format = CliExportFormat::Json;
          options.format = ExportFormat::Json;
        }
        "parquet" => {
          format = CliExportFormat::Parquet;
        }
        other => {
          return Err(CliError::usage(format!("--format is csv, json or parquet, got {other}")))
        }
      },
      "--delimiter" => options.delimiter = value("--delimiter")?,
      "--null" => options.null_text = value("--null")?,
      "--no-header" => options.include_header = false,
      "--bom" => options.byte_order_mark = true,
      "--timeout" => {
        timeout_seconds = number_after("--timeout", iter.next(), 1, MAX_TIMEOUT_SECONDS)?
      }
      _ => positional.push(argument.clone()),
    }
  }
  let [connection, sql] =
    <[String; 2]>::try_from(positional).map_err(|_| CliError::usage(usage))?;
  let out = out.ok_or_else(|| CliError::usage(usage))?;
  // 不覆盖：Agent 写错一个路径就会盖掉人的文件
  if out.exists() {
    return Err(CliError::usage(format!("{} already exists; choose another path", out.display())));
  }
  Ok(ExportArguments {
    connection,
    sql: read_argument(&sql)?,
    out,
    format,
    options,
    timeout: Duration::from_secs(timeout_seconds),
  })
}

pub(super) fn export(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let arguments = parse_export_arguments(rest)?;
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &arguments.connection)?;
  let started = Instant::now();
  let outcome = run_export(&service, &profile, &arguments, config_dir);
  audit::record(log_dir, "export", &profile.name, &arguments.sql, &outcome, started.elapsed());
  let summary = outcome?;
  let mut output = json!({
    "schema": OUTPUT_SCHEMA,
    "connection": profile.name,
    "format": arguments.format.as_str(),
    "file": summary.path,
    "rows": summary.rows_written,
    "bytes": summary.bytes_written,
    "elapsed_ms": started.elapsed().as_millis() as u64,
  });
  if arguments.format == CliExportFormat::Parquet {
    output["truncated"] = json!(summary.truncated);
  }
  Ok(output)
}

pub(super) fn run_export(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  arguments: &ExportArguments,
  config_dir: Option<&Path>,
) -> Result<CliExportResult, CliError> {
  read_only_gate::check(&arguments.sql)
    .map_err(|refusal| CliError::refused(refusal.to_string()))?;
  if !session::supports(&profile.db_type) {
    return Err(unsupported(&profile.db_type));
  }
  let resolved = resolve_verified(service, profile)?;
  match arguments.format {
    CliExportFormat::Csv | CliExportFormat::Json => {
      let summary = block_on_within(arguments.timeout, "the export", async {
        let opened = session::open(&resolved, config_dir).await.map_err(session_error)?;
        let summary =
          opened.export(&arguments.sql, &arguments.out, arguments.options.clone()).await;
        opened.close().await;
        summary.map_err(session_error)
      })?;
      Ok(CliExportResult {
        path: summary.path,
        rows_written: summary.rows_written,
        bytes_written: summary.bytes_written,
        truncated: false,
      })
    }
    CliExportFormat::Parquet => {
      use crate::services::local_analytics::{
        create_result_session, export_parquet_from_session, MAX_ANALYTICS_ROWS,
      };
      let query_result = block_on_within(arguments.timeout, "the export", async {
        session::run_read_only(&resolved, &arguments.sql, MAX_ANALYTICS_ROWS, config_dir)
          .await
          .map_err(session_error)
      })?;
      let QueryExecutionResult::Rows { columns, column_metadata, rows, truncated, .. } =
        query_result
      else {
        return Err(CliError::usage("the statement returned no rows to export"));
      };
      let (col_inputs, matrix) = result_to_analytics_inputs(&columns, &column_metadata, &rows);
      let analytics =
        create_result_session(&col_inputs, &matrix, MAX_ANALYTICS_ROWS).map_err(|error| {
          CliError {
            exit: Exit::Database,
            kind: "analytics",
            message: error.to_string(),
            diagnosis: None,
          }
        })?;
      let parquet =
        export_parquet_from_session(&analytics, None, &arguments.out).map_err(|error| {
          CliError {
            exit: Exit::Database,
            kind: "analytics",
            message: error.to_string(),
            diagnosis: None,
          }
        })?;
      Ok(CliExportResult {
        path: arguments.out.to_string_lossy().into_owned(),
        rows_written: parquet.row_count,
        bytes_written: parquet.bytes,
        truncated: truncated || analytics.is_truncated(),
      })
    }
  }
}

pub(super) fn result_to_analytics_inputs(
  columns: &[String],
  column_metadata: &[crate::services::query_executor::QueryColumnMetadata],
  rows: &[crate::services::query_executor::QueryRow],
) -> (Vec<crate::services::local_analytics::AnalyticsColumnInput>, Vec<Vec<Value>>) {
  let col_inputs = columns
    .iter()
    .enumerate()
    .map(|(index, name)| {
      let meta = column_metadata.get(index);
      crate::services::local_analytics::AnalyticsColumnInput {
        name: name.clone(),
        logical_type: meta.map(|m| m.logical_type.clone()),
        database_type: meta.map(|m| m.database_type.clone()),
      }
    })
    .collect();
  let matrix = rows
    .iter()
    .map(|row| columns.iter().map(|col| row.get(col).cloned().unwrap_or(Value::Null)).collect())
    .collect();
  (col_inputs, matrix)
}

pub(super) enum ProfileTarget {
  Query { connection: String, sql: String, timeout: Duration },
  LocalFile { path: PathBuf },
}

pub(super) struct ProfileArguments {
  target: ProfileTarget,
  row_limit: usize,
}

pub(super) fn parse_profile_arguments(rest: &[String]) -> Result<ProfileArguments, CliError> {
  use crate::services::local_analytics::MAX_ANALYTICS_ROWS;
  let usage = USAGE_PROFILE;
  let mut positional = Vec::new();
  let mut file: Option<PathBuf> = None;
  let mut row_limit = DEFAULT_PROFILE_ROW_LIMIT;
  let mut timeout_seconds: Option<u64> = None;
  let mut iter = rest.iter();
  while let Some(argument) = iter.next() {
    match argument.as_str() {
      "--file" => {
        let path = iter.next().cloned().ok_or_else(|| CliError::usage("--file takes a path"))?;
        if file.is_some() {
          return Err(CliError::usage(usage));
        }
        file = Some(PathBuf::from(path));
      }
      "--limit" => {
        row_limit = number_after("--limit", iter.next(), 1, MAX_ANALYTICS_ROWS as u64)? as usize
      }
      "--timeout" => {
        timeout_seconds = Some(number_after("--timeout", iter.next(), 1, MAX_TIMEOUT_SECONDS)?)
      }
      flag if flag.starts_with("--") => {
        return Err(CliError::usage(format!("unknown option {flag}; {usage}")))
      }
      _ => positional.push(argument.clone()),
    }
  }
  if let Some(path) = file {
    if !positional.is_empty() || timeout_seconds.is_some() {
      return Err(CliError::usage(usage));
    }
    return Ok(ProfileArguments { target: ProfileTarget::LocalFile { path }, row_limit });
  }
  let [connection, sql] =
    <[String; 2]>::try_from(positional).map_err(|_| CliError::usage(usage))?;
  let sql = read_argument(&sql)?;
  let timeout = Duration::from_secs(timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS));
  Ok(ProfileArguments { target: ProfileTarget::Query { connection, sql, timeout }, row_limit })
}

pub(super) fn profile_command(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  use crate::services::local_analytics::{
    create_result_session, open_local_file_session, profile_session,
  };
  let arguments = parse_profile_arguments(rest)?;
  let started = Instant::now();
  match arguments.target {
    ProfileTarget::LocalFile { path } => {
      if !path.is_file() {
        return Err(CliError {
          exit: Exit::Usage,
          kind: "file",
          message: format!("file not found: {}", path.display()),
          diagnosis: None,
        });
      }
      let session = open_local_file_session(&path, arguments.row_limit).map_err(|error| {
        CliError { exit: Exit::Usage, kind: "file", message: error.to_string(), diagnosis: None }
      })?;
      let profile = profile_session("cli-file-profile", &session).map_err(|error| CliError {
        exit: Exit::Database,
        kind: "analytics",
        message: error.to_string(),
        diagnosis: None,
      })?;
      Ok(json!({
        "schema": OUTPUT_SCHEMA,
        "file": path.to_string_lossy(),
        "file_kind": session.file_kind(),
        "rows": profile.row_count,
        "column_count": profile.column_count,
        "truncated": session.is_truncated(),
        "columns": profile.columns,
        "elapsed_ms": started.elapsed().as_millis() as u64,
      }))
    }
    ProfileTarget::Query { connection, sql, timeout } => {
      let service = open_service(config_dir)?;
      let conn_profile = find_open(&service, &connection)?;
      let query_args = QueryArguments {
        connection: connection.clone(),
        sql: sql.clone(),
        row_limit: arguments.row_limit,
        timeout,
        params: std::collections::HashMap::new(),
      };
      let outcome = run_query(&service, &conn_profile, &query_args, config_dir);
      audit::record(log_dir, "profile", &conn_profile.name, &sql, &outcome, started.elapsed());
      let result = outcome?;
      let QueryExecutionResult::Rows { columns, column_metadata, rows, truncated, .. } = result
      else {
        return Err(CliError::usage("the statement returned no rows to profile"));
      };
      let (col_inputs, matrix) = result_to_analytics_inputs(&columns, &column_metadata, &rows);
      let session =
        create_result_session(&col_inputs, &matrix, arguments.row_limit).map_err(|error| {
          CliError {
            exit: Exit::Database,
            kind: "analytics",
            message: error.to_string(),
            diagnosis: None,
          }
        })?;
      let profile = profile_session("cli-profile", &session).map_err(|error| CliError {
        exit: Exit::Database,
        kind: "analytics",
        message: error.to_string(),
        diagnosis: None,
      })?;
      Ok(json!({
        "schema": OUTPUT_SCHEMA,
        "connection": conn_profile.name,
        "rows": profile.row_count,
        "column_count": profile.column_count,
        "truncated": truncated || session.is_truncated(),
        "columns": profile.columns,
        "elapsed_ms": started.elapsed().as_millis() as u64,
      }))
    }
  }
}

/// 本机的 CSV 文件：嗅探分隔符与编码（UTF-8，否则 gb18030），给表头、开头几行和字段数对不上的行。
/// 不碰连接，也就不留痕
pub(super) fn csv_preview(rest: &[String]) -> Result<Value, CliError> {
  let usage = USAGE_CSV_PREVIEW;
  let mut path = None;
  let mut delimiter = None;
  let mut has_header = true;
  let mut rows = crate::services::csv_import::PREVIEW_ROWS;
  let mut iter = rest.iter();
  while let Some(argument) = iter.next() {
    match argument.as_str() {
      "--delimiter" => match iter.next().map(|text| text.as_bytes()) {
        Some([single]) => delimiter = Some(*single),
        _ => return Err(CliError::usage("--delimiter takes one ASCII character")),
      },
      "--no-header" => has_header = false,
      "--rows" => rows = number_after("--rows", iter.next(), 1, MAX_ROW_LIMIT as u64)? as usize,
      _ if path.is_none() => path = Some(PathBuf::from(argument)),
      _ => return Err(CliError::usage(usage)),
    }
  }
  let path = path.ok_or_else(|| CliError::usage(usage))?;
  let preview = crate::services::csv_import::preview_csv(&path, delimiter, has_header, rows)
    .map_err(|error| CliError {
      exit: Exit::Usage,
      kind: "file",
      message: error.to_string(),
      diagnosis: None,
    })?;
  let mut output = json!({ "schema": OUTPUT_SCHEMA, "file": path.to_string_lossy() });
  if let (Value::Object(output), Ok(Value::Object(body))) =
    (&mut output, serde_json::to_value(preview))
  {
    output.extend(body);
  }
  Ok(output)
}

/// 备份：PostgreSQL、MySQL / MariaDB、MongoDB 用服务端自己的工具（`pg_dump`、`mysqldump`、
/// `mongodump`，都只读），SQLite、DuckDB 在只读打开的库上用库自己的语句，同界面（`services::backup`）。
/// 不设时限：外部工具跑在阻塞线程里，到点丢掉 future 也停不下它，运行时收尾时照样要等它跑完
pub(super) fn backup(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let usage = USAGE_BACKUP;
  let mut positional = Vec::new();
  let mut out = None;
  let mut database = None;
  let mut iter = rest.iter();
  while let Some(argument) = iter.next() {
    let mut value = |flag: &str| {
      iter.next().cloned().ok_or_else(|| CliError::usage(format!("{flag} takes a value")))
    };
    match argument.as_str() {
      "--out" => out = Some(PathBuf::from(value("--out")?)),
      "--database" => database = Some(value("--database")?),
      _ => positional.push(argument.clone()),
    }
  }
  let [connection] = <[String; 1]>::try_from(positional).map_err(|_| CliError::usage(usage))?;
  let out = out.ok_or_else(|| CliError::usage(usage))?;
  if out.exists() {
    return Err(CliError::usage(format!("{} already exists; choose another path", out.display())));
  }
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &connection)?;
  if database.is_some() && profile.db_type != DatabaseType::MongoDB {
    return Err(CliError::usage("--database is for MongoDB: the database to back up"));
  }
  let started = Instant::now();
  let outcome = run_backup(&service, &profile, &out, database.as_deref(), config_dir);
  audit::record(log_dir, "backup", &profile.name, "", &outcome, started.elapsed());
  let kind = outcome?;
  Ok(json!({
    "schema": OUTPUT_SCHEMA,
    "connection": profile.name,
    "file": out.to_string_lossy(),
    "kind": kind,
    "elapsed_ms": started.elapsed().as_millis() as u64,
  }))
}

pub(super) fn run_backup(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  out: &Path,
  database: Option<&str>,
  config_dir: Option<&Path>,
) -> Result<crate::services::backup::BackupKind, CliError> {
  use crate::services::backup::{backup_with_tool, BACKUP_NO_DATABASE, BACKUP_TOOL_MISSING};
  let backup_error = |error: crate::services::QueryError| {
    let message = error.to_string();
    if message.starts_with(BACKUP_TOOL_MISSING) {
      CliError::unavailable("tool", message)
    } else if message.starts_with(BACKUP_NO_DATABASE) {
      CliError::usage(message)
    } else {
      CliError { exit: Exit::Database, kind: "backup", message, diagnosis: None }
    }
  };
  match profile.db_type {
    DatabaseType::PostgreSQL | DatabaseType::MySQL | DatabaseType::MongoDB => {
      let resolved = resolve_verified(service, profile)?;
      runtime()?.block_on(async {
        let (tunnels, port) = session::tunnel(&resolved)
          .await
          .map_err(|error| CliError::unavailable("connect", error))?;
        let outcome = backup_with_tool(&resolved, port, out, database).await.map_err(backup_error);
        tunnels.close().await;
        outcome
      })
    }
    DatabaseType::SQLite | DatabaseType::DuckDB => {
      let resolved = resolve_verified(service, profile)?;
      runtime()?.block_on(async {
        session::backup_embedded(&resolved, config_dir, out).await.map_err(
          |failure| match failure {
            session::Failure::Database(error) => backup_error(error),
            other => session_error(other),
          },
        )
      })
    }
    _ => Err(unsupported(&profile.db_type)),
  }
}

pub(super) fn query_output(
  profile: &ConnectionProfile,
  result: QueryExecutionResult,
  elapsed: Duration,
) -> Value {
  let elapsed_ms = elapsed.as_millis() as u64;
  match result {
    QueryExecutionResult::Rows { columns, rows, truncated, row_limit, .. } => {
      let rows: Vec<Value> = rows
        .into_iter()
        .map(|row| {
          Value::Array(columns.iter().map(|column| plain(row.get(column).cloned())).collect())
        })
        .collect();
      json!({
        "schema": OUTPUT_SCHEMA,
        "connection": profile.name,
        "columns": columns,
        "rows": rows,
        "row_count": rows.len(),
        "truncated": truncated,
        "row_limit": row_limit,
        "elapsed_ms": elapsed_ms,
      })
    }
    // 语句门只放读语句，走到这里的只会是没有结果集的读（极少见），照实说
    QueryExecutionResult::Affected { rows_affected } => json!({
      "schema": OUTPUT_SCHEMA,
      "connection": profile.name,
      "rows_affected": rows_affected,
      "elapsed_ms": elapsed_ms,
    }),
  }
}
