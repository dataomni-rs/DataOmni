# v1.1 之后的巩固与 v1.2 实用路线

> 2026-11-17 定稿时的状态：`v1.1.0`（2026-11-16）已正式发布，官网 `https://dataomni.dev`（中英双语、多 Agent MCP 预设、技术 SEO 与 GSC 验证）已上线，WinGet 上游 PR（`microsoft/winget-pkgs#481999`）与搜索引擎收录提交正在外部推进中。
> 本文记录接下来的取舍依据、推进次序、验收门与「明确不做」的边界；具体任务的勾选状态在 `TODOs.md`「v1.2 路线」里维护。

## 一、现状与核心判断

`v1.0.0` 到 `v1.1.0` 完成了两件大事：一是把桌面端的落盘兼容、日志脱敏、更新提示、依赖审计、前端零 `sql:*` 权限与严格 CSP 全部钉进自动化门禁；二是补齐了代码生成（C1）、结构比对（D3）、`:name` 绑定参数（D2）与内置只读 MCP 服务（C2），并建起了 `dataomni.dev` 官网。

在这个节点上重新盘点仓库，真正的短板不在「缺大功能」，而在下面四处真实缝隙：

| 维度 | 现状事实 | 风险与影响 |
| --- | --- | --- |
| 过渡层与发版耦合遗留 | 1. `QS5` 已撤掉前端 `@tauri-apps/plugin-sql` 与全部 `sql:*` 权限，但后端 `Cargo.toml` 与 `lib.rs` 仍挂着 `tauri-plugin-sql`（仅借用 `DbPool` / `DbInstances` 容器，却把 `plugin:sql\|*` 命令留在二进制里）；<br>2. `site/src/main.rs` 直接读 `src-tauri/tauri.conf.json` 的版本号拼 GitHub Releases 下载地址，且 `site.yml` 监听该文件自动部署；<br>3. `Cargo.toml` 缺官网与新仓库元数据，应用内「关于与诊断」及 macOS `Help` 菜单没有官网与问题反馈入口。 | 1. 留着不再需要的插件层与 `Cargo.lock` 字符串锁测试；<br>2. 下次准备发 `v1.2.0` 先在 `main` 改版本号、而草稿 Release 尚未点 Publish 时，官网下载按钮会提前指向未公开 tag 返回 404；<br>3. 用户在打包版内找不到官网文档与报缺陷入口。 |
| `v1.1.0` 新能力的跨平台 / 真库盲区 | `C2`（`dataomni mcp`）、`QS5`（SQLite 后端池）、`QS6`（CSP）只在 macOS `1.1.0` 打包版实机走过，Windows WebView2 与 Linux WebKitGTK 打包版未走；`D2`（绑定参数）与 `D3`（结构比对）缺 Oracle 真库与 DuckDB 往返脚本闭环。 | 单测与代码共用不能替代平台 WebView 差异（如 CSP 对 worker / `data:` 图片的拦截差异）和 Oracle 目录查询方言细节。 |
| 官网只有着陆页、缺实用说明书承接 | `dataomni.dev` 目前只有中英着陆页；仓库里已写好的用户手册、MCP 接入说明、CLI 契约和 17 种数据库兼容细节都深藏在 GitHub Markdown 里。 | 新用户装完遇到跳板机、Oracle 免配、openGauss 认证或 MCP 接入问题时无处查阅；搜索引擎长尾技术检索也缺少扎实的内容承接页。 |
| 老用户迁移与高频复用的最后一公里 | 从 Navicat / DBeaver 迁过来的用户手头常有几十条连接需逐个重填；日常排障反复跑的带 `:name` 参数查询还没有本地模板可存。 | 影响「装上第一天能不能立刻用起来」和「每天用顺不顺手」。 |

因此，下一阶段定死一条总纲：**奔着实用、好用、易用、功能稳定去——先扫清工程遗留，再巩固闭环已有功能，接着补齐官网实用文档承接，最后才审慎扩展高频刚需能力。**

---

## 二、四阶段推进次序与验收标准

严格按 **S（扫清遗留）→ V（巩固验证）→ W（官网文档）→ E（实用扩展）** 推进。前一阶段的阻塞缺陷未清零，不开下一阶段的新功能。

### 第一阶段（S）：扫清技术债与工程遗留

目标是把架构里的半截过渡代码、发版流水线隐患和基础元数据一次性收干净，保持主干精炼。

- **S0 `TODOs.md` 第三次归档**
  - **事实**：`TODOs.md` 当前已达 103 KB，超过文件头约定的 100 KB 归档线（其中绝大部分是 `v0.6.0` 至 `v1.1.0` 已验收的详细记录）。
  - **做法**：将已完成的 `v1.0`、`1.x`（已结项部分）、`QS/QT/QR`、`PA1–PA8`、`G1–G13` 原样归档至 `docs/archive/TODOs-2026-11-17.md`，同步更新 `AGENTS.md` 权威来源表；`TODOs.md` 只保留在途项、未触发重估条件的保留项与本路线任务。
  - **验收**：归档文件逐字保留原记录与提交号；`TODOs.md` 体积降回精简可读范围，仓库内引用路径无断链。

