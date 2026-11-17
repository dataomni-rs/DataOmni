# DataOmni 开发路线与完成度追踪

> **归档**：本文件只留**还没做完的**、**接下来要做的**，以及**「当前版本不做」的可执行重估条件**。做完的条目怎么做、怎么取舍、怎么反向验证，都原样留在归档里：
> - [`docs/archive/TODOs-2026-11-17.md`](docs/archive/TODOs-2026-11-17.md)：到 `v1.1.0` 与官网 `dataomni.dev` 上线为止。代码与测试里写的「TODOs R1–R7」「TODOs C1 / C2 / D2 / D3」「TODOs QS1–QS6 / QT1–QT5 / QR1–QR2」「TODOs PA1–PA8」「TODOs G1–G13」「TODOs A8a」指这一份。
> - [`docs/archive/TODOs-2026-10-06.md`](docs/archive/TODOs-2026-10-06.md)：到 `v0.5.1` 发布为止。代码与测试里写的「TODOs A6b」「TODOs B2b」「TODOs「P0-P3 无阻塞级缺陷」」指这一份。
> - [`docs/archive/TODOs-2026-09-27.md`](docs/archive/TODOs-2026-09-27.md)：更早的记录。「TODOs 4.2「ClickHouse」」这类章节号指这一份。
>
> 原则不变：动手前先评估，结论要带「当前版本」的限定；「不做」要写成可执行的重估条件；做完先提交，再回来改这里的状态。

## 使用规则

- `[ ]` 未开始　`[-]` 进行中　`[x]` 已完成并通过验收　`[!]` 阻塞（条目下说明原因）
- 完成时在条目末尾补上提交号和验证方式；做法、取舍和反向验证的记录写在条目下面
- 新接一种库，要补齐连接、元数据、执行、分页、导入导出和测试，达到核心验收标准后才标「已支持」
- 这份文件长到约 100 KB 时，照原样再归档一次
- **收到缺陷报告先搜归档**：回归时看到但没修的各处都写了重估条件，多半是「有人报」。有人报了，对应的条件就触发了

## 总体进度

| 阶段 | 目标 | 状态 |
|---|---|---|
| P0–P4 | 质量基线、连接与查询内核、日常工作闭环、管理与数据工程、外围与扩展 | [x] |
| P5 | 统一桌面 UI 与交互 | [x]（5.4 的 Windows 验证 2026-11-05 完成） |
| P6 | AI 设计、导出与备份 | [x] A0–A6c、A9、A8a（只读 CLI）完成；A7、A8 已做成 C1、C2；A2b 当前版本不做 |
| P7 | 更多数据库：国产库与云库 | [-] OceanBase、openGauss 完成；KingbaseES 缺安装包；B3 不做；B4 按触发条件 |
| 加固 | 依赖与前端安全边界、持续真库回归、发布与文档一致性（2026-11-08 / 11-09 两轮评估） | [x] QS1–QS6、QT1–QT5、QR1–QR2、PA1–PA8 全部完成 |
| v1.0 | 稳定桌面客户端（`rfcs/roadmap-1.0.md`） | [x] `v1.0.0` 2026-11-05 正式发布 |
| v1.1 与官网 | 代码生成、结构比对、绑定参数、只读 MCP 与 `dataomni.dev` 上线 | [-] U1、C1、D3、D2、C2、G1–G13 完成，`v1.1.0` 2026-11-16 发布，官网 `https://dataomni.dev`（`10d0872`）2026-11-17 上线；U2 winget（`#481999`）与搜索引擎收录外部推进中 |
| v1.2 路线 | 扫清遗留、巩固稳定、官网文档与实用扩展（`rfcs/roadmap-1.2.md`） | [-] 2026-11-17 定稿并启动：严格按 S（扫清遗留）→ V（巩固验证）→ W（官网文档）→ E（实用扩展）顺序推进 |

---

## v1.2 路线：扫清遗留、巩固稳定与实用扩展（2026-11-17 定）

规划、理由与「明确不做」见 `rfcs/roadmap-1.2.md`。在途外部事项（GSC / Bing 提交、winget 上游审核、Apple Developer ID 审批）由用户并行推进；仓内研发严格遵守**「实用、好用、易用、功能稳定」**总纲，按 **S → V → W → E** 四阶段顺序执行，前一阶段阻塞问题未清零不开下一阶段新功能。

