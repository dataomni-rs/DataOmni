//! `dataomni cli …`：给 Agent 与终端用的命令行（`rfcs/agent-cli.md`）。
//!
//! 和应用是同一个二进制：macOS 的钥匙串按代码身份授权，同一个二进制读得到界面存的口令，
//! 另编一个程序每次升级都要多弹一次框（§3.1 E1）。`main` 在起 Tauri 之前分流到这里，
//! 不建窗口。
//!
//! 输出给程序读：结果是 stdout 上的一个 JSON，错误是 stderr 上的一个 JSON，退出码区分
//! 「数据库报错 / 参数错 / 被权限门拒绝 / 连不上」（§7）。文字用英文：命令行的读者多半是
//! Agent，界面的语言设置在 WebView 里，这里读不到。

mod audit;
mod catalog;
mod dictionary;
mod diff;
mod elasticsearch;
mod format;
mod mcp;
mod mongo;
mod neo4j;
mod query;
mod redis;
mod runtime;
mod session;

use catalog::*;
use query::*;
use runtime::*;

use crate::models::{ConnectionProfile, DatabaseType};
use crate::services::connection_service::{app_config_dir, ConnectionService, APP_IDENTIFIER};
use crate::services::query_executor::QueryExecutionResult;
use crate::services::read_only_gate;
use crate::services::ssh_tunnel::TunnelRegistry;
use serde_json::{json, Value};
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 默认最多回多少行。Agent 的上下文按字计费，一张大表整张倒进去是浪费，要更多就明说
const DEFAULT_ROW_LIMIT: usize = 200;
const MAX_ROW_LIMIT: usize = 10_000;
/// `profile` 默认采样/装载行数（`rfcs/cli-mcp-usability.md` CM2）
const DEFAULT_PROFILE_ROW_LIMIT: usize = 10_000;
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const MAX_TIMEOUT_SECONDS: u64 = 600;
/// 读口令最多等多久。钥匙串要授权时这一步会一直卡着，等到有人点了弹框
const KEYCHAIN_DEADLINE: Duration = Duration::from_secs(10);

/// 输出格式的版本。字段改名或改含义时加一，只加字段不加
const OUTPUT_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
  Ok = 0,
  Database = 1,
  Usage = 2,
  /// 被权限门拒绝。Agent 靠它区分「换个写法」和「这条路不通」
  Refused = 3,
  /// 连不上、缺凭据、钥匙串在等授权
  Unavailable = 4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
  pub exit: Exit,
  pub kind: &'static str,
  pub message: String,
  /// `test` 连不上时断在哪一段（`connection_probe::diagnose` 的步骤）
  pub diagnosis: Option<Value>,
}

impl CliError {
  fn usage(message: impl Into<String>) -> Self {
    Self { exit: Exit::Usage, kind: "usage", message: message.into(), diagnosis: None }
  }

  fn unavailable(kind: &'static str, message: impl Into<String>) -> Self {
    Self { exit: Exit::Unavailable, kind, message: message.into(), diagnosis: None }
  }

  fn refused(message: impl Into<String>) -> Self {
    Self { exit: Exit::Refused, kind: "refused", message: message.into(), diagnosis: None }
  }

  /// 没开放的、生产的、不存在的连接都是这一句：不让调用方分辨出「有，但不给你」
  fn not_found(name: &str) -> Self {
    Self {
      exit: Exit::Usage,
      kind: "not_found",
      message: format!("no connection named {name} is open to the command line"),
      diagnosis: None,
    }
  }
}

const HELP: &str = "\
DataOmni: desktop database client, standalone read-only CLI, and MCP server.
CLI and MCP run headless without launching or requiring the desktop app.

Usage:
  dataomni                         Launch the desktop application
  dataomni <command> [arguments]   Run a standalone read-only CLI command
  dataomni cli <command> [...]     Same as above (explicit prefix)
  dataomni mcp                     Start the read-only MCP server over stdio

Commands:
  connections        List the connections open to the command line
  query <connection> <sql> [--limit N] [--timeout SECONDS]
        [--param K=V ...] [--param-null K ...] [--params-json JSON]
                     Run one read-only statement. <connection> is a name or id
                     from `connections`; <sql> may be - to read it from stdin.
                     Named :params are bound via prepared parameters and never
                     logged. At most 200 rows by default (--limit up to 10000),
                     30 s timeout.
  diff <source-connection> <target-connection> [--format json|markdown] [--timeout SECONDS]
                     Read-only schema diff between two open connections (tables,
                     columns, primary keys, indexes and foreign keys). Reports
                     differences only and never executes DDL.
  schema <connection> [<table> [--schema S]]
                     Without a table: the tables and views. With a table: its
                     columns, indexes (one row per index column) and foreign keys.
  explain <connection> <sql> [--timeout SECONDS]
                     The execution plan of one read-only statement, as a tree
                     (plan.roots) plus the database's own text (plan.raw). Never
                     ANALYZE: the statement itself does not run.
  dictionary <connection>
                     A Markdown data dictionary of the whole database: every
                     table's columns, types, nullability, primary and foreign
                     keys. Read it before writing SQL. Printed as Markdown, not JSON.
  mongo <connection> <operation> ...
                     MongoDB, read only: collections; find, count, aggregate,
                     explain and structure on <database>.<collection>. Filters and
                     pipelines are mongosh syntax; documents come back as relaxed
                     Extended JSON. Pipelines with $out or $merge are refused.
                     explain runs the query (executionStats) but returns no
                     documents. `dataomni mongo --help` shows its operations.
  redis <connection> <operation> ...
                     Redis, read only: keyspaces; scan [<pattern>]; get <key>;
                     command <name> [<argument>...], which runs only commands the
                     server itself flags readonly (asked with COMMAND INFO).
                     --db N picks the logical database. `dataomni redis --help`
                     shows its operations.
  neo4j <connection> labels | run <cypher> [--limit N]
                     Neo4j, read only: the labels and relationship types; or one
                     Cypher query, run only if the server classifies it as a read
                     (asked with EXPLAIN) and then in a read-mode transaction.
                     --db picks the database.
  es <connection> indices | request <METHOD> <path> [<body>]
                     Elasticsearch / OpenSearch, read only: the indices, aliases
                     and data streams; or one REST request. GET and HEAD pass
                     (except _refresh, _flush, _forcemerge, _cache); POST only to
                     search endpoints (_search, _count, _msearch, _mget, ...);
                     PUT and DELETE never. The body is parsed JSON when it is JSON.
  export <connection> <sql> --out FILE [--format csv|json|parquet] [--delimiter C]
         [--no-header] [--null TEXT] [--bom] [--timeout SECONDS]
                     Write every row of one read-only statement to a new local
                     file (never overwrites), the same bytes the DataOmni export
                     writes (or Apache Parquet via built-in DuckDB). 300 s timeout by default.
  profile <connection> <sql> [--limit N] [--timeout SECONDS]
          | profile --file <path> [--limit N]
                     Column-level data profile (inferred type, null ratio,
                     distinct count, min/max/quantiles, top-5 values) via
                     built-in sandboxed DuckDB. Default limit 10000 (up to 100000).
  ddl <connection> <table> [--schema S]
                     The definition the database itself gives for a table or
                     view (SHOW CREATE TABLE, sqlite_master, DBMS_METADATA, ...).
                     PostgreSQL has it for views only; use schema for its tables.
  backup <connection> --out PATH [--database D]
                     Back up to a new path the way DataOmni does: pg_dump (custom
                     format), mysqldump, mongodump (--database picks the database),
                     SQLite VACUUM INTO, DuckDB EXPORT DATABASE. All read-only on
                     the database; the dump tools must be installed. No timeout.
  csv-preview <file> [--delimiter C] [--no-header] [--rows N]
                     A local CSV file: the delimiter and encoding it was read with
                     (sniffed unless given), the header, the first rows (50 by
                     default) and the rows whose field count does not match.
  test <connection>  Connect the way query does, run nothing, report ok. When a
                     network connection fails, error.diagnosis says whether the
                     name resolved and the port answered.
  version            Print the DataOmni version
  guide              Print a usage guide written for agents, in the format of a
                     Claude Code skill (save it as SKILL.md)
  help               Print this help

Output is JSON on stdout (dictionary prints Markdown); errors are JSON on stderr.
When writing to an interactive terminal, JSON is pretty-printed and syntax-highlighted
automatically (compact plain JSON when piped). Global display flags: --pretty,
--compact (-c), --color auto|always|never, --no-color (also respects NO_COLOR).
Exit codes: 0 ok, 1 database error, 2 usage error, 3 refused by the access rules,
4 unavailable (cannot connect, missing credentials, keychain waiting for approval).

A connection is visible here only if \"Command line & agent access\" is set to
read-only in its DataOmni settings. Production connections are never visible.
Nothing on this command line can widen that.
";

/// 给 Agent 读的说明，格式是 Claude Code 的 Skill（`SKILL.md`），存下来就能用（设计文档 §7）
pub(crate) const GUIDE: &str = include_str!("guide.md");

pub(crate) use audit::{read_audit_log_from, AuditLogEntry, AUDIT_FILE, MAX_AUDIT_ENTRIES};

/// 命令行读写的两个目录。测试换成临时目录，不碰本机真实的配置与日志
struct Dirs {
  config: Option<PathBuf>,
  log: Option<PathBuf>,
}

/// 和 Tauri 的 `PathResolver::app_log_dir` 同一个算法
fn app_log_dir() -> Option<PathBuf> {
  #[cfg(target_os = "macos")]
  let dir = dirs::home_dir().map(|home| home.join("Library/Logs").join(APP_IDENTIFIER));
  #[cfg(not(target_os = "macos"))]
  let dir = dirs::data_local_dir().map(|dir| dir.join(APP_IDENTIFIER).join("logs"));
  dir
}

/// 安装包里的 Instant Client。和 Tauri 的 `resource_dir` 同一个算法，只是不起 Tauri
fn bundled_oracle_client() -> Option<PathBuf> {
  client_dir_beside(&std::env::current_exe().ok()?)
}

