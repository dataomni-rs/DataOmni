# v1.2.1 之后的交付闭环与 v1.3 路线

> 2026-11-18 定稿时的状态：`v1.2.0`（连接导入 `E1`、查询模板 `E2`、多页双语文档站 `W1–W4`、移除 `tauri-plugin-sql` `S1`、DuckDB/Oracle 23ai 闭环 `V2`）与 `v1.2.1`（Apple Developer ID 签名、内层 `.app` + 外层 `.dmg` 双层公证票据钉合、官方 Homebrew Tap `dataomni-rs/homebrew-tap` 上线）均已正式发布并完成本机真机验收。
> 本文记录 `v1.2.1` 之后重新盘点的真实工程优化项、跨平台闭环任务、签名解锁后的平滑更新与商店变体评估；具体任务的勾选状态在根目录 `TODOs.md`「v1.3 路线」里维护。

## 一、现状盘点与核心判断

到 `v1.2.1` 为止，DataOmni 在功能面（17 种数据库兼容、代码生成、结构比对、`:name` 绑定参数、查询模板、第三方连接导入、只读 CLI 与 MCP）和 macOS 独立分发体验（公证 `.dmg` + `brew install --cask` 零拦截）上已经形成完整闭环。

在跑完 `v1.2.1` 全套 CI、签名公证与 Homebrew 发布流水线后，仓库当前真实存在的待办与值得投入的优化集中在以下四个维度：

| 维度 | 实测事实与依据 | 风险与收益 |
| --- | --- | --- |
| CI / 发布流水线效率与自动化缝隙 | 1. `.github/workflows/ci.yml` 的免编译白名单写了 `scripts/winget-manifest.sh`，未包含新增的 `scripts/homebrew-cask.sh`，导致仅改打包脚本的 PR（`#48`）白跑了 28 分 30 秒的 Windows/Rust 全量编译；<br>2. `ci.yml`、`release.yml`、`site.yml`、`nightly-databases.yml` 中的 `actions/checkout@v4`、`actions/cache@v4`、`actions/upload-artifact@v4`、`actions/download-artifact@v4` 每次运行均产生 Node 20 弃用告警；<br>3. `site.yml` 已实现 `release: [published]` 自动发官网，但 `dataomni-rs/homebrew-tap` 仍需发版后手工跑脚本推送。 | 改动都在数十行 YAML 以内：补齐路径过滤可把打包类 PR 从 28 分钟降到 7 秒；升级 Actions 可清空流水线黄标噪音；自动推 Tap 可消除发版手工步骤遗漏。 |
| 跨平台打包版实机验证与定时回归稳态 | 1. `S1`（SQLite 后端自管池）、`QS6`（严格 CSP）与 `C2`（`dataomni mcp`）已在 macOS 打包版实机验证，但 Windows 11（WebView2，MSI/NSIS）与 Linux（WebKitGTK，deb/rpm）桌面环境尚未跑完实机核验清单（`V1`）；<br>2. `nightly-databases.yml` 七套真库最新轮次已全绿，仍需完成连续四周观察与兼容库不支持类型的夹具裁剪（`V3`）。 | 消除不同操作系统 WebView 对严格 CSP、二进制流与 stdio 子进程的最后平台盲区；保证定时回归「红 = 真实代码退化」。 |
| 签名公证解锁后的平滑更新与全渠道合规 | 1. `roadmap-1.0` `R3` / `roadmap-1.2` `E3`（应用内签名校验平滑更新）此前受制于 macOS 无公证签名而暂缓，现前置条件已在 `v1.2.1` 解锁；<br>2. 官网尚缺独立的 `/privacy/` 双语隐私与安全合规声明页（企业采购安全审计与未来应用商店上架的必填项）；<br>3. WinGet 上游审核（`microsoft/winget-pkgs#481999`）、Windows 代码签名评估与 MCP 生态目录投稿仍在推进中。 | 让非包管理器用户无需每次手动跳浏览器下载重装；同时识别 Homebrew Cask 等包管理器副本避免自更新覆盖冲突。 |
| 可选驱动解耦与商店沙盒变体准备 | 1. 当前内置 Oracle Instant Client 依赖 `disable-library-validation` 加载 Oracle 官方签名的 `.dylib`；而未来上架 Mac App Store（MAS）强制要求开启 `app-sandbox`、禁止 `disable-library-validation` 且要求包内全部动态库同签名；<br>2. 接入达梦（DM8）等受分发限制的国产驱动同样需要按 Cargo feature 拆分驱动（原 `B4`）；KingbaseES（原 `B1b`）等待试用镜像。 | 先按真实边界把重型/受限外部客户端库拆成默认开启的 Cargo feature，既保持官网/Homebrew 版开箱即用，又为国产专有驱动与 Mac App Store 沙盒变体打通编译前提。 |

