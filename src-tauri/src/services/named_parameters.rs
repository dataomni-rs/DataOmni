//! 命名绑定参数 `:name` 扫描与各方言占位符改写（`rfcs/bind-parameters.md`、`rfcs/cli-mcp-usability.md` CM3）。
//!
//! 与前端 `src/utils/sqlStatements.ts`（`sqlParameters`）和 `src/utils/queryParameters.ts`（`bindParameters`）
//! 保持逐字节一致，由 `fixtures/bind-parameters-conformance.json` 共同钉住。

use crate::models::DatabaseType;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedParameter {
  pub name: String,
  /// 冒号在原 SQL 中的字节偏移
  pub from: usize,
  /// 参数名结束后的字节偏移
  pub to: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamedParameterError {
  UnsupportedDialect,
  MissingParameters(Vec<String>),
  UnusedParameters(Vec<String>),
}

impl std::fmt::Display for NamedParameterError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::UnsupportedDialect => {
        write!(f, "named :parameters are not supported on this database dialect")
      }
      Self::MissingParameters(names) => {
        write!(f, "missing value for parameter(s): {}", names.join(", "))
      }
      Self::UnusedParameters(names) => {
        write!(f, "parameter(s) not referenced in SQL: {}", names.join(", "))
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewrittenQuery {
  pub sql: String,
  pub params: Vec<JsonValue>,
  pub names: Vec<String>,
}

enum LexerState {
  Normal,
  LineComment,
  BlockComment { depth: usize },
  Quote { quote: u8, backslash_escapes: bool },
  Bracket,
  DollarQuote { tag: String },
  QQuote { terminator: String },
}

fn nests_block_comments(dialect: &DatabaseType) -> bool {
  !matches!(dialect, DatabaseType::MySQL | DatabaseType::SQLite | DatabaseType::Oracle)
}

fn is_identifier_word_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'_'
}

fn prev_byte(bytes: &[u8], index: usize) -> Option<u8> {
  if index == 0 {
    None
  } else {
    bytes.get(index - 1).copied()
  }
}

fn is_escape_string_prefix(bytes: &[u8], quote_index: usize) -> bool {
  let Some(prev) = prev_byte(bytes, quote_index) else {
    return false;
  };
  if !matches!(prev, b'e' | b'E') {
    return false;
  }
  match prev_byte(bytes, quote_index - 1) {
    Some(before) => !(is_identifier_word_byte(before) || before == b'$'),
    None => true,
  }
}

fn starts_dash_comment(bytes: &[u8], index: usize, dialect: &DatabaseType) -> bool {
  if bytes.get(index..index + 2) != Some(b"--") {
    return false;
  }
  if *dialect != DatabaseType::MySQL {
    return true;
  }
  match bytes.get(index + 2) {
    None => true,
    Some(&next) => next <= 0x20,
  }
}

fn match_dollar_quote_tag(sql: &str, index: usize) -> Option<String> {
  let bytes = sql.as_bytes();
  if bytes.get(index) != Some(&b'$') {
    return None;
  }
  let mut cursor = index + 1;
  if let Some(&first) = bytes.get(cursor) {
    if first == b'$' {
      return Some("$$".to_string());
    }
    if first.is_ascii_alphabetic() || first == b'_' {
      cursor += 1;
      while let Some(&b) = bytes.get(cursor) {
        if is_identifier_word_byte(b) {
          cursor += 1;
        } else {
          break;
        }
      }
      if bytes.get(cursor) == Some(&b'$') {
        return Some(sql[index..=cursor].to_string());
      }
    }
  }
  None
}

fn match_oracle_q_quote(sql: &str, index: usize) -> Option<(usize, String)> {
  let bytes = sql.as_bytes();
  // 先按 ASCII 首字节过滤：主循环按字节推进，遇到多字节 UTF-8 后续字节时绝不能切 &sql[index..]
  if !matches!(bytes.get(index), Some(b'q' | b'Q' | b'n' | b'N')) {
    return None;
  }
  if let Some(prev) = prev_byte(bytes, index) {
    if is_identifier_word_byte(prev) || prev == b'$' || prev == b'#' {
      return None;
    }
  }
  let rest = &sql[index..];
  let mut chars = rest.char_indices();
  let (_, c0) = chars.next()?;
  let (_, c1) = chars.next()?;
  let (quote_char, delim_start_char) = if matches!(c0, 'n' | 'N') && matches!(c1, 'q' | 'Q') {
    let (_, c2) = chars.next()?;
    let (pos3, c3) = chars.next()?;
    (c2, (pos3, c3))
  } else if matches!(c0, 'q' | 'Q') {
    let (pos2, c2) = chars.next()?;
    (c1, (pos2, c2))
  } else {
    return None;
  };
  if quote_char != '\'' {
    return None;
  }
  let (delim_pos, delim) = delim_start_char;
  if delim.is_whitespace() {
    return None;
  }
  let closer = match delim {
    '[' => ']',
    '{' => '}',
    '(' => ')',
    '<' => '>',
    other => other,
  };
  let opening_len = delim_pos + delim.len_utf8();
  Some((opening_len, format!("{closer}'")))
}

fn match_parameter(sql: &str, index: usize) -> Option<NamedParameter> {
  let bytes = sql.as_bytes();
  if bytes.get(index) != Some(&b':') {
    return None;
  }
  if let Some(prev) = prev_byte(bytes, index) {
    if is_identifier_word_byte(prev) || prev == b'$' || prev == b':' {
      return None;
    }
  }
  let name_start = index + 1;
  let first = *bytes.get(name_start)?;
  if !(first.is_ascii_alphabetic() || first == b'_') {
    return None;
  }
  let mut end = name_start + 1;
  while let Some(&b) = bytes.get(end) {
    if is_identifier_word_byte(b) {
      end += 1;
    } else {
      break;
    }
  }
  if bytes.get(end) == Some(&b'.') {
    return None;
  }
  Some(NamedParameter { name: sql[name_start..end].to_string(), from: index, to: end })
}

/// 扫描语句中出现的所有 `:name` 命名参数（按出现次序）。
pub fn scan_parameters(sql: &str, dialect: &DatabaseType) -> Vec<NamedParameter> {
  let bytes = sql.as_bytes();
  let mut parameters = Vec::new();
  let identifier_brackets = matches!(dialect, DatabaseType::SqlServer | DatabaseType::SQLite);
  let nested_comments = nests_block_comments(dialect);
  let mysql_like = matches!(dialect, DatabaseType::MySQL | DatabaseType::ClickHouse);
  let mut state = LexerState::Normal;
  let mut index = 0;

  while index < bytes.len() {
    match &mut state {
      LexerState::Normal => {
        if let Some(tag) = match_dollar_quote_tag(sql, index) {
          index += tag.len();
          state = LexerState::DollarQuote { tag };
          continue;
        }
        if *dialect == DatabaseType::Oracle {
          if let Some((opening_len, terminator)) = match_oracle_q_quote(sql, index) {
            index += opening_len;
            state = LexerState::QQuote { terminator };
            continue;
          }
        }
        if let Some(param) = match_parameter(sql, index) {
          index = param.to;
          parameters.push(param);
          continue;
        }
        if starts_dash_comment(bytes, index, dialect) || (mysql_like && bytes[index] == b'#') {
          index += if bytes[index] == b'#' { 1 } else { 2 };
          state = LexerState::LineComment;
          continue;
        }
        if bytes.get(index..index + 2) == Some(b"/*") {
          index += 2;
          state = LexerState::BlockComment { depth: 1 };
          continue;
        }
        let ch = bytes[index];
        if ch == b'[' {
          index += 1;
          state = LexerState::Bracket;
          continue;
        }
        if matches!(ch, b'\'' | b'"' | b'`') {
          let backslash_escapes = mysql_like
            || (ch == b'\''
              && *dialect == DatabaseType::PostgreSQL
              && is_escape_string_prefix(bytes, index));
          index += 1;
          state = LexerState::Quote { quote: ch, backslash_escapes };
          continue;
        }
        if ch.is_ascii_alphabetic() || ch == b'_' {
          index += 1;
          while let Some(&b) = bytes.get(index) {
            if is_identifier_word_byte(b) || b == b'$' {
              index += 1;
            } else {
              break;
            }
          }
          continue;
        }
        index += 1;
      }
      LexerState::DollarQuote { tag } => {
        if bytes[index..].starts_with(tag.as_bytes()) {
          index += tag.len();
          state = LexerState::Normal;
        } else {
          index += 1;
        }
      }
      LexerState::Bracket => {
        if !identifier_brackets {
          if let Some(param) = match_parameter(sql, index) {
            index = param.to;
            parameters.push(param);
            continue;
          }
        }
        if bytes.get(index..index + 2) == Some(b"]]") {
          index += 2;
        } else {
          if bytes[index] == b']' {
            state = LexerState::Normal;
          }
          index += 1;
        }
      }
      LexerState::QQuote { terminator } => {
        if bytes[index..].starts_with(terminator.as_bytes()) {
          index += terminator.len();
          state = LexerState::Normal;
        } else {
          index += 1;
        }
      }
      LexerState::LineComment => {
        if matches!(bytes[index], b'\n' | b'\r') {
          state = LexerState::Normal;
        }
        index += 1;
      }
      LexerState::BlockComment { depth } => {
        if nested_comments && bytes.get(index..index + 2) == Some(b"/*") {
          *depth += 1;
          index += 2;
        } else if bytes.get(index..index + 2) == Some(b"*/") {
          index += 2;
          if *depth == 1 {
            state = LexerState::Normal;
          } else {
            *depth -= 1;
          }
        } else {
          index += 1;
        }
      }
      LexerState::Quote { quote, backslash_escapes } => {
        let quote = *quote;
        let esc = *backslash_escapes;
        let ch = bytes[index];
        index += 1;
        if ch == b'\\' && esc {
          if index < bytes.len() {
            index += 1;
          }
        } else if ch == quote {
          if bytes.get(index) == Some(&quote) {
            index += 1;
          } else {
            state = LexerState::Normal;
          }
        }
      }
    }
  }

  parameters
}

fn placeholder_for(dialect: &DatabaseType, position: usize) -> Option<String> {
  match dialect {
    DatabaseType::MySQL | DatabaseType::SQLite | DatabaseType::DuckDB => Some("?".to_string()),
    DatabaseType::PostgreSQL => Some(format!("${position}")),
    DatabaseType::SqlServer => Some(format!("@P{position}")),
    DatabaseType::Oracle => Some(format!(":{position}")),
    _ => None,
  }
}

/// 将 SQL 中的 `:name` 改写为对应方言的占位符，并按出现顺序生成绑定值列表。
/// 若 SQL 中无命名参数且 `values` 为空，原样返回；若存在缺参或未引用参数则显式报错。
pub fn rewrite_named_parameters(
  sql: &str,
  dialect: &DatabaseType,
  values: &HashMap<String, JsonValue>,
) -> Result<RewrittenQuery, NamedParameterError> {
  let found = scan_parameters(sql, dialect);
  if found.is_empty() {
    if !values.is_empty() {
      let mut unused: Vec<String> = values.keys().cloned().collect();
      unused.sort();
      return Err(NamedParameterError::UnusedParameters(unused));
    }
    return Ok(RewrittenQuery { sql: sql.to_string(), params: Vec::new(), names: Vec::new() });
  }

  if placeholder_for(dialect, 1).is_none() {
    return Err(NamedParameterError::UnsupportedDialect);
  }

  let mut missing = Vec::new();
  let mut missing_seen = HashSet::new();
  let mut referenced = HashSet::new();
  for param in &found {
    referenced.insert(param.name.as_str());
    if !values.contains_key(&param.name) && missing_seen.insert(param.name.as_str()) {
      missing.push(param.name.clone());
    }
  }
  if !missing.is_empty() {
    return Err(NamedParameterError::MissingParameters(missing));
  }

  let mut unused: Vec<String> =
    values.keys().filter(|k| !referenced.contains(k.as_str())).cloned().collect();
  if !unused.is_empty() {
    unused.sort();
    return Err(NamedParameterError::UnusedParameters(unused));
  }

  let mut rewritten = String::with_capacity(sql.len() + found.len() * 2);
  let mut params = Vec::with_capacity(found.len());
  let mut names = Vec::with_capacity(found.len());
  let mut copied = 0;

  for param in &found {
    let Some(marker) = placeholder_for(dialect, params.len() + 1) else {
      return Err(NamedParameterError::UnsupportedDialect);
    };
    let value = values.get(&param.name).cloned().unwrap_or(JsonValue::Null);
    rewritten.push_str(&sql[copied..param.from]);
    rewritten.push_str(&marker);
    copied = param.to;
    params.push(value);
    names.push(param.name.clone());
  }
  rewritten.push_str(&sql[copied..]);

  Ok(RewrittenQuery { sql: rewritten, params, names })
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde::Deserialize;

  const CORPUS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/bind-parameters-conformance.json"
  ));

  #[derive(Deserialize)]
  struct Corpus {
    cases: Vec<Case>,
  }

  #[derive(Deserialize)]
  struct Case {
    why: String,
    dialect: DatabaseType,
    sql: String,
    names: Vec<String>,
    rewritten: Option<String>,
    #[serde(default)]
    unsupported: bool,
  }

  #[test]
  fn matches_shared_bind_parameters_conformance_corpus() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("parse conformance corpus");
    assert!(!corpus.cases.is_empty());

    for case in corpus.cases {
      let scanned: Vec<String> =
        scan_parameters(&case.sql, &case.dialect).into_iter().map(|p| p.name).collect();
      assert_eq!(scanned, case.names, "scan mismatch for: {}", case.why);

      let mut values = HashMap::new();
      for name in &case.names {
        values.insert(name.clone(), JsonValue::String(format!("val_{name}")));
      }

      let outcome = rewrite_named_parameters(&case.sql, &case.dialect, &values);
      if case.unsupported {
        assert_eq!(
          outcome,
          Err(NamedParameterError::UnsupportedDialect),
          "expected unsupported dialect for: {}",
          case.why
        );
      } else {
        let rewritten = outcome.expect(&case.why);
        assert_eq!(Some(rewritten.sql), case.rewritten, "rewrite mismatch for: {}", case.why);
        assert_eq!(rewritten.names, case.names, "names mismatch for: {}", case.why);
        assert_eq!(rewritten.params.len(), case.names.len());
      }
    }
  }

  #[test]
  fn rejects_missing_and_unused_parameters() {
    let mut values = HashMap::new();
    values.insert("a".to_string(), JsonValue::from(1));
    assert_eq!(
      rewrite_named_parameters("SELECT :a, :b", &DatabaseType::SQLite, &values),
      Err(NamedParameterError::MissingParameters(vec!["b".to_string()]))
    );

    values.insert("b".to_string(), JsonValue::from(2));
    values.insert("extra".to_string(), JsonValue::from(3));
    assert_eq!(
      rewrite_named_parameters("SELECT :a, :b", &DatabaseType::SQLite, &values),
      Err(NamedParameterError::UnusedParameters(vec!["extra".to_string()]))
    );
  }

  #[test]
  fn handles_multibyte_utf8_in_oracle_and_dollar_quoted_sql() {
    let oracle_sql = "SELECT '中文🚀', q'[测试文本 :ignored 🎉]' FROM 订单表 WHERE 状态 = :status";
    let oracle_params = scan_parameters(oracle_sql, &DatabaseType::Oracle);
    let names: Vec<&str> = oracle_params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["status"]);

    let pg_sql = "SELECT $tag$中文内容 :ignored 🚀$tag$, :user_id";
    let pg_params = scan_parameters(pg_sql, &DatabaseType::PostgreSQL);
    let pg_names: Vec<&str> = pg_params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(pg_names, vec!["user_id"]);
  }
}