/// 先解析软链接：macOS 上经 `~/.local/bin/dataomni`（安装脚本建的）启动时，`current_exe`
/// 给的是软链接自己的路径（实测），照它找 `../Resources` 就找到了 `~/.local/Resources`
fn client_dir_beside(exe: &Path) -> Option<PathBuf> {
  let exe = exe.canonicalize().ok()?;
  let exe_dir = exe.parent()?;
  #[cfg(target_os = "macos")]
  let resources = exe_dir.join("../Resources");
  #[cfg(target_os = "linux")]
  let resources = exe_dir
    .join("../lib/DataOmni")
    .canonicalize()
    .unwrap_or_else(|_| PathBuf::from("/usr/lib/DataOmni"));
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  let resources = exe_dir.to_path_buf();
  Some(resources.join("instantclient"))
}

/// 进程入口分流结果：决定拉起桌面端、无头 CLI 还是 stdio MCP 服务。
#[derive(Debug, PartialEq, Eq)]
pub enum Invocation<'a> {
  /// 无参数或桌面启动器参数（如 macOS Finder 的 `-psn_*`、显式 `gui`），拉起桌面窗口
  Gui,
  /// 无头只读 CLI（支持 `dataomni cli <cmd>`、`dataomni <cmd>`、`dataomni --help`、`dataomni -V` 等）
  Cli(&'a [String]),
  /// 无头只读 MCP stdio 服务（`dataomni mcp`）
  Mcp(&'a [String]),
}

/// 根据程序名之后的参数（`&args[1..]`）决定启动形态：
/// - 无参数、`gui` 或 macOS 桌面启动器参数（`-psn_*` / `-NS*` / `-Apple*`）→ 启动桌面端；
/// - `cli ...` → 剥掉 `cli` 前缀进入无头 CLI（保持全部既有脚本与 Agent Skill 兼容）；
/// - `mcp ...` → 进入无头 MCP；
/// - 其余任何终端参数（`--help` / `-V` / `connections` / `query` / 拼写错误等）→ 走无头 CLI，绝不意外弹出界面窗口。
pub fn classify_invocation(args: &[String]) -> Invocation<'_> {
  let Some((first, rest)) = args.split_first() else {
    return Invocation::Gui;
  };
  match first.as_str() {
    "gui" if rest.is_empty() => Invocation::Gui,
    "cli" => Invocation::Cli(rest),
    "mcp" => Invocation::Mcp(rest),
    flag if flag.starts_with("-psn_") || flag.starts_with("-NS") || flag.starts_with("-Apple") => {
      Invocation::Gui
    }
    _ => Invocation::Cli(args),
  }
}

const MCP_HELP: &str = "\
DataOmni MCP server: read-only Model Context Protocol server over stdio.
Runs headless without launching or requiring the desktop application.

Usage:
  dataomni mcp

Connect your MCP host (Claude Code, Claude Desktop, Cursor, etc.) to this
command over stdio. Only connections marked \"Command line & agent access:
Read-only\" in DataOmni are visible; production connections are never visible.
";

/// 入口：`args` 是 CLI 命令与参数。返回进程退出码
pub fn run(args: &[String]) -> i32 {
  if let Some(dir) = bundled_oracle_client() {
    crate::services::oracle::pin_client_dir(dir);
  }
  let stdout = std::io::stdout();
  let stdout_is_tty = stdout.is_terminal();
  let stderr = std::io::stderr();
  let stderr_is_tty = stderr.is_terminal();
  let env = format::TermEnv::from_process();
  let dirs = Dirs { config: app_config_dir(), log: app_log_dir() };
  run_with_io(
    args,
    &dirs,
    &mut stdout.lock(),
    &mut stderr.lock(),
    stdout_is_tty,
    stderr_is_tty,
    &env,
  )
}

/// `dataomni mcp`：stdio 上的 MCP 服务（`rfcs/mcp-server.md`）。工具调用拼成参数后走同一个
/// `dispatch`，权限门只有一份。读到标准输入结束就退出
pub fn serve_mcp(args: &[String]) -> i32 {
  let stdout = std::io::stdout();
  let stderr = std::io::stderr();
  if let Some(code) = handle_mcp_args(args, &mut stdout.lock(), &mut stderr.lock()) {
    return code;
  }
  if let Some(dir) = bundled_oracle_client() {
    crate::services::oracle::pin_client_dir(dir);
  }
  // 常驻进程里不能因为钥匙串弹框没人点就退出：整个服务会断掉，Agent 那边只看到「服务没了」。
  // 改为一直等，用户点了「始终允许」这次调用就接着跑
  KEYCHAIN_DEADLINE_EXITS.store(false, Ordering::SeqCst);
  let dirs = Dirs { config: app_config_dir(), log: app_log_dir() };
  let stdin = std::io::stdin();
  mcp::serve(&mut stdin.lock(), &mut stdout.lock(), &dirs)
}

fn handle_mcp_args(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
  if args.is_empty() {
    return None;
  }
  if matches!(args, [flag] if matches!(flag.as_str(), "-h" | "--help" | "help")) {
    let _ = write!(out, "{MCP_HELP}");
    return Some(Exit::Ok as i32);
  }
  let _ =
    writeln!(err, "dataomni mcp takes no arguments, got {} (run `dataomni mcp --help`)", args[0]);
  Some(Exit::Usage as i32)
}

const EMPTY_CONNECTIONS_TTY_HINT: &str = "\
Hint: no connections are open to the CLI. In DataOmni, edit a non-production \
connection and set \"Command line & agent access\" to \"Read-only\" \
(production connections are never visible).";

#[cfg(test)]
fn run_with(args: &[String], dirs: &Dirs, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
  run_with_io(args, dirs, out, err, false, false, &format::TermEnv::default())
}

#[cfg(test)]
fn run_with_tty(
  args: &[String],
  dirs: &Dirs,
  out: &mut dyn Write,
  err: &mut dyn Write,
  stdout_is_tty: bool,
) -> i32 {
  run_with_io(args, dirs, out, err, stdout_is_tty, stdout_is_tty, &format::TermEnv::default())
}

fn run_with_io(
  args: &[String],
  dirs: &Dirs,
  out: &mut dyn Write,
  err: &mut dyn Write,
  stdout_is_tty: bool,
  stderr_is_tty: bool,
  env: &format::TermEnv,
) -> i32 {
  let (options, clean_args) = match format::extract_output_options(args) {
    Ok(parsed) => parsed,
    Err(error) => {
      let err_style = format::OutputOptions::default().resolve(stderr_is_tty, env);
      let _ = format::write_json_line(err, &error_body(&error), err_style);
      return error.exit as i32;
    }
  };
  let out_style = options.resolve(stdout_is_tty, env);
  let err_style = options.resolve(stderr_is_tty, env);
  let result = dispatch(&clean_args, dirs);
  // 写不进 stdout / stderr（管道被关了）时也没有别处可说，退出码照样给
  match result {
    Ok(Output::Json(value)) => {
      let _ = format::write_json_line(out, &value, out_style);
      if stdout_is_tty && is_empty_connections_response(&clean_args, &value) {
        let _ = format::write_hint_line(err, EMPTY_CONNECTIONS_TTY_HINT, err_style.color);
      }
      Exit::Ok as i32
    }
    Ok(Output::Markdown(text)) => {
      let _ = format::write_markdown(out, &text, out_style.color);
      Exit::Ok as i32
    }
    Ok(Output::Text(text)) => {
      let _ = write!(out, "{text}");
      Exit::Ok as i32
    }
    Err(error) => {
      let _ = format::write_json_line(err, &error_body(&error), err_style);
      error.exit as i32
    }
  }
}

/// 静态判定：仅当执行 `connections` 且开放列表与不可用列表均为空时返回 true，不泄露隐藏或生产连接是否存在
fn is_empty_connections_response(args: &[String], value: &Value) -> bool {
  args.first().is_some_and(|cmd| cmd == "connections")
    && value.get("connections").and_then(Value::as_array).is_some_and(Vec::is_empty)
    && value.get("unavailable").and_then(Value::as_array).is_some_and(Vec::is_empty)
}

/// 错误在 stderr 上的那份 JSON。MCP 的工具错误里放的也是它，Agent 照样按 `kind` 分
fn error_body(error: &CliError) -> Value {
  let mut body = json!({
    "schema": OUTPUT_SCHEMA,
    "error": { "kind": error.kind, "message": error.message },
  });
  if let Some(diagnosis) = &error.diagnosis {
    body["error"]["diagnosis"] = diagnosis.clone();
  }
  body
}

enum Output {
  Json(Value),
  Markdown(String),
  Text(String),
}

const USAGE_QUERY: &str =
  "usage: dataomni query <connection> <sql> [--limit N] [--timeout SECONDS] [--param K=V ...] [--param-null K ...] [--params-json JSON]";
const USAGE_DIFF: &str =
  "usage: dataomni diff <source-connection> <target-connection> [--format json|markdown] [--timeout SECONDS]";
const USAGE_SCHEMA: &str = "usage: dataomni schema <connection> [<table> [--schema S]]";
const USAGE_DDL: &str = "usage: dataomni ddl <connection> <table> [--schema S]";
const USAGE_EXPLAIN: &str = "usage: dataomni explain <connection> <sql> [--timeout SECONDS]";
const USAGE_TEST: &str = "usage: dataomni test <connection>";
const USAGE_DICTIONARY: &str = "usage: dataomni dictionary <connection>";
const USAGE_EXPORT: &str =
  "usage: dataomni export <connection> <sql> --out FILE [--format csv|json|parquet] \
  [--delimiter C] [--no-header] [--null TEXT] [--bom] [--timeout SECONDS]";
const USAGE_PROFILE: &str =
  "usage: dataomni profile <connection> <sql> [--limit N] [--timeout SECONDS] | dataomni profile --file <path> [--limit N]";
const USAGE_CSV_PREVIEW: &str =
  "usage: dataomni csv-preview <file> [--delimiter C] [--no-header] [--rows N]";
const USAGE_BACKUP: &str = "usage: dataomni backup <connection> --out PATH [--database D]";

