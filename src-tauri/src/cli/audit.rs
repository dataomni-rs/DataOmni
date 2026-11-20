//! 碰库的每次调用写一行（`rfcs/agent-cli.md` §5.3）。
//!
//! **不记语句原文**：字面量里可能有个人数据，而日志里本来就不许出现敏感字段。记第一个
//! 关键字、长度和 SHA-256，够和 Agent 那一侧的记录对上。单独一个文件：应用开着时
//! `dataomni.log` 归 `tauri-plugin-log` 轮转，两个进程一起写同一个文件说不清谁先谁后。

use super::CliError;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

pub(crate) const AUDIT_FILE: &str = "dataomni-cli.log";
/// 和应用日志同一个上限。超过就把旧的挪成 `.1`，只留一份
const MAX_BYTES: u64 = 2 * 1024 * 1024;
/// GUI 审计查看器单次最多返回的记录数
pub(crate) const MAX_AUDIT_ENTRIES: usize = 200;

/// 从 `dataomni-cli.log` 只读解析出的单条审计记录。
/// 不包含 SQL 原文，仅包含落盘时的元数据摘要。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
  pub timestamp: String,
  pub command: String,
  pub connection: String,
  pub first_keyword: String,
  pub length: usize,
  pub sha256: String,
  pub result: String,
  pub ms: u64,
}

pub(super) fn record<T>(
  log_dir: Option<&Path>,
  command: &str,
  connection: &str,
  sql: &str,
  outcome: &Result<T, CliError>,
  elapsed: Duration,
) {
  let Some(log_dir) = log_dir else { return };
  let digest = Sha256::digest(sql.as_bytes());
  let hash: String = digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
  let first = sql.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
  let first: String = first.chars().filter(char::is_ascii_alphabetic).take(16).collect();
  let result = match outcome {
    Ok(_) => "ok".to_string(),
    Err(error) => error.kind.to_string(),
  };
  let line = format!(
    "{} {command} connection={connection:?} first={first} length={} sha256={hash} result={result} ms={}\n",
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
    sql.len(),
    elapsed.as_millis(),
  );
  // 写不进日志不该让查询失败：留痕是给人事后看的，不是这次调用的一部分
  let _ = append(log_dir, &line);
}

fn append(log_dir: &Path, line: &str) -> std::io::Result<()> {
  std::fs::create_dir_all(log_dir)?;
  let path = log_dir.join(AUDIT_FILE);
  if std::fs::metadata(&path).map(|meta| meta.len() > MAX_BYTES).unwrap_or(false) {
    std::fs::rename(&path, log_dir.join(format!("{AUDIT_FILE}.1")))?;
  }
  let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
  file.write_all(line.as_bytes())
}

/// 解析 Rust `{:?}` 风格的双引号字符串（支持 `\"`、`\\`、`\n`、`\r`、`\t`、`\u{...}`），
/// 返回解码后的字符串与剩余后缀。
fn parse_debug_quoted(input: &str) -> Option<(String, &str)> {
  let mut chars = input.char_indices();
  if chars.next()?.1 != '"' {
    return None;
  }
  let mut out = String::new();
  while let Some((index, ch)) = chars.next() {
    match ch {
      '"' => return Some((out, &input[index + 1..])),
      '\\' => {
        let (_, escaped) = chars.next()?;
        match escaped {
          '"' => out.push('"'),
          '\'' => out.push('\''),
          '\\' => out.push('\\'),
          'n' => out.push('\n'),
          'r' => out.push('\r'),
          't' => out.push('\t'),
          '0' => out.push('\0'),
          'u' => {
            if chars.next()?.1 != '{' {
              return None;
            }
            let mut hex = String::new();
            let mut closed = false;
            for (_, hex_ch) in chars.by_ref() {
              if hex_ch == '}' {
                closed = true;
                break;
              }
              hex.push(hex_ch);
            }
            if !closed || hex.is_empty() || hex.len() > 6 {
              return None;
            }
            let code = u32::from_str_radix(&hex, 16).ok()?;
            out.push(char::from_u32(code)?);
          }
          _ => return None,
        }
      }
      other => out.push(other),
    }
  }
  None
}

/// 只读解析 `dataomni-cli.log` 中的单行记录；损坏或截断行直接返回 `None`。
pub(crate) fn parse_audit_line(line: &str) -> Option<AuditLogEntry> {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return None;
  }
  let (timestamp, rest) = trimmed.split_once(char::is_whitespace)?;
  let rest = rest.trim_start();
  let (command, rest) = rest.split_once(char::is_whitespace)?;
  let rest = rest.trim_start();
  let after_conn_prefix = rest.strip_prefix("connection=")?;
  let (connection, tail) = parse_debug_quoted(after_conn_prefix)?;

  let mut first_keyword: Option<String> = None;
  let mut length: Option<usize> = None;
  let mut sha256: Option<String> = None;
  let mut result: Option<String> = None;
  let mut ms: Option<u64> = None;

  for token in tail.split_whitespace() {
    let Some((key, value)) = token.split_once('=') else { continue };
    match key {
      "first" => first_keyword = Some(value.to_string()),
      "length" => length = value.parse::<usize>().ok(),
      "sha256" => sha256 = Some(value.to_string()),
      "result" => result = Some(value.to_string()),
      "ms" => ms = value.parse::<u64>().ok(),
      _ => {}
    }
  }

  Some(AuditLogEntry {
    timestamp: timestamp.to_string(),
    command: command.to_string(),
    connection,
    first_keyword: first_keyword?,
    length: length?,
    sha256: sha256?,
    result: result?,
    ms: ms?,
  })
}

