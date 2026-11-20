# CLI 与 MCP 可用性增强（CM）

> 立项日期：2026-11-20。当前状态：**设计稿（未开始实现）**。任务状态维护在根目录 `TODOs.md`「v1.4 夯实根基路线 · 第二阶段（CM）」。
> 本设计完全建立在 [rfcs/agent-cli.md](./agent-cli.md) 与 [rfcs/mcp-server.md](./mcp-server.md) 既有安全基座之上：**零放宽任何只读权限、`PROD` 不可见与 `<id>#agent` 凭证校验规则**。

## 1. 要回答的问题

DataOmni 的 `dataomni cli` 与 `dataomni mcp` 在安全模型上已经跑通，但实际在日常终端与 Claude Code / Cursor / Claude Desktop 中高频使用时，有四处明显的可用性断层：

1. **接入门槛高，GUI 里无引导**：用户装完应用后，不知道本机打包版二进制的实际路径，也得自己手抄 MCP JSON 配置或 `claude mcp add` 命令；同时看不到 Agent 最近通过 CLI/MCP 查了哪些库、哪条语句被拒。
2. **Agent 探索数据太费上下文，且缺 Parquet 导出**：Agent 想摸清一张表或一段查询结果的数据分布，只能反复发聚合 SQL 或拉 200 行原始数据；终端数据流水线想把查询结果直接导出成 `.parquet` 也没法一步到位。
3. **缺少命名参数与只读结构差异命令**：GUI 已经有 `:name` 绑定参数与跨库结构比对，但在 CLI 里跑参数化脚本或让 Agent 检查「dev 库和 staging 库表结构差哪了」还做不到。
4. **物理表名缺乏业务语义**：`dictionary` 只输出物理表名、列名与类型，遇到缩写字段或状态码，Agent 只能靠猜，经常写错过滤条件或 `JOIN` 口径。

## 2. 已核对的代码事实

| 事实 | 位置 | 对设计的影响 |
| --- | --- | --- |
| CLI 与 MCP 共用同一个 `dispatch(args, dirs)` 入口与只读权限门 | `src-tauri/src/cli/mod.rs`、`src-tauri/src/cli/mcp.rs` | 新增只读能力在 CLI 层实现一次，MCP 按安全边界决定是否暴露，权限逻辑永远只有一份 |
| CLI / MCP 审计日志已经写入独立的 `dataomni-cli.log`（上限 2 MB、轮转 `.1`，不记语句原文） | `src-tauri/src/cli/audit.rs` | GUI 审计查看器直接只读解析该文件，无需新增数据库或修改日志写入格式 |
| MCP 刻意不暴露碰本机文件系统的工具（`export`、`backup`、`csv-preview`） | `src-tauri/src/cli/mcp.rs`（§3.3） | 新增 `profile` 时，CLI 可支持 `--file`，但 **MCP `profile` 严禁收本机文件路径**，只接受已开放连接 + 只读 SQL |
| 后端已有 `bind_sqlite` / `bind_mysql` / `bind_postgres` 等参数绑定，但 `:name` 词法替换目前仅在前端 `src/utils/queryParameters.ts` | `src-tauri/src/services/query_parameters.rs`、`src/utils/queryParameters.ts` | `cli query --param` 需把 `:name` 扫描/替换移植到 Rust，并用共用语料文件钉住前后端一致 |
| `connections.json` 保留 `unknown_fields` 与 `newer_entries` 原样写回 | `src-tauri/src/services/connection_service.rs` | 为连接增加可选本地语义备注字段不会破坏旧版降级兼容 |
| 结构比对核心逻辑目前在前端纯函数 `schemaDiff.ts`，目录查询在 Rust `er_diagram` / `schema_metadata` | `src/utils/schemaDiff.ts`、`src-tauri/src/cli/mod.rs` | `cli diff` 可复用 CLI 已有 `schema` 目录拉取 + 精简 Rust 表/列/索引/外键比对，只出报告不生成破坏性执行 |

## 3. 四项增强设计（CM1–CM4）

### CM1：GUI 内 MCP / CLI 一键接入引导与只读审计日志查看