- **S1 移除后端残留的 `tauri-plugin-sql`（收口 `2.4` 与 `QS5`）**
  - **事实**：前端已无任何 `plugin:sql` 调用与权限；后端仅在 `sqlx_pool.rs` 等处用 `tauri_plugin_sql::{DbPool, DbInstances}` 存 MySQL / PostgreSQL / SQLite 连接池。
  - **做法**：在 `services/sqlx_pool.rs` 定义本地枚举 `SqlxPool`（`Sqlite` / `MySql` / `Postgres`），复用已有的 `PoolRegistry`（与 `SqlServerRegistry`、`MongoRegistry` 一致）替换 `DbInstances`；从 `src-tauri/Cargo.toml` 与 `src-tauri/src/lib.rs` 移除 `tauri-plugin-sql`；更新 `plugin_decode.rs` 说明并删除 `database_smoke.rs` 中扫描 `Cargo.lock` 盯防插件版本的脆弱测试。
  - **验收**：`src/` 与 `src-tauri/` 不再依赖 `tauri-plugin-sql`；二进制中不再注册 `plugin:sql|*` 命令；`bun run check`、`bun run rust:clippy:no-ai`、`bun run rust:audit` 及 SQLite 真库冒烟全部通过。

- **S2 解耦官网已发布版本号与开发期 `tauri.conf.json`，消除提前 404 隐患**
  - **事实**：`release.yml` 打 tag 时先产出草稿（Draft）Release 等人工核对后再公开，而 `site.yml` 在 `src-tauri/tauri.conf.json` 推上 `main` 时就会立即构建上线。两者共用同一个版本号源会导致发版窗口期官网下载链接 404。
  - **做法**：为 `site/` 引入独立的「官网当前公开版本」单一配置源（如 `site/release.json` 或受控常量），从 `.github/workflows/site.yml` 的 `paths` 中摘掉 `src-tauri/tauri.conf.json`，并支持在 GitHub Release `published` 事件触发（或发版确认公开后更新配置）时部署官网。
  - **验收**：单测验证升 `tauri.conf.json` 版本不会改变未发布前的官网下载链接；`site` 构建与格式校验全过。

- **S3 补齐包元数据与应用内「官网 / 反馈」直达入口**
  - **做法**：
    1. `src-tauri/Cargo.toml` 补齐 `license = "MIT"`、`homepage = "https://dataomni.dev"`、`repository = "https://github.com/dataomni-rs/DataOmni"`（注：`tauri.conf.json` 的 `bundle.publisher` 会改变 Windows MSI ARP `Manufacturer`，在 `microsoft/winget-pkgs#481999` 合入前保持不动，留待下次发版连同 `scripts/winget-manifest.sh` 一起对齐）；
    2. 在设置「关于与诊断」（`src/components/AboutSection.tsx`）补充「官方网站」与「问题反馈」按钮（走后端受控的 `open_url` 白名单或固定常量，不恢复前端 `opener` 插件权限），并在 macOS `src-tauri/src/app_menu.rs` 的 `Help` 菜单补上同名入口。
  - **验收**：组件单测、`i18n/catalog.test.ts` 与 `databaseHandle.test.ts` 权限门禁全绿，真实界面中英文渲染核对无折行错位。

- **S4 在途外部渠道落地后的仓内同步（事件触发即收口）**
  - **WinGet（承接 `U2`）**：`microsoft/winget-pkgs#481999` 合入后，在 `site/` Windows 安装卡片、`README.md` 与 `README.en.md` 加上 `winget install DataOmni.DataOmni`，勾选 `U2`。
  - **macOS 签名公证与 Homebrew**：若 Apple Developer ID 获批，在 `release.yml` 按密钥存在性条件接入签名与 `notarytool` 公证、提交 `Homebrew/homebrew-cask` 并更新官网 macOS 首次打开提示；若审批受阻或延期，按需启用 `dataomni-rs/homebrew-tap` 兜底。

---

### 第二阶段（V）：巩固稳定已有功能（跨平台与真库闭环）

不造新概念，专门消除 `v1.1.0` 已落地能力在平台与数据库方言上的未验盲区，发现一处修一处并补回归门。

