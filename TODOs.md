# DataOmni 开发路线与完成度追踪

> **归档**：本文件只留**还没做完的**、**接下来要做的**，以及**「当前版本不做」的可执行重估条件**。做完的条目怎么做、怎么取舍、怎么反向验证，都原样留在归档里：
> - [`docs/archive/TODOs-2026-11-18.md`](docs/archive/TODOs-2026-11-18.md)：到 `v1.2.1`（Apple Developer ID 签名公证与官方 Homebrew Tap 上线）为止。代码与测试里写的「TODOs S0–S4」「TODOs V2 / V4」「TODOs W1–W4」「TODOs E1–E2」指这一份。
> - [`docs/archive/TODOs-2026-11-17.md`](docs/archive/TODOs-2026-11-17.md)：到 `v1.1.0` 与官网 `dataomni.dev` 上线为止。代码与测试里写的「TODOs R1–R7」「TODOs C1 / C2 / D2 / D3」「TODOs QS1–QS6 / QT1–QT5 / QR1–QR2」「TODOs PA1–PA8」「TODOs G1–G13」「TODOs A8a」指这一份。
> - [`docs/archive/TODOs-2026-10-06.md`](docs/archive/TODOs-2026-10-06.md)：到 `v0.5.1` 发布为止。代码与测试里写的「TODOs A6b」「TODOs B2b」「TODOs「P0-P3 无阻塞级缺陷」」指这一份。
> - [`docs/archive/TODOs-2026-09-27.md`](docs/archive/TODOs-2026-09-27.md)：更早的记录。「TODOs 4.2「ClickHouse」」这类章节号指这一份。
>
> 原则不变：动手前先评估，结论要带「当前版本」的限定；「不做」要写成可执行的重估条件；做完先提交，再回来改这里的状态；**应用/配置变更与 `site/` 官网变更严禁混在同一个 PR 中**。

## 使用规则

- `[ ]` 未开始　`[-]` 进行中　`[x]` 已完成并通过验收　`[!]` 阻塞（条目下说明原因）
- 完成时在条目末尾补上提交号和验证方式；做法、取舍和反向验证的记录写在条目下面
- 新接一种库，要补齐连接、元数据、执行、分页、导入导出和测试，达到核心验收标准后才标「已支持」
- 这份文件长到约 100 KB 时，照原样再归档一次
- **收到缺陷报告先搜归档**：回归时看到但没修的各处都写了重估条件，多半是「有人报」。有人报了，对应的条件就触发了

## 总体进度

| 阶段 | 目标 | 状态 |
|---|---|---|
| P0–P5 | 核心连接查询内核、日常工作闭环、NoSQL 工作区、统一桌面 UI 与交互 | [x] |
| P6–加固 | AI 设计与备份、只读 CLI、依赖审计、前端零 `sql:*` 权限与严格 CSP | [x] A0–A6c、A9、A8a（只读）、QS1–QS6、QT1–QT5、QR1–QR2、PA1–PA8 完成 |
| v1.0 | 稳定桌面客户端（`rfcs/roadmap-1.0.md`） | [x] `v1.0.0` 2026-11-05 正式发布 |
| v1.1 与官网 | 代码生成、结构比对、绑定参数、只读 MCP 与 `dataomni.dev` 上线 | [x] `v1.1.0` 2026-11-16 发布，官网 `https://dataomni.dev` 2026-11-17 上线 |
| v1.2 | 移除 `tauri-plugin-sql`、多页双语文档、连接迁移导入、查询模板、macOS 签名公证、Homebrew Tap、应用内签名更新、双语隐私页与驱动 feature 解耦（`rfcs/roadmap-1.2.md`、`rfcs/roadmap-1.3.md` 核心项） | [x] `v1.2.0`（2026-11-17）、`v1.2.1`（2026-11-18）与 `v1.2.2`（2026-11-19，含 `P0–P3`、`U1`、`U2`、`M1`）正式发布 |
| v1.3.0 夯实根基 | 内置 DuckDB 统一分析与可视化（DA）、CLI/MCP 可用性增强（CM）、应用内克制型 AI 增强（AI）与跨平台实机验收（`V1`） | [x] `DA0–DA3`、`CM1–CM4`、`AI0–AI2`、顶层无头 CLI 与 TTY 高亮、3 查询批量 `diff` 与 MCP SSH 复用、分析/外键跳转 UX 打磨均已合入 `main`（PR `#59`–`#63`），`v1.3.0` 标签已推送触发发版（`V1` 跨平台实机验收继续跟踪） |
| v1.4 多模型分析 | 结构化表级抽样与多表对账、Redis 空间/TTL/BigKey 体检、MongoDB 嵌套 Schema 漂移与僵尸索引、ES 原生聚合/Mapping 诊断、Neo4j 拓扑体检与分析增强（[`rfcs/multi-engine-analytics.md`](rfcs/multi-engine-analytics.md)） | [-] `AN0–AN6` 已合入 `main`（PR `#64`）；`AN7–AN10` 高价值低膨胀增强推进中 |
| 持续运维与条件项 | `P4` 依赖补丁、`V2` Nightly 稳态、`U3` 外部渠道与目录、`M2` 国产库、`M3` MAS 沙盒变体（`rfcs/roadmap-1.3.md` 长期跟踪项） | [-] 按上游审核、实机环境与外部触发条件持续推进，不阻塞 `v1.3.0` 发版 |

---

## v1.2.2 交付闭环与持续运维 / 条件触发项（原 `rfcs/roadmap-1.3.md` 跟踪池）

