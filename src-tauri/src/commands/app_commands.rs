//! 应用自身的信息：报缺陷时要带上的诊断信息，和日志文件在哪。

use crate::services::http_endpoint::error_chain;
use tauri::{AppHandle, Manager};

/// 日志文件名（不带扩展名）。`tauri-plugin-log` 写到系统的应用日志目录下，加上 `.log`
pub const LOG_FILE_NAME: &str = "dataomni";
/// 日志所在的文件夹打不开：系统没有文件管理器可调，或者日志目录取不到
pub const LOG_REVEAL_FAILED: &str = "DATAOMNI_LOG_REVEAL_FAILED";
/// 没问到最新版本：离线、被墙、GitHub 限流，或者回答的形状不对。启动时的检查不显示它，只进日志
pub const UPDATE_CHECK_FAILED: &str = "DATAOMNI_UPDATE_CHECK_FAILED";
/// 系统浏览器没能打开固定外链
pub const EXTERNAL_LINK_OPEN_FAILED: &str = "DATAOMNI_EXTERNAL_LINK_OPEN_FAILED";

const LATEST_RELEASE_API: &str =
  "https://api.github.com/repos/dataomni-rs/DataOmni/releases/latest";
const RELEASE_PAGE: &str = "https://github.com/dataomni-rs/DataOmni/releases/tag/v";
const UPDATE_CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// 允许从设置或原生菜单打开的固定外部链接（拒绝前端传入任意网址）
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectLink {
  Website,
  WebsiteZh,
  Issues,
}

impl ProjectLink {
  pub const fn url(self) -> &'static str {
    match self {
      Self::Website => "https://dataomni.dev/",
      Self::WebsiteZh => "https://dataomni.dev/zh/",
      Self::Issues => "https://github.com/dataomni-rs/DataOmni/issues",
    }
  }
}

#[derive(Debug, serde::Serialize)]
pub struct Diagnostics {
  pub app_version: String,
  pub os: &'static str,
  pub arch: &'static str,
  /// WebView 的版本：WKWebView、WebView2、WebKitGTK。界面上的怪事多半跟它走
  pub webview_version: Option<String>,
  pub ai: bool,
  pub log_file: Option<String>,
}

fn log_file(app: &AppHandle) -> Option<std::path::PathBuf> {
  app.path().app_log_dir().ok().map(|dir| dir.join(format!("{LOG_FILE_NAME}.log")))
}

#[tauri::command]
pub fn diagnostics(app: AppHandle) -> Diagnostics {
  Diagnostics {
    app_version: app.package_info().version.to_string(),
    os: std::env::consts::OS,
    arch: std::env::consts::ARCH,
    webview_version: tauri::webview_version().ok(),
    ai: cfg!(feature = "ai"),
    log_file: log_file(&app).map(|path| path.display().to_string()),
  }
}

/// 在文件管理器里选中日志文件。还没写过日志时文件不存在，就打开它所在的目录
#[tauri::command]
pub fn reveal_log_file(app: AppHandle) -> Result<(), String> {
  let path = log_file(&app).ok_or_else(|| LOG_REVEAL_FAILED.to_string())?;
  let target =
    if path.exists() { path } else { path.parent().map(|dir| dir.to_path_buf()).unwrap_or(path) };
  tauri_plugin_opener::reveal_item_in_dir(&target)
    .map_err(|error| format!("{LOG_REVEAL_FAILED}: {error}"))
}

/// GitHub 的 latest release 回答里取版本号，去掉开头的 `v`。
/// 草稿和预发布不会出现在 `/releases/latest` 里，所以 `v1.0.0-rc.1` 不会被提示给用户
fn latest_version(body: &str) -> Option<String> {
  let value: serde_json::Value = serde_json::from_str(body).ok()?;
  let tag = value.get("tag_name")?.as_str()?;
  let version = tag.strip_prefix('v').unwrap_or(tag);
  is_version(version).then(|| version.to_string())
}

/// 只认版本号会用到的字符：它要拼进打开的网址里
fn is_version(text: &str) -> bool {
  !text.is_empty()
    && text
      .chars()
      .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
}

/// 问 GitHub 最新发布的版本号。走系统代理（不加 `no_proxy`）：要翻出去的人本来就配着代理
#[tauri::command]
pub async fn latest_release() -> Result<String, String> {
  let failed = |error: &dyn std::fmt::Display| format!("{UPDATE_CHECK_FAILED}: {error}");
  let client = reqwest::Client::builder()
    .timeout(UPDATE_CHECK_TIMEOUT)
    // GitHub API 不带 User-Agent 直接 403
    .user_agent(concat!("DataOmni/", env!("CARGO_PKG_VERSION")))
    .build()
    .map_err(|error| failed(&error_chain(&error)))?;
  let response = client
    .get(LATEST_RELEASE_API)
    .header("Accept", "application/vnd.github+json")
    .send()
    .await
    .map_err(|error| failed(&error_chain(&error)))?;
  let status = response.status();
  let body = response.text().await.map_err(|error| failed(&error_chain(&error)))?;
  if !status.is_success() {
    return Err(failed(&status));
  }
  let version = latest_version(&body).ok_or_else(|| failed(&"tag_name"))?;
  log::info!("最新发布的版本: {version}");
  Ok(version)
}