fn subcommand_help(command: &str) -> Option<String> {
  let usage = match command {
    "query" => USAGE_QUERY,
    "diff" => USAGE_DIFF,
    "schema" => USAGE_SCHEMA,
    "ddl" => USAGE_DDL,
    "explain" => USAGE_EXPLAIN,
    "test" => USAGE_TEST,
    "dictionary" => USAGE_DICTIONARY,
    "export" => USAGE_EXPORT,
    "profile" => USAGE_PROFILE,
    "csv-preview" => USAGE_CSV_PREVIEW,
    "backup" => USAGE_BACKUP,
    "mongo" => mongo::USAGE,
    "redis" => redis::USAGE,
    "neo4j" => neo4j::USAGE,
    "es" => elasticsearch::USAGE,
    "connections" | "guide" | "version" | "--version" | "-V" => return Some(HELP.to_string()),
    _ => return None,
  };
  Some(format!("{usage}\n"))
}

fn is_subcommand_help(rest: &[String]) -> bool {
  matches!(rest, [flag] if matches!(flag.as_str(), "--help" | "-h"))
}

fn dispatch(args: &[String], dirs: &Dirs) -> Result<Output, CliError> {
  let config_dir = dirs.config.as_deref();
  let Some((command, rest)) = args.split_first() else {
    return Ok(Output::Text(HELP.to_string()));
  };
  if is_subcommand_help(rest) {
    if let Some(help) = subcommand_help(command) {
      return Ok(Output::Text(help));
    }
  }
  match command.as_str() {
    "help" | "--help" | "-h" => Ok(Output::Text(HELP.to_string())),
    "guide" => {
      no_arguments(command, rest)?;
      Ok(Output::Text(GUIDE.to_string()))
    }
    "version" | "--version" | "-V" => {
      no_arguments(command, rest)?;
      Ok(Output::Json(json!({ "schema": OUTPUT_SCHEMA, "version": env!("CARGO_PKG_VERSION") })))
    }
    "connections" => {
      no_arguments(command, rest)?;
      let service = open_service(config_dir)?;
      Ok(Output::Json(list_open(&service)))
    }
    "query" => query(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "diff" => diff_command(rest, config_dir, dirs.log.as_deref()),
    "schema" => schema(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "explain" => explain(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "test" => test(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "dictionary" => dictionary(rest, config_dir, dirs.log.as_deref()).map(Output::Markdown),
    "mongo" => mongo(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "redis" => redis(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "neo4j" => neo4j(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "es" => es(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "csv-preview" => csv_preview(rest).map(Output::Json),
    "export" => export(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "profile" => profile_command(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "ddl" => ddl(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    "backup" => backup(rest, config_dir, dirs.log.as_deref()).map(Output::Json),
    other => Err(CliError::usage(format!("unknown command {other}; run `dataomni --help`"))),
  }
}

/// `-` 表示从标准输入读：语句、条件、管道都可能长得不便写在命令行上
fn read_argument(text: &str) -> Result<String, CliError> {
  if text != "-" {
    return Ok(text.to_string());
  }
  let mut read = String::new();
  std::io::stdin()
    .read_to_string(&mut read)
    .map_err(|error| CliError::usage(format!("cannot read stdin: {error}")))?;
  Ok(read)
}

fn number_after(flag: &str, value: Option<&String>, min: u64, max: u64) -> Result<u64, CliError> {
  value
    .and_then(|value| value.parse::<u64>().ok())
    .filter(|number| (min..=max).contains(number))
    .ok_or_else(|| CliError::usage(format!("{flag} takes a number from {min} to {max}")))
}

fn mongo(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let arguments = mongo::parse(rest)?;
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &arguments.connection)?;
  let started = Instant::now();
  let outcome = run_mongo(&service, &profile, &arguments);
  let command = arguments.command();
  audit::record(log_dir, command, &profile.name, &arguments.subject, &outcome, started.elapsed());
  let body = outcome?;
  let mut output = json!({ "schema": OUTPUT_SCHEMA, "connection": profile.name });
  if let (Value::Object(output), Value::Object(body)) = (&mut output, body) {
    output.extend(body);
  }
  output["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
  Ok(output)
}

fn run_mongo(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  arguments: &mongo::Arguments,
) -> Result<Value, CliError> {
  if profile.db_type != DatabaseType::MongoDB {
    return Err(CliError::usage(format!("{} is not a MongoDB connection", profile.name)));
  }
  let resolved = resolve_verified(service, profile)?;
  block_on_within(arguments.timeout, "the MongoDB request", mongo::run(&resolved, arguments))
}

fn redis(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let arguments = redis::parse(rest)?;
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &arguments.connection)?;
  let started = Instant::now();
  let outcome = run_redis(&service, &profile, &arguments);
  let command = arguments.command();
  audit::record(log_dir, command, &profile.name, &arguments.subject, &outcome, started.elapsed());
  let body = outcome?;
  let mut output = json!({ "schema": OUTPUT_SCHEMA, "connection": profile.name });
  if let (Value::Object(output), Value::Object(body)) = (&mut output, body) {
    output.extend(body);
  }
  output["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
  Ok(output)
}

fn run_redis(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  arguments: &redis::Arguments,
) -> Result<Value, CliError> {
  if profile.db_type != DatabaseType::Redis {
    return Err(CliError::usage(format!("{} is not a Redis connection", profile.name)));
  }
  let resolved = resolve_verified(service, profile)?;
  block_on_within(arguments.timeout, "the Redis request", redis::run(&resolved, arguments))
}

fn neo4j(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let arguments = neo4j::parse(rest)?;
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &arguments.connection)?;
  let started = Instant::now();
  let outcome = run_neo4j(&service, &profile, &arguments);
  let command = arguments.command();
  audit::record(log_dir, command, &profile.name, &arguments.subject, &outcome, started.elapsed());
  let body = outcome?;
  let mut output = json!({ "schema": OUTPUT_SCHEMA, "connection": profile.name });
  if let (Value::Object(output), Value::Object(body)) = (&mut output, body) {
    output.extend(body);
  }
  output["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
  Ok(output)
}

fn run_neo4j(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  arguments: &neo4j::Arguments,
) -> Result<Value, CliError> {
  if profile.db_type != DatabaseType::Neo4j {
    return Err(CliError::usage(format!("{} is not a Neo4j connection", profile.name)));
  }
  let resolved = resolve_verified(service, profile)?;
  block_on_within(arguments.timeout, "the Neo4j request", neo4j::run(&resolved, arguments))
}

fn es(
  rest: &[String],
  config_dir: Option<&Path>,
  log_dir: Option<&Path>,
) -> Result<Value, CliError> {
  let arguments = elasticsearch::parse(rest)?;
  let service = open_service(config_dir)?;
  let profile = find_open(&service, &arguments.connection)?;
  let started = Instant::now();
  let outcome = run_es(&service, &profile, &arguments);
  let command = arguments.command();
  audit::record(log_dir, command, &profile.name, &arguments.subject, &outcome, started.elapsed());
  let body = outcome?;
  let mut output = json!({ "schema": OUTPUT_SCHEMA, "connection": profile.name });
  if let (Value::Object(output), Value::Object(body)) = (&mut output, body) {
    output.extend(body);
  }
  output["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
  Ok(output)
}

fn run_es(
  service: &ConnectionService,
  profile: &ConnectionProfile,
  arguments: &elasticsearch::Arguments,
) -> Result<Value, CliError> {
  if profile.db_type != DatabaseType::Elasticsearch {
    return Err(CliError::usage(format!("{} is not an Elasticsearch connection", profile.name)));
  }
  let resolved = resolve_verified(service, profile)?;
  block_on_within(
    arguments.timeout,
    "the Elasticsearch request",
    elasticsearch::run(&resolved, arguments),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  const SECRET: &str = "cli-test-secret-7f3a";

  /// 免密的库：不需要开放凭证（`ConnectionService::resolve_for_agents`），测试碰不到钥匙串
  pub(super) fn profile(id: &str, name: &str, environment: &str, agent_access: &str) -> Value {
    json!({
      "id": id, "name": name, "db_type": "postgresql", "host": "db.internal", "port": 5432,
      "database": "app", "username": "reader", "password": "", "ssl": false,
      "options": {}, "tags": [], "environment": environment, "agent_access": agent_access,
    })
  }

  /// 旧版留在文件里的明文口令：有口令，就要凭证
  fn profile_with_secret(id: &str, name: &str) -> Value {
    let mut value = profile(id, name, "development", "read");
    value["password"] = json!(SECRET);
    value
  }

  pub(super) struct ConfigDir(pub(super) PathBuf);

  impl ConfigDir {
    pub(super) fn with(connections: &[Value]) -> Self {
      let dir = Self::with_raw(connections);
      ConnectionService::write_test_agent_grants(&dir.0, connections);
      dir
    }

    /// 只写 `connections.json`，不签任何钥匙串开放凭证（用于测试手改配置文件被拦截）
    pub(super) fn with_raw(connections: &[Value]) -> Self {
      let dir = std::env::temp_dir().join(format!("dataomni-cli-{}", uuid::Uuid::new_v4()));
      if let Err(error) = std::fs::create_dir_all(&dir) {
        panic!("{error}");
      }
      if let Err(error) =
        std::fs::write(dir.join("connections.json"), Value::from(connections.to_vec()).to_string())
      {
        panic!("{error}");
      }
      Self(dir)
    }

    /// 写入测试连接配置并按真实 v2 HMAC 算法为免密开放连接签发测试钥匙串凭证
    pub(super) fn write_connections(&self, connections: &[Value]) {
      if let Err(error) = std::fs::write(
        self.0.join("connections.json"),
        Value::from(connections.to_vec()).to_string(),
      ) {
        panic!("{error}");
      }
      ConnectionService::write_test_agent_grants(&self.0, connections);
    }

    pub(super) fn config_file(&self) -> Vec<u8> {
      std::fs::read(self.0.join("connections.json")).unwrap_or_default()
    }
  }

  impl Drop for ConfigDir {
    fn drop(&mut self) {
      let _ = std::fs::remove_dir_all(&self.0);
    }
  }

  fn run_in(dir: &ConfigDir, args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let dirs = Dirs { config: Some(dir.0.clone()), log: Some(dir.0.join("logs")) };
    let code = run_with(&args, &dirs, &mut out, &mut err);
    (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
  }

  fn names(stdout: &str) -> Vec<String> {
    let value: Value = serde_json::from_str(stdout).unwrap_or_default();
    value["connections"]
      .as_array()
      .map(|list| {
        list.iter().filter_map(|item| item["name"].as_str().map(str::to_string)).collect()
      })
      .unwrap_or_default()
  }

  /// §8 第 3、4 条：生产连接即使字段被改成只读也看不见；没开放的看不见
  #[test]
  fn lists_only_connections_open_to_agents() {
    let dir = ConfigDir::with(&[
      profile("a", "open-dev", "development", "read"),
      profile("b", "prod-marked-read", "production", "read"),
      profile("c", "closed", "development", "off"),
      profile("d", "open-staging", "staging", "read"),
    ]);
    let (code, stdout, stderr) = run_in(&dir, &["connections"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(names(&stdout), vec!["open-dev", "open-staging"]);
  }

  /// §8 第 8 条 + H6 (S1)：手写进文件的「开放」，无论有无口令，没有钥匙串凭证都不算
  #[test]
  fn a_connection_marked_open_only_in_the_file_is_hidden() {
    let dir = ConfigDir::with(&[
      profile_with_secret("a", "forged"),
      profile("b", "open", "development", "read"),
    ]);
    let (code, stdout, stderr) = run_in(&dir, &["connections"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(names(&stdout), vec!["open"]);
    let (code, _, stderr) = run_in(&dir, &["query", "forged", "SELECT 1"]);
    assert_eq!(code, Exit::Usage as i32);
    assert_eq!(json_of(&stderr)["error"]["kind"], "not_found");

    // 无口令连接仅手改文件开放同样不可见（H6 / S1）
    let raw_dir = ConfigDir::with_raw(&[profile("c", "forged-nopass", "development", "read")]);
    let (code, stdout, stderr) = run_in(&raw_dir, &["connections"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(names(&stdout).is_empty(), "unsigned passwordless connection must stay hidden");
    let (code, _, stderr) = run_in(&raw_dir, &["query", "forged-nopass", "SELECT 1"]);
    assert_eq!(code, Exit::Usage as i32);
    assert_eq!(json_of(&stderr)["error"]["kind"], "not_found");
  }

  /// §8 第 6、7 条：输出里没有口令；跑完配置文件一个字节都不变（旧版的明文口令也不迁移）
  #[test]
  fn never_prints_secrets_or_rewrites_the_config() {
    let dir = ConfigDir::with(&[profile_with_secret("a", "open-dev")]);
    let before = dir.config_file();
    for args in [&["connections"][..], &["version"], &["help"], &["nope"]] {
      let (_, stdout, stderr) = run_in(&dir, args);
      assert!(!stdout.contains(SECRET) && !stderr.contains(SECRET), "{args:?}: {stdout}{stderr}");
    }
    assert_eq!(dir.config_file(), before);
  }

  #[test]
  fn errors_are_json_on_stderr_with_their_exit_code() {
    let dir = ConfigDir::with(&[]);
    let (code, stdout, stderr) = run_in(&dir, &["nope"]);
    assert_eq!((code, stdout.as_str()), (Exit::Usage as i32, ""));
    let error: Value = serde_json::from_str(&stderr).unwrap_or_default();
    assert_eq!(error["error"]["kind"], "usage");
    assert_eq!(run_in(&dir, &["connections", "extra"]).0, Exit::Usage as i32);
  }

  #[test]
  fn version_and_help() {
    let dir = ConfigDir::with(&[]);
    for flag in ["version", "--version", "-V"] {
      let (code, stdout, _) = run_in(&dir, &[flag]);
      let value: Value = serde_json::from_str(&stdout).unwrap_or_default();
      assert_eq!((code, value["version"].as_str()), (0, Some(env!("CARGO_PKG_VERSION"))));
    }
    let (code, stdout, _) = run_in(&dir, &[]);
    assert_eq!(code, 0);
    assert!(stdout.contains("dataomni <command> [arguments]"));
    assert!(stdout.contains("dataomni cli <command>"));
    assert!(stdout.contains("dataomni mcp"));

    let (code, stdout, stderr) = run_in(&dir, &["connections", "--help"]);
    assert_eq!(code, 0, "connections --help stderr={stderr}");
    assert!(stdout.contains("Commands:"));

    for (cmd, expected) in [
      ("query", "usage: dataomni query <connection> <sql>"),
      ("profile", "usage: dataomni profile <connection> <sql>"),
      ("diff", "usage: dataomni diff <source-connection> <target-connection>"),
      ("mongo", "find <database>.<collection>"),
      ("redis", "scan [<pattern>]"),
      ("neo4j", "run <cypher>"),
      ("es", "request <METHOD> <path>"),
    ] {
      let (code, stdout, stderr) = run_in(&dir, &[cmd, "--help"]);
      assert_eq!(code, 0, "{cmd} --help stderr={stderr}");
      assert!(stdout.contains(expected), "{cmd} --help stdout={stdout}");
      assert!(
        !stdout.contains("dataomni cli "),
        "{cmd} --help still shows legacy prefix: {stdout}"
      );
    }
  }

  #[test]
  fn classify_invocation_separates_gui_cli_and_mcp_cleanly() {
    let to_vec = |items: &[&str]| items.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();

    // 无参、显式 gui 与 macOS 桌面启动器参数启动界面
    assert_eq!(classify_invocation(&to_vec(&[])), Invocation::Gui);
    assert_eq!(classify_invocation(&to_vec(&["gui"])), Invocation::Gui);
    assert_eq!(classify_invocation(&to_vec(&["-psn_0_123456"])), Invocation::Gui);
    assert_eq!(
      classify_invocation(&to_vec(&["-NSDocumentRevisionsDebugMode", "YES"])),
      Invocation::Gui
    );
    assert_eq!(classify_invocation(&to_vec(&["-AppleLanguages", "(en)"])), Invocation::Gui);

    // 显式 cli 前缀与顶层直接调用均走无头 CLI（不启界面）
    let explicit = to_vec(&["cli", "query", "dev", "SELECT 1"]);
    assert_eq!(classify_invocation(&explicit), Invocation::Cli(&explicit[1..]));
    let bare_cli = to_vec(&["cli"]);
    assert_eq!(classify_invocation(&bare_cli), Invocation::Cli(&[]));

    for direct in [
      to_vec(&["--help"]),
      to_vec(&["-h"]),
      to_vec(&["help"]),
      to_vec(&["--version"]),
      to_vec(&["-V"]),
      to_vec(&["connections"]),
      to_vec(&["query", "dev", "SELECT 1"]),
      to_vec(&["profile", "--file", "a.csv"]),
      to_vec(&["unknown-typo"]),
    ] {
      assert_eq!(classify_invocation(&direct), Invocation::Cli(&direct[..]));
    }

    // mcp 走无头 MCP，支持 mcp --help 且拒绝未知参数
    let mcp_bare = to_vec(&["mcp"]);
    assert_eq!(classify_invocation(&mcp_bare), Invocation::Mcp(&[]));
    let mcp_help = to_vec(&["mcp", "--help"]);
    assert_eq!(classify_invocation(&mcp_help), Invocation::Mcp(&mcp_help[1..]));

    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(handle_mcp_args(&to_vec(&["--help"]), &mut out, &mut err), Some(0));
    assert!(String::from_utf8_lossy(&out).contains("DataOmni MCP server"));
    assert!(err.is_empty());

    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
      handle_mcp_args(&to_vec(&["--bogus"]), &mut out, &mut err),
      Some(Exit::Usage as i32)
    );
    assert!(String::from_utf8_lossy(&err).contains("dataomni mcp --help"));
  }

  pub(super) fn file_profile(
    id: &str,
    name: &str,
    db_type: &str,
    path: &Path,
    environment: &str,
  ) -> Value {
    json!({
      "id": id, "name": name, "db_type": db_type, "host": "", "port": 0,
      "database": path.to_string_lossy(), "username": "", "ssl": false,
      "options": {}, "tags": [], "environment": environment, "agent_access": "read",
    })
  }

  /// 临时目录里一个 1000 行的 SQLite 库
  pub(super) fn seeded_sqlite(dir: &Path) -> PathBuf {
    let path = dir.join("app.db");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
      let options =
        sqlx::sqlite::SqliteConnectOptions::new().filename(&path).create_if_missing(true);
      let pool = sqlx::SqlitePool::connect_with(options).await.expect("create sqlite");
      sqlx::raw_sql(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);
         WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1000)
         INSERT INTO items SELECT i, 'item ' || i FROM n;",
      )
      .execute(&pool)
      .await
      .expect("seed");
      pool.close().await;
    });
    path
  }

  pub(super) fn sqlite_count(path: &Path) -> i64 {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
      let pool =
        sqlx::SqlitePool::connect(&format!("sqlite:{}", path.display())).await.expect("open");
      let count: (i64,) =
        sqlx::query_as("SELECT count(*) FROM items").fetch_one(&pool).await.expect("count");
      pool.close().await;
      count.0
    })
  }

  pub(super) fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_default()
  }

  #[test]
  fn query_reads_rows_with_a_default_limit() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);

    let (code, stdout, stderr) =
      run_in(&dir, &["query", "local", "SELECT id, name FROM items WHERE id <= 2 ORDER BY id"]);
    assert_eq!(code, 0, "{stderr}");
    let output = json_of(&stdout);
    assert_eq!(output["columns"], json!(["id", "name"]));
    assert_eq!(output["rows"], json!([[1, "item 1"], [2, "item 2"]]));
    assert_eq!(output["truncated"], json!(false));

    // §8 第 5 条：1000 行默认只回 200，并且说了被截断
    let (code, stdout, _) = run_in(&dir, &["query", "s", "SELECT * FROM items"]);
    let output = json_of(&stdout);
    assert_eq!((code, output["row_count"].as_u64()), (0, Some(200)));
    assert_eq!(output["truncated"], json!(true));
    let (_, stdout, _) = run_in(&dir, &["query", "s", "SELECT * FROM items", "--limit", "5"]);
    assert_eq!(json_of(&stdout)["row_count"].as_u64(), Some(5));
  }

  /// §8 第 1、2 条：写、多条语句、`ATTACH` 新文件都被拒，退出码 3；库里的行数不变，
  /// 目录里没有多出文件
  #[test]
  fn query_refuses_writes_and_escapes() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);
    let escaped = dir.0.join("escaped.db");
    let attach = format!("ATTACH '{}' AS x", escaped.display());
    let vacuum = format!("VACUUM INTO '{}'", escaped.display());
    for sql in [
      "DELETE FROM items",
      "DROP TABLE items",
      "INSERT INTO items SELECT * FROM items",
      "WITH x AS (SELECT 1) DELETE FROM items",
      "SELECT 1; DELETE FROM items",
      attach.as_str(),
      vacuum.as_str(),
    ] {
      let (code, stdout, stderr) = run_in(&dir, &["query", "local", sql]);
      assert_eq!(code, Exit::Refused as i32, "{sql}: {stdout}{stderr}");
      assert_eq!(json_of(&stderr)["error"]["kind"], "refused", "{sql}");
    }
    assert_eq!(sqlite_count(&db), 1000);
    assert!(!escaped.exists());
  }

  /// §8 第 3、4 条：生产的、没开放的连接，`query` 也报「找不到」，不是「被拒」
  #[test]
  fn query_cannot_reach_hidden_connections() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    let mut closed = file_profile("c", "closed", "sqlite", &db, "development");
    closed["agent_access"] = json!("off");
    dir.write_connections(&[file_profile("p", "prod", "sqlite", &db, "production"), closed]);
    for name in ["prod", "p", "closed", "c", "missing"] {
      let (code, _, stderr) = run_in(&dir, &["query", name, "SELECT 1"]);
      assert_eq!(code, Exit::Usage as i32, "{name}");
      assert_eq!(json_of(&stderr)["error"]["kind"], "not_found", "{name}");
    }
  }

  /// DuckDB 以只读、关掉外部访问的方式打开；读照常
  #[test]
  fn query_reads_duckdb_files() {
    let dir = ConfigDir::with(&[]);
    let file = dir.0.join("warehouse.duckdb");
    {
      let connection = ::duckdb::Connection::open(&file).expect("create duckdb");
      connection
        .execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1), (2);")
        .expect("seed");
    }
    dir.write_connections(&[file_profile("d", "duck", "duckdb", &file, "testing")]);
    let (code, stdout, stderr) = run_in(&dir, &["query", "duck", "SELECT sum(x) AS total FROM t"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(json_of(&stdout)["columns"], json!(["total"]));
    let (code, _, _) = run_in(&dir, &["query", "duck", "COPY t TO 'out.csv'"]);
    assert_eq!(code, Exit::Refused as i32);
    let (code, stdout, stderr) = run_in(&dir, &["schema", "duck", "t"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(json_of(&stdout)["columns"][0]["name"], json!("x"), "{stdout}");

    let backup = dir.0.join("warehouse-backup");
    let (code, stdout, stderr) =
      run_in(&dir, &["backup", "duck", "--out", &backup.to_string_lossy()]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(json_of(&stdout)["kind"], json!("duckdb-directory"), "{stdout}");
    assert!(backup.join("schema.sql").exists());
  }

  /// 留痕里有这次调用，但没有语句原文（字面量里可能有个人数据）
  #[test]
  fn query_is_recorded_without_the_statement_text() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);
    run_in(&dir, &["query", "local", "SELECT * FROM items WHERE name = 'alice@example.com'"]);
    run_in(&dir, &["query", "local", "DELETE FROM items"]);
    let log =
      std::fs::read_to_string(dir.0.join("logs").join(audit::AUDIT_FILE)).unwrap_or_default();
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines.len(), 2, "{log}");
    assert!(lines[0].contains("first=SELECT") && lines[0].contains("result=ok"), "{log}");
    assert!(lines[1].contains("first=DELETE") && lines[1].contains("result=refused"), "{log}");
    assert!(!log.contains("alice@example.com"), "{log}");
  }

  #[test]
  fn typed_values_are_flattened_without_losing_precision() {
    let tagged = |kind: &str, value: &str| Some(json!({ "type": kind, "value": value }));
    assert_eq!(plain(tagged("bigint", "42")), json!(42));
    assert_eq!(plain(tagged("bigint", "-9007199254740991")), json!(-9007199254740991_i64));
    assert_eq!(plain(tagged("bigint", "9007199254740993")), json!("9007199254740993"));
    assert_eq!(plain(tagged("decimal", "1.50")), json!("1.50"));
    assert_eq!(plain(tagged("json", r#"{"a":[1]}"#)), json!({ "a": [1] }));
    assert_eq!(
      plain(Some(json!({ "type": "x", "value": "v", "extra": 1 }))),
      json!({ "type": "x", "value": "v", "extra": 1 })
    );
    assert_eq!(plain(Some(json!("text"))), json!("text"));
    assert_eq!(plain(None), Value::Null);
  }

  #[test]
  fn schema_lists_objects_and_describes_a_table() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
      let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db.display())).await.expect("open");
      sqlx::raw_sql(
        "CREATE TABLE orders (id INTEGER PRIMARY KEY, item_id INTEGER NOT NULL REFERENCES items(id), note TEXT);
         CREATE INDEX orders_item ON orders(item_id);
         CREATE VIEW recent AS SELECT * FROM orders;",
      )
      .execute(&pool)
      .await
      .expect("schema");
      pool.close().await;
    });
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);

    let (code, stdout, stderr) = run_in(&dir, &["schema", "local"]);
    assert_eq!(code, 0, "{stderr}");
    let objects = json_of(&stdout)["objects"].clone();
    assert_eq!(
      objects,
      json!([
        { "schema": null, "name": "items", "kind": "table" },
        { "schema": null, "name": "orders", "kind": "table" },
        { "schema": null, "name": "recent", "kind": "view" },
      ])
    );

    let (code, stdout, stderr) = run_in(&dir, &["schema", "local", "orders"]);
    assert_eq!(code, 0, "{stderr}");
    let table = json_of(&stdout);
    let columns: Vec<(Value, bool, bool)> = table["columns"]
      .as_array()
      .map(|columns| {
        columns
          .iter()
          .map(|column| {
            (
              column["name"].clone(),
              column["nullable"].as_bool().unwrap_or(true),
              column["primary_key"].as_bool().unwrap_or(false),
            )
          })
          .collect()
      })
      .unwrap_or_default();
    assert_eq!(
      columns,
      vec![
        (json!("id"), true, true),
        (json!("item_id"), false, false),
        (json!("note"), true, false)
      ]
    );
    assert_eq!(table["foreign_keys"][0]["referenced_table"], json!("items"), "{stdout}");
    assert!(stdout.contains("orders_item"), "{stdout}");

    let (code, _, stderr) = run_in(&dir, &["schema", "local", "missing"]);
    assert_eq!(
      (code, json_of(&stderr)["error"]["kind"].clone()),
      (Exit::Usage as i32, json!("not_found"))
    );
  }

  /// 只取计划不执行：SQLite 给的是「按主键查」这一步；写语句照样被语句门拒掉，不会被包进 EXPLAIN
  #[test]
  fn explain_returns_the_plan_without_running_the_statement() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);

    let (code, stdout, stderr) =
      run_in(&dir, &["explain", "local", "SELECT name FROM items WHERE id = 3"]);
    assert_eq!(code, 0, "{stderr}");
    let output = json_of(&stdout);
    assert_eq!(output["plan"]["analyzed"], json!(false));
    let operation = output["plan"]["roots"][0]["operation"].as_str().unwrap_or_default();
    assert!(operation.contains("items"), "{stdout}");

    for sql in ["DELETE FROM items", "EXPLAIN ANALYZE SELECT 1", "SELECT 1; DELETE FROM items"] {
      let (code, _, stderr) = run_in(&dir, &["explain", "local", sql]);
      assert_eq!(code, Exit::Refused as i32, "{sql}: {stderr}");
    }
    assert_eq!(sqlite_count(&db), 1000);
  }

  /// 整库一次拿到：每张表一节，外键两头都写上；输出是 Markdown 不是 JSON
  #[test]
  fn dictionary_describes_every_table_and_its_foreign_keys() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
      let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db.display())).await.expect("open");
      sqlx::raw_sql("CREATE TABLE orders (id INTEGER PRIMARY KEY, item_id INTEGER NOT NULL REFERENCES items(id))")
        .execute(&pool)
        .await
        .expect("orders");
      pool.close().await;
    });
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);

    let (code, stdout, stderr) = run_in(&dir, &["dictionary", "local"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
      stdout.starts_with("# local\n\nSQLite · tables: 2 · foreign-key links: 1 · "),
      "{stdout}"
    );
    assert!(stdout.contains("## items\n"), "{stdout}");
    assert!(stdout.contains("| `id` | INTEGER | no | ✓ |  |"), "{stdout}");
    assert!(stdout.contains("| `item_id` | INTEGER | no |  | `items.id` |"), "{stdout}");
    assert!(stdout.contains("Referenced by: `orders.item_id`"), "{stdout}");

    assert_eq!(run_in(&dir, &["dictionary"]).0, Exit::Usage as i32);
    assert_eq!(run_in(&dir, &["dictionary", "nope"]).0, Exit::Usage as i32);
  }

  /// 连得上就是 `ok`；文件不在就是「连不上」，原因在报错里
  #[test]
  fn test_opens_the_connection_read_only() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    let missing = dir.0.join("missing.db");
    dir.write_connections(&[
      file_profile("s", "local", "sqlite", &db, "development"),
      file_profile("m", "gone", "sqlite", &missing, "development"),
    ]);

    let (code, stdout, stderr) = run_in(&dir, &["test", "local"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(json_of(&stdout)["ok"], json!(true));

    let (code, _, stderr) = run_in(&dir, &["test", "gone"]);
    assert_eq!(code, Exit::Unavailable as i32, "{stderr}");
    assert_eq!(json_of(&stderr)["error"]["kind"], "connect");
    assert!(!missing.exists(), "testing must not create the database file");
  }

  /// 网络库连不上时附上诊断：断在哪一段，Agent 才知道下一步是改地址还是去看服务
  #[test]
  fn a_failed_test_says_where_it_broke() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
      .and_then(|listener| listener.local_addr())
      .map(|address| address.port())
      .expect("a free port");
    let mut closed = profile("p", "down", "development", "read");
    closed["host"] = json!("127.0.0.1");
    closed["port"] = json!(port);
    let dir = ConfigDir::with(&[closed]);

    let (code, _, stderr) = run_in(&dir, &["test", "down"]);
    assert_eq!(code, Exit::Unavailable as i32, "{stderr}");
    let error = &json_of(&stderr)["error"];
    let steps: Vec<(Value, Value)> = error["diagnosis"]
      .as_array()
      .map(|steps| steps.iter().map(|step| (step["name"].clone(), step["ok"].clone())).collect())
      .unwrap_or_default();
    assert_eq!(
      steps,
      vec![(json!("resolve"), json!(true)), (json!("tcp"), json!(false))],
      "{stderr}"
    );

    // 库没起来是「连不上」（4），不是语句超时（1）：Agent 据此决定是重试还是换语句
    let (code, _, stderr) = run_in(&dir, &["query", "down", "SELECT 1"]);
    assert_eq!(code, Exit::Unavailable as i32, "{stderr}");
    assert_eq!(json_of(&stderr)["error"]["kind"], "connect");

    // 备份交给 pg_dump：它在就报它自己的错，不在就说缺工具；两种都不留文件
    let out = std::env::temp_dir().join(format!("dataomni-cli-backup-{port}.dump"));
    let (code, _, stderr) = run_in(&dir, &["backup", "down", "--out", &out.to_string_lossy()]);
    let expected = match crate::services::backup::find_tool("pg_dump") {
      Some(_) => (Exit::Database as i32, "backup"),
      None => (Exit::Unavailable as i32, "tool"),
    };
    assert_eq!(
      (code, json_of(&stderr)["error"]["kind"].as_str().unwrap_or_default()),
      expected,
      "{stderr}"
    );
    assert!(!out.exists());
  }

  /// Mongo 连接走 `mongo`：SQL 命令指过去；`mongo` 用在别的连接上是参数错；连不上是 4
  #[test]
  fn mongo_connections_take_the_mongo_command() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
      .and_then(|listener| listener.local_addr())
      .map(|address| address.port())
      .expect("a free port");
    let mut mongo = profile("m", "docs", "development", "read");
    mongo["db_type"] = json!("mongodb");
    mongo["host"] = json!("127.0.0.1");
    mongo["port"] = json!(port);
    mongo["username"] = json!("");
    let dir = ConfigDir::with(&[mongo, profile("p", "sql", "development", "read")]);

    let (code, _, stderr) = run_in(&dir, &["query", "docs", "SELECT 1"]);
    assert_eq!(code, Exit::Refused as i32, "{stderr}");
    assert!(stderr.contains("dataomni mongo"), "{stderr}");
    let (code, _, stderr) = run_in(&dir, &["mongo", "sql", "collections"]);
    assert_eq!(code, Exit::Usage as i32, "{stderr}");
    let (code, _, stderr) =
      run_in(&dir, &["mongo", "docs", "aggregate", "db.c", "[{ $out: 'x' }]"]);
    assert_eq!(code, Exit::Refused as i32, "{stderr}");

    let (code, _, stderr) = run_in(&dir, &["mongo", "docs", "collections"]);
    assert_eq!(code, Exit::Unavailable as i32, "{stderr}");
    assert_eq!(json_of(&stderr)["error"]["kind"], "connect");
  }

  /// 分隔符是嗅探出来的（要求开头几行字段数一致）；字段数不对的行单独列出；文件不在是参数错
  #[test]
  fn csv_preview_sniffs_the_delimiter_and_lists_ragged_rows() {
    let dir = ConfigDir::with(&[]);
    let file = dir.0.join("people.csv");
    std::fs::write(&file, "id;name\n1;ada\n2;bob\n3;cy\n").expect("csv");
    let path = file.to_string_lossy().into_owned();
    let (code, stdout, stderr) = run_in(&dir, &["csv-preview", &path]);
    assert_eq!(code, 0, "{stderr}");
    let output = json_of(&stdout);
    assert_eq!(output["delimiter"], json!(";"), "{stdout}");
    assert_eq!(output["headers"], json!(["id", "name"]));
    assert_eq!(output["rows"].as_array().map(Vec::len), Some(3));

    let (code, stdout, _) = run_in(&dir, &["csv-preview", &path, "--no-header", "--rows", "1"]);
    assert_eq!(code, 0);
    assert_eq!(json_of(&stdout)["rows"], json!([["id", "name"]]));

    let ragged = dir.0.join("ragged.csv");
    std::fs::write(&ragged, "id;name\n1;ada\n2;bob;extra\n").expect("csv");
    let ragged = ragged.to_string_lossy().into_owned();
    let (code, stdout, _) = run_in(&dir, &["csv-preview", &ragged, "--delimiter", ";"]);
    assert_eq!(code, 0);
    assert_eq!(json_of(&stdout)["ragged"][0]["fields"], json!(3), "{stdout}");

    let missing = dir.0.join("missing.csv").to_string_lossy().into_owned();
    assert_eq!(run_in(&dir, &["csv-preview", &missing]).0, Exit::Usage as i32);
    assert_eq!(run_in(&dir, &["csv-preview", &path, "--delimiter", "ab"]).0, Exit::Usage as i32);
  }

  /// 整份结果写进新文件，字节与界面导出相同；写语句被拒、不留文件；已有的文件不覆盖
  #[test]
  fn export_writes_every_row_to_a_new_file() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);
    let csv = dir.0.join("items.csv");
    let csv_path = csv.to_string_lossy().into_owned();
    let (code, stdout, stderr) = run_in(
      &dir,
      &["export", "local", "SELECT id, name FROM items ORDER BY id", "--out", &csv_path],
    );
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(json_of(&stdout)["rows"], json!(1000), "{stdout}");
    let written = std::fs::read_to_string(&csv).unwrap_or_default();
    assert!(written.starts_with("id,name\n1,item 1\n2,item 2\n"), "{written:.40}");
    assert_eq!(written.lines().count(), 1001);

    let json_path = dir.0.join("items.json").to_string_lossy().into_owned();
    let (code, _, stderr) = run_in(
      &dir,
      &[
        "export",
        "local",
        "SELECT id FROM items WHERE id <= 2",
        "--out",
        &json_path,
        "--format",
        "json",
      ],
    );
    assert_eq!(code, 0, "{stderr}");
    let rows: Value =
      serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap_or_default())
        .unwrap_or_default();
    // SQLite 的 INTEGER 标成 bigint，安全范围内照样是数，同界面导出（`export_writer::json_field`）
    assert_eq!(rows, json!([{ "id": 1 }, { "id": 2 }]));

    let refused = dir.0.join("refused.csv");
    let refused_path = refused.to_string_lossy().into_owned();
    let (code, _, _) =
      run_in(&dir, &["export", "local", "DELETE FROM items", "--out", &refused_path]);
    assert_eq!(code, Exit::Refused as i32);
    assert!(!refused.exists());
    assert_eq!(sqlite_count(&db), 1000);
    let (code, _, stderr) = run_in(&dir, &["export", "local", "SELECT 1", "--out", &csv_path]);
    assert_eq!(code, Exit::Usage as i32, "{stderr}");
    assert!(stderr.contains("already exists"), "{stderr}");
    for bad in [
      &["export", "local", "SELECT 1"][..],
      &["export", "local", "SELECT 1", "--out", "x", "--format", "sql"],
    ] {
      assert_eq!(run_in(&dir, bad).0, Exit::Usage as i32, "{bad:?}");
    }
  }

  /// 数据库自己的原文：SQLite 连着索引一起给；不存在的表是「找不到」
  #[test]
  fn ddl_gives_the_definition_the_database_keeps() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
      let pool =
        sqlx::SqlitePool::connect(&format!("sqlite:{}", db.display())).await.expect("open");
      sqlx::raw_sql("CREATE INDEX items_name ON items(name)").execute(&pool).await.expect("index");
      pool.close().await;
    });
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);
    let (code, stdout, stderr) = run_in(&dir, &["ddl", "local", "items"]);
    assert_eq!(code, 0, "{stderr}");
    let ddl = json_of(&stdout)["ddl"].as_str().unwrap_or_default().to_string();
    assert!(ddl.starts_with("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);"), "{ddl}");
    assert!(ddl.ends_with("CREATE INDEX items_name ON items(name);"), "{ddl}");
    let (code, _, stderr) = run_in(&dir, &["ddl", "local", "nope"]);
    assert_eq!(code, Exit::Usage as i32);
    assert_eq!(json_of(&stderr)["error"]["kind"], "not_found");
    assert_eq!(run_in(&dir, &["ddl", "local"]).0, Exit::Usage as i32);
  }

  /// SQLite 的备份在只读打开的库上做（`VACUUM INTO`），得到一份能直接打开的库；不覆盖已有的
  #[test]
  fn backup_copies_a_sqlite_database_without_writing_to_it() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);
    let before = std::fs::read(&db).unwrap_or_default();
    let copy = dir.0.join("copy.db");
    let copy_path = copy.to_string_lossy().into_owned();
    let (code, stdout, stderr) = run_in(&dir, &["backup", "local", "--out", &copy_path]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(json_of(&stdout)["kind"], json!("sqlite-file"), "{stdout}");
    assert_eq!(sqlite_count(&copy), 1000);
    assert_eq!(std::fs::read(&db).unwrap_or_default(), before, "the source is untouched");

    let (code, _, stderr) = run_in(&dir, &["backup", "local", "--out", &copy_path]);
    assert_eq!(code, Exit::Usage as i32);
    assert!(stderr.contains("already exists"), "{stderr}");
    let other = dir.0.join("other.db").to_string_lossy().into_owned();
    for bad in [&["backup", "local"][..], &["backup", "local", "--out", &other, "--database", "x"]]
    {
      assert_eq!(run_in(&dir, bad).0, Exit::Usage as i32, "{bad:?}");
    }
  }

  /// 每个命令都在 `help` 与 `guide` 里写着，dispatch 也认得它；加了命令忘了写说明，这里红
  #[test]
  fn every_command_is_documented_in_help_and_guide() {
    const COMMANDS: &[&str] = &[
      "connections",
      "query",
      "diff",
      "schema",
      "explain",
      "dictionary",
      "mongo",
      "redis",
      "neo4j",
      "es",
      "export",
      "profile",
      "ddl",
      "backup",
      "csv-preview",
      "test",
      "version",
      "guide",
      "help",
    ];
    let dir = ConfigDir::with(&[]);
    for command in COMMANDS {
      assert!(HELP.contains(&format!("\n  {command}")), "help does not list {command}");
      let (_, _, stderr) = run_in(&dir, &[command]);
      assert!(!stderr.contains("unknown command"), "{command}: {stderr}");
      if !["help", "version", "guide"].contains(command) {
        assert!(
          GUIDE.contains(&format!("dataomni {command}"))
            || GUIDE.contains(&format!("dataomni cli {command}")),
          "guide does not show {command}"
        );
      }
    }
    let (code, stdout, _) = run_in(&dir, &["guide"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("---\nname: dataomni\ndescription: \""), "{stdout:.80}");
  }

  /// 经软链接启动也找得到安装包里的 Instant Client
  #[cfg(target_os = "macos")]
  #[test]
  fn the_bundled_client_is_found_through_a_symlink() {
    let dir = ConfigDir::with(&[]);
    let macos = dir.0.join("DataOmni.app/Contents/MacOS");
    std::fs::create_dir_all(&macos).expect("bundle");
    std::fs::write(macos.join("dataomni"), "").expect("binary");
    let link = dir.0.join("dataomni");
    std::os::unix::fs::symlink(macos.join("dataomni"), &link).expect("symlink");
    let found = client_dir_beside(&link).unwrap_or_default();
    let real = macos.canonicalize().unwrap_or_default();
    assert_eq!(found, real.join("../Resources/instantclient"));
  }

  #[test]
  fn query_arguments_are_checked() {
    let dir = ConfigDir::with(&[]);
    for args in [
      &["query"][..],
      &["query", "x"],
      &["query", "x", "SELECT 1", "extra"],
      &["query", "x", "SELECT 1", "--limit", "0"],
      &["query", "x", "SELECT 1", "--limit", "10001"],
      &["query", "x", "SELECT 1", "--timeout"],
      &["schema"],
      &["schema", "x", "t", "extra"],
      &["schema", "x", "--schema", "public"],
      &["explain"],
      &["explain", "x"],
      &["explain", "x", "SELECT 1", "--limit", "5"],
      &["test"],
      &["dictionary", "x", "y"],
      &["test", "x", "extra"],
    ] {
      assert_eq!(run_in(&dir, args).0, Exit::Usage as i32, "{args:?}");
    }
  }

  /// 没有配置文件（从没打开过应用）就是一个连接都没有，不是错误
  #[test]
  fn a_missing_config_lists_nothing() {
    let dir = ConfigDir::with(&[]);
    let _ = std::fs::remove_file(dir.0.join("connections.json"));
    let (code, stdout, _) = run_in(&dir, &["connections"]);
    assert_eq!((code, names(&stdout).len()), (0, 0));
  }

  /// CM2：Parquet 导出与 `profile`（数据库连接模式 + `--file` 本地文件模式）端到端闭环
  #[test]
  fn export_parquet_and_profile_work_end_to_end() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    dir.write_connections(&[file_profile("s", "local", "sqlite", &db, "development")]);

    // 1. export --format parquet 写出合法 Parquet 文件且绝不覆盖已有文件
    let parquet = dir.0.join("items.parquet");
    let parquet_path = parquet.to_string_lossy().into_owned();
    let (code, stdout, stderr) = run_in(
      &dir,
      &[
        "export",
        "local",
        "SELECT id, name FROM items ORDER BY id",
        "--out",
        &parquet_path,
        "--format",
        "parquet",
      ],
    );
    assert_eq!(code, 0, "{stderr}");
    let export_body = json_of(&stdout);
    assert_eq!(export_body["format"], json!("parquet"));
    assert_eq!(export_body["rows"], json!(1000));
    assert!(export_body["bytes"].as_u64().unwrap_or(0) > 0);
    assert_eq!(export_body["truncated"], json!(false));

    let (code, _, stderr) =
      run_in(&dir, &["export", "local", "SELECT 1", "--out", &parquet_path, "--format", "parquet"]);
    assert_eq!(code, Exit::Usage as i32, "{stderr}");
    assert!(stderr.contains("already exists"), "{stderr}");

    // 2. profile --file 分析导出的 Parquet 文件（不写审计日志）
    let (code, stdout, stderr) = run_in(&dir, &["profile", "--file", &parquet_path]);
    assert_eq!(code, 0, "{stderr}");
    let file_profile_body = json_of(&stdout);
    assert_eq!(file_profile_body["file_kind"], json!("parquet"));
    assert_eq!(file_profile_body["rows"], json!(1000));
    assert_eq!(file_profile_body["truncated"], json!(false));
    let cols = file_profile_body["columns"].as_array().expect("file profile cols");
    assert_eq!(cols.len(), 2);
    assert_eq!(cols[0]["name"], json!("id"));
    assert_eq!(cols[0]["distinctCount"], json!(1000));
    assert!(cols[0]["p50"].is_number());

    // 3. profile <conn> <sql> 走只读门、截断提示并写审计日志
    let (code, stdout, stderr) = run_in(
      &dir,
      &["profile", "local", "SELECT id, name FROM items ORDER BY id", "--limit", "25"],
    );
    assert_eq!(code, 0, "{stderr}");
    let conn_profile_body = json_of(&stdout);
    assert_eq!(conn_profile_body["connection"], json!("local"));
    assert_eq!(conn_profile_body["rows"], json!(25));
    assert_eq!(conn_profile_body["truncated"], json!(true));
    assert_eq!(conn_profile_body["column_count"], json!(2));

    let audit_log =
      std::fs::read_to_string(dir.0.join("logs").join(AUDIT_FILE)).unwrap_or_default();
    assert!(audit_log.contains(" profile "), "{audit_log}");
    assert!(!audit_log.contains("ORDER BY id"), "{audit_log}");

    // 4. 写语句被拒，混用参数报 usage
    let (code, _, _) = run_in(&dir, &["profile", "local", "DELETE FROM items"]);
    assert_eq!(code, Exit::Refused as i32);
    for bad in [
      &["profile"][..],
      &["profile", "--file", &parquet_path, "local", "SELECT 1"],
      &["profile", "--file", &parquet_path, "--timeout", "10"],
      &["profile", "--file"],
      &["profile", "local", "SELECT 1", "--limit", "0"],
    ] {
      assert_eq!(run_in(&dir, bad).0, Exit::Usage as i32, "{bad:?}");
    }
  }

  /// CM3 参数化查询门：SQLite 与 DuckDB 支持 `--param` / `--param-null` / `--params-json`，
  /// 严禁把参数值明文写入 `dataomni-cli.log`
  #[test]
  fn query_supports_named_parameters_without_logging_values() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    let duck_path = dir.0.join("sample.duckdb");
    {
      let conn = duckdb::Connection::open(&duck_path).expect("create duckdb");
      conn
        .execute_batch(
          "CREATE TABLE metrics (id INT, tag VARCHAR, note VARCHAR);
           INSERT INTO metrics VALUES (1, 'alpha', 'ok'), (2, 'beta', NULL);",
        )
        .expect("seed duckdb");
    }
    dir.write_connections(&[
      file_profile("s", "local", "sqlite", &db, "development"),
      file_profile("d", "duck", "duckdb", &duck_path, "development"),
    ]);

    // 1. SQLite: --params-json 保留数字类型，--param 传字符串
    let secret_sentinel = "SECRET_PARAM_SENTINEL_9981";
    let (code, stdout, stderr) = run_in(
      &dir,
      &[
        "query",
        "local",
        "SELECT id, name, :note AS note FROM items WHERE id = :target_id",
        "--params-json",
        r#"{"target_id": 7}"#,
        "--param",
        &format!("note={secret_sentinel}"),
      ],
    );
    assert_eq!(code, 0, "{stderr}");
    let body = json_of(&stdout);
    assert_eq!(body["rows"], json!([[7, "item 7", secret_sentinel]]));

    // 2. DuckDB: --param + --param-null
    let (code, stdout, stderr) = run_in(
      &dir,
      &[
        "query",
        "duck",
        "SELECT id, :empty_val AS n FROM metrics WHERE tag = :tag",
        "--param",
        "tag=beta",
        "--param-null",
        "empty_val",
      ],
    );
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(json_of(&stdout)["rows"], json!([[2, null]]));

    // 3. 安全红线：参数值严禁写入审计日志
    let audit_log =
      std::fs::read_to_string(dir.0.join("logs").join(AUDIT_FILE)).unwrap_or_default();
    assert!(audit_log.contains(" query "), "{audit_log}");
    assert!(!audit_log.contains(secret_sentinel), "parameter value leaked into audit log!");

    // 4. 缺参、多参、非法 JSON 与写语句校验
    let (code, _, _) = run_in(&dir, &["query", "local", "SELECT * FROM items WHERE id = :missing"]);
    assert_eq!(code, Exit::Usage as i32);
    let (code, _, _) = run_in(&dir, &["query", "local", "SELECT 1", "--param", "unused=1"]);
    assert_eq!(code, Exit::Usage as i32);
    let (code, _, _) =
      run_in(&dir, &["query", "local", "DELETE FROM items WHERE id = :id", "--param", "id=1"]);
    assert_eq!(code, Exit::Refused as i32);
  }

  /// CM3 只读结构差异门：`cli diff` 识别新增表/缺失表/列与索引变更；任一侧为 PROD 或未开放一律报 not_found
  #[test]
  fn diff_reports_schema_differences_and_enforces_bilateral_access() {
    let dir = ConfigDir::with(&[]);
    let src_db = dir.0.join("src.sqlite");
    let dst_db = dir.0.join("dst.sqlite");
    {
      let rt = tokio::runtime::Runtime::new().expect("runtime");
      rt.block_on(async {
        let src_pool = sqlx::SqlitePool::connect_with(
          sqlx::sqlite::SqliteConnectOptions::new().filename(&src_db).create_if_missing(true),
        )
        .await
        .expect("src sqlite");
        sqlx::raw_sql(
          "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL, role TEXT DEFAULT 'member');
           CREATE UNIQUE INDEX idx_users_email ON users(email);
           CREATE TABLE events (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users(id));",
        )
        .execute(&src_pool)
        .await
        .expect("seed src");
        src_pool.close().await;

        let dst_pool = sqlx::SqlitePool::connect_with(
          sqlx::sqlite::SqliteConnectOptions::new().filename(&dst_db).create_if_missing(true),
        )
        .await
        .expect("dst sqlite");
        sqlx::raw_sql(
          "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT, legacy TEXT);
           CREATE TABLE old_temp (id INTEGER PRIMARY KEY);",
        )
        .execute(&dst_pool)
        .await
        .expect("seed dst");
        dst_pool.close().await;
      });
    }
    let mut hidden_profile = file_profile("h", "hidden", "sqlite", &dst_db, "development");
    hidden_profile["agent_access"] = json!("off");
    dir.write_connections(&[
      file_profile("a", "dev", "sqlite", &src_db, "development"),
      file_profile("b", "staging", "sqlite", &dst_db, "staging"),
      file_profile("p", "prod", "sqlite", &dst_db, "production"),
      hidden_profile,
    ]);

    // 1. JSON 差异报告
    let (code, stdout, stderr) = run_in(&dir, &["diff", "dev", "staging"]);
    assert_eq!(code, 0, "{stderr}");
    let body = json_of(&stdout);
    assert_eq!(body["schema"], json!(1));
    assert_eq!(body["same_dialect"], json!(true));
    assert_eq!(body["summary"]["tablesAdded"], json!(1));
    assert_eq!(body["summary"]["tablesRemoved"], json!(1));
    assert_eq!(body["summary"]["tablesChanged"], json!(1));
    assert_eq!(body["tables_added"], json!(["events"]));
    assert_eq!(body["tables_removed"], json!(["old_temp"]));
    assert_eq!(body["tables_changed"][0]["table"], json!("users"));

    // 2. Markdown 摘要格式
    let (code, stdout, stderr) = run_in(&dir, &["diff", "dev", "staging", "--format", "markdown"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("# Schema Diff: dev → staging"), "{stdout}");
    assert!(stdout.contains("events"), "{stdout}");
    assert!(stdout.contains("old_temp"), "{stdout}");

    // 3. 双边权限门：无论源还是目标是 PROD 或未开放，报错 kind 和文案完全一致（不泄露哪侧存在）
    let (c1, _, e1) = run_in(&dir, &["diff", "dev", "prod"]);
    let (c2, _, e2) = run_in(&dir, &["diff", "prod", "dev"]);
    let (c3, _, e3) = run_in(&dir, &["diff", "dev", "hidden"]);
    for (code, err) in [(c1, &e1), (c2, &e2), (c3, &e3)] {
      assert_eq!(code, Exit::Usage as i32, "{err}");
      assert_eq!(json_of(err)["error"]["kind"], json!("not_found"), "{err}");
    }
    assert_eq!(json_of(&e1)["error"]["message"], json_of(&e2)["error"]["message"]);
    assert_eq!(json_of(&e1)["error"]["message"], json_of(&e3)["error"]["message"]);
  }

  /// CM4 语义备注无侵入门：无侧文件时 dictionary/schema 输出不变；有侧文件时注入备注段与表级说明
  #[test]
  fn connection_semantics_injects_into_dictionary_and_schema_without_breaking_baseline() {
    let dir = ConfigDir::with(&[]);
    let db = seeded_sqlite(&dir.0);
    dir.write_connections(&[file_profile("conn-sem", "local", "sqlite", &db, "development")]);

    // 1. 无侧文件：dictionary 不含语义块，schema 不含 semantics 字段
    let (code, baseline_dict, stderr) = run_in(&dir, &["dictionary", "local"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!baseline_dict.contains("## Business Semantics & Notes"), "{baseline_dict}");

    let (code, baseline_schema, stderr) = run_in(&dir, &["schema", "local", "items"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(json_of(&baseline_schema).get("semantics").is_none(), "{baseline_schema}");

    // 2. 写入 semantics/conn-sem.md 后：dictionary 在物理表之前注入，schema 附带对应表小节
    crate::services::connection_semantics::save_semantics(
      &dir.0,
      "conn-sem",
      "All amounts are in cents.\n\n### items\n- `name` is the customer-facing SKU label.\n",
    )
    .expect("save semantics");

    let (code, enriched_dict, stderr) = run_in(&dir, &["dictionary", "local"]);
    assert_eq!(code, 0, "{stderr}");
    let sem_pos =
      enriched_dict.find("## Business Semantics & Notes").expect("semantics heading in dictionary");
    let table_pos = enriched_dict.find("## items").expect("table heading in dictionary");
    assert!(sem_pos < table_pos, "semantics must appear before physical tables");
    assert!(enriched_dict.contains("All amounts are in cents."));

    let (code, enriched_schema, stderr) = run_in(&dir, &["schema", "local", "items"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
      json_of(&enriched_schema)["semantics"],
      json!("- `name` is the customer-facing SKU label.")
    );
  }

  #[test]
  fn connections_prints_static_hint_on_stderr_only_for_empty_tty_output() {
    let dir = ConfigDir::with(&[
      profile("hidden-1", "dev-closed", "development", "off"),
      profile("prod-1", "prod-secret-db", "production", "read"),
    ]);
    let dirs = Dirs { config: Some(dir.0.clone()), log: None };
    let args = vec!["connections".to_string()];

    // 交互终端 + 空列表：stdout 默认美化并高亮 JSON（去 ANSI 后可解析），stderr 打印静态引导提示
    let (mut out_tty, mut err_tty) = (Vec::new(), Vec::new());
    let code = run_with_tty(&args, &dirs, &mut out_tty, &mut err_tty, true);
    assert_eq!(code, 0);
    let stdout_tty = String::from_utf8_lossy(&out_tty);
    assert!(stdout_tty.contains("\x1b["), "expected ANSI highlight on TTY: {stdout_tty}");
    assert!(stdout_tty.lines().count() > 1, "expected pretty JSON on TTY: {stdout_tty}");
    assert_eq!(json_of(&format::strip_ansi(&stdout_tty))["connections"], json!([]));
    let stderr_tty = format::strip_ansi(&String::from_utf8_lossy(&err_tty));
    assert!(stderr_tty.contains("Read-only"), "{stderr_tty}");
    assert!(!stderr_tty.contains("dev-closed") && !stderr_tty.contains("prod-secret-db"));

    // 管道（非 TTY）+ 空列表：stdout 保持紧凑纯 JSON（零 ANSI），stderr 保持静默
    let (mut out_pipe, mut err_pipe) = (Vec::new(), Vec::new());
    let code = run_with_tty(&args, &dirs, &mut out_pipe, &mut err_pipe, false);
    assert_eq!(code, 0);
    let stdout_pipe = String::from_utf8_lossy(&out_pipe);
    assert!(!stdout_pipe.contains("\x1b["), "{stdout_pipe}");
    assert_eq!(stdout_pipe.lines().count(), 1, "{stdout_pipe}");
    assert!(err_pipe.is_empty());

    // 交互终端 + 有开放连接：不打印空列表提示
    let open_dir = ConfigDir::with(&[profile("open-1", "dev-open", "development", "read")]);
    let open_dirs = Dirs { config: Some(open_dir.0.clone()), log: None };
    let (mut out_open, mut err_open) = (Vec::new(), Vec::new());
    let code = run_with_tty(&args, &open_dirs, &mut out_open, &mut err_open, true);
    assert_eq!(code, 0);
    assert_eq!(
      json_of(&format::strip_ansi(&String::from_utf8_lossy(&out_open)))["connections"][0]["name"],
      json!("dev-open")
    );
    assert!(err_open.is_empty());
  }

  #[test]
  fn cli_output_flags_override_tty_and_pipe_defaults() {
    let dir = ConfigDir::with(&[profile("open-1", "dev-open", "development", "read")]);
    let dirs = Dirs { config: Some(dir.0.clone()), log: None };

    // 终端下 --compact --no-color 强制紧凑无色
    let args = vec!["connections".to_string(), "--compact".to_string(), "--no-color".to_string()];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_with_tty(&args, &dirs, &mut out, &mut err, true), 0);
    let text = String::from_utf8_lossy(&out);
    assert!(!text.contains("\x1b["), "{text}");
    assert_eq!(text.lines().count(), 1, "{text}");
    assert_eq!(json_of(&text)["connections"][0]["name"], json!("dev-open"));

    // 管道下 --pretty --color always 强制美化高亮（便于 `| less -R`）
    let args = vec![
      "connections".to_string(),
      "--pretty".to_string(),
      "--color".to_string(),
      "always".to_string(),
    ];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_with_tty(&args, &dirs, &mut out, &mut err, false), 0);
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("\x1b[1;36m\"connections\"\x1b[0m"), "{text}");
    assert!(text.lines().count() > 5, "{text}");
    assert_eq!(json_of(&format::strip_ansi(&text))["connections"][0]["name"], json!("dev-open"));

    // 终端下错误 JSON 同样美化高亮；管道下紧凑无色
    let bad_args = vec!["unknown-cmd".to_string()];
    let (mut out_tty, mut err_tty) = (Vec::new(), Vec::new());
    assert_eq!(
      run_with_tty(&bad_args, &dirs, &mut out_tty, &mut err_tty, true),
      Exit::Usage as i32
    );
    let err_tty_text = String::from_utf8_lossy(&err_tty);
    assert!(err_tty_text.contains("\x1b["), "{err_tty_text}");
    assert!(err_tty_text.lines().count() > 2, "{err_tty_text}");
    assert_eq!(json_of(&format::strip_ansi(&err_tty_text))["error"]["kind"], json!("usage"));
  }

  #[test]
  fn mcp_session_guard_exposes_shared_tunnels_and_clears_on_drop() {
    assert!(shared_mcp_tunnels().is_none());
    {
      let _guard = McpSessionGuard::enter().expect("mcp guard");
      let first = shared_mcp_tunnels().expect("shared tunnels active");
      let second = shared_mcp_tunnels().expect("same shared tunnels");
      assert!(Arc::ptr_eq(&first, &second));
    }
    assert!(shared_mcp_tunnels().is_none());
  }
}