原 [`rfcs/roadmap-1.3.md`](rfcs/roadmap-1.3.md) 中的全部核心代码项（`P0–P3`、`U1`、`U2`、`M1`）均已合并并随 `v1.2.2` 正式发布；以下保留该池中的实机验收门（`V1`，随 `v1.3.0` 一起验收收口）及外部/按条件触发的长期跟踪项（`P4`、`V2`、`U3`、`M2`、`M3`）。

### 第一阶段（P）：CI / 发布流水线提效与自动化收口

- [x] P0 `TODOs.md` 第四次归档与 `v1.3` 路线重排（2026-11-18）
  - 已把包含 `v1.2.0`、`v1.2.1`（`S0–S4`、`V2`、`V4`、`W1–W4`、`E1`、`E2`）验收记录的原文件逐字备份至 `docs/archive/TODOs-2026-11-18.md`，新增 [`rfcs/roadmap-1.3.md`](rfcs/roadmap-1.3.md) 并同步更新 `AGENTS.md` 权威来源表。
- [x] P1 补齐 `.github/workflows/ci.yml` 打包脚本路径过滤白名单（`31875ca`，`src/ciWorkflows.test.ts` 先红后绿）
  - 已将 `scripts/homebrew-cask.sh`、`README.en.md`、`.github/workflows/homebrew-tap.yml` 与 `.github/workflows/windows-install.yml` 纳入 `Detect changed scopes` 免编译白名单；新增 `src/ciWorkflows.test.ts` 直接提取执行 `ci.yml` 的分支规则，反向验证打包辅助路径 `app_changed=false` 而 `README.md`、`scripts/install-macos.sh`、`src-tauri/*` 仍触发 `app_changed=true`。
- [x] P2 升级 GitHub Actions 官方步骤版本，清零 Node 20 弃用告警（`01bcaf2`，PR `#50` 运行 `44334307233` 6 项任务 warning 告警数 0）
  - 已将全部工作流（`ci.yml`、`release.yml`、`site.yml`、`nightly-databases.yml`、`windows-install.yml`）中的 `actions/checkout` 升至 `@v7`、`actions/cache` 升至 `@v6`、`actions/upload-artifact` 升至 `@v7`、`actions/download-artifact` 升至 `@v8`、`actions/upload-pages-artifact` 与 `actions/deploy-pages` 升至 `@v5`（全部 `using: node24`），并核实 `ubuntu-latest` 已平滑运行在 Ubuntu 26.04.1 LTS（`release.yml` Linux 发版包继续钉死 `ubuntu-22.04` 保 glibc 兼容）；`src/ciWorkflows.test.ts` 增加最低主版本门禁。
- [x] P3 发版公开后自动同步 `dataomni-rs/homebrew-tap`（`5b701e5`，`macos-latest` 运行 `44336357193` 56 秒实测通过）
  - 已新增 `.github/workflows/homebrew-tap.yml`（监听 `release: [published]` 排除 `draft` / `prerelease`，并支持 `workflow_dispatch` 指定版本），在 `macos-latest` 上调用 `scripts/homebrew-cask.sh` 复核服务端 SHA-256、`xcrun stapler validate`、`spctl` 公证票据与 `brew style`；配置 `HOMEBREW_TAP_TOKEN` 时幂等更新 `dataomni-rs/homebrew-tap` 的 `Casks/dataomni.rb`，未配置时输出 `::notice::` 正常结束；同步更新 `packaging/homebrew/README.md`。
- [ ] P4 跟进构建期 glob 依赖安全补丁（收口 `QS2 / QS3`）
  - 重估触发条件：上游发布修复 `GHSA-g3hv-75wg-m249` / `GHSA-fc5q-39xc-vjpf` 的 `minimatch` 与 `picomatch` 版本后，执行 `bun update` 并摘除 `package.json` `audit:frontend` 中的临时 `--ignore`（2026-11-18 核查 npm：`picomatch` 最新仍为 `4.0.7`，`typescript-eslint` 依赖的 `minimatch` `10.x` 最新仍为 `10.2.6`，尚未发布补丁版）。

### 第二阶段（V）：跨平台打包版实机闭环与 Nightly 真库稳态

- [ ] V1 Windows 11 与 Linux 打包版实机闭环验收（承接 `v1.2` `V1`；实机验收步骤与一键脚本见 [`docs/packaged-verification-checklist.md`](docs/packaged-verification-checklist.md)，平台专项清单见 [`docs/windows-checklist.md`](docs/windows-checklist.md) 与 [`docs/linux-checklist.md`](docs/linux-checklist.md)）
  - 在 Linux（WebKitGTK 桌面容器，`.deb` / `.rpm`）与 Windows 11（WebView2，`.msi` / `-setup.exe`）打包产物上逐项实测：
    1. SQLite 经后端 `sqlx_pool` 打开、对象树与结构页读取、编辑预览提交、多连接独立关闭、不存在路径不误建空文件；
    2. 开启严格 CSP 后，SQL 编辑器、ER 图 PNG 导出、MongoDB / Redis / Neo4j / Elasticsearch 四大工作区及更新检查正常，`dataomni.log` 零 CSP 报错；
    3. 打包二进制执行 `dataomni mcp` 经 stdio 跑通只读查询、写语句 `refused` 与 `PROD` 连接 `not_found`。
- [ ] V2 持续真库回归（Nightly Smoke）四周稳态收口（承接 `v1.2` `V3` / `QT3`）
  - 跟踪 `.github/workflows/nightly-databases.yml` 连续四周定时运行记录；按服务端真实能力裁剪兼容库上的不支持类型夹具（如 CockroachDB / openGauss 的 `INT[][]`、`XML`），确保「绿 = 正常，红 = 真实退化」。