总纲保持不变：**奔着实用、好用、易用、功能稳定去——按 P（流水线提效）→ V（跨平台与真库闭环）→ U（平滑更新与分发合规）→ M（驱动解耦、国产库与商店变体）四阶段推进。**

---

## 二、四阶段推进次序与验收标准

### 第一阶段（P）：CI / 发布流水线提效与自动化收口

目标是零业务风险地把研发与发版基础设施磨顺，减少无效等待与手工操作。

- **P0 `TODOs.md` 第四次归档与路线重排**
  - **做法**：将已完成 `v1.2.0`、`v1.2.1`（`S0–S4`、`V2`、`V4`、`W1–W4`、`E1`、`E2`）验收记录的原文件逐字备份至 `docs/archive/TODOs-2026-11-18.md`，同步更新 `AGENTS.md` 权威来源表；根目录 `TODOs.md` 切换为精简清晰的新路线任务清单与可执行重估门。
  - **验收**：归档完整保留所有提交号与反向验证记录；仓库文档引用无断链。

- **P1 补齐 `.github/workflows/ci.yml` 打包脚本路径过滤**
  - **做法**：在 `ci.yml` 的 `Detect changed scopes` 免编译白名单中补入 `scripts/homebrew-cask.sh`（核对其余仅用于归档/发布的纯打包辅助路径），保持应用代码、构建脚本与 `README.md` 改动照常触发全量 `app_changed=true`。
  - **验收**：仅修改 `packaging/*` 或 `scripts/homebrew-cask.sh` / `scripts/winget-manifest.sh` 时 `app_changed=false` 且 `site_changed=false`；触碰 `src/`、`src-tauri/`、`scripts/install-macos.sh` 或 `scripts/schema-diff-verify.sh` 时仍触发 `app_changed=true`。

- **P2 升级 GitHub Actions 官方步骤版本，清零 Node 20 弃用告警**
  - **做法**：统一升级 `.github/workflows/{ci,release,site,nightly-databases}.yml` 中的官方 Action 至原生支持 Node 24 的稳定大版本，并核查 `ubuntu-latest` 迁移提示涉及的系统包兼容性。
  - **验收**：PR CI 运行完成后 `ANNOTATIONS` 区不再出现 Node 20 弃用告警，全部既有门禁全绿。

- **P3 发版公开后自动同步 `dataomni-rs/homebrew-tap`**
  - **做法**：新增受控工作流（或在发布后钩子中），在 GitHub Release `published`（过滤 `prerelease == false` 与 `draft == false`）时下载当次发布的 `DataOmni_<ver>_aarch64.dmg`，运行 `scripts/homebrew-cask.sh <ver>` 校验服务端 SHA-256 与公证票据，经最小权限 Deploy Key / Fine-grained PAT 自动更新 `dataomni-rs/homebrew-tap` 的 `Casks/dataomni.rb`（并同步回写主仓或开 PR）。未配置跨仓密钥时静默跳过并打印提示，不阻塞发版。
  - **验收**：脚本对未公证包退出码仍为 1；配置密钥后公开正式版可自动将 Tap 升至对应版本且 `brew style` 零告警。

- **P4 跟进构建期 glob 依赖安全补丁（收口 `QS2 / QS3`）**
  - **触发条件**：上游发布修复 `GHSA-g3hv-75wg-m249` / `GHSA-fc5q-39xc-vjpf` 的 `minimatch` 与 `picomatch` 版本。
  - **验收**：`bun update` 后摘除 `package.json` `audit:frontend` 中的临时 `--ignore`，`bun run audit:frontend` 与 `bun run check` 全绿。

---

