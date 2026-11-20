//! 应用自身的信息：报缺陷时要带上的诊断信息，和日志文件在哪。

use crate::services::http_endpoint::error_chain;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// 日志文件名（不带扩展名）。`tauri-plugin-log` 写到系统的应用日志目录下，加上 `.log`
pub const LOG_FILE_NAME: &str = "dataomni";
/// 日志所在的文件夹打不开：系统没有文件管理器可调，或者日志目录取不到
pub const LOG_REVEAL_FAILED: &str = "DATAOMNI_LOG_REVEAL_FAILED";
/// 没问到最新版本：离线、被墙、GitHub 限流，或者回答的形状不对。启动时的检查不显示它，只进日志
pub const UPDATE_CHECK_FAILED: &str = "DATAOMNI_UPDATE_CHECK_FAILED";
/// 更新包 Ed25519 签名校验未通过（压缩包被篡改、签名损坏或版本不匹配）
pub const UPDATE_SIGNATURE_INVALID: &str = "DATAOMNI_UPDATE_SIGNATURE_INVALID";
/// 更新包下载失败（网络中断、服务端 404/5xx 等）
pub const UPDATE_DOWNLOAD_FAILED: &str = "DATAOMNI_UPDATE_DOWNLOAD_FAILED";
/// 尚未下载并通过验签就请求安装更新
pub const UPDATE_NOT_DOWNLOADED: &str = "DATAOMNI_UPDATE_NOT_DOWNLOADED";
/// 安装已验签的更新包失败（目录无写权限、跨卷重命名失败或安装器未能拉起）
pub const UPDATE_INSTALL_FAILED: &str = "DATAOMNI_UPDATE_INSTALL_FAILED";
/// 系统浏览器没能打开固定外链
pub const EXTERNAL_LINK_OPEN_FAILED: &str = "DATAOMNI_EXTERNAL_LINK_OPEN_FAILED";

const LATEST_RELEASE_API: &str =
  "https://api.github.com/repos/dataomni-rs/DataOmni/releases/latest";
const RELEASE_PAGE: &str = "https://github.com/dataomni-rs/DataOmni/releases/tag/v";
const UPDATE_CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const UPDATE_DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
const UPDATE_PROGRESS_EVENT: &str = "app-update-progress";
const PROGRESS_EMIT_STEP_BYTES: u64 = 128 * 1024;

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

enum StagedAppUpdate {
  Available(tauri_plugin_updater::Update),
  Verified { update: tauri_plugin_updater::Update, bytes: Vec<u8> },
}