### 第三阶段（U）：应用内签名平滑更新与全渠道分发合规

- [x] U1 应用内签名校验平滑更新（承接 `v1.2` `E3` / `roadmap-1.0` `R3`，前置签名公证已解锁）（`f9678bf`、`edf7e96`）
  - [x] 设计说明与安装渠道识别引导（`f9678bf`）：已撰写 [`rfcs/auto-update.md`](rfcs/auto-update.md)，落地后端 `install_channel`（区分 `homebrew`、`systemPackage`、`standalone`，排除源码开发构建误判）与前端 `UpdateNotice` Homebrew Cask 一键复制 `brew upgrade --cask dataomni`，`UpdateNotice.test.tsx` 先红后绿。
  - [x] 独立安装包 `tauri-plugin-updater` 签名替换（`edf7e96`、`v1.2.2` 运行 `44445610282`）：已接入生产 Minisign Ed25519 公钥（`52232E86440485D4`）与 `latest.json` HTTPS 端点，落地后端受控三命令（`check_app_update` / `download_app_update` / `install_app_update`，拦截包管理器与开发构建自覆盖）、`UpdateNotice` 三步显式确认与 `App` 事务/任务/快照退出守卫，以及 `scripts/build-latest-json.py` 与 `release.yml` 公证解包门禁；`v1.2.2` 正式版全套 10 项签名产物、`latest.json` 5 个平台目标 Key ID 验签、Windows `1.2.1 → 1.2.2` 覆盖升级及 `dataomni-rs/homebrew-tap`（`a44970b`）全部验收通过。
- [x] U2 官网双语隐私与安全合规声明页（`/privacy/` 与 `/zh/privacy/`，独立官网 PR `#51` / `1642fc0` 已上线）
  - 已在 `site/` 新增中英双语隐私政策与数据安全声明页（零遥测、无云账号、凭据只进系统钥匙串、三类出网请求完整披露、只读 Agent `PROD` 物理不可见、`--no-default-features` 编译剔除），补齐页脚、侧边栏与 `sitemap.xml` 并修复窄屏 `.docs-sidebar` 横向溢出；`site:check`、桌面深浅色与真实 375px 视口（`scrollWidth=375`）及线上 `https://dataomni.dev/privacy/`、`https://dataomni.dev/zh/privacy/`（HTTP 200）全部核验通过。
- [-] U3 在途外部渠道收口、官网 SEO/GEO 与生态目录推广
  - [x] `HOMEBREW_TAP_TOKEN` 跨仓自动同步闭环（2026-11-19，运行 `44457288934` 实测幂等通过；本机脚本已归档 `~/.private_keys/`）。
  - [ ] WinGet（已提首单 PR：`microsoft/winget-pkgs#481999` 等待上游人工合入）：首单未合入前不重复开第二单；合入后运行 `scripts/winget-manifest.sh` 提交最新稳定版，分别在 `README.md` / `README.en.md` 与 `site/`（独立 PR）补上 `winget install DataOmni.DataOmni`，并在下次发版时对齐 `src-tauri/tauri.conf.json` 的 `bundle.publisher`。
  - [ ] 上游 `Homebrew/homebrew-cask`：待 `dataomni.dev` 域名注册满 30 天且主仓星标达标（他人提 ≥75 / 维护者自提 ≥225）后，将 `packaging/homebrew/Casks/dataomni.rb` 原样提交至官方核心库。
  - [ ] Windows 代码签名评估：结合 Windows 用户反馈评估接入 Azure Trusted Signing 云端签名消除 SmartScreen 首次运行提示。
  - [ ] 生态目录按准入门槛分批投稿（规范与合规文案已归档 `packaging/directories/README.md`，暂缓对外提交）：
    - 零星标渠道（准备投递前先补根目录 `glama.json` 认领 `dataomni-rs` 组织仓库）：`Glama.ai` → `punkpeye/awesome-mcp-servers`（替代已停收社区 README PR 的 `modelcontextprotocol/servers`）、官方 MCP Registry（`registry.modelcontextprotocol.io`）/ Smithery、`chrysler-stone/awesome-tauri-apps`（替代已移除应用区的 `tauri-apps/awesome-tauri`）；
    - 星标门禁渠道：待主仓 `> 50 stars` 后按 `[dataomni-rs/DataOmni]` 规范提交 `rust-unofficial/awesome-rust`。

### 第四阶段（M）：可选驱动解耦、国产库扩展与商店沙盒变体（按条件触发）

- [x] M1 数据库驱动按 Cargo feature 解耦（承接 `B4`）（`0f64974`）
  - 已将 `oracle`（ODPI-C 动态库驱动）拆为默认开启的 Cargo feature（`default = ["ai", "oracle"]`），关闭时不编译链接 `oracle` crate 且不注入 Linux `instantclient` `DT_RPATH`；`DatabaseType` 枚举保持完整以无损读写跨版本 `connections.json`，未编入时以零尺寸空枚举封闭池类型并在 `resolve_for_connection` 与 CLI/MCP 返回 `DATAOMNI_UNSUPPORTED_DATABASE`；新增 `compiled_drivers` 供 `ConnectionForm` 动态隐藏未编译驱动（编辑旧配置保留只读显示），CI 新增 `--no-default-features` 单测门禁（默认 455 条、无默认特性 443 条、前端 193 文件 2331 条全绿）。
- [ ] M2 国产与更多数据库按安装包接入（承接 `E4` / `B1b`）
  - [!] KingbaseES（人大金仓）：当前阻塞于缺试用安装包/镜像；拿到镜像后补齐连接、目录元数据、分页、导入导出与真库冒烟门禁。
  - [ ] 达梦（DM8）：有明确接入诉求时基于 `M1` feature 隔离落地。
