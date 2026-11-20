//! 连接级本地业务语义备注（`semantics/<id>.md`，`rfcs/cli-mcp-usability.md` CM4）。
//!
//! 独立于 `connections.json` 存放：
//! - 不增加 `connections.json` 体积，且不纳入 `<id>#agent` 连接指纹（改备注无需重算口令 HMAC）；
//! - 删连接时一并清理；
//! - 单文件上限 32 KB 纯 UTF-8 Markdown；
//! - 供 `cli dictionary`、MCP `dictionary` / `schema` 以及 GUI 数据字典 / AI 语义草拟（AI2）共用。

use std::path::{Path, PathBuf};

/// 单连接语义文档上限（32 KB）
pub const MAX_SEMANTICS_BYTES: usize = 32 * 1024;
/// 保存的语义备注超过 32 KB 上限
pub const SEMANTICS_TOO_LARGE: &str = "DATAOMNI_SEMANTICS_TOO_LARGE";
/// 连接 ID 非法（防止路径穿越）
pub const SEMANTICS_INVALID_ID: &str = "DATAOMNI_SEMANTICS_INVALID_ID";

const SEMANTICS_DIR: &str = "semantics";
const TRUNCATION_NOTICE: &str = "\n\n> [Truncated at 32 KB limit]";

fn validate_connection_id(id: &str) -> Result<(), String> {
  if id.is_empty()
    || id.contains('/')
    || id.contains('\\')
    || id.contains("..")
    || id.contains('\0')
  {
    return Err(format!("{SEMANTICS_INVALID_ID}: {id}"));
  }
  Ok(())
}

pub fn semantics_path(config_dir: &Path, connection_id: &str) -> Result<PathBuf, String> {
  validate_connection_id(connection_id)?;
  Ok(config_dir.join(SEMANTICS_DIR).join(format!("{connection_id}.md")))
}

/// 读取连接语义备注（GUI 编辑用）。文件不存在或空白返回 `Ok(None)`。
pub fn read_semantics(config_dir: &Path, connection_id: &str) -> Result<Option<String>, String> {
  let path = semantics_path(config_dir, connection_id)?;
  let content = match std::fs::read_to_string(&path) {
    Ok(text) => text,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(error) => {
      return Err(format!("{}: {} · {error}", crate::commands::FILE_READ_FAILED, path.display()))
    }
  };
  if content.trim().is_empty() {
    return Ok(None);
  }
  Ok(Some(content))
}

/// 供 CLI / MCP 读取连接语义备注：若手工在磁盘上写超 32 KB，按 UTF-8 字符边界显式截断并附加提示，
/// 既不炸掉 Agent 上下文也不崩溃。
pub fn read_semantics_for_agent(config_dir: &Path, connection_id: &str) -> Option<String> {
  let raw = read_semantics(config_dir, connection_id).ok().flatten()?;
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return None;
  }
  if trimmed.len() <= MAX_SEMANTICS_BYTES {
    return Some(trimmed.to_string());
  }
  let budget = MAX_SEMANTICS_BYTES.saturating_sub(TRUNCATION_NOTICE.len());
  let mut cut = budget.min(trimmed.len());
  while cut > 0 && !trimmed.is_char_boundary(cut) {
    cut -= 1;
  }
  Some(format!("{}{}", trimmed[..cut].trim_end(), TRUNCATION_NOTICE))
}

/// 保存连接语义备注（原子写盘；为空白时删除侧文件；超过 32 KB 显式拒绝）。
pub fn save_semantics(config_dir: &Path, connection_id: &str, content: &str) -> Result<(), String> {
  let path = semantics_path(config_dir, connection_id)?;
  if content.len() > MAX_SEMANTICS_BYTES {
    return Err(format!("{SEMANTICS_TOO_LARGE}: {} · {MAX_SEMANTICS_BYTES}", content.len()));
  }
  if content.trim().is_empty() {
    match std::fs::remove_file(&path) {
      Ok(()) => return Ok(()),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
      Err(error) => {
        return Err(format!("{}: {} · {error}", crate::commands::FILE_WRITE_FAILED, path.display()))
      }
    }
  }
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).map_err(|error| {
      format!("{}: {} · {error}", crate::commands::FILE_WRITE_FAILED, parent.display())
    })?;
  }
  let temp = path.with_extension("md.part");
  std::fs::write(&temp, content.as_bytes()).map_err(|error| {
    format!("{}: {} · {error}", crate::commands::FILE_WRITE_FAILED, temp.display())
  })?;
  std::fs::rename(&temp, &path).map_err(|error| {
    let _ = std::fs::remove_file(&temp);
    format!("{}: {} · {error}", crate::commands::FILE_WRITE_FAILED, path.display())
  })?;
  Ok(())
}

/// 删除连接时顺带清理其语义侧文件（文件不存在时静默忽略）。
pub fn delete_semantics(config_dir: &Path, connection_id: &str) {
  if let Ok(path) = semantics_path(config_dir, connection_id) {
    let _ = std::fs::remove_file(path);
  }
}