/// 进程内暂存的应用更新状态：字节在后端内存里经 Ed25519 验签后再交给安装步骤，不从前端回传包体
#[derive(Default)]
pub struct AppUpdateState(std::sync::Mutex<Option<StagedAppUpdate>>);

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableAppUpdate {
  pub version: String,
  pub current_version: String,
  pub notes: Option<String>,
  pub pub_date: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDownloadProgress {
  pub downloaded_bytes: u64,
  pub total_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedAppUpdate {
  pub version: String,
  pub bytes_len: usize,
}

fn is_macos_app_bundle_executable(path: &std::path::Path) -> bool {
  let Some(macos_dir) = path.parent() else {
    return false;
  };
  let Some(contents_dir) = macos_dir.parent() else {
    return false;
  };
  let Some(app_dir) = contents_dir.parent() else {
    return false;
  };
  macos_dir.file_name() == Some(std::ffi::OsStr::new("MacOS"))
    && contents_dir.file_name() == Some(std::ffi::OsStr::new("Contents"))
    && app_dir.extension() == Some(std::ffi::OsStr::new("app"))
}

/// 仅独立安装的正式打包副本允许走应用内自替换：
/// - Homebrew Cask 走 `brew upgrade --cask dataomni`，Linux `.deb`/`.rpm` 走系统包管理器；
/// - 开发期 `target/debug/dataomni` 不允许自替换（macOS 解包逻辑会按两级父目录误覆盖构建输出目录）
fn supports_in_app_update_from(
  os: &str,
  exe_path: Option<&std::path::Path>,
  channel: InstallChannel,
  bundle_type: Option<tauri::utils::config::BundleType>,
) -> bool {
  use tauri::utils::config::BundleType;
  if channel != InstallChannel::Standalone {
    return false;
  }
  match os {
    "macos" => exe_path.is_some_and(is_macos_app_bundle_executable),
    "windows" => matches!(bundle_type, Some(BundleType::Msi | BundleType::Nsis)),
    _ => false,
  }
}

fn supports_in_app_update() -> bool {
  let exe = std::env::current_exe().ok();
  let channel = install_channel();
  supports_in_app_update_from(
    std::env::consts::OS,
    exe.as_deref(),
    channel,
    tauri::utils::platform::bundle_type(),
  )
}

fn classify_updater_error(error: &tauri_plugin_updater::Error, fallback_code: &str) -> String {
  use tauri_plugin_updater::Error as UpdaterError;
  let code = match error {
    UpdaterError::Minisign(_)
    | UpdaterError::Base64(_)
    | UpdaterError::SignatureUtf8(_)
    | UpdaterError::SignedVersionMismatch { .. }
    | UpdaterError::MissingSignedVersion => UPDATE_SIGNATURE_INVALID,
    _ => fallback_code,
  };
  format!("{code}: {error}")
}

fn available_from_update(update: &tauri_plugin_updater::Update) -> AvailableAppUpdate {
  let pub_date =
    update.date.and_then(|date| date.format(&time::format_description::well_known::Rfc3339).ok());
  AvailableAppUpdate {
    version: update.version.clone(),
    current_version: update.current_version.clone(),
    notes: update.body.clone(),
    pub_date,
  }
}

async fn check_update_with_builder(
  builder: tauri_plugin_updater::UpdaterBuilder,
  state: &AppUpdateState,
) -> Result<Option<AvailableAppUpdate>, String> {
  crate::services::sqlx_pool::ensure_tls_provider();
  let updater =
    builder.build().map_err(|error| classify_updater_error(&error, UPDATE_CHECK_FAILED))?;
  let found =
    updater.check().await.map_err(|error| classify_updater_error(&error, UPDATE_CHECK_FAILED))?;
  let mut guard = state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
  match found {
    Some(mut update) => {
      update.timeout = Some(UPDATE_DOWNLOAD_TIMEOUT);
      let info = available_from_update(&update);
      let keep_verified = matches!(
        &*guard,
        Some(StagedAppUpdate::Verified { update: existing, .. }) if existing.version == update.version
      );
      if !keep_verified {
        *guard = Some(StagedAppUpdate::Available(update));
      }
      Ok(Some(info))
    }
    None => {
      *guard = None;
      Ok(None)
    }
  }
}

async fn download_staged_update<F: FnMut(UpdateDownloadProgress)>(
  state: &AppUpdateState,
  mut on_progress: F,
) -> Result<VerifiedAppUpdate, String> {
  crate::services::sqlx_pool::ensure_tls_provider();
  let candidate = {
    let guard = state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match &*guard {
      Some(StagedAppUpdate::Verified { update, bytes }) => {
        return Ok(VerifiedAppUpdate { version: update.version.clone(), bytes_len: bytes.len() });
      }
      Some(StagedAppUpdate::Available(update)) => update.clone(),
      None => return Err(UPDATE_NOT_DOWNLOADED.to_string()),
    }
  };

  let mut downloaded: u64 = 0;
  let mut last_emitted: u64 = 0;
  let bytes = candidate
    .download(
      |chunk_len, total_bytes| {
        downloaded = downloaded.saturating_add(chunk_len as u64);
        let completed = total_bytes.is_some_and(|total| downloaded >= total);
        if completed || downloaded.saturating_sub(last_emitted) >= PROGRESS_EMIT_STEP_BYTES {
          last_emitted = downloaded;
          on_progress(UpdateDownloadProgress { downloaded_bytes: downloaded, total_bytes });
        }
      },
      || {},
    )
    .await
    .map_err(|error| classify_updater_error(&error, UPDATE_DOWNLOAD_FAILED))?;

  on_progress(UpdateDownloadProgress {
    downloaded_bytes: bytes.len() as u64,
    total_bytes: Some(bytes.len() as u64),
  });

  let info = VerifiedAppUpdate { version: candidate.version.clone(), bytes_len: bytes.len() };
  let mut guard = state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
  *guard = Some(StagedAppUpdate::Verified { update: candidate, bytes });
  Ok(info)
}

fn take_verified_update(
  state: &AppUpdateState,
) -> Result<(tauri_plugin_updater::Update, Vec<u8>), String> {
  let mut guard = state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
  match guard.take() {
    Some(StagedAppUpdate::Verified { update, bytes }) => Ok((update, bytes)),
    other => {
      *guard = other;
      Err(UPDATE_NOT_DOWNLOADED.to_string())
    }
  }
}

/// 检查当前平台是否有经签名的应用内升级包可用（非独立安装或开发构建直接返回 `None`）
#[tauri::command]
pub async fn check_app_update(
  app: AppHandle,
  state: tauri::State<'_, AppUpdateState>,
) -> Result<Option<AvailableAppUpdate>, String> {
  if !supports_in_app_update() {
    return Ok(None);
  }
  check_update_with_builder(app.updater_builder().timeout(UPDATE_CHECK_TIMEOUT), &state).await
}

/// 下载最新版本的安装包并在内存中完成 Ed25519 签名校验（不改写磁盘上的当前应用，不重启）
#[tauri::command]
pub async fn download_app_update(
  app: AppHandle,
  state: tauri::State<'_, AppUpdateState>,
) -> Result<VerifiedAppUpdate, String> {
  if !supports_in_app_update() {
    return Err(UPDATE_NOT_DOWNLOADED.to_string());
  }
  if state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).is_none() {
    let found =
      check_update_with_builder(app.updater_builder().timeout(UPDATE_CHECK_TIMEOUT), &state)
        .await?;
    if found.is_none() {
      return Err(UPDATE_NOT_DOWNLOADED.to_string());
    }
  }
  download_staged_update(&state, |progress| {
    let _ = app.emit(UPDATE_PROGRESS_EVENT, progress);
  })
  .await
}

/// 用户显式确认且前端已冲刷草稿、关闭会话后，安装已验签的更新包并重启
#[tauri::command]
pub fn install_app_update(
  app: AppHandle,
  state: tauri::State<'_, AppUpdateState>,
) -> Result<(), String> {
  let (update, bytes) = take_verified_update(&state)?;
  if let Err(error) = update.install(&bytes) {
    let message = classify_updater_error(&error, UPDATE_INSTALL_FAILED);
    // 安装失败（如权限不足或磁盘满）时把已验签包放回暂存区，用户处理后重试无需再下载一遍
    let mut guard = state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = Some(StagedAppUpdate::Verified { update, bytes });
    return Err(message);
  }
  #[cfg(not(windows))]
  app.restart();
  #[cfg(windows)]
  {
    let _ = app;
    Ok(())
  }
}

/// GUI「命令行与 Agent」接入引导信息。绝不返回任何连接口令或密钥。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCliInfo {
  pub executable_path: String,
  pub on_path: bool,
  pub os: &'static str,
  pub open_connection_count: usize,
  pub audit_log_path: Option<String>,
  pub guide_markdown: &'static str,
}