### 第二阶段（V）：跨平台打包版实机闭环与 Nightly 真库稳态

- **V1 Windows 11 与 Linux 打包版实机闭环验收**
  - **范围**：在 Linux（WebKitGTK 桌面环境，`.deb` / `.rpm`）与 Windows 11（WebView2，`.msi` / `-setup.exe`）正式构建包上逐项验证：
    1. SQLite 经后端 `sqlx_pool` 打开、对象树与结构页读取、单元格编辑预览提交、多连接独立关闭、不存在路径不误建空文件；
    2. 严格 CSP 下 SQL 编辑器、ER 图导出 PNG、MongoDB / Redis / Neo4j / Elasticsearch 四大工作区渲染及更新检查正常，`dataomni.log` 零 CSP 违规记录；
    3. 打包二进制直接运行 `dataomni mcp` 经 stdio 跑通只读查询、写语句 `refused` 与 `PROD` 连接 `not_found`。
  - **验收**：两平台清单全部实测通过并记录结果；若发现平台 WebView 差异缺陷，先补回归单测再发修订包复验。

- **V2 持续真库回归（Nightly Smoke）稳态收口（承接原 `V3` / `QT3`）**
  - **范围**：跟踪 `.github/workflows/nightly-databases.yml` 连续四周定时运行结果；对 CockroachDB / openGauss 等兼容库本身不支持的数据类型夹具（如多维数组 `INT[][]`、`XML`）按服务端真实能力裁剪测试断言。
  - **验收**：连续四周定时任务无非退化假红，「绿 = 正常，红 = 真实退化」。

---

### 第三阶段（U）：应用内签名平滑更新与全渠道分发合规

- **U1 应用内签名校验平滑更新（承接原 `E3` / `R3`）**
  - **前置设计**：动手前先写最小设计说明 `rfcs/auto-update.md`，明确三件事：
    1. **密钥与清单**：采用 `tauri-plugin-updater` 的 Ed25519 公私钥机制（公钥进 `tauri.conf.json`，私钥进 GitHub Actions Secret），`release.yml` 产出签名更新包与 `latest.json`；
    2. **渠道防冲突**：检测运行路径或安装来源——通过 Homebrew Cask（或 Linux 系统包 `.deb` / `.rpm`）管理的安装引导用户使用包管理器升级（如复制 `brew upgrade --cask dataomni`），避免自更新覆盖包管理器账本；Windows 在未配代码签名前评估安装器交互体验；
    3. **用户控制权**：完全复用设置页已有的「自动检查更新」开关（关闭时零后台请求），更新前清晰展示版本号与变更说明链接，由用户显式点击确认后下载替换，**绝不静默后台强更重启**（保护未提交事务与打开的查询标签）。
  - **验收**：纯函数渠道判定与版本比对单测先红后绿；签名篡改反向验证必败；macOS 真机从旧测试包平滑升至新签名包后钥匙串免重输口令直连成功。

- **U2 官网双语隐私与安全合规声明页（`/privacy/` 与 `/zh/privacy/`）**
  - **做法**：严格走独立官网 PR（仅改 `site/`），复用 `site/src/docs.rs` 文档壳新增双语隐私与安全边界声明：明确零遥测、无云账号、数据库直连、口令/私钥密码/API Key 仅存操作系统钥匙串、可选版本检查可关、可选 AI 直连模型服务且支持 `--no-default-features` 编译期物理剔除、Agent 访问默认关闭且 `PROD` 物理不可见。页脚补齐入口并纳入 `verify_html_invariants` 门禁。
  - **验收**：`bun run site:check` 全绿，桌面端与 375px 移动端深浅色渲染核对通过。

- **U3 在途外部渠道收口、Windows 签名评估与生态目录推广**
  - **WinGet（收口原 `S4 / U2`）**：`microsoft/winget-pkgs#481999` 合入后用 `scripts/winget-manifest.sh` 提交最新稳定版，分别在主仓 README 与官网（独立 PR）补上 `winget install DataOmni.DataOmni`，并在下次发版时对齐 `tauri.conf.json` 的 `bundle.publisher`；
  - **上游 `Homebrew/homebrew-cask`**：待 GitHub 仓库星标（≥75 / 自提 ≥225）且 `dataomni.dev` 域名满 30 天后，将 `packaging/homebrew/Casks/dataomni.rb` 原样提交至官方核心库；
  - **Windows 代码签名评估**：按 Windows 用户安装反馈评估接入 Azure Trusted Signing 云端签名消除 SmartScreen 拦截；
  - **生态目录投稿**：按 `packaging/directories/README.md` 向 `modelcontextprotocol/servers`、Smithery、Glama、`awesome-tauri`、`awesome-rust` 提交收录。

