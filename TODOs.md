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
| v1.2 | 移除 `tauri-plugin-sql`、多页双语文档、连接迁移导入、查询模板、macOS 签名公证与 Homebrew Tap（`rfcs/roadmap-1.2.md`） | [x] `v1.2.0`（2026-11-17）与 `v1.2.1`（2026-11-18）正式发布，`dataomni-rs/homebrew-tap` 已上线；未结验证与渠道项并入 `v1.3` |
| v1.3 路线 | 流水线提效、跨平台闭环、平滑更新与合规分发（`rfcs/roadmap-1.3.md`） | [-] `P0–P3`、`U1`、`U2`、`M1` 已完成并正式发布 `v1.2.2`（2026-11-19，带 `latest.json` 与 `dataomni-rs/homebrew-tap` 同步）；`P4`、`V1`、`V2`、`U3`、`M2`、`M3` 按实机与外部条件持续推进 |
| v1.4 夯实根基 | 内置 DuckDB 统一分析与可视化、CLI/MCP 可用性增强、应用内克制型 AI 增强 | [-] 设计文档已定稿（`rfcs/duckdb-analytics.md`、`rfcs/cli-mcp-usability.md`、`rfcs/ai-sql-assist.md`），`DA0` 已完成，正按 DA → CM → AI 推进 |

---

## v1.3 路线：流水线提效、跨平台闭环、平滑更新与合规分发（2026-11-18 定）

规划、理由、验收标准与「明确不做」见 [`rfcs/roadmap-1.3.md`](rfcs/roadmap-1.3.md)。继续严守**「实用、好用、易用、功能稳定」**总纲，按 **P（流水线提效）→ V（跨平台闭环）→ U（平滑更新与分发合规）→ M（驱动解耦、国产库与商店变体）** 推进。

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

- [ ] V1 Windows 11 与 Linux 打包版实机闭环验收（承接 `v1.2` `V1`）
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

## v1.4 夯实根基路线：数据分析与可视化、CLI/MCP 与应用内 AI 可用性增强（2026-11-20 定）

聚焦 DataOmni 单机个人版核心底座，暂不混入企业/团队功能。按 **DA（内置 DuckDB 统一分析与可视化）→ CM（CLI / MCP 可用性增强）→ AI（应用内克制型 AI 增强）** 推进，三块互相复用、零重复造轮子；动手前先跑完对应 RFC 里的验证实验。

### 第一阶段（DA）：内置 DuckDB 统一结果集分析与可视化（[`rfcs/duckdb-analytics.md`](rfcs/duckdb-analytics.md)）

- [x] DA0 内存 DuckDB 装载、嵌套 JSON 投影、沙盒逃逸与取消实验（E1–E5）（`b54da35`，5 条集成实验全通含 E3 反向验证）
  - 实测结果与拍板结论已写回 [`rfcs/duckdb-analytics.md`](rfcs/duckdb-analytics.md) §3：10 万行 × 20 列 debug 构建装载 `1.5s` / 画像 `2.59s` / DuckDB 内存 `54.5 MiB`（拍板上限 `100,000` 行 + `512MB`）；5,000 条异构 JSON 纯内存扫键 + `Appender` `120ms`（快 `read_json_auto` 4 倍，零临时文件且精确区分缺键与 `null`）；本地文件模式须先物化建表再锁死 `enable_external_access = false` + `lock_configuration = true`（`VIEW` 锁后查询会被拒）；`interrupt()` `0ms` 中止 4 亿组合自连接并可立即复用连接；`BIGINT`/`HUGEINT`/`DECIMAL(38,s)`/`BLOB` 精度映射与溢出退 `VARCHAR` 验证通过。
- [ ] DA1 内存 DuckDB 分析沙盒（`services/local_analytics.rs`）与列级数据画像（Data Profiling）
  - 落地受容量上限保护、关标签即销毁的 `LocalAnalyticsRegistry`；实现 SQL 结果集与 MongoDB/ES JSON 文档的统一列画像（空值率、唯一值、Min/Max/P50/P95、迷你直方图、Top-5、NoSQL 字段覆盖率/Schema 漂移），按需触发，不拖慢普通查询。
- [ ] DA2 结果集二次 SQL / 透视、Parquet 导出与本地文件即席探索
  - 在结果面板提供基于内存表 `result_set` 的二次只读 SQL 与 `GROUP BY` / `PIVOT` 快捷模板，支持导出 CSV / JSON / Parquet；支持直接打开本地 `.parquet` / `.csv` / `.json` 文件进行只读画像、SQL 与图表分析，不污染 `connections.json`。