fn is_binary_on_path_in(path_env: Option<&std::ffi::OsStr>, os: &str) -> bool {
  let Some(path_env) = path_env else {
    return false;
  };
  let names: &[&str] = if os == "windows" { &["dataomni.exe", "dataomni"] } else { &["dataomni"] };
  std::env::split_paths(path_env).any(|dir| names.iter().any(|name| dir.join(name).is_file()))
}

fn resolve_cli_executable_path(os: &str) -> String {
  if let Ok(exe) = std::env::current_exe() {
    let resolved = exe.canonicalize().unwrap_or(exe);
    return resolved.display().to_string();
  }
  match os {
    "macos" => "/Applications/DataOmni.app/Contents/MacOS/dataomni".to_string(),
    "windows" => "C:\\Program Files\\DataOmni\\dataomni.exe".to_string(),
    _ => "/usr/bin/dataomni".to_string(),
  }
}

/// 返回当前 DataOmni 二进制路径、PATH 可见性、已开放给 Agent 的非生产连接数与内置 `SKILL.md` 文本。
#[tauri::command]
pub fn agent_cli_info(
  app: AppHandle,
  service_state: tauri::State<'_, crate::commands::connection_commands::ConnectionServiceState>,
) -> Result<AgentCliInfo, String> {
  use crate::models::{AgentAccess, ConnectionEnvironment};

  let os = std::env::consts::OS;
  let executable_path = resolve_cli_executable_path(os);
  let on_path = is_binary_on_path_in(std::env::var_os("PATH").as_deref(), os);
  let audit_log_path =
    app.path().app_log_dir().ok().map(|dir| dir.join(crate::cli::AUDIT_FILE).display().to_string());

  let open_connection_count = {
    let mut guard = service_state
      .lock()
      .map_err(|error| format!("{}: {error}", crate::commands::SERVICE_STATE_UNAVAILABLE))?;
    let service = match &mut *guard {
      Some(service) => service,
      slot => slot.insert(
        crate::services::ConnectionService::new(&app)
          .map_err(|error| format!("{}: {error}", crate::commands::SERVICE_INIT_FAILED))?,
      ),
    };
    service
      .get_connections()
      .iter()
      .filter(|conn| {
        conn.environment != ConnectionEnvironment::Production
          && conn.agent_access == AgentAccess::Read
      })
      .count()
  };

  Ok(AgentCliInfo {
    executable_path,
    on_path,
    os,
    open_connection_count,
    audit_log_path,
    guide_markdown: crate::cli::GUIDE,
  })
}

