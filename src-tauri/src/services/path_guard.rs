//! IPC 文件读写路径守卫（H5 / S6）。
//!
//! 统一阻断前端经任意文件类 IPC 命令读写 DataOmni 内部保留目录（应用配置目录 `connections.json` /
//! `semantics/` / `dm8/` 以及应用日志目录 `dataomni.log` / `dataomni-cli.log`），防御：
//! - 不存在父目录叠加 `..` 路径穿越（逐段向上回溯最近存在祖先再词法折叠 + 逐级解析符号链接）
//! - 指向保留目录的已有或悬空符号链接
//! - 跨平台日志目录不在配置目录下（如 macOS `~/Library/Logs/com.dataomni.app`）

use crate::services::connection_service::{app_config_dir, APP_IDENTIFIER};
use std::path::{Component, Path, PathBuf};

/// 路径落在应用自己的配置或日志保留目录里。数据里带的是那条路径
pub const PATH_RESERVED: &str = "DATAOMNI_PATH_RESERVED";

const MAX_SYMLINK_DEPTH: usize = 16;

/// 收集当前平台下所有受保护的应用内部目录（配置目录 + 日志目录）。
pub fn reserved_directories(app_handle: Option<&tauri::AppHandle>) -> Vec<PathBuf> {
  let mut dirs = Vec::new();
  let mut push_unique = |candidate: PathBuf| {
    let normalized = resolve_for_comparison(&candidate);
    if !dirs.iter().any(|existing: &PathBuf| existing == &normalized) {
      dirs.push(normalized);
    }
  };

  if let Some(config) = app_config_dir() {
    push_unique(config);
  }
  #[cfg(target_os = "macos")]
  if let Some(home) = dirs::home_dir() {
    push_unique(home.join("Library").join("Logs").join(APP_IDENTIFIER));
  }
  if let Some(data_local) = dirs::data_local_dir() {
    push_unique(data_local.join(APP_IDENTIFIER).join("logs"));
  }
  if let Some(app) = app_handle {
    use tauri::Manager;
    if let Ok(config) = app.path().app_config_dir() {
      push_unique(config);
    }
    if let Ok(logs) = app.path().app_log_dir() {
      push_unique(logs);
    }
  }
  dirs
}

/// 拒绝落在应用配置或日志保留目录内的路径。
pub fn refuse_reserved_path(target: &Path) -> Result<(), String> {
  refuse_reserved_path_with_app(target, None)
}

/// 同 [`refuse_reserved_path`]，同时纳入 Tauri `AppHandle` 解析出的配置与日志目录。
pub fn refuse_reserved_path_with_app(
  target: &Path,
  app_handle: Option<&tauri::AppHandle>,
) -> Result<(), String> {
  let reserved = reserved_directories(app_handle);
  refuse_inside_any(target, &reserved)
}

pub(crate) fn refuse_inside(target: &Path, reserved: &Path) -> Result<(), String> {
  refuse_inside_any(target, &[reserved.to_path_buf()])
}

pub(crate) fn refuse_inside_any(target: &Path, reserved_dirs: &[PathBuf]) -> Result<(), String> {
  let resolved_target = resolve_for_comparison(target);
  for reserved in reserved_dirs {
    let resolved_reserved = resolve_for_comparison(reserved);
    if resolved_target.starts_with(&resolved_reserved) {
      return Err(format!("{PATH_RESERVED}: {}", target.display()));
    }
  }
  Ok(())
}

/// 规范化路径用于安全边界比对：
/// 1. 相对路径先锚定到当前工作目录；
/// 2. 逐段处理，对存在的路径做 `canonicalize`，对悬空符号链接展开 `read_link`（上限 16 层防环），
///    对尚不存在的尾段按词法折叠 `.` 与 `..`，彻底阻断「不存在父目录 + `..`」绕过。
pub fn resolve_for_comparison(target: &Path) -> PathBuf {
  resolve_with_depth(target, 0)
}