/// 从 `log_dir` 只读读取最近的 `dataomni-cli.log`（及轮转后的 `.1`）审计记录，
/// 按倒序（最新在前）返回，最多 `MAX_AUDIT_ENTRIES` 条；文件不存在时返回空列表。
pub(crate) fn read_audit_log_from(
  log_dir: &Path,
  limit: usize,
) -> Result<Vec<AuditLogEntry>, String> {
  let cap = limit.clamp(1, MAX_AUDIT_ENTRIES);
  let mut entries = Vec::new();
  let candidates = [log_dir.join(AUDIT_FILE), log_dir.join(format!("{AUDIT_FILE}.1"))];

  for path in candidates {
    if entries.len() >= cap {
      break;
    }
    let content = match std::fs::read_to_string(&path) {
      Ok(text) => text,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
      Err(error) => return Err(format!("DATAOMNI_AUDIT_LOG_READ_FAILED: {error}")),
    };
    for line in content.lines().rev() {
      if let Some(entry) = parse_audit_line(line) {
        entries.push(entry);
        if entries.len() >= cap {
          break;
        }
      }
    }
  }

  Ok(entries)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cli::Exit;

  struct TestDir(std::path::PathBuf);

  impl TestDir {
    fn new(tag: &str) -> Self {
      let path =
        std::env::temp_dir().join(format!("dataomni-audit-{tag}-{}", uuid::Uuid::new_v4()));
      std::fs::create_dir_all(&path).expect("create temp dir");
      Self(path)
    }

    fn path(&self) -> &Path {
      &self.0
    }
  }

  impl Drop for TestDir {
    fn drop(&mut self) {
      let _ = std::fs::remove_dir_all(&self.0);
    }
  }

  #[test]
  fn round_trips_recorded_lines_with_quotes_spaces_and_unicode() {
    let dir = TestDir::new("roundtrip");
    let ok_outcome: Result<(), CliError> = Ok(());
    let refused_outcome: Result<(), CliError> = Err(CliError {
      exit: Exit::Refused,
      kind: "refused",
      message: "write".into(),
      diagnosis: None,
    });

    record(
      Some(dir.path()),
      "query",
      "dev \"orders\" \\ 研发库",
      "SELECT count(*) FROM orders",
      &ok_outcome,
      Duration::from_millis(42),
    );
    record(
      Some(dir.path()),
      "schema",
      "local sqlite",
      "",
      &refused_outcome,
      Duration::from_millis(3),
    );

    let entries = read_audit_log_from(dir.path(), 50).expect("read audit log");
    assert_eq!(entries.len(), 2);
    // 倒序：最新的在最前
    assert_eq!(entries[0].command, "schema");
    assert_eq!(entries[0].connection, "local sqlite");
    assert_eq!(entries[0].first_keyword, "");
    assert_eq!(entries[0].length, 0);
    assert_eq!(entries[0].result, "refused");
    assert_eq!(entries[0].ms, 3);

    assert_eq!(entries[1].command, "query");
    assert_eq!(entries[1].connection, "dev \"orders\" \\ 研发库");
    assert_eq!(entries[1].first_keyword, "SELECT");
    assert_eq!(entries[1].length, "SELECT count(*) FROM orders".len());
    assert_eq!(entries[1].result, "ok");
    assert_eq!(entries[1].ms, 42);
    assert_eq!(entries[1].sha256.len(), 16);
  }

  #[test]
  fn skips_malformed_or_truncated_lines_without_failing() {
    let dir = TestDir::new("malformed");
    let raw = concat!(
      "not a valid line\n",
      "2026-11-20T10:00:00+08:00 query connection=\"unclosed\n",
      "2026-11-20T10:00:01+08:00 query connection=\"dev db\" first=SELECT length=10 sha256=0123456789abcdef result=ok ms=12\n",
      "\n",
      "2026-11-20T10:00:02+08:00 query connection=\"broken\" first=SELECT length=abc sha256=0123456789abcdef result=ok ms=1\n",
    );
    std::fs::write(dir.path().join(AUDIT_FILE), raw).expect("write log");

    let entries = read_audit_log_from(dir.path(), 200).expect("read audit log");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].connection, "dev db");
    assert_eq!(entries[0].result, "ok");
    assert_eq!(entries[0].ms, 12);
  }

  #[test]
  fn returns_empty_when_log_file_is_missing_and_respects_limit_across_rotation() {
    let dir = TestDir::new("rotation");
    assert!(read_audit_log_from(dir.path(), 20).expect("missing file").is_empty());

    std::fs::write(
      dir.path().join(format!("{AUDIT_FILE}.1")),
      "2026-11-20T09:00:00+08:00 query connection=\"old\" first=SELECT length=8 sha256=aaaaaaaaaaaaaaaa result=ok ms=5\n",
    )
    .expect("write rotated log");
    std::fs::write(
      dir.path().join(AUDIT_FILE),
      "2026-11-20T10:00:00+08:00 query connection=\"new\" first=SELECT length=8 sha256=bbbbbbbbbbbbbbbb result=database ms=9\n",
    )
    .expect("write current log");

    let all = read_audit_log_from(dir.path(), 200).expect("read both files");
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].connection, "new");
    assert_eq!(all[1].connection, "old");

    let capped = read_audit_log_from(dir.path(), 1).expect("read capped");
    assert_eq!(capped.len(), 1);
    assert_eq!(capped[0].connection, "new");
  }
}