- [ ] M3 Mac App Store（MAS）沙盒构建变体（可选上架渠道，前置 `U2` + `M1`）
  - 决定上架 MAS 时先写 `rfcs/mac-app-store.md`：基于关闭内置 `oracle` 的 feature 变体，补齐 `Entitlements.mas.plist` / `Entitlements.mas.inherit.plist`（`app-sandbox`、`network.client`、`network.server`、`files.user-selected.read-write`）与 `ITSAppUsesNonExemptEncryption = false`；为 SQLite（含同目录伴生文件权限）、DuckDB、SSH 私钥与 TLS 证书实现 macOS Security-Scoped Bookmarks 跨重启持久化，沙盒下将 `known_hosts` 迁入应用容器目录，并提供一键打开的内置示例 SQLite 库供审核体验。

---

## v1.3.0 夯实根基路线：数据分析与可视化、CLI/MCP 与应用内 AI 可用性增强（2026-11-20 定）

聚焦 DataOmni 单机个人版核心底座，暂不混入企业/团队功能。原交付闭环核心项已提前随 `v1.2.2` 发布，本阶段（**DA → CM → AI**）连同上方 **V1（Win11 / Linux 跨平台实机验收）** 作为正式的 **`v1.3.0`** 发版目标；三块互相复用、零重复造轮子，动手前先跑完对应 RFC 里的验证实验。

### 第一阶段（DA）：内置 DuckDB 统一结果集分析与可视化（[`rfcs/duckdb-analytics.md`](rfcs/duckdb-analytics.md)）

- [x] DA0 内存 DuckDB 装载、嵌套 JSON 投影、沙盒逃逸与取消实验（E1–E5）（`b54da35`，5 条集成实验全通含 E3 反向验证）
  - 实测结果与拍板结论已写回 [`rfcs/duckdb-analytics.md`](rfcs/duckdb-analytics.md) §3：10 万行 × 20 列 debug 构建装载 `1.5s` / 画像 `2.59s` / DuckDB 内存 `54.5 MiB`（拍板上限 `100,000` 行 + `512MB`）；5,000 条异构 JSON 纯内存扫键 + `Appender` `120ms`（快 `read_json_auto` 4 倍，零临时文件且精确区分缺键与 `null`）；本地文件模式须先物化建表再锁死 `enable_external_access = false` + `lock_configuration = true`（`VIEW` 锁后查询会被拒）；`interrupt()` `0ms` 中止 4 亿组合自连接并可立即复用连接；`BIGINT`/`HUGEINT`/`DECIMAL(38,s)`/`BLOB` 精度映射与溢出退 `VARCHAR` 验证通过。
- [x] DA1 内存 DuckDB 分析沙盒（`services/local_analytics.rs`）与列级数据画像（Data Profiling）（`1a9f2b9`，`cargo test --lib` 464 条含 6 条沙盒/精度/取消/NoSQL 漂移门、`clippy` 默认与 `--no-default-features` 零告警、前端 `eslint` + `tsc` + 195 文件 2344 条全绿）
  - 落地无 `AppHandle` 依赖的 `LocalAnalyticsRegistry`（10 万行上限、512MB 内存上限、8 会话 LRU 淘汰、装载后立即 `enable_external_access=false` + `lock_configuration=true` 反向验证、支持 `interrupt()` 取消恢复与卸载销毁）；实现 SQL 结果集与 MongoDB/ES 异构 JSON 文档（零临时文件、精确 `presentRatio`/`explicitNullCount`）统一列画像，新增 `DataProfileDialog` 并按需接入 SQL 查询结果、表浏览、MongoDB 集合页与 Elasticsearch 控制台。
- [x] DA2 结果集二次 SQL / 透视、Parquet 导出与本地文件即席探索（`69504ef`，`cargo test --lib` 466 条含二次查询/PIVOT/Parquet 往返/本地文件锁沙盒门、`clippy` 默认与 `--no-default-features` 零告警、前端 `eslint` + `tsc` + 197 文件 2349 条全绿）
  - 在 `services/local_analytics.rs` 新增 `open_local_file_session`（物化 `.parquet`/`.csv`/`.tsv`/`.json`/`.ndjson` 后立即锁死 `enable_external_access = false` + `lock_configuration = true`）、`query_session`（`read_only_gate` + 沙盒锁双层防线，复用 `duckdb::decode`）与 `export_parquet_from_session`（沙盒内跑只读 SQL 取数、独立瞬态连接原子写盘）；落地 `LocalAnalyticsDialog`（`GROUP BY` / `PIVOT` / 重复空值快捷模板、列画像点击即过滤、图表、CSV/JSON/Parquet 导出）与侧边栏「打开本地数据文件」免连接探索入口。
- [x] DA3 原生 SVG 图表扩展、DuckDB 聚合破 200 行上限与 SVG/PNG 导出（`b31e453`，`eslint` + `tsc` + 198 文件 2356 条全绿，含 `resultChart.test.ts` 33 条与 `ResultChartDialog.test.tsx`）
  - 在纯函数 `src/utils/resultChart.ts` 扩展 `stacked-bar`、`area`、`scatter`（2,000 点上限显式提示）、`heatmap`（24×24 交叉热力图）及 Markdown 摘要，坚持不做双 Y 轴、零重型图库依赖；超 200 行时在 `ResultChartDialog` 内提供显式内存 DuckDB 聚合（`SUM/AVG/COUNT/MAX/MIN` + Top 10/20/50，标题栏标明口径并在聚合完成后立即销毁临时会话）；复用 `src/utils/svgExport.ts` 导出带主题背景色的 SVG 与 2× PNG。

