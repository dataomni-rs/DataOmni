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
| v1.3 路线 | 流水线提效、跨平台闭环、应用内签名平滑更新、合规与可选沙盒变体（`rfcs/roadmap-1.3.md`） | [-] 2026-11-18 定稿：按 **P → V → U → M** 四阶段推进 |

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
  - [x] 独立安装包 `tauri-plugin-updater` 签名替换（`edf7e96`）：已接入生产 Minisign Ed25519 公钥（`52232E86440485D4`）与 `latest.json` HTTPS 端点，落地后端受控三命令（`check_app_update` / `download_app_update` / `install_app_update`，拦截包管理器与开发构建自覆盖）、`UpdateNotice` 三步显式确认与 `App` 事务/任务/快照退出守卫，以及 `scripts/build-latest-json.py` 与 `release.yml` 公证解包门禁；包体/签名 1 字节篡改反向验证单测、中英深浅色真实渲染与全量门禁通过（发版前将本机私钥写入 `TAURI_SIGNING_PRIVATE_KEY` GitHub Secret 即可自动签出更新产物）。
- [x] U2 官网双语隐私与安全合规声明页（`/privacy/` 与 `/zh/privacy/`，独立官网 PR `#51` / `1642fc0` 已上线）
  - 已在 `site/` 新增中英双语隐私政策与数据安全声明页（零遥测、无云账号、凭据只进系统钥匙串、三类出网请求完整披露、只读 Agent `PROD` 物理不可见、`--no-default-features` 编译剔除），补齐页脚、侧边栏与 `sitemap.xml` 并修复窄屏 `.docs-sidebar` 横向溢出；`site:check`、桌面深浅色与真实 375px 视口（`scrollWidth=375`）及线上 `https://dataomni.dev/privacy/`、`https://dataomni.dev/zh/privacy/`（HTTP 200）全部核验通过。
- [-] U3 在途外部渠道收口、Windows 签名评估与生态目录推广
  - [ ] WinGet（已提 PR：`microsoft/winget-pkgs#481999` 等待上游人工合入）：合入后运行 `scripts/winget-manifest.sh` 提交最新稳定版，分别在 `README.md` / `README.en.md` 与 `site/`（独立 PR）补上 `winget install DataOmni.DataOmni`，并在下次发版时对齐 `src-tauri/tauri.conf.json` 的 `bundle.publisher`。
  - [ ] 上游 `Homebrew/homebrew-cask`：待主仓星标（≥75 / 自提 ≥225）且 `dataomni.dev` 域名注册满 30 天后，将 `packaging/homebrew/Casks/dataomni.rb` 原样提交至官方核心库。
  - [ ] Windows 代码签名评估：结合 Windows 用户反馈评估接入 Azure Trusted Signing 云端签名消除 SmartScreen 首次运行提示。
  - [ ] 生态目录投稿：按 `packaging/directories/README.md` 推进 `modelcontextprotocol/servers`、Smithery、Glama、`awesome-tauri`、`awesome-rust` 及搜索引擎收录。

### 第四阶段（M）：可选驱动解耦、国产库扩展与商店沙盒变体（按条件触发）

- [x] M1 数据库驱动按 Cargo feature 解耦（承接 `B4`）（`0f64974`）
  - 已将 `oracle`（ODPI-C 动态库驱动）拆为默认开启的 Cargo feature（`default = ["ai", "oracle"]`），关闭时不编译链接 `oracle` crate 且不注入 Linux `instantclient` `DT_RPATH`；`DatabaseType` 枚举保持完整以无损读写跨版本 `connections.json`，未编入时以零尺寸空枚举封闭池类型并在 `resolve_for_connection` 与 CLI/MCP 返回 `DATAOMNI_UNSUPPORTED_DATABASE`；新增 `compiled_drivers` 供 `ConnectionForm` 动态隐藏未编译驱动（编辑旧配置保留只读显示），CI 新增 `--no-default-features` 单测门禁（默认 455 条、无默认特性 443 条、前端 193 文件 2331 条全绿）。
- [ ] M2 国产与更多数据库按安装包接入（承接 `E4` / `B1b`）
  - [!] KingbaseES（人大金仓）：当前阻塞于缺试用安装包/镜像；拿到镜像后补齐连接、目录元数据、分页、导入导出与真库冒烟门禁。
  - [ ] 达梦（DM8）：有明确接入诉求时基于 `M1` feature 隔离落地。
- [ ] M3 Mac App Store（MAS）沙盒构建变体（可选上架渠道，前置 `U2` + `M1`）
  - 决定上架 MAS 时先写 `rfcs/mac-app-store.md`：基于关闭内置 `oracle` 的 feature 变体，补齐 `Entitlements.mas.plist` / `Entitlements.mas.inherit.plist`（`app-sandbox`、`network.client`、`network.server`、`files.user-selected.read-write`）与 `ITSAppUsesNonExemptEncryption = false`；为 SQLite（含同目录伴生文件权限）、DuckDB、SSH 私钥与 TLS 证书实现 macOS Security-Scoped Bookmarks 跨重启持久化，沙盒下将 `known_hosts` 迁入应用容器目录，并提供一键打开的内置示例 SQLite 库供审核体验。

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
