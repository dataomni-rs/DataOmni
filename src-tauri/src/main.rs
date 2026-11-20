// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  // 终端调用（`dataomni --help`、`dataomni connections`、`dataomni cli ...`、`dataomni mcp`）
  // 在起 Tauri 之前直接无头执行并退出，不启动桌面窗口；和应用共用同一二进制，钥匙串才能认得它
  let args: Vec<String> = std::env::args().collect();
  match dataomni_lib::cli::classify_invocation(&args[1..]) {
    dataomni_lib::cli::Invocation::Gui => dataomni_lib::run(),
    dataomni_lib::cli::Invocation::Cli(cli_args) => {
      #[cfg(windows)]
      attach_parent_console();
      std::process::exit(dataomni_lib::cli::run(cli_args));
    }
    dataomni_lib::cli::Invocation::Mcp(mcp_args) => {
      #[cfg(windows)]
      attach_parent_console();
      std::process::exit(dataomni_lib::cli::serve_mcp(mcp_args));
    }
  }
}

/// 发布版是 GUI 子系统，从终端直接敲时没有标准输出。借用父进程（终端）的控制台；
/// 已经被重定向到管道或文件的句柄（Agent 就是这样调的）原样留着
#[cfg(windows)]
fn attach_parent_console() {
  use windows_sys::Win32::System::Console::{
    AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_HANDLE,
    STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
  };
  const STREAMS: [STD_HANDLE; 3] = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];
  // SAFETY: 只读写本进程的标准句柄表，传入的都是系统定义的常量
  unsafe {
    let before = STREAMS.map(|stream| GetStdHandle(stream));
    if before[1..].iter().all(|handle| !handle.is_null()) {
      return;
    }
    if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
      return;
    }
    for (stream, handle) in STREAMS.into_iter().zip(before) {
      if !handle.is_null() {
        SetStdHandle(stream, handle);
      }
    }
  }
}
