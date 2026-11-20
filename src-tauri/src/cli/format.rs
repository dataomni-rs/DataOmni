//! CLI 终端输出格式化与 ANSI 语法高亮。
//!
//! 默认策略遵循 Unix CLI 惯例：
//! - 交互终端（TTY）：2 空格缩进 Pretty JSON + ANSI 语法高亮（Markdown 输出也轻度高亮）；
//! - 管道 / 重定向 / Agent 捕获（非 TTY）：保持紧凑单行 JSON、零 ANSI 转义符，与旧版完全字节兼容；
//! - 支持 `--pretty` / `--compact` (`-c`) / `--color <auto|always|never>` / `--no-color`，
//!   以及 `NO_COLOR`、`CLICOLOR`、`CLICOLOR_FORCE`、`FORCE_COLOR`、`TERM=dumb`。

use super::CliError;
use serde_json::Value;
use std::io::{self, Write};

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_KEY: &str = "\x1b[1;36m";
const ANSI_STRING: &str = "\x1b[32m";
const ANSI_NUMBER: &str = "\x1b[33m";
const ANSI_BOOL: &str = "\x1b[35m";
const ANSI_DIM: &str = "\x1b[2m";
const ANSI_HEADING: &str = "\x1b[1;36m";
const ANSI_HINT: &str = "\x1b[33m";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum JsonLayout {
  #[default]
  Auto,
  Pretty,
  Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ColorMode {
  #[default]
  Auto,
  Always,
  Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct OutputOptions {
  pub layout: JsonLayout,
  pub color: ColorMode,
}

/// 终端色彩相关的环境变量快照，独立抽取以便单测确定性覆盖，不改进程全局 env
#[derive(Debug, Clone, Default)]
pub(super) struct TermEnv {
  pub no_color: Option<String>,
  pub clicolor: Option<String>,
  pub clicolor_force: Option<String>,
  pub force_color: Option<String>,
  pub term: Option<String>,
}

impl TermEnv {
  pub fn from_process() -> Self {
    Self {
      no_color: std::env::var("NO_COLOR").ok(),
      clicolor: std::env::var("CLICOLOR").ok(),
      clicolor_force: std::env::var("CLICOLOR_FORCE").ok(),
      force_color: std::env::var("FORCE_COLOR").ok(),
      term: std::env::var("TERM").ok(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResolvedStyle {
  pub pretty: bool,
  pub color: bool,
}

impl OutputOptions {
  pub fn resolve(self, stream_is_tty: bool, env: &TermEnv) -> ResolvedStyle {
    let pretty = match self.layout {
      JsonLayout::Auto => stream_is_tty,
      JsonLayout::Pretty => true,
      JsonLayout::Compact => false,
    };
    let color = match self.color {
      ColorMode::Always => true,
      ColorMode::Never => false,
      ColorMode::Auto => auto_color_enabled(stream_is_tty, env),
    };
    ResolvedStyle { pretty, color }
  }
}

fn auto_color_enabled(stream_is_tty: bool, env: &TermEnv) -> bool {
  // no-color.org 规范：非空 NO_COLOR 一律关闭彩色
  if env.no_color.as_deref().is_some_and(|v| !v.is_empty()) {
    return false;
  }
  let forced = [&env.clicolor_force, &env.force_color]
    .into_iter()
    .any(|var| var.as_deref().is_some_and(|v| !v.is_empty() && v != "0"));
  if forced {
    return true;
  }
  if env.clicolor.as_deref() == Some("0") || env.term.as_deref() == Some("dumb") {
    return false;
  }
  stream_is_tty
}

/// 这些子命令选项会消费紧随其后的一个参数，抽取全局输出开关时必须原样跳过其值
fn takes_following_value(flag: &str) -> bool {
  matches!(
    flag,
    "--limit"
      | "--timeout"
      | "--param"
      | "--param-null"
      | "--params-json"
      | "--format"
      | "--schema"
      | "--out"
      | "--delimiter"
      | "--null"
      | "--file"
      | "--database"
      | "--db"
      | "--rows"
      | "--sort"
      | "--skip"
      | "--pipeline"
      | "--type"
      | "--cursor"
      | "--position"
  )
}

fn parse_color_mode(raw: &str) -> Result<ColorMode, CliError> {
  match raw {
    "auto" => Ok(ColorMode::Auto),
    "always" => Ok(ColorMode::Always),
    "never" => Ok(ColorMode::Never),
    _ => Err(CliError::usage(format!("--color must be auto, always, or never, got {raw}"))),
  }
}

/// 从 CLI 参数中剥离全局输出格式开关，保留子命令参数与透传参数不变。
pub(super) fn extract_output_options(
  args: &[String],
) -> Result<(OutputOptions, Vec<String>), CliError> {
  let mut options = OutputOptions::default();
  let mut remaining = Vec::with_capacity(args.len());
  let mut iter = args.iter();
  let mut subcommand: Option<&str> = None;
  let mut redis_positionals = 0usize;

  while let Some(arg) = iter.next() {
    if arg == "--" {
      remaining.push(arg.clone());
      remaining.extend(iter.cloned());
      break;
    }

    match arg.as_str() {
      "--pretty" => {
        options.layout = JsonLayout::Pretty;
        continue;
      }
      "--compact" | "-c" => {
        options.layout = JsonLayout::Compact;
        continue;
      }
      "--no-color" => {
        options.color = ColorMode::Never;
        continue;
      }
      "--color" => {
        let Some(value) = iter.next() else {
          return Err(CliError::usage("--color requires auto, always, or never"));
        };
        options.color = parse_color_mode(value)?;
        continue;
      }
      _ => {
        if let Some(value) = arg.strip_prefix("--color=") {
          options.color = parse_color_mode(value)?;
          continue;
        }
      }
    }

    remaining.push(arg.clone());
    if takes_following_value(arg) {
      if let Some(next) = iter.next() {
        remaining.push(next.clone());
      }
      continue;
    }

    if !arg.starts_with('-') {
      if subcommand.is_none() {
        subcommand = Some(arg.as_str());
      } else if subcommand == Some("redis") {
        redis_positionals += 1;
        // `redis <connection> [flags] command <name> [raw-args...]`：
        // `command` 之后全部透传给 Redis，不再拦截任何形如 `--compact` / `-c` 的参数
        if redis_positionals == 2 && arg == "command" {
          remaining.extend(iter.cloned());
          break;
        }
      }
    }
  }

  Ok((options, remaining))
}

/// 将 JSON 值按指定布局与色彩写入输出流并追加换行。
/// 当 `pretty == false && color == false` 时直出紧凑 JSON，与旧版逐字节一致。
pub(super) fn write_json_line(
  out: &mut dyn Write,
  value: &Value,
  style: ResolvedStyle,
) -> io::Result<()> {
  if !style.pretty && !style.color {
    serde_json::to_writer(&mut *out, value).map_err(io::Error::other)?;
    return out.write_all(b"\n");
  }
  write_json_value(out, value, style, 0)?;
  out.write_all(b"\n")
}

fn write_json_value(
  out: &mut dyn Write,
  value: &Value,
  style: ResolvedStyle,
  depth: usize,
) -> io::Result<()> {
  match value {
    Value::Null => write_styled(out, "null", ANSI_DIM, style.color),
    Value::Bool(flag) => {
      write_styled(out, if *flag { "true" } else { "false" }, ANSI_BOOL, style.color)
    }
    Value::Number(num) => {
      if style.color {
        out.write_all(ANSI_NUMBER.as_bytes())?;
        write!(out, "{num}")?;
        out.write_all(ANSI_RESET.as_bytes())
      } else {
        write!(out, "{num}")
      }
    }
    Value::String(text) => {
      if style.color {
        out.write_all(ANSI_STRING.as_bytes())?;
        serde_json::to_writer(&mut *out, text).map_err(io::Error::other)?;
        out.write_all(ANSI_RESET.as_bytes())
      } else {
        serde_json::to_writer(&mut *out, text).map_err(io::Error::other)
      }
    }
    Value::Array(items) => {
      if items.is_empty() {
        return write_styled(out, "[]", ANSI_DIM, style.color);
      }
      write_styled(out, "[", ANSI_DIM, style.color)?;
      if style.pretty {
        out.write_all(b"\n")?;
      }
      for (index, item) in items.iter().enumerate() {
        if index > 0 {
          write_styled(out, ",", ANSI_DIM, style.color)?;
          if style.pretty {
            out.write_all(b"\n")?;
          }
        }
        if style.pretty {
          write_indent(out, depth + 1)?;
        }
        write_json_value(out, item, style, depth + 1)?;
      }
      if style.pretty {
        out.write_all(b"\n")?;
        write_indent(out, depth)?;
      }
      write_styled(out, "]", ANSI_DIM, style.color)
    }
    Value::Object(map) => {
      if map.is_empty() {
        return write_styled(out, "{}", ANSI_DIM, style.color);
      }
      write_styled(out, "{", ANSI_DIM, style.color)?;
      if style.pretty {
        out.write_all(b"\n")?;
      }
      for (index, (key, item)) in map.iter().enumerate() {
        if index > 0 {
          write_styled(out, ",", ANSI_DIM, style.color)?;
          if style.pretty {
            out.write_all(b"\n")?;
          }
        }
        if style.pretty {
          write_indent(out, depth + 1)?;
        }
        if style.color {
          out.write_all(ANSI_KEY.as_bytes())?;
          serde_json::to_writer(&mut *out, key).map_err(io::Error::other)?;
          out.write_all(ANSI_RESET.as_bytes())?;
        } else {
          serde_json::to_writer(&mut *out, key).map_err(io::Error::other)?;
        }
        write_styled(out, if style.pretty { ": " } else { ":" }, ANSI_DIM, style.color)?;
        write_json_value(out, item, style, depth + 1)?;
      }
      if style.pretty {
        out.write_all(b"\n")?;
        write_indent(out, depth)?;
      }
      write_styled(out, "}", ANSI_DIM, style.color)
    }
  }
}

fn write_styled(out: &mut dyn Write, text: &str, ansi: &str, color: bool) -> io::Result<()> {
  if color {
    out.write_all(ansi.as_bytes())?;
    out.write_all(text.as_bytes())?;
    out.write_all(ANSI_RESET.as_bytes())
  } else {
    out.write_all(text.as_bytes())
  }
}

fn write_indent(out: &mut dyn Write, depth: usize) -> io::Result<()> {
  const SPACES: &[u8] = b"                                ";
  let mut remaining = depth * 2;
  while remaining > 0 {
    let chunk = remaining.min(SPACES.len());
    out.write_all(&SPACES[..chunk])?;
    remaining -= chunk;
  }
  Ok(())
}

/// 轻度高亮 Markdown（`dictionary`、`diff --format markdown`）。
/// 关闭彩色时原样输出；开启彩色时剥离 ANSI 后与原文逐字节等价。
pub(super) fn write_markdown(out: &mut dyn Write, text: &str, color: bool) -> io::Result<()> {
  if !color {
    return out.write_all(text.as_bytes());
  }
  let mut in_fence = false;
  for raw_line in text.split_inclusive('\n') {
    let (line, ending) = match raw_line.strip_suffix("\r\n") {
      Some(stripped) => (stripped, "\r\n"),
      None => match raw_line.strip_suffix('\n') {
        Some(stripped) => (stripped, "\n"),
        None => (raw_line, ""),
      },
    };
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
      in_fence = !in_fence;
      write_styled(out, line, ANSI_DIM, true)?;
    } else if in_fence {
      write_styled(out, line, ANSI_STRING, true)?;
    } else if is_markdown_heading(trimmed) {
      write_styled(out, line, ANSI_HEADING, true)?;
    } else if trimmed.starts_with('>') || is_markdown_table_rule(trimmed) {
      write_styled(out, line, ANSI_DIM, true)?;
    } else {
      write_markdown_inline(out, line, trimmed.starts_with('|'))?;
    }
    out.write_all(ending.as_bytes())?;
  }
  Ok(())
}

fn is_markdown_heading(trimmed: &str) -> bool {
  let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
  (1..=6).contains(&hashes) && trimmed.as_bytes().get(hashes) == Some(&b' ')
}

fn is_markdown_table_rule(trimmed: &str) -> bool {
  trimmed.starts_with('|')
    && trimmed.contains('-')
    && trimmed.bytes().all(|b| matches!(b, b'|' | b'-' | b':' | b' '))
}

fn write_markdown_inline(out: &mut dyn Write, line: &str, is_table_row: bool) -> io::Result<()> {
  let bytes = line.as_bytes();
  let mut cursor = 0usize;
  let mut pos = 0usize;
  while pos < bytes.len() {
    match bytes[pos] {
      b'`' => {
        if let Some(rel_end) = line[pos + 1..].find('`') {
          let end = pos + 1 + rel_end + 1;
          out.write_all(&bytes[cursor..pos])?;
          write_styled(out, &line[pos..end], ANSI_STRING, true)?;
          pos = end;
          cursor = end;
          continue;
        }
      }
      b'|' if is_table_row => {
        out.write_all(&bytes[cursor..pos])?;
        write_styled(out, "|", ANSI_DIM, true)?;
        pos += 1;
        cursor = pos;
        continue;
      }
      _ => {}
    }
    pos += 1;
  }
  out.write_all(&bytes[cursor..])
}

/// 终端交互引导提示（stderr）：彩色终端下柔和黄字，非彩色原样输出
pub(super) fn write_hint_line(out: &mut dyn Write, text: &str, color: bool) -> io::Result<()> {
  write_styled(out, text, ANSI_HINT, color)?;
  out.write_all(b"\n")
}

#[cfg(test)]
pub(super) fn strip_ansi(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let mut chars = input.chars().peekable();
  while let Some(ch) = chars.next() {
    if ch == '\x1b' && chars.peek() == Some(&'[') {
      let _ = chars.next();
      for seq in chars.by_ref() {
        if seq.is_ascii_alphabetic() {
          break;
        }
      }
    } else {
      out.push(ch);
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
  }

  #[test]
  fn tty_defaults_to_pretty_and_colored_while_pipe_stays_compact_plain() {
    let env = TermEnv::default();
    assert_eq!(
      OutputOptions::default().resolve(true, &env),
      ResolvedStyle { pretty: true, color: true }
    );
    assert_eq!(
      OutputOptions::default().resolve(false, &env),
      ResolvedStyle { pretty: false, color: false }
    );
  }

  #[test]
  fn env_vars_follow_no_color_and_clicolor_conventions() {
    let no_color = TermEnv { no_color: Some("1".into()), ..TermEnv::default() };
    assert!(!OutputOptions::default().resolve(true, &no_color).color);

    let empty_no_color = TermEnv { no_color: Some(String::new()), ..TermEnv::default() };
    assert!(OutputOptions::default().resolve(true, &empty_no_color).color);

    let forced = TermEnv { clicolor_force: Some("1".into()), ..TermEnv::default() };
    assert!(OutputOptions::default().resolve(false, &forced).color);
    assert!(!OutputOptions::default().resolve(false, &forced).pretty);

    // NO_COLOR 优先级高于 CLICOLOR_FORCE，除非命令行显式 --color always
    let both = TermEnv {
      no_color: Some("1".into()),
      clicolor_force: Some("1".into()),
      ..TermEnv::default()
    };
    assert!(!OutputOptions::default().resolve(true, &both).color);
    let always = OutputOptions { color: ColorMode::Always, ..OutputOptions::default() };
    assert!(always.resolve(false, &both).color);

    let dumb = TermEnv { term: Some("dumb".into()), ..TermEnv::default() };
    assert!(!OutputOptions::default().resolve(true, &dumb).color);
  }

  #[test]
  fn json_pretty_and_highlighted_round_trips_to_identical_value() {
    let payload = json!({
      "schema": 1,
      "connection": "claw",
      "escaped": "line1\n\"quoted\"\\中文",
      "count": 42,
      "ok": true,
      "empty_obj": {},
      "empty_arr": [],
      "nullable": null,
      "objects": [
        { "schema": "xiaohongshu2", "name": "users", "kind": "table" }
      ]
    });

    // 非 TTY 紧凑输出与 serde_json 逐字节一致
    let mut compact_plain = Vec::new();
    write_json_line(&mut compact_plain, &payload, ResolvedStyle { pretty: false, color: false })
      .expect("compact");
    assert_eq!(String::from_utf8(compact_plain).expect("utf8"), format!("{payload}\n"));

    // TTY 格式化 + 高亮：含 ANSI 码与多行缩进，去 ANSI 后可完整解析回原值
    let mut pretty_color = Vec::new();
    write_json_line(&mut pretty_color, &payload, ResolvedStyle { pretty: true, color: true })
      .expect("pretty color");
    let rendered = String::from_utf8(pretty_color).expect("utf8");
    assert!(rendered.contains("\x1b[1;36m\"objects\"\x1b[0m"), "{rendered}");
    assert!(rendered.contains("\x1b[32m\"users\"\x1b[0m"), "{rendered}");
    assert!(rendered.contains("\x1b[33m42\x1b[0m"), "{rendered}");
    assert!(rendered.contains("\x1b[35mtrue\x1b[0m"), "{rendered}");
    assert!(rendered.contains("\x1b[2mnull\x1b[0m"), "{rendered}");
    assert!(rendered.lines().count() > 10, "{rendered}");

    let stripped = strip_ansi(&rendered);
    assert!(stripped.contains("\"empty_obj\": {}"), "{stripped}");
    assert!(stripped.contains("\"empty_arr\": []"), "{stripped}");
    let reparsed: Value = serde_json::from_str(&stripped).expect("valid json after strip_ansi");
    assert_eq!(reparsed, payload);
  }

  #[test]
  fn markdown_highlighting_preserves_exact_text_after_stripping_ansi() {
    let md = "# Data Dictionary\n\n> Generated by DataOmni\n\n| Column | Type |\n| --- | --- |\n| `id` | `bigint` |\n\n```sql\nSELECT 1;\n```\n";
    let mut plain = Vec::new();
    write_markdown(&mut plain, md, false).expect("plain md");
    assert_eq!(String::from_utf8(plain).expect("utf8"), md);

    let mut colored = Vec::new();
    write_markdown(&mut colored, md, true).expect("colored md");
    let rendered = String::from_utf8(colored).expect("utf8");
    assert!(rendered.contains("\x1b[1;36m# Data Dictionary\x1b[0m"), "{rendered}");
    assert!(rendered.contains("\x1b[32m`id`\x1b[0m"), "{rendered}");
    assert_eq!(strip_ansi(&rendered), md);
  }

  #[test]
  fn extract_output_options_strips_global_flags_without_eating_values_or_redis_passthrough() {
    let (opts, rest) =
      extract_output_options(&s(&["schema", "claw", "--compact", "--color", "always"]))
        .expect("parse");
    assert_eq!(opts, OutputOptions { layout: JsonLayout::Compact, color: ColorMode::Always });
    assert_eq!(rest, s(&["schema", "claw"]));

    let (opts, rest) =
      extract_output_options(&s(&["--pretty", "--no-color", "connections"])).expect("parse");
    assert_eq!(opts, OutputOptions { layout: JsonLayout::Pretty, color: ColorMode::Never });
    assert_eq!(rest, s(&["connections"]));

    // 子命令带值参数不会被误识别为全局开关
    let (opts, rest) = extract_output_options(&s(&[
      "query",
      "claw",
      "SELECT 1",
      "--param",
      "k=--compact",
      "--color=never",
    ]))
    .expect("parse");
    assert_eq!(opts.color, ColorMode::Never);
    assert_eq!(rest, s(&["query", "claw", "SELECT 1", "--param", "k=--compact"]));

    // redis command 后的原始参数原样透传
    let (opts, rest) =
      extract_output_options(&s(&["redis", "r", "--db", "2", "command", "GET", "--compact", "-c"]))
        .expect("parse");
    assert_eq!(opts, OutputOptions::default());
    assert_eq!(rest, s(&["redis", "r", "--db", "2", "command", "GET", "--compact", "-c"]));

    // 非法 --color 值报 usage 错
    assert!(extract_output_options(&s(&["connections", "--color", "rainbow"])).is_err());
    assert!(extract_output_options(&s(&["connections", "--color"])).is_err());
  }
}