### 第二阶段（CM）：CLI 与 MCP 可用性增强（[`rfcs/cli-mcp-usability.md`](rfcs/cli-mcp-usability.md)）

严守既有只读、`PROD` 物理不可见与 `<id>#agent` 凭证校验红线；MCP 严禁暴露任意本机文件读写路径。

- [x] CM1 GUI 内 MCP / CLI 一键接入引导与只读审计日志查看器（`9610f20`，`cargo test --lib` 470 条含 4 条审计日志解析与 PATH 探测单测、`clippy` 默认与 `--no-default-features` 零告警、前端 `eslint` + `tsc` + 200 文件 2361 条全绿）
  - 后端新增 `agent_cli_info()`（返回真实可执行路径、PATH 可见性、已开放非生产连接数、审计日志路径与内置 `guide.md`，零口令返回）与 `read_agent_audit_log(limit?)`（只读倒序解析 `dataomni-cli.log` 及其 `.1` 轮转文件，含双引号/转义/Unicode 连接名与截断行容错）；前端落地 `agentConfigSnippets` 纯函数、`AgentCliSection` 设置小节（Claude Code 一键命令、Claude Desktop / Cursor `mcpServers` JSON、复制/导出 `SKILL.md` 及按结果与关键字筛选的不可篡改审计视图）以及连接表单「开放只读访问」下方的直达入口。
- [x] CM2 CLI / MCP 接入内置 DuckDB 能力（`export --format parquet` 与 `profile` 画像）（`10b7e39`，`cargo test --lib cli::` 57 条全通含 Parquet 导出/画像往返与 MCP `file` 参数隔离反向门、`clippy` 默认与 `--no-default-features` 零告警）
  - 直接复用 `services/local_analytics.rs`：CLI 新增 `export --format parquet`（沿用只读事务与绝不覆盖已有文件规则）及 `profile <conn> "<sql>" [--limit N]`（默认 10,000 行、最高 100,000 行，写审计日志且不记 SQL 原文）与 `profile --file <path>`（仅 CLI 可用，不碰连接）；MCP 新增只读 `profile` 工具，schema 显式关闭 `additionalProperties` 且无 `file` 参数，反向验证传 `file` 或偷渡 `--file` 均报 `usage`。
- [x] CM3 `cli query --param` 参数化查询与 `cli diff` 只读结构差异报告（`7d1e38f`，`cargo test --lib -- named_parameters cli::` 63 条全通 + 前端 `queryParameters.test.ts` 16 条含共用语料双测、`clippy` 默认与 `--no-default-features` 零告警）
  - 将 `:name` 扫描与各方言占位符改写沉淀至 `services/named_parameters.rs` 并新增共用语料 `fixtures/bind-parameters-conformance.json` 钉住前后端一致；`cli query` 新增 `--param` / `--param-null` / `--params-json`、MCP `query` 新增 `params`（单测验证参数哨兵值绝不落盘 `dataomni-cli.log`，并顺带修复 `duckdb-rs` 读取 `SQLNULL` 绑定列时的内部 panic）；新增 `cli/diff.rs`、`cli diff <src> <dst> [--format json|markdown]` 与 MCP `diff`（双边均经 Agent 权限与 `<id>#agent` 校验，任一侧为 `PROD` 或未开放统一返回相同 `not_found` 文案，只出差异报告绝不执行 DDL）。
- [x] CM4 连接级本地业务语义备注（Glossary）注入 `dictionary` / MCP（`a1d8aaa`，`cargo test --lib` 481 条全通含 `connection_semantics` 与 `dictionary` 无侵入逐字节兼容门、`clippy` 默认与 `--no-default-features` 零告警、前端 200 文件 2362 条全绿）
  - 新增 `services/connection_semantics.rs`，按连接存储独立侧文件 `semantics/<id>.md`（上限 32 KB、防路径穿越、删连接同步清理、不碰 `connections.json` 与 `<id>#agent` 指纹，超限读取按 UTF-8 字符边界显式截断）；连接表单新增 Markdown 语义备注编辑框；无备注时 `dictionary` 逐字节兼容 `fixtures/dictionary-conformance.json`，有备注时自动附入 `## Business Semantics & Notes`，单表 `cli/MCP schema` 按 `### <table>` 小节按需返回 `"semantics"` 字段。

### 第三阶段（AI）：应用内克制型 AI 可用性增强（[`rfcs/ai-sql-assist.md`](rfcs/ai-sql-assist.md)）

整块继续留在 Cargo feature `ai` 后；零数据行外发、发包原文全程可展开查看、绝不自动执行任何 SQL。

- [x] AI0 既有缺陷修补与 AI 配置底座完善（`345b75e`，Rust `ai_commands` + `services::ai` 10 条全通含 500ms 内取消门、`clippy` 默认与 `--no-default-features` 零告警、前端 `eslint` + `tsc` + `aiSettings.test.ts` 7 条全绿）
  - [x] 修补 `src/utils/aiSettings.ts` `isAiKeyMissing` 误查 `DATAOMNI_CREDENTIAL_MISSING` 缺陷，对齐后端 `DATAOMNI_AI_KEY_MISSING` 并补回归单测；
  - [x] 新增 `ai_cancel(requestId)` 中止正在进行的模型请求、设置页「测试连接」按钮、本地无 Key 端点（Ollama / `127.0.0.1` / `::1`，自动绕过系统代理）免拦截支持，以及多预设（`#ai` / `#ai:local`）与「`STAGING`/`PROD` 仅允许本地模型」环境路由守卫（无损兼容旧版 `dataomni.ai-settings`）。