### 第一阶段（S）：扫清技术债与工程遗留

- [x] S0 `TODOs.md` 第三次归档（2026-11-17；归档前 112 KB → 归档后 17.6 KB）
  - 已把验收完成的 `v1.0`、`1.x`（已结项条目）、`QS/QT/QR`、`PA1–PA8`、`G1–G13` 与 `A8a` 全文原样归档至 `docs/archive/TODOs-2026-11-17.md`，同步更新 `AGENTS.md` 权威来源表；根目录 `TODOs.md` 只保留本路线任务、在途外部项与未触发的可执行重估条件。
- [x] S1 移除后端残留的 `tauri-plugin-sql`（收口 `2.4` 与 `QS5`）（`b3ca533`，2026-11-17）
  - 已做：在 `services/sqlx_pool.rs` 定义本地 `DbPool`（`Sqlite` / `MySql` / `Postgres`）并复用同步 `PoolRegistry` 作为 `SqlxRegistry`（`std::sync::Mutex` 从编译期阻止跨 `.await` 持锁，彻底杜绝 F24）；`PoolRegistry` 补 `take` 供 `close` 摘池后异步关闭；从 `src-tauri/Cargo.toml` 与 `src-tauri/src/lib.rs` 移除 `tauri-plugin-sql`，删除 `database_smoke.rs` 对 `Cargo.lock` 插件版本号的字符串锁测试；`databaseHandle.test.ts` 新增门禁断言 `Cargo.toml` 与 `lib.rs` 不再出现该插件。
  - 验证：实现前 `databaseHandle.test.ts` 反向变红；实现后 `bun run lint`、`bun run typecheck`、前端 186 个文件 2305 条单测、后端 451 条单测与冒烟测试、双模式 `clippy` 及 `cargo audit` 全绿。
- [ ] S2 解耦官网公开版本号与开发期 `src-tauri/tauri.conf.json`，消除发版窗口期 404 隐患
  - 现状：`site/src/main.rs` 直接读 `tauri.conf.json` 的 `version` 拼 GitHub Releases 下载地址，且 `.github/workflows/site.yml` 监听该文件推送自动上线；下次升开发版本号而草稿 Release 尚未点 Publish 时，官网下载按钮会提前指向未公开 tag 返回 404。
  - 做法：为 `site/` 引入独立的已公开发布版本配置源，从 `site.yml` 的 push 监听路径摘除 `src-tauri/tauri.conf.json`，支持在 Release `published` 后更新部署，并补单测钉住版本校验。
- [ ] S3 补齐包元数据与应用内「官网 / 问题反馈」直达入口
  - `src-tauri/Cargo.toml` 补齐 `license = "MIT"`、`homepage = "https://dataomni.dev"`、`repository = "https://github.com/dataomni-rs/DataOmni"`（`tauri.conf.json` 的 `bundle.publisher` 在 `microsoft/winget-pkgs#481999` 合入前保持不动，留待下次发版连同 `scripts/winget-manifest.sh` 对齐）。
  - 设置「关于与诊断」（`src/components/AboutSection.tsx`）与 macOS `Help` 菜单（`src-tauri/src/app_menu.rs`）补充「官方网站（`dataomni.dev`）」与「问题反馈（GitHub Issues）」入口，走后端受控 `open_url`，前端权限条数保持 5 条不变，渲染核对中英界面。
- [ ] S4 外部渠道落地后的仓内同步（事件触发即收口）
  - WinGet：`microsoft/winget-pkgs#481999` 合入后，在 `site/` Windows 卡片、`README.md` 与 `README.en.md` 补上 `winget install DataOmni.DataOmni`，勾选 `U2`。
  - macOS 签名公证与 Homebrew：视 Apple Developer ID 进展接入 `release.yml` 签名公证与 `Homebrew/homebrew-cask`（或启用 `dataomni-rs/homebrew-tap` 兜底），同步更新官网 macOS 安装提示。

### 第二阶段（V）：巩固稳定已有功能（跨平台与真库闭环）