/// 在浏览器里打开某个版本的发布页。网址在这里拼，前端只给版本号——不给前端开一个「打开任意网址」的口子
#[tauri::command]
pub fn open_release_page(version: String) -> Result<(), String> {
  if !is_version(&version) {
    return Err(format!("{UPDATE_CHECK_FAILED}: {version}"));
  }
  tauri_plugin_opener::open_url(format!("{RELEASE_PAGE}{version}"), None::<&str>)
    .map_err(|error| format!("{UPDATE_CHECK_FAILED}: {error}"))
}

/// 当前运行副本由哪种渠道安装（见 `rfcs/auto-update.md` §3）
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallChannel {
  Standalone,
  Homebrew,
  SystemPackage,
}

fn detect_install_channel_from(
  os: &str,
  exe_path: Option<&std::path::Path>,
  homebrew_caskroom_present: bool,
) -> InstallChannel {
  if os == "macos"
    && homebrew_caskroom_present
    && exe_path.is_some_and(|path| path.ends_with("DataOmni.app/Contents/MacOS/dataomni"))
  {
    return InstallChannel::Homebrew;
  }
  if os == "linux" && exe_path.is_some_and(|path| path.starts_with("/usr/")) {
    return InstallChannel::SystemPackage;
  }
  InstallChannel::Standalone
}

#[tauri::command]
pub fn compiled_drivers() -> Vec<crate::models::DatabaseType> {
  crate::models::DatabaseType::compiled_drivers()
}

#[tauri::command]
pub fn install_channel() -> InstallChannel {
  let exe = std::env::current_exe().ok();
  let caskroom_present = std::path::Path::new("/opt/homebrew/Caskroom/dataomni").is_dir()
    || std::path::Path::new("/usr/local/Caskroom/dataomni").is_dir();
  detect_install_channel_from(std::env::consts::OS, exe.as_deref(), caskroom_present)
}

/// 在浏览器里打开官网或问题反馈页。目标写成枚举，不在 IPC 层暴露任意 URL
#[tauri::command]
pub fn open_project_link(target: ProjectLink) -> Result<(), String> {
  tauri_plugin_opener::open_url(target.url(), None::<&str>)
    .map_err(|error| format!("{EXTERNAL_LINK_OPEN_FAILED}: {error}"))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn project_links_map_only_to_fixed_official_https_destinations() {
    let en: ProjectLink = serde_json::from_str("\"website\"").expect("website enum");
    let zh: ProjectLink = serde_json::from_str("\"websiteZh\"").expect("websiteZh enum");
    let issues: ProjectLink = serde_json::from_str("\"issues\"").expect("issues enum");
    assert_eq!(en.url(), "https://dataomni.dev/");
    assert_eq!(zh.url(), "https://dataomni.dev/zh/");
    assert_eq!(issues.url(), "https://github.com/dataomni-rs/DataOmni/issues");
    assert!(serde_json::from_str::<ProjectLink>("\"https://evil.example\"").is_err());
  }

  #[test]
  fn takes_the_version_from_the_latest_release() {
    let body = r#"{"tag_name":"v0.6.0","name":"DataOmni v0.6.0","draft":false,"prerelease":false}"#;
    assert_eq!(latest_version(body), Some("0.6.0".to_string()));
    assert_eq!(latest_version(r#"{"tag_name":"1.0.0-rc.1"}"#), Some("1.0.0-rc.1".to_string()));
  }

  #[test]
  fn rejects_answers_that_are_not_a_release() {
    // 限流时的回答：没有 tag_name
    assert_eq!(latest_version(r#"{"message":"API rate limit exceeded"}"#), None);
    assert_eq!(latest_version("<html>"), None);
    // 拼进网址的字符只能是版本号的字符
    assert_eq!(latest_version(r#"{"tag_name":"v1.0/../../evil"}"#), None);
    assert!(open_release_page("1.0.0?x=1".to_string()).is_err());
  }

  #[test]
  fn detects_homebrew_system_package_and_standalone_install_channels() {
    use std::path::Path;

    assert_eq!(
      detect_install_channel_from(
        "macos",
        Some(Path::new("/Applications/DataOmni.app/Contents/MacOS/dataomni")),
        true,
      ),
      InstallChannel::Homebrew,
    );
    // 源码构建即使本机装过 Homebrew 版，也不判成包管理器副本
    assert_eq!(
      detect_install_channel_from(
        "macos",
        Some(Path::new("/Users/dev/DataOmni/target/debug/dataomni")),
        true,
      ),
      InstallChannel::Standalone,
    );
    // 直接从 .dmg 拖入 /Applications（无 Caskroom 目录）
    assert_eq!(
      detect_install_channel_from(
        "macos",
        Some(Path::new("/Applications/DataOmni.app/Contents/MacOS/dataomni")),
        false,
      ),
      InstallChannel::Standalone,
    );
    assert_eq!(
      detect_install_channel_from("linux", Some(Path::new("/usr/bin/dataomni")), false),
      InstallChannel::SystemPackage,
    );
    assert_eq!(
      detect_install_channel_from(
        "windows",
        Some(Path::new("C:\\Program Files\\DataOmni\\dataomni.exe")),
        false,
      ),
      InstallChannel::Standalone,
    );
  }

  #[test]
  fn compiled_drivers_reflect_oracle_feature_gate() {
    let drivers = compiled_drivers();
    assert!(drivers.contains(&crate::models::DatabaseType::PostgreSQL));
    assert_eq!(drivers.contains(&crate::models::DatabaseType::Oracle), cfg!(feature = "oracle"));
  }
}