/// 只读读取 `dataomni-cli.log` 中最近的 CLI / MCP 审计摘要（倒序，最多 200 条）。
#[tauri::command]
pub fn read_agent_audit_log(
  app: AppHandle,
  limit: Option<usize>,
) -> Result<Vec<crate::cli::AuditLogEntry>, String> {
  let Ok(log_dir) = app.path().app_log_dir() else {
    return Ok(Vec::new());
  };
  crate::cli::read_audit_log_from(&log_dir, limit.unwrap_or(crate::cli::MAX_AUDIT_ENTRIES))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn detects_dataomni_binary_on_path_per_platform() {
    let base = std::env::temp_dir().join(format!("dataomni-path-test-{}", uuid::Uuid::new_v4()));
    let unix_dir = base.join("unix");
    let win_dir = base.join("win");
    std::fs::create_dir_all(&unix_dir).expect("create unix temp dir");
    std::fs::create_dir_all(&win_dir).expect("create win temp dir");

    let joined = std::env::join_paths([&unix_dir]).expect("join paths");
    assert!(!is_binary_on_path_in(Some(joined.as_os_str()), "macos"));
    assert!(!is_binary_on_path_in(None, "linux"));

    std::fs::write(unix_dir.join("dataomni"), b"").expect("create unix bin");
    assert!(is_binary_on_path_in(Some(joined.as_os_str()), "macos"));
    assert!(is_binary_on_path_in(Some(joined.as_os_str()), "linux"));

    let win_joined = std::env::join_paths([&win_dir]).expect("join win paths");
    assert!(!is_binary_on_path_in(Some(win_joined.as_os_str()), "windows"));
    std::fs::write(win_dir.join("dataomni.exe"), b"").expect("create win bin");
    assert!(is_binary_on_path_in(Some(win_joined.as_os_str()), "windows"));

    let _ = std::fs::remove_dir_all(&base);
  }

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

  #[test]
  fn supports_in_app_update_excludes_package_managers_and_dev_binaries() {
    use std::path::Path;
    use tauri::utils::config::BundleType;

    // macOS 官网 .dmg 拖入 /Applications 的独立安装包：允许应用内升级
    assert!(supports_in_app_update_from(
      "macos",
      Some(Path::new("/Applications/DataOmni.app/Contents/MacOS/dataomni")),
      InstallChannel::Standalone,
      Some(BundleType::App),
    ));
    // Homebrew Cask 安装副本：禁止自覆盖，引导 brew upgrade
    assert!(!supports_in_app_update_from(
      "macos",
      Some(Path::new("/Applications/DataOmni.app/Contents/MacOS/dataomni")),
      InstallChannel::Homebrew,
      Some(BundleType::App),
    ));
    // macOS 源码开发构建（target/debug/dataomni 不在 .app 包内）：禁止自覆盖
    assert!(!supports_in_app_update_from(
      "macos",
      Some(Path::new("/Users/dev/DataOmni/target/debug/dataomni")),
      InstallChannel::Standalone,
      Some(BundleType::App),
    ));
    // Windows MSI / NSIS 安装副本：允许应用内升级
    assert!(supports_in_app_update_from(
      "windows",
      Some(Path::new("C:\\Program Files\\DataOmni\\dataomni.exe")),
      InstallChannel::Standalone,
      Some(BundleType::Msi),
    ));
    assert!(supports_in_app_update_from(
      "windows",
      Some(Path::new("C:\\Users\\dev\\AppData\\Local\\Programs\\DataOmni\\dataomni.exe")),
      InstallChannel::Standalone,
      Some(BundleType::Nsis),
    ));
    // Windows 源码开发构建（bundle_type 为 None）：禁止自覆盖
    assert!(!supports_in_app_update_from(
      "windows",
      Some(Path::new("C:\\repo\\target\\debug\\dataomni.exe")),
      InstallChannel::Standalone,
      None,
    ));
    // Linux 系统包（.deb / .rpm）：禁止自覆盖
    assert!(!supports_in_app_update_from(
      "linux",
      Some(Path::new("/usr/bin/dataomni")),
      InstallChannel::SystemPackage,
      Some(BundleType::Deb),
    ));
  }

  #[test]
  fn tauri_config_embeds_valid_minisign_pubkey_without_forcing_local_signing() {
    use base64::Engine;

    let raw = include_str!("../../tauri.conf.json");
    let config: serde_json::Value = serde_json::from_str(raw).expect("valid tauri.conf.json");
    let updater = &config["plugins"]["updater"];
    let pubkey_b64 = updater["pubkey"].as_str().expect("plugins.updater.pubkey");
    let decoded = base64::engine::general_purpose::STANDARD
      .decode(pubkey_b64.trim())
      .expect("base64 minisign public key");
    let decoded_str = std::str::from_utf8(&decoded).expect("utf-8 minisign public key");
    assert!(
      decoded_str.starts_with("untrusted comment: minisign public key:"),
      "unexpected pubkey header: {decoded_str}"
    );

    let endpoints = updater["endpoints"].as_array().expect("plugins.updater.endpoints");
    assert_eq!(endpoints.len(), 1);
    assert_eq!(
      endpoints[0].as_str(),
      Some("https://github.com/dataomni-rs/DataOmni/releases/latest/download/latest.json"),
    );

    // 本地/Fork 构建不强制要求私钥，createUpdaterArtifacts 只在 release.yml 按需叠加
    assert!(config["bundle"].get("createUpdaterArtifacts").is_none());
  }

  /// `rfcs/auto-update.md` §4.2 篡改反向验证门：
  /// 1. `tauri.conf.json` 的生产公钥可被 `minisign_verify::PublicKey` 正常解码，且对未授权包体必然验签拒绝；
  /// 2. 合法 Ed25519 签名的更新包通过验签，篡改 1 字节包体或损坏 `.sig` 签名时必然被 `minisign_verify` 拒绝，
  ///    且 `classify_updater_error` 稳定映射为 `DATAOMNI_UPDATE_SIGNATURE_INVALID`。
  #[test]
  fn signed_update_verifies_and_rejects_one_byte_payload_or_signature_tampering() {
    use base64::Engine;

    fn verify_minisign(
      data: &[u8],
      signature_b64: &str,
      pubkey_b64: &str,
    ) -> Result<(), tauri_plugin_updater::Error> {
      let pubkey_raw = base64::engine::general_purpose::STANDARD.decode(pubkey_b64.trim())?;
      let pubkey_str = std::str::from_utf8(&pubkey_raw)
        .map_err(|_| tauri_plugin_updater::Error::SignatureUtf8(pubkey_b64.to_string()))?;
      let pubkey = minisign_verify::PublicKey::decode(pubkey_str)?;

      let sig_raw = base64::engine::general_purpose::STANDARD.decode(signature_b64.trim())?;
      let sig_str = std::str::from_utf8(&sig_raw)
        .map_err(|_| tauri_plugin_updater::Error::SignatureUtf8(signature_b64.to_string()))?;
      let signature = minisign_verify::Signature::decode(sig_str)?;

      pubkey.verify(data, &signature, true)?;
      Ok(())
    }

    // 固定测试向量：Minisign 公钥与对载荷 `b"test"` 的合法 Ed25519 签名
    const TEST_PUBKEY: &str = "untrusted comment: minisign public key E7620F1842B4E81F\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const TEST_SIG: &str = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1633700835\tfile:test\tprehashed\nwLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==";

    let encoded_pubkey = base64::engine::general_purpose::STANDARD.encode(TEST_PUBKEY);
    let encoded_sig = base64::engine::general_purpose::STANDARD.encode(TEST_SIG);

    // 1. 正向验证：签名与包体字节完全匹配时通过验签
    verify_minisign(b"test", &encoded_sig, &encoded_pubkey).expect("valid signature must pass");

    // 2. 反向验证（包体篡改 1 字节）：b"test" -> b"tesu"，必须报 DATAOMNI_UPDATE_SIGNATURE_INVALID
    let payload_err = verify_minisign(b"tesu", &encoded_sig, &encoded_pubkey)
      .expect_err("tampered payload must fail signature verification");
    let mapped_payload_err = classify_updater_error(&payload_err, UPDATE_DOWNLOAD_FAILED);
    assert!(
      mapped_payload_err.starts_with(UPDATE_SIGNATURE_INVALID),
      "expected {UPDATE_SIGNATURE_INVALID}, got: {mapped_payload_err}"
    );

    // 3. 反向验证（签名损坏 1 字符）：包体未变但 `.sig` 被篡改，同样映射为 DATAOMNI_UPDATE_SIGNATURE_INVALID
    let tampered_sig =
      base64::engine::general_purpose::STANDARD.encode(TEST_SIG.replacen("RUQf", "RUQg", 1));
    let sig_err = verify_minisign(b"test", &tampered_sig, &encoded_pubkey)
      .expect_err("tampered signature must fail verification");
    let mapped_sig_err = classify_updater_error(&sig_err, UPDATE_DOWNLOAD_FAILED);
    assert!(
      mapped_sig_err.starts_with(UPDATE_SIGNATURE_INVALID),
      "expected {UPDATE_SIGNATURE_INVALID}, got: {mapped_sig_err}"
    );

    // 4. 反向验证（生产公钥隔离）：拿合法的第三方签名去撞 tauri.conf.json 里的生产公钥，必然被拒
    let config: serde_json::Value =
      serde_json::from_str(include_str!("../../tauri.conf.json")).expect("valid tauri.conf.json");
    let prod_pubkey = config["plugins"]["updater"]["pubkey"].as_str().expect("prod pubkey");
    let wrong_key_err = verify_minisign(b"test", &encoded_sig, prod_pubkey)
      .expect_err("foreign key signature must be rejected by production pubkey");
    let mapped_wrong_key = classify_updater_error(&wrong_key_err, UPDATE_DOWNLOAD_FAILED);
    assert!(mapped_wrong_key.starts_with(UPDATE_SIGNATURE_INVALID));

    // 5. 状态机门禁：未验签暂存区绝不允许取出安装字节
    let empty_state = AppUpdateState::default();
    assert_eq!(take_verified_update(&empty_state).err().as_deref(), Some(UPDATE_NOT_DOWNLOADED));
  }
}