- [ ] V1 Windows 与 Linux 打包版 `v1.1.0` 增量能力实机闭环（补齐 `C2`、`QS5`、`QS6` 留待项）
  - 在 Linux（WebKitGTK 桌面容器，deb/rpm）与 Windows 11（WebView2，MSI/NSIS）打包产物上逐项验证：
    1. SQLite 经后端连接池打开、对象树与结构页读取、编辑提交、多句柄独立关闭、缺失路径不误建空文件；
    2. 开启严格 CSP 后，编辑器、ER 图 PNG 导出、四大 NoSQL 工作区及更新检查正常，`dataomni.log` 零 CSP 报错；
    3. 打包二进制执行 `dataomni mcp` 经 stdio 跑通只读查询、写语句 `refused` 与生产库 `not_found`。
- [ ] V2 DuckDB 与 Oracle 23ai 真库补验绑定参数及结构比对（补齐 `D2`、`D3` 留待项）
  - DuckDB：接入 `scripts/schema-diff-verify.sh`（或独立真库往返测试），验证两个 DuckDB 库比对与同步脚本执行后再比为空，含 `--skip-kind` 反向验证。
  - Oracle 23ai Free：容器真库验证 `D2` `:name` 绑定参数（含 `q'[...]'` 字面量排除、NULL/数字绑定、报错参数名还原）及 `D3` `is_constraint` 目录查询与结构比对往返，纳入 `oracle_smoke.rs`。
- [ ] V3 持续真库回归（Nightly Smoke）稳定性收口（跟进 `QT3`）
  - 跟踪 `.github/workflows/nightly-databases.yml` 连续四周定时运行记录；按服务端真实能力裁剪兼容库上的不支持类型夹具（如 CockroachDB / openGauss 的 `INT[][]`、`XML`），确保「红 = 真实退化」。
- [ ] V4 日常高频交互稳态巡检
  - 实机复核 `PA2` 工作区快照防抖：连续打字、切标签、直接退出重开后草稿零丢失。
  - 复核网络闪断 / SSH 跳板机重启后，在表数据页提交修改或导出失败时的错误提示与重连引导（对应 `R6` 观察项；提示不足以引导重连时补明确重连入口）。

### 第三阶段（W）：官网实用文档体系（`dataomni.dev` 易用性承接）

- [ ] W1 `site/` 纯 Rust 静态生成器扩展多页文档布局（不引入第三方前端框架）
  - 新增文档共用布局（顶栏、侧边目录、标题锚点、代码块语法高亮与一键复制、中英对应页互跳），自动纳入 `sitemap.xml`、`BreadcrumbList` / `TechArticle` JSON-LD 与 `verify_html_invariants` 死链/长度门禁，双主题桌面与移动端渲染核对。
- [ ] W2 上线首批 4 篇核心双语实用文档页（内容与当前代码逐字对齐）
  - `/docs/quickstart/`（及 `/zh/docs/quickstart/`）：三平台安装、系统钥匙串授权、SSH 跳板机与 TLS 四档含义、生产环境护栏与单事务提交流程；
  - `/docs/mcp/`（及 `/zh/docs/mcp/`）：只读 Agent 访问开启、生产库物理隔离机制、7 种主流 AI 编程工具配置与排错；
  - `/docs/cli/`（及 `/zh/docs/cli/`）：`dataomni` 命令行配置、SQL 与四大 NoSQL 只读命令、JSON 输出与退出码（`0`–`4`）速查；
  - `/docs/databases/`（及 `/zh/docs/databases/`）：17 种数据库连接指南（Oracle 内置驱动免配、openGauss sha256 转发、DuckDB 直查 Parquet/JSON、MySQL 5.7 TLS）与完整真库兼容性矩阵。
- [ ] W3 上线 `/changelog/`（及 `/zh/changelog/`）双语版本更新记录页
  - 呈现 `v1.0.0`、`v1.1.0` 及后续版本的实用改进与兼容性说明，打通官网导航与应用更新提示。
- [ ] W4 外部技术生态目录标准材料就绪
  - 备齐提交 MCP 目录（`modelcontextprotocol/servers`、Smithery、Glama）与 `awesome-tauri` / `awesome-rust` 所需的英文简介、安全边界说明与标准配置示例。

### 第四阶段（E）：审慎扩展高价值实用能力（`v1.2.0` 候选池，前置 S + V 清零后开启）