- **接入引导卡片**（放在设置页「命令行与 Agent」独立小节，并在连接表单勾选「允许命令行与 Agent 只读访问」下方给直达链接）：
  1. **实际可执行路径检测**：后端命令 `agent_cli_info()` 返回当前运行二进制的真实绝对路径（macOS 为 `/Applications/DataOmni.app/Contents/MacOS/dataomni` 或当前 `.app` 路径，Windows 为 `dataomni.exe`，Linux 为 `/usr/bin/dataomni`）、是否已在 `PATH` 中可见、以及当前已开放给 Agent 的非生产连接数量（不返回任何口令）。
  2. **三类一键复制模板**（纯函数 `src/utils/agentConfigSnippets.ts` 生成，带单测）：
     - Claude Code：`claude mcp add dataomni -- "<path>" mcp`
     - Claude Desktop / Cursor：`mcpServers` 标准 JSON 片段（自动处理 Windows 反斜杠转义）
     - 导出 Agent Skill：一键将内置 `guide.md` 导出为 `SKILL.md` 文件或复制到剪贴板。
- **只读审计日志查看器**：
  - 后端新增只读命令 `read_agent_audit_log(limit?)`：读取应用日志目录下 `dataomni-cli.log`（最多倒序返回最近 200 条结构化记录：`timestamp`、`command`、`connection`、`firstKeyword`、`length`、`sha256`、`result`、`ms`），按连接名或结果（`ok` / `refused` / `not_found` / `database`）筛选。
  - **不提供删除或篡改单条日志的按钮**（保持审计留痕诚实；仅沿用现有 2 MB 自动轮转）。

### CM2：CLI / MCP 接入内置 DuckDB 能力（Parquet 导出与 `profile` 画像）

直接复用 `rfcs/duckdb-analytics.md` 中的 `services/local_analytics.rs`，零重复逻辑：

1. **`dataomni cli export ... --format parquet`**
   - 沿用现有 `cli export` 的只读门与「只写新文件、绝不覆盖已有文件」规则；
   - 流式读完只读查询结果后由内置 DuckDB 写出 `.parquet` 文件，stdout 返回 `{ schema: 1, format: "parquet", path, rows, bytes }`。
   - 与 `csv` / `json` 一样，**不暴露到 MCP**（不给无 shell 权限的 Agent 开本地写盘口子）。
2. **`dataomni cli profile` 与 MCP `profile` 工具**
   - CLI 形态：
     - `dataomni cli profile <connection> "<sql>" [--limit N] [--timeout SECONDS]`（走与 `query` 完全相同的权限门、只读事务与 `ROLLBACK`，默认最多采样/装载 10,000 行并显式返回 `truncated: true|false`）；
     - `dataomni cli profile --file <path>`（仅 CLI 可用，分析本地 CSV / Parquet / JSON 文件，不碰连接、不写审计）。
   - MCP 形态：
     - 新增 `profile` 工具（标注 `readOnlyHint: true`），入参仅为 `{ connection, sql, limit?, timeout_seconds? }`——**坚决不提供 `file` 参数**。
     - 返回紧凑 JSON（每列的 `inferredType`、`nullRatio`、`distinctCount`、`min`/`max`/`p50`/`p95`、Top-5 高频值、以及行数/是否截断），让 Agent 在不拉取大段原始数据的前提下秒懂数据特征。

### CM3：参数化查询（`cli query --param`）与只读结构比对（`cli diff`）

1. **`dataomni cli query <connection> "<sql>" --param name=value ...`**
   - 将 `:name` 参数扫描与各方言占位符改写（排除字符串字面量、注释、`::type`、Oracle `q'[...]'`）沉淀为后端纯函数 `services/named_parameters.rs`，并新增共用语料 `fixtures/bind-parameters-conformance.json` 钉住前后端行为完全一致；
   - 支持重复传入 `--param key=value` 与 `--param-null key`（或 `--params-json '{...}'` 保留精确数字/布尔/`null` 类型），参数值**严禁写入 `dataomni-cli.log`**（只记原始 SQL 的长度与 SHA-256）；
   - MCP `query` 工具同步支持可选对象参数 `params?: Record<string, string | number | boolean | null>`。
2. **`dataomni cli diff <source-connection> <target-connection>` 与 MCP `diff` 工具**
   - 两个连接**均必须已开放给 Agent 且通过 `<id>#agent` 凭证校验**（任一为生产库或未开放直接报 `not_found`，绝不泄露存在性）；
   - 仅拉取只读目录元数据（表、列、主键、索引、外键），输出结构差异 JSON（新增表、缺失表、列变化、索引/外键差异）或 `--format markdown` 摘要；
   - **只出只读差异报告，绝不执行任何同步 DDL**。

### CM4：连接级本地业务语义备注（Glossary）注入 `dictionary` / MCP