- [x] AI1 SQL 编辑器三把「手术刀」（`084cd6a`，`eslint` + `tsc` + 202 文件 2372 条全绿，含 `aiSqlAssist.test.ts` 与 `AiSqlAssistDialog.test.tsx`）
  - 落地纯函数 `src/utils/aiSqlAssist.ts`（类型层拒绝任何表数据行入参；E1–E3 预算与解析门：选表上限 15 张、`PROD` 默认 0 张、报错诊断只抽涉及表列、EXPLAIN 原文截断至 16 KB，容忍裸 JSON / 代码块 / `fixedSql: null`）与 `AiSqlAssistDialog`，在 SQL 编辑器工具栏（NL→SQL / 选中改写，自动融合 `CM4` 连接级语义备注）、`QueryErrorPanel`（报错一键诊断修复）与 `QueryPlanDialog`（EXPLAIN 瓶颈与候选索引解读）三处克制接入，全程支持展开查看 `[system]/[user]` 原文、中途取消、敏感环境守卫拦截，且绝不自动执行 SQL。
- [x] AI2 数据字典 AI 业务语义注释草拟（与 `CM4` 闭环）（`ce2c08c`，`eslint` + `tsc` + 204 文件 2376 条、Rust `cargo test --lib` 485 条与 `--no-default-features` 469 条、`clippy` 双构建全绿）
  - 落地纯函数 `src/utils/aiSemanticsDraft.ts`（每批最多 20 张表、零数据行外发、固定输出 `<!-- AI-inferred: review before trusting -->` / `[AI 推测]` 与 `### <table>` 分节）与 `AiSemanticsDraftDialog`，接入 ER 图工具栏（未编入 `ai` feature 或未启用时隐藏）；用户审阅修改后写入 `CM4` 的 `semantics/<id>.md`，反哺人读数据字典、编辑器 AI 助手与 CLI / MCP 外部 Agent。

---

## v1.4 规划：多数据模型深度分析与引擎原生诊断（[`rfcs/multi-engine-analytics.md`](rfcs/multi-engine-analytics.md)，2026-11-20 立项）

原则：不同数据引擎各自下推「引擎原生诊断」（受限、可取消、无权限局部降级），再与既有本地 DuckDB 沙盒及 `ResultChartDialog` 原生 SVG 复用闭环；严禁对线上库做无界全表/全键扫描，不做重型拖拽 BI 仪表盘。

### 前置实验（AN0）

- [x] AN0 多引擎分析边界与安全门实验（E1–E5，5 条集成实验全通含 E2 ACL 降级与 E5 沙盒逃逸反向门，实测数据已写回 [`rfcs/multi-engine-analytics.md`](rfcs/multi-engine-analytics.md) §3）
  - E1：200k 行 `WHERE ... LIMIT 5000`（无排序 `14.42ms`）vs `ORDER BY random()`（全扫 `Top N` `77.12ms`），拍板 7 方言无排序限流抽样；
  - E2：踩出真实陷阱——`redis-rs` pipeline 任一命令报 `NOPERM` 会丢弃整批 `TYPE`/`PTTL`，拍板首键独立探测 1 次 `MEMORY USAGE`（10k Key 允许 `225.75ms` / 禁止仅 1 次探测即降级 `153.70ms`）；
  - E3：5,000 条 12 层嵌套 + 1,200 动态键 BSON 纯内存展平 `87.46ms`，严控 `max_depth=8` / `max_paths=300`（截断 `8,656` 路径）；
  - E4：mock HTTP（`.no_proxy()`）验证 1,053 字段 Mapping 膨胀预警、`size: 0` 聚合与 `_cat/indices` `403` 独立降级；
  - E5：瞬态读连接物化 + `Appender` 向已锁沙盒附加 10k CSV（`202.26ms`）并跑 `LEFT JOIN`/`EXCEPT`（`55.88ms`），4 类逃逸全拒且反向门证实未锁连接可 `read_text`。

### 阶段 1（P0）：结构化数据深度分析升级

- [x] AN1 关系库表级抽样画像与按列语义智能视图（`local_analytics::tests` 10 条全通 + 前端 `tableSampleQuery`/`analyticsProfile`/`DataProfileDialog`/`TableDataViewer`/`catalog`/`shortcuts` 85 条全绿 + `tsc`/`eslint`/`cargo fmt` 通过）
  - 新增 `src/utils/tableSampleQuery.ts`（7 类关系方言无排序限流抽样 SQL，严禁 `ORDER BY RAND()`）与 `runCancellableSampleQuery`（接入既有 `cancel_query` 驱动级取消）；`TableDataViewer` 与 `DataProfileDialog` 支持「当前页 / 表抽样（1,000 / 5,000 / 20,000 行，继承当前过滤条件、明确标注样本口径）」切换；
  - 后端 `local_analytics.rs` 为 `DATE`/`TIMESTAMP` 列补充自适应粒度 `timeBins`（12 桶时间分布）；`DataProfileDialog` 按列语义分流：默认折叠唯一/高基数 ID 列无意义 Top-5、渲染时间列内联 SVG Sparkline、数值列提供负值/`> P95` 离群值一键下钻，支持原库方言 `WHERE` 复制与直接「回表筛选」。