- [ ] E1 从主流客户端导入连接配置（Navicat `.ncx` / DBeaver `data-sources.json`）
  - 先写最小设计说明：只解析连接名、库类型、主机、端口、库名、用户名、SSH 隧道信息等非机密字段，默认关闭 Agent 访问；绝不破解第三方加密口令，首次连接时引导输入口令存入系统钥匙串。纯函数解析过正反单测后接入导入预览对话框。
- [ ] E2 本地参数化查询模板 / 常用 SQL 片段（承接 `4.4`，复用 `D2` `:name` 绑定参数）
  - 先写最小设计说明：本地落盘（纳入 `R1` 兼容样本门），支持在编辑器与命令面板（`⌘K`）检索插入并直接触发 `:name` 参数填值执行；不做云同步与复杂模板脚本语法。
- [ ] E3 签名公证就绪后的应用内平滑更新（承接 `roadmap-1.0` `R3`，按签名落地条件触发）
- [ ] E4 国产与更多数据库按证据扩展（承接 `P7`：拿到安装包后接 KingbaseES `B1b`；出现达梦需求时先做 `B4` 驱动按 feature 拆分）

---

## 在途外部事项（1.x）

- [-] U2 winget（已提 PR：`microsoft/winget-pkgs#481999`，CLA 已签、11/11 自动校验通过，等上游维护者合入）
  - 脚本 `scripts/winget-manifest.sh <版本>` 生成 `packaging/winget/manifests/d/DataOmni/DataOmni/<版本>/`（1.10.0 格式，自动提取 MSI `ProductCode`、`UpgradeCode`、NSIS `ProductCode` 与 ARP `Publisher`）。
  - Homebrew Cask 视 Apple Developer ID 申请进展再定（见 `S4`）。
- 社区发帖与搜索引擎后台提交由用户自行推进。

---

## 还没做完的旧条目与「当前版本不做」的重估条件

### 已归档阶段留下的重估门（无需翻归档即可速查）

- **R1 落盘兼容补充**：给设置或 `ssh_tunnel` 新增嵌套字段时，先在 `fixtures/on-disk-0.5/` 补样本再改。
- **R7 无表别名时的裸列补全**：有人报「不写 `o.` 补不出列」时，按光标所在语句的 `FROM` 推 `defaultTable`。
- **v0.3 Windows ARM64 原生包**：当前靠系统转译跑 x64 包（且 Oracle 无 Windows ARM64 Instant Client）；重估条件：有 ARM Windows 用户反馈转译慢或装不上。
- **QR2 前端分包**：主 JS 约 2.05 MB（gzip 605 KB），本地读取当前不拆；重估条件：三平台打包版实测冷启动慢且 Performance 确认 script evaluation 占首屏一半以上时，按独立工作区做 `React.lazy`。
- **PA1 单格大 BLOB 截断**：整行受 12 MB 上限保护，单格不截断（避免破坏 WHERE 行定位）；重估条件：有人报单格大 BLOB 卡顿或内存告警时，先测 50 MB `BYTEA` 峰值 RSS 再定「截断 + 禁编辑」。
- **G2 JOIN / 表达式里的 `_bin` 列类型名**：单表裸列已按目录声明类型修正，复杂查询仍显驱动类型（值显示与编辑不受影响）；重估条件：有人因此误以为是二进制时评估 `[patch]` `sqlx-mysql`。
- **G3 openGauss SM3 口令**：`sha256` 已由本地 `opengauss_relay` 支持；`password_encryption_type = 3`（SM3）等有人遇到再做。
- **G8 Redis Sentinel 多哨兵地址与自动切主重连**：有人切换后撞上 `READONLY` 或需填多个哨兵地址时重估。
- **方言特性当前不做（重估条件均为「有人报」）**：Oracle 原生 JSON / VECTOR 直接解码（现有 `JSON_SERIALIZE` 绕法）、SQL Server 超 `9×10¹¹` 的 `money` 精度、MongoDB `db.coll.find()` JS 语法、Oracle Wallet TLS（现走 SSH 隧道）、OceanBase 表达式默认值解析。

### 2.4 表数据浏览

- [x] 元数据与表数据查询彻底离开 `tauri-plugin-sql`（`QS5` `b4464a1` + `S1` `b3ca533`，2026-11-17 收口）
  - 前端 `@tauri-apps/plugin-sql` 与 `sql:*` 权限已由 `QS5` 撤除；后端残留的 `tauri-plugin-sql` 连接池容器已在 `S1` 中整包移除，目录查询统一走后端 `sqlx_pool::select`。