- **V1 Windows 与 Linux 打包版 `v1.1.0` 增量能力实机验收**
  - **范围**：在 Linux（WebKitGTK 桌面容器，deb/rpm）与 Windows 11（WebView2，MSI/NSIS）打包产物上逐项实测：
    1. SQLite 经后端连接池打开、对象树与结构页读取、数据编辑提交、多连接独立关闭、不存在的路径不误建空文件（`QS5` + `S1`）；
    2. 开启严格 CSP 后（`QS6`），SQL 编辑器、ER 图 PNG 导出、Mongo / Redis / Neo4j / ES 工作区渲染及更新检查正常，`dataomni.log` 零 CSP 报错；
    3. 打包二进制执行 `dataomni mcp` 经 stdio 跑通只读查询、写语句 `refused` 与生产连接 `not_found`（`C2`）。
  - **验收**：两平台实机清单逐项通过，日志无异常，任何平台差异缺陷修复后在新包上复验。

- **V2 DuckDB 与 Oracle 23ai 真库补验绑定参数及结构比对**
  - **范围**：
    1. DuckDB：接入 `scripts/schema-diff-verify.sh`（或独立真库往返测试），验证两个 DuckDB 库之间的表、列、主键、索引差异比对与同步脚本执行后再比为空，并跑通 `--skip-kind` 反向验证；
    2. Oracle 23ai Free：在容器真库上验证 `D2` `:name` 绑定参数（含 `q'[...]'` 引号字面量排除、NULL 与数字绑定、报错参数名还原）以及 `D3` Oracle `is_constraint` 目录查询和同方言结构比对往返。
  - **验收**：往返脚本退出码为 0，故意跳过一类变更时退出码非 0；Oracle 真库新增用例纳入 `oracle_smoke.rs`。

- **V3 持续真库回归（Nightly Smoke）稳定性收口**
  - **范围**：跟踪 `.github/workflows/nightly-databases.yml` 连续四周定时运行结果；对已知因兼容库不支持特定类型（如 CockroachDB / openGauss 上的 `INT[][]`、`XML` 导出夹具）造成的非应用缺陷噪音，按服务端能力精确裁剪测试夹具。
  - **验收**：定时回归「绿 = 正常，红 = 真实退化」，不在日志里留习以为常的假红。

- **V4 日常高频交互稳态巡检**
  - **范围**：
    1. 实机验证 `PA2` 工作区快照防抖：连续编辑 SQL 草稿、快速切换标签、直接退出窗口（`⌘Q` / `Alt+F4`）重开后草稿与光标上下文零丢失；
    2. 检查网络闪断 / SSH 跳板机重启后，在表数据页提交修改或导出失败时的错误反馈是否足够引导用户重连（对应 `R6` 留下的观察项；若提示含糊则补明确重连动作）。

---

### 第三阶段（W）：官网实用文档体系（`dataomni.dev` 易用性承接）

官网不做空洞的 SEO 关键词门户页，只做**用户装完真要查、搜索技术问题真能帮上忙**的双语技术文档。

- **W1 `site/` 静态生成器扩展多页文档布局（零前端框架膨胀）**
  - **做法**：继续沿用 `site/` 纯 Rust 零第三方依赖静态生成方案，不引入 VitePress / Docusaurus 等第二套前端构建链。新增文档页共用壳（顶部导航、左侧目录、章节锚点、代码块语法高亮与一键复制、中英对应页互跳），自动生成多页 `sitemap.xml`、`BreadcrumbList` / `TechArticle` 结构化数据，并纳入 `verify_html_invariants`（标题/描述长度、无死链、资源存在性）门禁。
  - **验收**：`bun run site:check` 全绿，桌面端与 375px 移动端双主题（浅色/深色）真实浏览器截图核对通过。

- **W2 上线首批 4 篇核心双语实用文档**
  - 基于仓库内已验证的事实（`docs/user-manual.md`、`rfcs/mcp-server.md`、`rfcs/agent-cli.md`、`README.md` 兼容矩阵）提炼编排，确保文档里的每一条命令、每一个配置项与当前代码逐字一致：
    1. `/docs/quickstart/`（及 `/zh/docs/quickstart/`）：三平台安装、系统钥匙串授权说明、SSH 跳板机与 TLS 四档校验说明、生产连接安全护栏与单事务预览提交流程；
    2. `/docs/mcp/`（及 `/zh/docs/mcp/`）：只读 Agent 访问开启步骤、生产库物理不可见机制、7 种主流工具（Claude Code / Codex / Cursor / Windsurf / Copilot / Gemini CLI / Claude Desktop）配置与排错；
    3. `/docs/cli/`（及 `/zh/docs/cli/`）：`dataomni` 命令行配置、SQL 与四大 NoSQL 工作区只读命令、JSON 输出结构与退出码（`0`–`4`）契约；
    4. `/docs/databases/`（及 `/zh/docs/databases/`）：17 种数据库（含内置 Oracle Instant Client、openGauss sha256 转发、DuckDB 直查本地 Parquet/JSON、MySQL 5.7 TLS 兼容说明）连接指南与真库兼容性矩阵。