- [x] AN2 DuckDB 工作台多表联动对账（结果集 × 本地文件 / 跨查询结果）（`local_analytics::tests` 11 条全通含 `attach_result_and_local_file_support_reconciliation_while_keeping_sandbox_locked` 沙盒逃逸反向门 + 前端 `analyticsQuery`/`LocalAnalyticsDialog`/`catalog` 27 条全绿 + `tsc`/`eslint`/`cargo fmt` 通过）
  - `services/local_analytics.rs`、`commands/analytics_commands.rs` 与 `lib.rs` 新增 `analytics_attach_result` / `analytics_attach_file`（本地文件经独立瞬态只读连接物化再 `Appender` 写入已锁沙盒，用户 SQL 连接全程不解锁，单会话上限 4 张表且严格校验合法表名）；
  - `LocalAnalyticsDialog` 与 `src/utils/analyticsQuery.ts` 支持附加本地 CSV/Parquet/TSV/JSON 或当前工作区另一查询结果，CodeMirror 补全自动纳入所有附加表列名，并内置同名键推断的 `LEFT JOIN 对账` 与 `EXCEPT 找差异` 快捷模板。

### 阶段 2（P1）：Redis 与 MongoDB 引擎专属分析

- [x] AN3 Redis 非阻塞 Keyspace 内存、TTL 与 BigKey 体检（Rust `services::redis::tests` 16 条全通含 mock RESP2 `MEMORY USAGE` ACL 降级与取消门 + 前端 `redisAnalytics`/`RedisKeyspaceAnalyticsDialog`/`RedisKeyBrowser`/`catalog` 32 条全绿 + `tsc`/`eslint`/`cargo fmt` 通过）
  - `services/redis.rs`、`commands/redis_commands.rs` 与 `lib.rs` 新增 `redis_analyze_keyspace` / `redis_cancel_analysis` 与 `RedisAnalysisState` 协作取消登记表（复用 `SCAN` 单机/Cluster 游标，严禁 `KEYS *`；遵循 `AN0-E2` 首键单次探测 `MEMORY USAGE`，ACL 禁用时平滑降级为元素数/长度估算，受 15s 与 20,000 键双上限保护）；
  - 落地纯函数 `src/utils/redisAnalytics.ts`、`RedisKeyspaceAnalyticsDialog` 与 `RedisKeyBrowser` 入口：可配分隔符（`:` / `/` / `.`）前缀树内存占比、TTL 5 档健康度分布与点击下钻、BigKey Top 50 点击直达 Key 详情，并为 Hash/ZSet 单键值提供一键桥接至 DuckDB 二次分析。
- [x] AN4 MongoDB `$sample` 递归嵌套 Schema 漂移与索引体检（Rust `services::mongodb::tests` 12 条全通含 12 层嵌套+动态键截断与类型漂移单测 + 前端 `mongoAnalytics`/`MongoSchemaAnalyticsDialog`/`catalog` 24 条全绿 + `tsc`/`eslint`/`cargo fmt` 通过）
  - `services/mongodb.rs`、`commands/mongodb_commands.rs` 与 `lib.rs` 新增 `mongodb_analyze_collection` 与纯内存 `analyze_bson_documents`（基于 `$sample`，支持前置当前 `filter`，默认 1,000 / 上限 10,000，硬限深度 8 / 路径 300 / 单数组抽样 20 防爆炸，精确区分缺失与显式 `null`、识别 BSON 混合类型漂移与数组长度分位数），并行拉取 `$collStats`/`$indexStats` 识别长期 0 命中候选僵尸索引并在视图或权限不足时局部降级；
  - 落地 `src/utils/mongoAnalytics.ts` 与 `MongoSchemaAnalyticsDialog`，接入 `MongoCollectionViewer`「Schema / 索引」入口（点击异常类型或缺失标签一键生成 `{ "path": { $type / $exists } }` 回填筛选），同时为集合查询与聚合管道结果接入 `ResultChartDialog` 一键出图。

### 阶段 3（P2–P3）：Elasticsearch 原生聚合分析与 Neo4j 图拓扑体检

- [x] AN5 Elasticsearch 原生 `aggs` 下推探索与 Mapping / 分片健康诊断（前端 `esAnalytics`/`EsIndexAnalyticsDialog`/`EsConsole`/`catalog` 26 条全绿含 `_cat/indices` `403` 降级与 Mapping 1,000+ 字段膨胀门 + `tsc`/`eslint` 通过）
  - 复用既有 `elasticsearch_run`（零冗余后端代理），落地纯函数 `src/utils/esAnalytics.ts` 与 `EsIndexAnalyticsDialog`，接入 `EsIndexStructureView`「原生聚合分析」入口；
  - 读权限路径自动汇总 `_mapping` 叶子字段数（≥ 800 / 1,000 膨胀预警）、`text + .keyword` 双存计数与 `date`/`keyword`/数值字段清单，下推 `size: 0` 的 `date_histogram` / `terms + missing` / `extended_stats + percentiles` 并直连 `ResultChartDialog` 原生 SVG 出图；监控权限路径独立查 `_cat/indices` 与 `_cat/shards` 检查软删除率及过大/过碎/未分配分片，遇 `403` 局部折叠不阻塞只读聚合。
