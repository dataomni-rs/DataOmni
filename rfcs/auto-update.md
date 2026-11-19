# 应用内签名校验平滑更新设计（U1）

> 状态：2026-11-19 已实现（跟代码走：`install_channel`、`tauri-plugin-updater` Ed25519 签名验签、三步显式升级确认与 `scripts/build-latest-json.py` 发布清单汇总均已落地）。

## 1. 背景与目标

在 `v1.2.1` 完成 macOS Apple Developer ID 签名、双层 Apple 公证钉合与官方 `dataomni-rs/homebrew-tap` 上线之前，DataOmni 只做「启动后最多每 24 小时查询一次 GitHub Latest Release 并展示提示条」（`src/utils/updateCheck.ts`、`UpdateNotice.tsx`），点击「查看」在系统浏览器打开对应 Release 页面。

前置条件解锁后，`U1` 要解决两个真实问题：
1. **包管理器渠道防冲突**：通过 `brew install --cask dataomni-rs/tap/dataomni`（macOS）或 `.deb` / `.rpm`（Linux）安装的副本由包管理器维护版本账本（Caskroom 或 `/usr/bin/dataomni` root 权限文件）。如果在应用内直接自覆盖 `/Applications/DataOmni.app`，会导致 `brew list --cask --versions` 与实际二进制脱节；在 Linux `.deb` / `.rpm` 下则会因无 `/usr/bin` 写权限直接报错。
2. **独立安装包免浏览器平滑升级**：通过官网 `.dmg` 拖入 `/Applications` 的 macOS 用户与 Windows 安装器用户，应当能在确认新版本说明后，经 Ed25519 签名校验直接下载替换，无需每次手动跳浏览器下载重装；且 macOS 同团队证书（`PGX36YNATP`）升级后系统钥匙串免重输口令直连。

## 2. 安全与交互红线（绝不妥协）

1. **绝不后台静默下载或强制重启**：数据库客户端常持有未提交事务（`BEGIN`）、SSH 隧道长连接、未保存的查询脚本与大结果集。更新流程固定为三步显式动作：
   - 顶栏下方轻量提示条展示新版本号；
   - 用户主动点击「升级」后展示版本说明摘要与当前未保存/事务提示；
   - 下载并完成 Ed25519 签名校验后，由用户主动点击「立即重启完成更新」（或留待下次关闭重开生效）。
2. **完全受现有开关控制**：复用 `Settings -> About & Diagnostics` 已有的「自动检查更新」（`dataomni.update-check` `enabled`），关闭时启动期零联网请求（无论 GitHub API 还是 `latest.json`）。
3. **无密钥开发构建零侵入**：`src-tauri/tauri.conf.json` 默认不开硬编码的 `bundle.createUpdaterArtifacts: true`，仅在 `.github/workflows/release.yml` 检测到 `TAURI_SIGNING_PRIVATE_KEY` Secret 非空时通过 `--config` 叠加开启，保证本地 `bun tauri build`、`scripts/install-macos.sh` 与外部 Fork 构建无需私钥即可正常打包。

## 3. 安装渠道判定（`install_channel`）

后端命令 `install_channel`（`src-tauri/src/commands/app_commands.rs`）基于当前进程路径与本机包管理器目录做无副作用的纯判定：

| 返回枚举 | 判定条件 | 更新交互行为 |
| --- | --- | --- |
| `homebrew` | `os == "macos"`，可执行文件位于 `.../DataOmni.app/Contents/MacOS/dataomni`，且 `/opt/homebrew/Caskroom/dataomni` 或 `/usr/local/Caskroom/dataomni` 存在 | 不触发自覆盖替换；提示条直接提供一键复制 `brew upgrade --cask dataomni` 与「查看」发布页 |
| `systemPackage` | `os == "linux"`，且可执行文件路径位于 `/usr/`（`.deb` / `.rpm` 系统包安装位置） | 不触发自覆盖替换（Linux 不分发 AppImage，见 TODOs 5.4）；保留「查看」跳转发布页下载对应 `.deb` / `.rpm` |
| `standalone` | 其余情形（macOS `.dmg` 独立拖装、Windows `.msi` / `-setup.exe`、开发构建） | 接入 `tauri-plugin-updater` 后走 Ed25519 签名校验应用内下载更新；未启用更新插件前打开官方发布页 |

> 边界情况：开发机同时装了 Homebrew 版 `/Applications/DataOmni.app` 与源码目录 `target/debug/dataomni` 时，源码二进制路径不以 `DataOmni.app/Contents/MacOS/dataomni` 结尾，判定为 `standalone`，不会误判成 Homebrew 副本。

## 4. `tauri-plugin-updater` 密钥与发布流水线方案

### 4.1 Ed25519 密钥托管
1. 在本机仓库外密钥目录生成专用更新签名密钥对：
   ```bash
   bun tauri signer generate -w ~/.private_keys/dataomni-updater.key
   ```
2. 将公钥字符串写入 `src-tauri/tauri.conf.json`：
   ```json
   "plugins": {
     "updater": {
       "pubkey": "<BASE64_MINISIGN_PUBKEY>",
       "endpoints": [
         "https://github.com/dataomni-rs/DataOmni/releases/latest/download/latest.json"
       ],
       "windows": {
         "installMode": "passive"
       }
     }
   }
   ```
3. 将私钥与密码上传至 `dataomni-rs/DataOmni` 仓库 GitHub Actions Secrets：
   - `TAURI_SIGNING_PRIVATE_KEY`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

### 4.2 `release.yml` 产物与清单生成
1. `package` 矩阵在检测到 `TAURI_SIGNING_PRIVATE_KEY` 非空时追加 `--config '{"bundle":{"createUpdaterArtifacts":true}}'`：
   - **macOS (`aarch64`)**：在 Apple Developer ID 签名与 `.app` 公证钉合完成后产出 `DataOmni.app.tar.gz` 与 `.sig`（内含已钉合公证票据的 `DataOmni.app`）；
   - **Windows (`x64`)**：产出签名校验用的更新包与 `.sig`；
   - **Linux (`x86_64`)**：因不发布 AppImage，不写入 `latest.json` 的 `linux-x86_64` 平台项（Linux `.deb` / `.rpm` 走 `systemPackage` 分支）。
2. `release` 汇总任务从正式版（非预发布 `-rc`）构建产物组装标准 Tauri v2 `latest.json`（含 `version`、`pub_date`、`notes`、`platforms.darwin-aarch64`、`platforms.windows-x86_64`），随安装包一并挂入草稿 Release；公开 Release 后 `releases/latest/download/latest.json` 原子生效。
3. **篡改反向验证门**：单测构造合法 `latest.json` 与篡改过 1 字节的 `.sig` / 压缩包，断言 `tauri-plugin-updater` 验签必然拒绝且不落地任何可执行文件。