- **存储方式**：
  - 在 `ConnectionProfile` 上新增可选轻量字段 `semantic_notes: Option<String>`（或按连接 id 存放在配置目录 `semantics/<id>.md`，避免把 `connections.json` 撑大——**推荐侧文件 `semantics/<id>.md`**，完全不增加 `connections.json` 体积，删连接时一并清理，且**不纳入 `<id>#agent` 连接指纹**，修改备注无需重算口令 HMAC）。
  - 上限控制：单连接语义文档上限 32 KB，纯 UTF-8 文本/Markdown，支持在 GUI「连接设置 / 数据字典」里编辑，也可以被 `rfcs/ai-sql-assist.md` 的 AI 注释草拟写入。
- **消费方式**：
  - `dataomni cli dictionary <connection>` 在物理表结构之前自动附上 `## Business Semantics & Notes` 区块（无备注时不输出，保持现有 `fixtures/dictionary-conformance.json` 零破坏）；
  - MCP `dictionary` 工具自动受益；MCP `schema` 在查单表时若侧文件包含对应表名小节（`### <table_name>`），按需附带该表说明。
  - 若备注里无内容，输出与 `v1.2.2` 逐字节一致。

## 4. 安全红线核查表

| 红线 | 本次增强的守卫方式 |
| --- | --- |
| 不放宽只读与 `PROD` 物理不可见 | `profile`、`diff`、`query --param` 全部经过既有 `resolve_for_agents` + `read_only_gate` + 只读事务 `ROLLBACK` |
| MCP 不碰用户任意本机文件 | `export --format parquet` 与 `profile --file` 仅限 CLI；MCP 工具表严格排除文件路径参数 |
| 不泄露口令与查询字面量 | `agent_cli_info()` 绝不返回连接凭据；`--param` 绑定值不入日志；审计查看器只展示已有摘要行 |
| 不破坏现有 CLI/MCP 契约 | 现有命令输出维持 `"schema": 1`（只加可选字段，不改旧字段语义）；`help` 与 `guide.md` 一致性门禁继续全量覆盖新命令 |

## 5. 验收门与反向验证

1. **接入配置生成单测**：`agentConfigSnippets.test.ts` 覆盖 macOS（含空格路径）、Windows（反斜杠转义）、Linux 三平台 JSON 合法性与命令可复制性。
2. **审计日志解析单测**：解析真实 `dataomni-cli.log` 格式（含双引号连接名、异常/截断行不崩）并按 `result` 过滤。
3. **MCP 文件系统隔离反向门**：`mcp.rs` 单测断言 `tools/list` 不含 `export` / `backup` / `csv-preview`，且 `tools/call profile` 传 `file` 参数必报 `usage`（反向验证：允许该参数时单测必红）。
4. **绑定参数共用语料门**：`fixtures/bind-parameters-conformance.json` 前后端双测通过；`cli query --param` 对 SQLite/DuckDB 执行正确，且 `dataomni-cli.log` 扫描不到参数值明文。
5. **`cli diff` 双边权限门**：源/目标任一连接为 `PROD` 或未开放时，一律返回 `not_found`（不区分是源还是目标未开放）。
6. **语义备注无侵入门**：无侧文件时 `dictionary` 输出仍逐字节通过 `fixtures/dictionary-conformance.json`；有侧文件时包含备注段且超 32 KB 显式截断/报错。

## 6. 当前版本不做（及可执行重估条件）

| 不做项 | 为什么当前不做 | 可执行重估条件 |
| --- | --- | --- |
| CLI / MCP 第三批写操作（`exec`、`import`） | 只读安全心智是核心卖点，写操作安全面完全不同 | 有明确非生产自动化写场景并完成 `rfcs/agent-cli.md` §6 护栏评审 |
| SQL Server 接入只读 CLI / MCP | 缺少数据库级 `START TRANSACTION READ ONLY` 防线 | 有真实 SQL Server 用户明确接受单层词法白名单 + 只读账号声明时再单独立项 |
| MCP 长驻连接与 SSH 隧道复用 | 复用池必须保证 GUI 关闭权限后旧连接即时作废，增加状态复杂度 | 经慢隧道高频多轮调用且建连耗时占比 > 50% 成为真实反馈痛点时（复用同时每次仍核验 `connections.json` mtime 与 `<id>#agent` HMAC） |
| HTTP / SSE 传输或引入 `rmcp` SDK | 手写 stdio 传输极简零依赖，完全满足本地客户端需求 | 出现不支持 stdio 的主流本地客户端，或后续独立企业网关阶段再另建服务 |
