# 应用内克制型 AI 可用性增强（AI）

> 立项日期：2026-11-20。当前状态：**已实现（AI0–AI2，2026-11-20，跟代码走）**。任务状态维护在根目录 `TODOs.md`「v1.3.0 夯实根基路线 · 第三阶段（AI）」。
> 本设计严格遵守 [rfcs/ai-design-and-export.md](./ai-design-and-export.md) 与 [rfcs/editions-and-branching.md](./editions-and-branching.md) 的既有红线：**整块留在 Cargo feature `ai` 后面、零数据行外发、发包原文全程可展开审阅、绝不自动执行任何 SQL**。

## 1. 要回答的问题

DataOmni 目前的内置 AI 只做了「从零设计新表 / MongoDB 集合 / Neo4j 图模型」（[AiDesignView.tsx](../src/components/AiDesignView.tsx) 等），覆盖的不到开发者日常数据库时间的 5%。日常 90% 的真实痛点在下面三件事：

1. **配置与调用体验有明显短板**：
   - 既有缺陷：后端 [ai_commands.rs](../src-tauri/src/commands/ai_commands.rs#L21) 抛出的缺 Key 错误码是 `DATAOMNI_AI_KEY_MISSING`，前端 [aiSettings.ts](../src/utils/aiSettings.ts#L88) 却在查 `DATAOMNI_CREDENTIAL_MISSING`，导致未配 Key 时提示走错分支；
   - 设置页 [AiSettingsSection.tsx](../src/components/AiSettingsSection.tsx) 没有「测试连接」，填错地址或模型名只能等到真生成时碰壁；
   - 后端请求写死 120 秒超时且不可中途取消，模型服务卡住时用户只能干等；
   - 全应用只有单一全局模型端点，想让开发环境用云端大模型、而把敏感/预发环境限制在本地 Ollama，目前做不到。
2. **SQL 编辑器高频场景缺少轻助手**：写复杂关联查询、看到数据库晦涩报错、或者面对一大段 `EXPLAIN` 执行计划树时，用户现在必须把表结构、报错、计划手动复制去外部聊天窗口再贴回来。
3. **表结构语义注释靠手写太耗时**：连接里几十张表若能一键草拟业务注释（明确标出「AI 推测」），人审阅后直接存进 `rfcs/cli-mcp-usability.md`（CM4）的连接级本地语义备注，既方便人看，也直接反哺 CLI / MCP 外部 Agent。

**核心边界**：**绝不在 DataOmni 里塞一个多轮聊天侧边栏或自主查数 Agent**（那是外部 Agent 通过 MCP 做的事）。应用内 AI 只做嵌入既有界面、单步可控、只出草稿/建议的「手术刀」。

## 2. 已核对的代码事实

| 事实 | 位置 | 对设计的影响 |
| --- | --- | --- |
| 后端 AI 调用仅 130 行核心代码，支持 `Anthropic` 与 `OpenAI` 兼容协议，整块在 `#[cfg(feature = "ai")]` 后面 | `src-tauri/src/services/ai.rs`、`src-tauri/src/commands/ai_commands.rs` | 只需给 `ai_complete` 加可取消句柄（`AbortHandle`）与最小连通探测，无需引入重型 LLM SDK |
| 前端有现成的透明发包展示面板 `AiRequestPanel` | `src/components/AiRequestPanel.tsx` | 新增的三把手术刀全部复用「展开查看完整 system/user 原文」的交互习惯 |
| SQL 编辑器已有补全目录（表名、列名、类型、外键） | `src/utils/sqlCompletionSchema.ts`、`src-tauri/src/services/completion_catalog.rs` | NL→SQL 与报错修复直接从内存目录组装精简 DDL 片段，不额外扫库 |
| 报错面板已经持有错误码、行列位置、detail、hint、constraint、table 与出错 SQL | `src/components/QueryErrorPanel.tsx` | 「AI 诊断与修复」只需一个按钮，零额外数据拉取 |
| 执行计划对话框已经持有解析好的 `PlanTree`、`worstEstimate` 与原始计划文本 `plan.raw` | `src/components/QueryPlanDialog.tsx`、`src/utils/planInsights.ts` | 「AI 解读执行计划」直接挂在计划弹窗工具栏 |
| Key 存系统钥匙串（`#ai`），不进前端状态 | `src-tauri/src/commands/ai_commands.rs` | 新增本地/第二套配置时（如本地 Ollama 免 Key 或备用 Key `#ai:<profileId>`），延续零明文落盘规则 |

## 3. 动手前先做的验证实验（AI-E）

沿用 `rfcs/ai-design-and-export.md` §2.5（A0 实验）的实证做法，在写界面前先用脚本验证提示词与结构化输出可靠性：

| 编号 | 验证内容 | 样本与标准 | 结果决定什么 |
| --- | --- | --- | --- |
| E1 | **NL → SQL 草稿**：PostgreSQL / MySQL / SQLite / Oracle 四方言各 3 条需求（含多表 JOIN、窗口函数、日期过滤） | 统计 JSON 合法率、语法在真库/EXPLAIN 通过率、发包 token 规模（按 5 / 15 / 30 张表三档测） | 决定默认上下文选表上限（初定最多选 15 张相关表，不无脑塞全库） |
| E2 | **报错诊断修复**：收集 10 类典型真库报错（语法错、列名错、类型不匹配、GROUP BY 漏列、外键/唯一约束冲突） | 统计模型返回 `{ explanation, fixedSql, notes }` 的首轮修复准确度 | 确认只发「出错语句 + 错误结构 + 涉及到的表列」即可命中，不需发整库 |
| E3 | **EXPLAIN 优化解读**：5 份 PostgreSQL / MySQL 真实慢查询计划（全表扫描、估计行数严重偏差、filesort） | 核对建议是否具体指向计划里的真实节点与列，而不是空话套话 | 调优 Prompt，要求输出必须引用计划中的具体表/索引名 |

## 4. 分阶段设计（AI0–AI2）

### AI0：既有缺陷修复与配置底座补齐（最先做）

1. **修复缺 Key 错误码匹配缺陷**（独立小提交，先红后绿）：
   - 将 [aiSettings.ts](../src/utils/aiSettings.ts#L88) 的 `isAiKeyMissing` 修正为匹配 `DATAOMNI_AI_KEY_MISSING`（并补 `aiSettings.test.ts` 反向验证）。
2. **「测试连接」与「请求可取消」**：
   - 后端新增 `ai_cancel(requestId)`：在 Tauri `State` 维护 `AiRequestRegistry`（`HashMap<String, tokio::task::AbortHandle>`），`ai_complete` 接受可选 `requestId`，用户点「取消」时立刻中止 `reqwest` future 并清理槽位，不再死等 120 秒。
   - 设置页新增「测试连接」按钮：发一条极短探测请求（`max_tokens: 16` / 明确小输出），返回延迟与成功状态或服务端原始报错。
   - **支持无 Key 本地端点（Ollama / LM Studio）**：当 `baseUrl` 指向 `localhost` / `127.0.0.1` / `::1` 且钥匙串未存 Key 时允许发空/占位 Bearer（不再被 `AI_KEY_MISSING` 硬拦）。
3. **多预设与按连接环境路由**：
   - 将设置升级为轻量的「模型配置列表」（兼容旧版 `dataomni.ai-settings` 落盘无缝迁移）：支持预置 1 个默认云端配置 + 1 个本地私有配置（如 Ollama），钥匙串按 `#ai`（默认）与 `#ai:<id>` 隔离。
   - 增加一个高价值开关：**「预发/生产环境（`STAGING` / `PROD`）仅允许使用本地/内网模型」**（或彻底禁止对 `PROD` 触发 AI）。触发拦截时前端纯函数与弹窗明确告知原因，零静默外发。

### AI1：SQL 编辑器三把「手术刀」

全部做成**纯函数 Prompt 构造 + 结构化 JSON 响应解析**（放在 `src/utils/aiSqlAssist.ts` 并配完整单测），共享同一个极简结果展示面板（可展开查看发出的 `system`/`user` 原文，按钮只有「插入编辑器 / 替换选中语句 / 复制」，**绝无「直接执行」按钮**）：

1. **自然语言生成 / 改写 SQL**
   - **入口**：SQL 编辑器工具栏「AI 写 SQL」（快捷键可选），`ai_available` 为假或设置未开启时不显示。
   - **上下文控制**：自动按当前编辑区已写 SQL 提取涉及表名作为默认勾选，允许用户手动增减勾选表（上限 15 张表，只发 `CREATE TABLE` 精简列/键定义，**绝不发任何表数据行**；`PROD` 连接默认 0 张表勾选，需用户显式确认且受 AI0 环境路由守卫约束）。
   - **输出**：`{ sql: string, explanation: string, warnings: string[] }`。若是对现有选中 SQL 做改写，并排展示原 SQL 与新 SQL 差异，用户点「采用」才回填编辑器。
2. **报错一键诊断与修复**
   - **入口**：[QueryErrorPanel.tsx](../src/components/QueryErrorPanel.tsx) 右上角「复制」旁新增「AI 诊断修复」按钮。
   - **发包内容**：数据库方言与版本号、报错结构（`code` / `message` / `detail` / `hint` / `constraint` / `table` / 行列位置）、出错 SQL、以及 SQL 中引用的表结构。
   - **输出**：`{ cause: string, fixedSql: string | null, notes: string[] }`；点击「替换出错语句」回填编辑器（若为纯权限/数据冲突类错误，`fixedSql` 为 `null`，只给排查说明，不瞎改 SQL）。
3. **EXPLAIN 执行计划优化解读**
   - **入口**：[QueryPlanDialog.tsx](../src/components/QueryPlanDialog.tsx) 在计划加载完成后显示「AI 解读计划」按钮。
   - **发包内容**：数据库方言、原 SQL、`planInsights` 算出的最差估算节点摘要、`plan.raw`（截断至合理长度上限，如 16 KB）以及涉及表的现有索引列表。
   - **输出**：`{ summary: string, bottlenecks: string[], suggestions: Array<{ title: string, sql?: string, rationale: string }> }`。候选 `CREATE INDEX` 只提供「复制到新标签」供人走正常 DDL 审阅流程，不直接执行。

### AI2：数据字典 AI 业务语义注释草拟（与 CM4 闭环）

- **入口**：在数据字典导出 / 连接级语义备注编辑页提供「AI 草拟业务说明」按钮。
- **做法**：分批（每批最多 20 张表）发送表名、列名、类型、主外键与已有注释，模型按表生成精简中文/英文业务说明，所有自动生成段落头部固定带 `<!-- AI-inferred: review before trusting -->` / `[AI 推测]` 提示。
- **落盘闭环**：用户审阅删改后点击保存，直接写入 `rfcs/cli-mcp-usability.md`（CM4）的 `semantics/<connection_id>.md`——从此人看字典、编辑器 AI、以及外部 Agent（`cli dictionary` / MCP）共用同一份干净的业务语义资产。

## 5. 验收门与反向验证

1. **特征裁剪门**：`cargo check --no-default-features`、`cargo test --no-default-features`、`bun run rust:clippy:no-ai` 全绿；不带 `ai` 构建时 `ai_available()` 返回 `false`，编辑器、报错面板、执行计划弹窗与设置页不露出任何 AI 入口。
2. **缺 Key 与取消门**：未填 Key 调云端端点必触发 `isAiKeyMissing === true` 专用提示（反向验证：改回旧错误码测试必红）；连接本地 `127.0.0.1` 模拟慢响应服务，调用 `ai_cancel(requestId)` 后 500ms 内中止返回取消错误。
3. **环境路由与零数据行门**：纯函数单测验证开启「`PROD` 仅允许本地模型」后，指向公网 URL 的配置在 `PROD` 连接上被明确拦截；反向验证：放行公网 URL 时测试必红；断言所有 Prompt 构造函数的输入类型不包含 `ResultRow` / 表行数据。
4. **旧配置兼容门**：`fixtures/on-disk-0.5/` 风格的旧版 `dataomni.ai-settings` JSON 升级读取无损，不丢 `baseUrl`、`model`、`protocol`、`enabled`。
5. **纯函数解析门**：`aiSqlAssist.test.ts` 覆盖裸 JSON、```json 代码块包裹、字段缺失、非 JSON 文本与 `fixedSql: null` 场景。
6. **真界面门**：中英文双语下核对设置页、编辑器 AI 抽屉、报错诊断、计划解读的排版与暗黑主题，确保「发出去的原文」可完整展开、无字面 `{placeholder}` 残留。

## 6. 当前版本不做（及可执行重估条件）

| 不做项 | 为什么当前不做 | 可执行重估条件 |
| --- | --- | --- |
| 多轮 Chat 侧边栏 / 自主多步查数 Agent | 与 Claude Code / Cursor / 外部 MCP 定位重叠，且会把桌面客户端拖成臃肿聊天外壳 | 不做（复杂多步 Agent 统一走 `dataomni mcp` / `dataomni cli`） |
| AI 自动执行 SQL（尤其是写操作 / DDL） | 破坏「所有执行都经过人眼审阅与环境确认」的安全底线 | 不重估（永久安全红线） |
| 流式 Token 打字机输出（SSE） | 三把手术刀均为结构化 JSON 输出，拿到完整 JSON 才能安全解析渲染；先做好「可取消 + 加载状态」更简单稳定 | 当单次生成普遍超过 15 秒且用户明确反馈等待体感差时，再评估按协议加流式进度 |
| 本地向量库 / Embedding RAG 索引 | 引入重型依赖与索引同步负担；日常查询 95% 只涉及 1–10 张表，显式勾选 + 当前语句表提取足够精准 | 单库表数 > 500 且手动选表成为真实高频痛点时再评估轻量 BM25 本地检索 |