/// 从连接级 Markdown 语义备注中按表名提取 `### <table>`（或 `### <schema>.<table>`）小节，
/// 供 `cli schema <conn> <table>` 与 MCP `schema` 按需附带该表说明。
pub fn extract_table_section(markdown: &str, table: &str, schema: Option<&str>) -> Option<String> {
  let bare_target = table.trim().to_ascii_lowercase();
  if bare_target.is_empty() {
    return None;
  }
  let qualified_target = schema
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .map(|s| format!("{}.{}", s.to_ascii_lowercase(), bare_target));

  let mut in_fence = false;
  let mut capturing = false;
  let mut captured: Vec<&str> = Vec::new();
  let mut matched_qualified = false;
  let mut bare_match: Option<String> = None;

  for line in markdown.lines() {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
      in_fence = !in_fence;
      if capturing {
        captured.push(line);
      }
      continue;
    }
    if !in_fence && trimmed.starts_with('#') {
      let level = trimmed.bytes().take_while(|&b| b == b'#').count();
      if level <= 3 && trimmed.as_bytes().get(level).is_some_and(|&b| b == b' ' || b == b'\t') {
        if capturing {
          let body = captured.join("\n").trim().to_string();
          if !body.is_empty() {
            if matched_qualified {
              return Some(body);
            }
            if bare_match.is_none() {
              bare_match = Some(body);
            }
          }
          capturing = false;
          captured.clear();
          matched_qualified = false;
        }
        if level == 3 {
          let heading = trimmed[3..].trim().trim_matches('`').trim().to_ascii_lowercase();
          if qualified_target.as_deref() == Some(heading.as_str()) {
            capturing = true;
            matched_qualified = true;
          } else if heading == bare_target {
            capturing = true;
            matched_qualified = false;
          }
        }
        continue;
      }
    }
    if capturing {
      captured.push(line);
    }
  }

  if capturing {
    let body = captured.join("\n").trim().to_string();
    if !body.is_empty() {
      if matched_qualified {
        return Some(body);
      }
      if bare_match.is_none() {
        bare_match = Some(body);
      }
    }
  }

  bare_match
}

#[cfg(test)]
mod tests {
  use super::*;

  struct TestDir(PathBuf);

  impl TestDir {
    fn new(tag: &str) -> Self {
      let path =
        std::env::temp_dir().join(format!("dataomni-semantics-{tag}-{}", uuid::Uuid::new_v4()));
      std::fs::create_dir_all(&path).expect("create temp dir");
      Self(path)
    }
  }

  impl Drop for TestDir {
    fn drop(&mut self) {
      let _ = std::fs::remove_dir_all(&self.0);
    }
  }

  #[test]
  fn saves_reads_deletes_and_enforces_size_limit_and_safe_ids() {
    let dir = TestDir::new("crud");
    assert_eq!(read_semantics(&dir.0, "conn-1").expect("missing"), None);

    save_semantics(&dir.0, "conn-1", "### orders\n- status: 1=paid").expect("save");
    assert_eq!(
      read_semantics(&dir.0, "conn-1").expect("read").as_deref(),
      Some("### orders\n- status: 1=paid")
    );

    // 超 32 KB 保存必须拒绝
    let oversized = "a".repeat(MAX_SEMANTICS_BYTES + 1);
    let err = save_semantics(&dir.0, "conn-1", &oversized).expect_err("must reject >32KB");
    assert!(err.starts_with(SEMANTICS_TOO_LARGE), "{err}");

    // 非法 ID 路径穿越必须拒绝
    for bad in ["", "../escape", "a/b", "a\\b"] {
      assert!(save_semantics(&dir.0, bad, "x").is_err(), "{bad}");
    }

    // 磁盘手工写超 32 KB 时 Agent 读取显式截断
    let path = semantics_path(&dir.0, "conn-big").expect("path");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    let big_utf8 = "中".repeat(MAX_SEMANTICS_BYTES); // 96 KB UTF-8
    std::fs::write(&path, &big_utf8).expect("write oversized");
    let agent_text = read_semantics_for_agent(&dir.0, "conn-big").expect("truncated read");
    assert!(agent_text.len() <= MAX_SEMANTICS_BYTES);
    assert!(agent_text.ends_with(TRUNCATION_NOTICE));

    // 清空内容删除侧文件
    save_semantics(&dir.0, "conn-1", "   \n").expect("clear");
    assert!(!semantics_path(&dir.0, "conn-1").expect("path").exists());
  }

  #[test]
  fn extracts_table_specific_markdown_section_preferring_schema_qualified_heading() {
    let doc = "\
## General
- Currency is stored in cents.

### `orders`
- status: 1=paid, 2=refunded
```sql
### fake_heading_inside_code_fence
SELECT 1;
```

### public.orders
- Qualified schema note for orders.

### users
- Soft-deleted via `deleted_at`.
";
    assert_eq!(
      extract_table_section(doc, "orders", Some("public")).as_deref(),
      Some("- Qualified schema note for orders.")
    );
    let bare = extract_table_section(doc, "orders", None).expect("bare orders");
    assert!(bare.contains("status: 1=paid, 2=refunded"));
    assert!(bare.contains("### fake_heading_inside_code_fence"));
    assert_eq!(
      extract_table_section(doc, "USERS", None).as_deref(),
      Some("- Soft-deleted via `deleted_at`.")
    );
    assert_eq!(extract_table_section(doc, "missing", None), None);
  }
}