---

### 第四阶段（M）：可选驱动解耦、国产库扩展与商店沙盒变体（按需触发）

- **M1 数据库驱动按 Cargo feature 解耦（承接原 `B4`）**
  - **为什么做**：把带外部动态库或平台限制的驱动（首先以 `oracle` 为切入点）收进默认开启的 Cargo feature（`default = ["ai", "oracle"]`）。官网 DMG 与 Homebrew 版默认全开、体验零变化；需要无闭源动态库构建（如内网审计或 Mac App Store 变体）时可通过 Cargo feature 编译期干净剥离，前端通过能力查询隐藏未编译进来的驱动项。
  - **验收**：默认构建与 `--no-default-features` 均通过 `cargo clippy` 与单测门禁，官网/Homebrew 独立安装包仍完整内置 Oracle Instant Client。

- **M2 国产与更多数据库按安装包接入（承接原 `E4` / `B1b`）**
  - **触发条件**：拿到 **KingbaseES（人大金仓）** 试用镜像，或出现明确的 **达梦（DM8）** 接入需求（基于 `M1` feature 隔离）。
  - **验收**：跑通连接、结构元数据、SQL 执行、分页、导入导出与真库冒烟测试全套门禁后才标「已支持」。

- **M3 Mac App Store（MAS）沙盒构建变体（可选分发渠道，前置 `U2` + `M1`）**
  - **触发条件**：决定启动 Mac App Store 上架时开启，动手前先写 `rfcs/mac-app-store.md`。
  - **核心范围**：
    1. 基于 `M1` 关闭内置 `oracle` 特性（规避 MAS 严禁 `disable-library-validation` 与 Oracle 库不可重签名的许可证冲突）；
    2. 新增 `Entitlements.mas.plist`（`app-sandbox`、`network.client`、`network.server`、`files.user-selected.read-write`）与 WebKit 子进程 `Entitlements.mas.inherit.plist`，补齐 `ITSAppUsesNonExemptEncryption = false`；
    3. 为 SQLite（含同目录 `-wal`/`-shm` 权限处理）、DuckDB、SSH 私钥与自定义 TLS 证书补齐 macOS **Security-Scoped Bookmarks** 持久化访问，沙盒模式下将 SSH `known_hosts` 存入应用容器目录并收起外部连接默认目录扫描；
    4. 提供免配置一键体验的内置只读/内存示例 SQLite 库，确保 App Store 审核员无外部数据库服务器即可完整体验核心工作台。

---

## 三、当前阶段明确不做（及重估条件）

| 明确不做 | 为什么不做 | 可执行的重估条件 |
| --- | --- | --- |
| 后台静默强制自动更新并重启应用 | 数据库客户端常持有未提交事务、SSH 隧道与长查询结果，静默强更重启会直接毁掉现场 | 不重估（交互安全红线） |
| 为上 Mac App Store 而阉割官网 DMG / Homebrew 版能力 | 独立分发版（DMG / Homebrew）面向专业开发者，必须完整保留内置 Oracle Instant Client、`~/.ssh` 直读、`dataomni cli` 与 `dataomni mcp` | 不重估（若做 MAS 只走独立 feature 构建变体） |
| 未收到真实需求前开启 `dataomni cli` 第三批写操作（`A8a`） | 只读 CLI 与只读 MCP 已满足排障与分析闭环，命令行写操作风险面完全不同 | 有明确生产外自动化写场景诉求并完成 `rfcs/agent-cli.md` §6 护栏评审时 |
| 无性能瓶颈证据的前端拆包（`QR2`）或大文件形式化重构 | 当前主包 gzip 605 KB 本地秒开，2300+ 前端单测与 450+ 后端单测全绿 | 见 `TODOs.md` 对应可执行重估门 |