### 3.1 结构与对象管理

- [-] 查看建表 DDL
  - MySQL / SQLite 已完成（`e7b6bc1`）；批量重建导出已在 5.3 完成（`82e3e33`）；**PostgreSQL 单表原生 `SHOW CREATE TABLE` 当前版本不做**。
  - 重估条件（可执行）：`services::schema_metadata::tests::postgres_has_no_ddl_query` 变红。

### 4.3 非关系型数据库专属工作区

- [ ] MongoDB 文档浏览与查询模型（扩展项）
  - 明确不做：JS 写法的 mongosh 多命令脚本；LDAP / Kerberos / AWS IAM 认证；删库与改集合名；导出聚合结果与 JSON 数组 / CSV 导入导出；`hidden` / `collMod` 改索引；乐观锁以外的并发控制。
  - 重估条件：有人拿着 mongosh 脚本、这类账号或 `--jsonArray` 文件来，或有人要。
- [ ] Redis 键空间、类型和值浏览（扩展项）
  - 单机、Sentinel、集群已完成（`8cd7ea1`、`037eab0`）；还没做：客户端证书、RESP3 推送与发布订阅。
  - 重估条件：有人拿着这种部署来。
- [ ] 每种数据库有独立的能力声明
  - 重估条件：出现第一个前后端能力判断不一致的缺陷，届时做共用语料（仿 `fixtures/export-conformance.json`）。

### 4.4 可选外围能力

- [ ] 连接和查询模板（已纳入 `v1.2` 路线 **`E1`** 连接迁移导入与 **`E2`** 本地参数化查询模板）
- [ ] ~~插件或扩展机制~~ **评估完成：不做**（同类诉求到第三个再谈机制）
- [ ] ~~团队配置同步~~ **当前版本不做**（需服务端；已有本地连接导入导出 `cd1c442`）

### 5.1–5.4 布局、设计系统与桌面交互保留项

- [-] 5.1 多条结果之间单独拖动调整（各条结果已有高度上限与逐条收起 `a26cf41`；有人明确提出再做）
- [-] 5.2 间距、字号、阴影、层级 token **当前版本不做**（颜色与圆角已完成 `1cdcd61`，门是 `designTokens.test.ts`；密度铺向工具栏/侧边栏时再做）
- [-] 5.2 编辑器 Tab 宽度、自动换行、快捷键改键（有人提再做；网格字号固定按 13px 等宽估算列宽，不开放）
- [-] 5.3 Oracle 隐式多结果集（SQL Server / MySQL 多结果集已完成 `83dfc59`；有人要看 Oracle 存储过程第二个结果集时再改通道设计）
- [-] 5.3 应用菜单不搬业务动作（macOS 基础菜单与 `Help` 入口见 `S3`，业务动作由命令面板统一承载，门：`shortcuts.test.ts`、`windowCapabilities.test.ts`）
- [-] 5.4 带 Oracle 的 Linux AppImage 不发（重估条件：AppImage 打包能排除 `instantclient` 目录不改 ELF）

### P6 / P7 还开着的

- [ ] ~~A2b 整库结构导出~~ **当前版本不做**（勾选多表导出建表语句已在 `82e3e33` 支持；整库等价导出等有人要时用 `pg_dump --schema-only` 等原生工具）
- [ ] A8a 第三批：命令行写操作（第一、二批只读与 `dataomni mcp` 已全部完成；写操作按 `rfcs/agent-cli.md` §6 等有人要再做）
- [!] B1b KingbaseES：**阻塞，没有安装包**。解除条件：拿到试用安装包（见 `E4`）。
- [ ] ~~B3 云库认证~~ **不做**（2026-10-05 定）
- [ ] B4 驱动按 Cargo feature 拆分
  - 重估条件（满足任一）：出现不许随包分发或仅部分平台有客户端库的数据库（如达梦）；或安装包体积成为实际阻碍。

## 暂不优先

- [ ] 团队实时协作
- [ ] 云端数据同步
- [ ] AI 自动执行写操作
- [ ] 大型 BI 仪表盘
- [ ] 在适配器体系完成前继续增加数据库图标