- [x] AN6 Neo4j 图谱拓扑与超级节点（Supernode）体检（前端 `neo4jAnalytics`/`Neo4jTopologyAnalyticsDialog`/`CypherWorkbench`/`catalog` 31 条全绿含只读 Cypher 门禁 + `tsc`/`eslint`/`cargo fmt` 通过）
  - `commands/neo4j_commands.rs` `neo4j_run` 开放可选 `read_only` 协议级只读事务标志；落地 `src/utils/neo4jAnalytics.ts` 与 `Neo4jTopologyAnalyticsDialog`，接入 `CypherWorkbench`「图谱体检」入口；
  - 分层执行：第一层经 O(1) count store 统计各 Label 节点数与 Relationship Type 边数并直连 `ResultChartDialog` 出图；第二层按需受限采样（500 / 1,000 / 5,000 节点，兼容 Neo4j 5 `COUNT { (n)--() }` 与 Neo4j 4 `size()` 回退）统计度分位数（P50 / P95 / Max）、孤立节点占比、Top 20 超级节点与稀疏属性覆盖率，点击超级节点一键载入并执行只读邻域 Cypher 在图视图展开。

### 阶段 4：分析高价值低膨胀增强（`AN7–AN10`）

- [x] AN7 画像体检增强：顶部异常摘要条、文本空串/首尾空格检测与数值 `<` / `>` 一键回表筛选（Rust `local_analytics::tests` 13 条 + 前端 `analyticsProfile`/`DataProfileDialog`/`LocalAnalyticsDialog`/`catalog` 39 条全绿）
- [x] AN8 画像体检 Markdown 报告一键复制导出（前端 `analyticsProfile`/`DataProfileDialog`/`LocalAnalyticsDialog`/`catalog` 40 条全绿，不外泄样本值）
- [x] AN9 数值列相关性矩阵（内存 DuckDB 单次扫描 `CORR` 复用 `ResultChartDialog` 热力图，上限 12 列；`analyticsQuery`/`ResultChartDialog`/`DataProfileDialog`/`LocalAnalyticsDialog`/`catalog` 全绿）
- [ ] AN10 基于脱敏统计摘要的 AI 体检解读（剥离样本值、零原始行外发，留在 `ai` feature 后面）

---

## 「当前版本不做」的保留项与可执行重估条件

### 架构与性能重估门

- **R1 落盘兼容补充**：给设置或 `ssh_tunnel` 新增嵌套字段时，先在 `fixtures/on-disk-0.5/` 补样本再改。
- **QR2 前端分包**：主 JS 约 2.05 MB（gzip 605 KB），本地读取当前不拆；重估条件：三平台打包版实测冷启动慢且 Performance 确认 script evaluation 占首屏一半以上时，按独立工作区做 `React.lazy`。
- **PA1 单格大 BLOB 截断**：整行受 12 MB 上限保护，单格不截断（避免破坏 WHERE 行定位）；重估条件：有人报单格大 BLOB 卡顿或内存告警时，先测 50 MB `BYTEA` 峰值 RSS 再定「截断 + 禁编辑」。
- **v0.3 Windows ARM64 原生包**：当前靠系统转译跑 x64 包（且 Oracle 无 Windows ARM64 Instant Client）；重估条件：有 ARM Windows 用户反馈转译慢或装不上。
- **5.4 带 Oracle 的 Linux AppImage 不发**：重估条件：AppImage 打包能排除 `instantclient` 目录不改 ELF。
- **每种数据库独立能力声明表**：重估条件：出现第一个前后端能力判断不一致的缺陷，届时做共用语料（仿 `fixtures/export-conformance.json`）。

### 方言、工作区与编辑器长尾重估门（触发条件均为「收到真实用户反馈」）

- **R7 无表别名时的裸列补全**：有人报「不写 `o.` 补不出列」时，按光标所在语句的 `FROM` 推 `defaultTable`。
- **3.1 PostgreSQL 单表原生 `SHOW CREATE TABLE`**：批量重建导出已完成（`82e3e33`）；单表查看当前版本不做（可执行门：`services::schema_metadata::tests::postgres_has_no_ddl_query`）。
- **G2 JOIN / 表达式里的 `_bin` 列类型名**：单表裸列已按目录声明类型修正，复杂查询仍显驱动类型（值显示与编辑不受影响）；有人因此误判时评估 `[patch]` `sqlx-mysql`。
- **G3 openGauss SM3 口令**：`sha256` 已由本地 `opengauss_relay` 支持；`password_encryption_type = 3`（SM3）等有人遇到再做。
- **G8 Redis Sentinel 多哨兵地址与自动切主重连**：有人切换后撞上 `READONLY` 或需填多个哨兵地址时重估。
- **4.3 MongoDB 扩展项**：JS 语法多命令脚本、LDAP / Kerberos / AWS IAM 认证、导出聚合结果与 `--jsonArray` 导入导出、`collMod` 改索引——有人拿着这类部署或脚本来时重估。
- **4.3 Redis 扩展项**：客户端证书、RESP3 推送与 Pub/Sub——有人拿着这种部署来时重估。
- **5.1–5.3 界面与编辑器偏好保留项**：多结果集之间拖动分隔条、间距/字号/阴影 token 扩展、编辑器 Tab 宽度/自动换行/快捷键改键、Oracle 存储过程第二个隐式结果集——有人明确提出再做。
- **A8a 第三批命令行写操作**：只读 CLI 与只读 MCP 已完成；写操作按 `rfcs/agent-cli.md` §6 等明确场景再做。
- **其余长尾方言项**：Oracle 原生 JSON / VECTOR 直接解码（现有 `JSON_SERIALIZE` 绕法）、SQL Server 超 `9×10¹¹` 的 `money` 精度、Oracle Wallet TLS（现走 SSH 隧道）、OceanBase 表达式默认值解析、A2b 整库结构导出。

## 明确不做 / 暂不优先

- 插件或扩展机制（同类诉求到第三个再谈机制）
- 云账号体系、团队实时协作与云端配置同步（已有本地导入导出）
- B3 云厂商专有 IAM 数据库认证
- AI 自动执行数据库写操作
- 大型 BI 仪表盘