- **W3 上线 `/changelog/` 双语版本更新记录页**
  - **做法**：整理 `v1.0.0`、`v1.1.0` 及后续版本的面向用户更新说明（新能力、修复与升级兼容性承诺），在官网顶栏/页脚与应用更新提示之间形成清晰闭环。

- **W4 外部生态目录标准元数据就绪**
  - **做法**：在 `packaging/`（或 `docs/`）备齐提交 MCP 目录（`modelcontextprotocol/servers`、Smithery、Glama）与 `awesome-tauri` / `awesome-rust` 所需的英文简介、安全特性说明与配置示例，保证外部提交口径与官网完全一致。

---

### 第四阶段（E）：审慎扩展高价值实用能力（`v1.2.0` 候选池）

**前置门槛**：第一、二阶段完成。每开一项前先写最小设计说明，坚持「核心逻辑占主体、纯函数在 `src/utils/` 先过正反单测、界面改动必渲染核对」。

- **E1 从主流客户端导入连接配置（Navicat `.ncx` / DBeaver `data-sources.json`）**
  - **为什么做**：解决老用户切换到 DataOmni 时的第一道体力门槛——手头几十个测试/开发库连接无需逐表重填。
  - **边界**：只导入连接名、库类型、主机、端口、库名、用户名、SSH 隧道地址与用户名等非机密结构信息；导入的连接一律默认关闭 Agent 访问；**不解密、不触碰第三方软件私有的加密口令字段**，首次连接时引导用户输入口令并存入系统钥匙串。
  - **验收**：各格式真实脱敏样本夹具的正反单测 + 导入预览勾选对话框实机渲染验证。

- **E2 本地参数化查询模板 / 常用 SQL 片段（承接 `4.4`，复用 `D2` `:name` 绑定参数）**
  - **为什么做**：日常排障与数据核查最常跑的是十几条固定骨架、只换 `:id` / `:date` 参数的语句，每次去历史记录里翻效率低。
  - **边界**：纯本地 JSON 落盘（纳入 `R1` 落盘兼容门样本，升级不丢、降级不毁），支持按名称/连接类型筛选，在编辑器或命令面板（`⌘K`）一键插入并直接走 `D2` 绑定参数运行；不做云端同步，不做自定义模板宏语言。

- **E3 应用内签名校验平滑更新（承接 `roadmap-1.0` `R3`）**
  - **触发条件**：Apple Developer ID 公证就绪（或用户明确需要免跳浏览器的内建更新）。保留设置里的开关，补齐更新签名密钥管理与 `release.yml` 的 `latest.json` 生成校验。

- **E4 国产与更多数据库支持（承接 `P7` `B1b` / `B4`）**
  - **触发条件**：拿到 **KingbaseES（人大金仓）** 试用安装包/镜像（`B1b`），或出现明确的 **达梦（DM8）** 接入需求（届时先落地 `B4` 驱动按 Cargo feature 拆分）。新库必须跑通连接、目录元数据、查询、分页、导入导出与真库冒烟全套门禁后才标「已支持」。

---

## 三、当前阶段明确不做（及重估条件）

| 明确不做 | 为什么不做 | 可执行的重估条件 |
| --- | --- | --- |
| 为凑搜索词在官网批量生成几十个薄内容门户页（如 `/mysql-gui/`、`/redis-gui/`） | 高度重复的模板页违背工程审美，且易触发搜索引擎低质内容降权；1 篇扎实的 `/docs/databases/` 远胜 20 个空壳落地页 | 某单一数据库确有独立一整篇深度排障/实践内容（≥1500 字真实干货）需要承载时 |
| 在 `site/` 引入 React / Next.js / VitePress / Docusaurus | 当前纯 Rust 静态生成器 0 外部依赖、毫秒级构建、HTML/CSS 完全受控且自带不变量测试；引入第二套前端栈纯属维护负担 | 文档规模增长到数十篇以上且纯静态模板明显阻碍编写效率时 |
| 导入第三方连接时破解其保存的加密口令 | 各家加密方案随版本变化且涉及安全合规风险；口令重输一次进系统钥匙串是最干净透明的做法 | 不重估（安全边界） |
| 云账号体系、团队云端配置同步、AI 自动执行写操作 | 违背 DataOmni「零遥测离线直连、凭据不出本机、Agent 物理只读」的核心信任根基 | 见 `TODOs.md`「暂不优先」 |
| 在第一、二阶段清零前重构大文件或抽象统一 `Driver` trait | 现有代码有穷尽 `match` 与 2300+ 条测试护航，无缺陷证据时不搞纯形式重构 | 接入第 8 种独立 SQL 驱动（如达梦触发 `B4`）时按真实边界切分 |