- [ ] DA3 原生 SVG 图表扩展、DuckDB 聚合破 200 行上限与 SVG/PNG 导出
  - 在 `src/utils/resultChart.ts` 纯函数层扩展 `stacked-bar`、`area`、`scatter`、`heatmap`（不做双 Y 轴，不引入重型图库）；超 200 行时提供显式内存 DuckDB 聚合/分桶选项（标明口径、不悄悄采样）；复用 `src/utils/svgExport.ts` 导出 SVG / 2× PNG 与复制 Markdown 摘要。

### 第二阶段（CM）：CLI 与 MCP 可用性增强（[`rfcs/cli-mcp-usability.md`](rfcs/cli-mcp-usability.md)）

严守既有只读、`PROD` 物理不可见与 `<id>#agent` 凭证校验红线；MCP 严禁暴露任意本机文件读写路径。

- [ ] CM1 GUI 内 MCP / CLI 一键接入引导与只读审计日志查看器
  - 后端新增 `agent_cli_info()` 与 `read_agent_audit_log()`（直接只读解析现有 `dataomni-cli.log`）；前端提供 Claude Code / Claude Desktop / Cursor 一键复制配置、导出 `SKILL.md` 及按结果筛选的只读审计记录视图。
- [ ] CM2 CLI / MCP 接入内置 DuckDB 能力（`export --format parquet` 与 `profile` 画像）
  - 复用 `services/local_analytics.rs`：CLI 新增 `export --format parquet` 与 `profile <conn> "<sql>"` / `profile --file <path>`；MCP 新增只读 `profile` 工具（仅接开放连接 + SQL，显式拒绝 `file` 参数），帮外部 Agent 在不拉原始行的前提下掌握数据分布。
- [ ] CM3 `cli query --param` 参数化查询与 `cli diff` 只读结构差异报告
  - 移植 `:name` 参数扫描/改写至后端并新增 `fixtures/bind-parameters-conformance.json` 钉住前后端一致（参数值严禁入审计日志），MCP `query` 同步支持 `params`；新增 `cli diff <src> <dst>` 与 MCP `diff`（双边均须通过 Agent 权限校验，只出差异报告不执行 DDL）。
- [ ] CM4 连接级本地业务语义备注（Glossary）注入 `dictionary` / MCP
  - 按连接存储独立侧文件 `semantics/<id>.md`（上限 32 KB，不影响 `connections.json` 与 `<id>#agent` 指纹）；无备注时输出逐字节兼容 `fixtures/dictionary-conformance.json`，有备注时自动附入 `cli dictionary` 与 MCP `dictionary` / `schema`。

### 第三阶段（AI）：应用内克制型 AI 可用性增强（[`rfcs/ai-sql-assist.md`](rfcs/ai-sql-assist.md)）

整块继续留在 Cargo feature `ai` 后；零数据行外发、发包原文全程可展开查看、绝不自动执行任何 SQL。

- [ ] AI0 既有缺陷修补与 AI 配置底座完善
  - [ ] 修补 `src/utils/aiSettings.ts` `isAiKeyMissing` 误查 `DATAOMNI_CREDENTIAL_MISSING` 缺陷，对齐后端 `DATAOMNI_AI_KEY_MISSING` 并补回归单测；
  - [ ] 新增 `ai_cancel(requestId)` 中止正在进行的模型请求、设置页「测试连接」按钮、本地无 Key 端点（Ollama / `127.0.0.1`）免拦截支持，以及多预设与「`STAGING`/`PROD` 仅允许本地模型」环境路由守卫（无损兼容旧版 `dataomni.ai-settings`）。
- [ ] AI1 SQL 编辑器三把「手术刀」（动手前先完成 `rfcs/ai-sql-assist.md` §3 样本实验）
  - 纯函数 `src/utils/aiSqlAssist.ts` + 透明发包面板，支持：① 自然语言生成/改写 SQL（手动选表上限 15 张，`PROD` 默认 0 表）；② `QueryErrorPanel` 报错一键诊断与修复对比；③ `QueryPlanDialog` EXPLAIN 执行计划瓶颈解读与候选索引建议（只出草稿/复制，绝不直接执行）。
- [ ] AI2 数据字典 AI 业务语义注释草拟（与 `CM4` 闭环）
  - 按表分批草拟带 `[AI 推测]` 标识的业务说明，用户审阅修改后写入 `CM4` 的 `semantics/<id>.md`，反哺人读、编辑器 AI 与 CLI / MCP 外部 Agent。

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