fn resolve_with_depth(target: &Path, depth: usize) -> PathBuf {
  let anchored = if target.is_absolute() {
    target.to_path_buf()
  } else {
    std::env::current_dir().map(|cwd| cwd.join(target)).unwrap_or_else(|_| target.to_path_buf())
  };
  if let Ok(resolved) = anchored.canonicalize() {
    return resolved;
  }

  let mut resolved = PathBuf::new();
  for component in anchored.components() {
    match component {
      Component::Prefix(prefix) => resolved.push(prefix.as_os_str()),
      Component::RootDir => resolved.push(component.as_os_str()),
      Component::CurDir => {}
      Component::ParentDir => {
        resolved.pop();
      }
      Component::Normal(part) => {
        resolved.push(part);
        if let Ok(canonical) = resolved.canonicalize() {
          resolved = canonical;
        } else if depth < MAX_SYMLINK_DEPTH {
          if let Ok(meta) = std::fs::symlink_metadata(&resolved) {
            if meta.file_type().is_symlink() {
              if let Ok(link_target) = std::fs::read_link(&resolved) {
                let next = if link_target.is_absolute() {
                  link_target
                } else {
                  resolved.parent().map(|parent| parent.join(&link_target)).unwrap_or(link_target)
                };
                resolved = resolve_with_depth(&next, depth + 1);
              }
            }
          }
        }
      }
    }
  }
  resolved
}

/// 校验多文件写出目标：必须严格留在已规范化的 `root` 目录内，且不得触及任何保留目录。
pub fn check_contained_target(root: &Path, target: &Path) -> Result<(), String> {
  refuse_reserved_path(target)?;
  let resolved_root = resolve_for_comparison(root);
  let resolved_target = resolve_for_comparison(target);
  if !resolved_target.starts_with(&resolved_root) {
    return Err(format!("{PATH_RESERVED}: {}", target.display()));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn blocks_nonexistent_parent_dot_dot_traversal_into_reserved_dir() {
    let root = std::env::temp_dir().join(format!("dataomni-pg-dotdot-{}", std::process::id()));
    let reserved = root.join("config");
    let elsewhere = root.join("exports");
    std::fs::create_dir_all(&reserved).expect("reserved");
    std::fs::create_dir_all(&elsewhere).expect("elsewhere");

    // 关键回归（H5 / S6）：父目录不存在 + 多层 `..` 绕回保留目录并指向尚不存在的子目录
    let attack = elsewhere
      .join("nonexistent-a")
      .join("nonexistent-b")
      .join("..")
      .join("..")
      .join("..")
      .join("config")
      .join("new-sub")
      .join("evil.json");
    let err = refuse_inside(&attack, &reserved).expect_err("must block nonexistent parent ..");
    assert!(err.starts_with(PATH_RESERVED), "{err}");

    // 保留目录与独立日志目录同时受保护
    let log_dir = root.join("logs");
    std::fs::create_dir_all(&log_dir).expect("logs");
    let log_attack =
      elsewhere.join("ghost").join("..").join("..").join("logs").join("dataomni.log");
    assert!(refuse_inside_any(&log_attack, &[reserved.clone(), log_dir.clone()]).is_err());

    // 兄弟目录与合法路径正常放行
    let safe = elsewhere.join("sub").join("out.parquet");
    assert!(refuse_inside_any(&safe, &[reserved, log_dir]).is_ok());

    std::fs::remove_dir_all(&root).ok();
  }

  #[cfg(unix)]
  #[test]
  fn blocks_dangling_symlink_and_directory_symlink_escape() {
    let root = std::env::temp_dir().join(format!("dataomni-pg-symlink-{}", std::process::id()));
    let reserved = root.join("config");
    let exports = root.join("exports");
    std::fs::create_dir_all(&reserved).expect("reserved");
    std::fs::create_dir_all(&exports).expect("exports");

    // 悬空符号链接指向保留目录内尚未创建的文件
    let dangling = exports.join("dangling.json");
    std::os::unix::fs::symlink(reserved.join("future.json"), &dangling).expect("dangling symlink");
    assert!(
      refuse_inside(&dangling, &reserved).is_err(),
      "dangling symlink into reserved dir must be blocked"
    );

    // 导出目录内含指向外部/保留目录的子目录符号链接
    let sub_link = exports.join("entities");
    std::os::unix::fs::symlink(&reserved, &sub_link).expect("dir symlink");
    let escaped = sub_link.join("User.ts");
    assert!(refuse_inside(&escaped, &reserved).is_err(), "symlinked subdir into reserved");
    assert!(
      check_contained_target(&exports, &escaped).is_err(),
      "target escaping root via symlink must be rejected"
    );

    std::fs::remove_dir_all(&root).ok();
  }
}
