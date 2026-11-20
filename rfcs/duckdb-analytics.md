# 内置 DuckDB 统一结果集分析、列级画像与可视化升级（DA）

> 立项日期：2026-11-20。当前状态：**设计稿（未开始实现）**。任务状态维护在根目录 `TODOs.md`「v1.4 夯实根基路线 · 第一阶段（DA）」。
> 原则不变：先跑 §3 的基准与边界实验，结果填回本文再动手；结论限定在当前版本（`v1.2.2`）；不做重型 BI 仪表盘，只做对个人开发者与日常排障高频有用的确定性能力。

## 1. 要回答的问题

现在 DataOmni 已经把 `duckdb`（含 `parquet`、`json` 扩展）完整编进了安装包，但普通用户查完 MySQL、PostgreSQL、Oracle、MongoDB 或 Elasticsearch 之后，面临三个高频摩擦：

1. **看不清这批结果长什么样**：空值多不多、有多少唯一值、金额/耗时中位数与 P95 是多少、有没有异常极值——必须自己再写一堆 `COUNT / MIN / MAX / PERCENTILE` 回线上库跑。
2. **不敢反复打线上库做二次筛选/透视**：查出一批几千上万行的结果（或 MongoDB/ES 的嵌套 JSON 文档），想换个维度 `GROUP BY` / `PIVOT` 或和本地 CSV 对比，目前只能导出再开别的工具。
3. **快速图表上限太紧且不能带走**：[ResultChartDialog.tsx](../src/components/ResultChartDialog.tsx) 目前只有柱状图与折线图、超过 200 行直接拒画，也不支持导出 SVG / PNG 或复制 Markdown 摘要。

核心解法：**用内置内存 DuckDB 充当「本地统一计算沙盒」**——把任意数据库已查出的结果集（或本地 CSV / Parquet / JSON 文件）一次性装载为内存临时表，一套后端代码同时支撑**列级数据画像、二次 SQL/透视、Parquet 导出、以及破除 200 行限制的原生 SVG 图表**，且对原数据库零额外压力。

```text
[11 类 SQL / NoSQL 查询结果 或 本地 CSV/Parquet/JSON]
                       │  一次性装载（受行/字节上限保护，关标签即销毁）
                       ▼
        [内置内存 DuckDB 沙盒（services/local_analytics.rs）]
                       │
      ┌────────────────┼────────────────┬────────────────────┐
      ▼                ▼                ▼                    ▼
 列级数据画像     二次 SQL / 透视   导出 Parquet / CSV   图表聚合查询
(空值/分位/直方图) (零线上库压力)  (复用于 CLI export)  (交给纯函数 SVG)
```

## 2. 已核对的代码事实

| 事实 | 位置 | 对设计的影响 |
| --- | --- | --- |
| `duckdb = "1.20000.1"` 已启用 `bundled`、`parquet`、`json` 三个 feature | `src-tauri/Cargo.toml` | 零新增第三方 crate，不增加包体积 |
| 已有 `:memory:` 内存库、`enable_external_access(false)` + `lock_configuration=true` 沙盒、以及 `interrupt()` 真取消 | `src-tauri/src/services/duckdb.rs` | 沙盒隔离与慢查询取消直接沿用既有模式 |
| 全部关系库查询结果统一为 `ResultColumn` + `SerializedResultValue`（含 `bigint`/`decimal`/`date`/`time`/`datetime`/`json`/`binary` 标签对象） | `src/contracts/resultSet.ts`、`src-tauri/src/services/query_executor.rs` | 只需写一份 `SerializedResultValue -> DuckDB` 列映射，11 类 SQL/NoSQL 结果通吃 |
| 现有图表全部决策都在纯函数 `resultChart.ts` 中，组件只做手写 SVG 渲染 | `src/utils/resultChart.ts`、`src/components/ResultChartDialog.tsx` | 扩展图型不引入 ECharts / Chart.js 等重依赖，100% 兼容严格 CSP |
| 已有带行内计算样式的 SVG 序列化与 2× 高清 PNG 导出 | `src/utils/svgExport.ts` | 图表导出 SVG/PNG 直接复用 `serializeSvgWithInlineStyles` |
| 前端表格有意不做虚拟滚动（每页 ≤ 200 行），后端流式查询默认有行/字节上限 | `AGENTS.md`、`src/utils/gridPagination.ts` | 二次 SQL 结果同样走分页网格，不打破既有渲染基线 |

## 3. 动手前先做的实验（DA0）

每条实验跑出实测数据并填回本节后再写正式实现；实验脚本或 `#[ignore]` 基准放单测/临时工具，不污染生产路径。

| 编号 | 要验证的问题 | 怎么验 | 结果决定什么 |
| --- | --- | --- | --- |
| E1 | 10 万行 × 20 列混合类型结果集（整数、小数、长文本、时间戳、NULL、BigInt）经 DuckDB `Appender` 装载进内存表并执行列画像（`SUMMARIZE` + 直方图 + Top-5）耗时与峰值 RSS 是多少？ | Rust 集成基准测试，记录 1 万 / 5 万 / 10 万行三档装载耗时、画像耗时、峰值内存 | 定默认装载上限（行上限、内存 `memory_limit`）与是否需要异步进度/取消 |
| E2 | 5,000 条异构嵌套的 MongoDB / Elasticsearch JSON 文档（含字段缺失、嵌套对象、数组、同字段类型漂移）直接喂给内存 DuckDB `read_json_auto` 时，Schema 推断行为与缺失率计算是否稳定？ | 构造 5 组脏 JSON 样本在内存 DuckDB 实跑 | 决定 MongoDB/ES 走内存 JSON 文本流还是临时只读管道，以及类型冲突列的降级规则（降为 `VARCHAR`） |
| E3 | 内存沙盒设置 `enable_external_access = false` + `lock_configuration = true` 后，用户在二次 SQL 里执行 `read_text('/etc/hosts')`、`COPY result_set TO '/tmp/x'`、`ATTACH '/tmp/x.db'` 是否全部被拒？ | 单元测试反向验证（删掉配置时用例必红） | 确认二次 SQL 无法逃逸到本地文件系统（本地文件探索模式另开明确只读绑定路径，不开放任意路径写入） |
| E4 | 对内存 DuckDB 执行 CPU 密集型二次 SQL（如大表自连接）时，调用 `interrupt()` 能否在 1 秒内中止且沙盒表仍可继续查询？ | 单元测试验证中止后再次 `SELECT count(*) FROM result_set` 成功 | 确认前端「取消分析」按钮的交互契约 |
| E5 | `SerializedResultValue` 中的超大 `bigint`（> `2^53`）、高精度 `decimal`（38 位）、`binary` 装进 DuckDB 后，二次查询回传是否保持无损或有明确标注？ | 边界值单测 | 决定映射列类型：`bigint` → `HUGEINT`/`BIGINT`，`decimal` 溢出回退 `VARCHAR` 并标记不可数值统计，`binary` 仅统计非空率与字节长 |

## 4. 架构设计：内存 DuckDB 沙盒（`services/local_analytics.rs`）

### 4.1 生命周期与资源上限
- 新增 `LocalAnalyticsRegistry`（托管在 Tauri `State`，按 `analytics_id: String` 索引），每个分析会话持有一条 `:memory:` `duckdb::Connection`、创建时间戳、元信息（来源类型、行数、列数）以及活跃查询的 `InterruptHandle`。
- **硬上限**（初值由 DA0 E1 实测拍板）：
  - 单会话 DuckDB 设 `SET memory_limit = '512MB'`、`SET threads = 4`；
  - 默认允许同时存活最多 8 个分析会话，超限时按 LRU 淘汰最久未使用的非活跃会话；
  - 前端标签页关闭、切换连接或关闭分析抽屉/对话框时主动调用 `analytics_close(analytics_id)` 释放内存。
- **两类明确隔离的打开模式**：
  1. **结果集模式**（`analytics_load_result` / `analytics_load_json_docs`）：装完表后立即执行 `SET enable_external_access = false; SET lock_configuration = true;`，用户二次 SQL 只能访问内存表 `result_set`（及只读系统函数），物理杜绝读写磁盘。
  2. **本地文件只读探索模式**（`analytics_open_file`）：仅允许用户经系统文件选择器显式选中的 `.parquet` / `.csv` / `.tsv` / `.json` / `.ndjson` 文件，后端按只读视图 `CREATE VIEW result_set AS SELECT * FROM ...` 绑定后即锁死外部访问（或在 `read_only_gate` 中禁止 `COPY` / `ATTACH` / `EXPORT` / `read_*` 访问其他路径，具体由 DA0 E3 实测确认）。

### 4.2 Tauri 命令清单（最小集合）
1. `analytics_load_result(columns, rows)` → `{ analyticsId, rowCount, columnCount, truncated }`
2. `analytics_load_json_docs(documents)` → `{ analyticsId, rowCount, columns }`（供 MongoDB / ES 复用）
3. `analytics_open_file(path)` → `{ analyticsId, rowCount, columns, fileKind }`
4. `analytics_profile(analyticsId)` → `ColumnProfile[]`
5. `analytics_query(analyticsId, sql, limit?, timeoutSeconds?)` → 复用 `QueryExecutionResult`（仅允许单条只读 SQL，走现有 `read_only_gate`）
6. `analytics_cancel(analyticsId)` → `()`
7. `analytics_export_parquet(analyticsId, sql?, outputPath)` → `{ rowCount, bytes }`（拒绝覆盖已有文件或经系统保存对话框确认）
8. `analytics_close(analyticsId)` → `()`

## 5. 三项核心能力设计

### DA1：列级数据画像（Data Profiling）
- **输出结构**（前后端共用契约 `src/contracts/analytics.ts`）：
  - 通用字段：`name`、`inferredType`、`totalCount`、`nullCount`、`nullRatio`、`distinctCount`、`distinctRatio`；
  - 数值/日期列：`min`、`max`、`mean`、`stddev`、`p25`、`p50`、`p75`、`p95`、8–12 格等宽直方图 `bins: { lower, upper, count }[]`；
  - 分类/文本列：`minLength`、`maxLength`、`topValues: { value, count, ratio }[]`（Top 5）；
  - MongoDB / ES 文档列：额外标出 `presentRatio`（字段存在率，区分「字段缺失」与「字段值为 `null`」），一眼识别 Schema 漂移。
- **界面呈现**：
  - 在结果工具栏提供「数据画像」抽屉/切换页（**不自动在每次查询后重算，按需点击触发**，避免拖慢普通查询）；
  - 每列一张紧凑卡片（迷你 SVG 直方图 / Top-K 进度条 / 空值率红黄提示），点击某一列或某个高频值可一键生成对应的二次过滤 SQL。

### DA2：结果集二次 SQL / 透视 & 本地文件即席探索
- **二次 SQL 面板**：紧邻结果网格，自带简洁语法提示（表名固定 `result_set`，列名为当前列），内置 3 个一键模板按钮：
  - 「按列分组汇总（`GROUP BY`）」
  - 「交叉透视（DuckDB 原生 `PIVOT`）」
  - 「去重 / 异常值排查」
- 结果直接复用 `QueryResultScrollTable` 分页渲染，并支持**直接再画图**或导出 CSV / JSON / **Parquet**。
- **本地文件入口**：在顶部菜单 / 侧边栏加「打开本地数据文件（Parquet / CSV / JSON）」，打开一个独立分析标签页，直接具备 DA1 画像 + DA2 二次 SQL + DA3 图表，不污染 `connections.json`。

### DA3：原生 SVG 图表扩展与导出
- 全部坐标、分桶、色板、刻度计算落在 `src/utils/resultChart.ts` 纯函数（可单测），组件只画 SVG：
  - **新增图型**：在 `bar`、`line` 基础上补齐：
    - `stacked-bar`（堆叠柱状图：看构成与总量）
    - `area`（面积图：看时序趋势堆积）
    - `scatter`（散点图：两数值列相关性与离群点，最多 2,000 点，超限显式提示用 DuckDB 聚合或切热力图）
    - `heatmap`（二维分箱/交叉矩阵热力图：两分类列 + 一数值度量，或两数值列经 DuckDB 二维分桶，天然适合大样本分布）
  - **破除 200 行硬上限**：当原始结果超过 `MAX_CHART_POINTS`（200）时，图表对话框不再只给一句拒绝文案，而是提供显式聚合选项（例如「按分类列 `SUM / AVG / COUNT` 取 Top 20」「按时间列按小时/天分桶」），由后端内存 DuckDB 算完后返回精简点集并在图标题副标明聚合口径（绝不悄悄采样）。
  - **导出与分享**：复用 `src/utils/svgExport.ts` 新增「导出 SVG」「导出高清 PNG（2×）」「复制 Markdown 表格摘要」。
  - **严格保留克制**：**不做双 Y 轴**（同一像素高度表达两个不同量纲极易误导，见 `ResultChartDialog.tsx` 既有注释）；不做花哨动画与拖拽仪表盘。

## 6. 与 CLI / MCP 的共用边界

`services/local_analytics.rs` 不依赖 `tauri::AppHandle`（与 `ConnectionService::from_path` 同样设计），因此：
- `rfcs/cli-mcp-usability.md` 的 `cli profile`、`mcp profile` 直接调用同一个 `profile_rows()` / `profile_file()` 函数；
- `cli export --format parquet` 直接复用同一个 `write_parquet()` 函数；
- 画像字段名、直方图桶数、截断语义全仓库只有一份实现，不会出现 GUI 与 CLI 口径分叉。

## 7. 验收门与反向验证

先写测试确认变红，再写实现：

1. **类型与精度门**：`bigint`（超 JS 安全整数）、`decimal`、`NULL`、空字符串、布尔、`date`/`timestamp`、`binary` 装载进内存 DuckDB 后，画像空值率、分位数与二次 SQL 结果逐项精确断言。
2. **沙盒逃逸反向门**：在结果集模式下执行 `SELECT * FROM read_text(...)`、`COPY result_set TO ...`、`ATTACH ...`、`SET enable_external_access = true` 全部失败；反向验证：注释掉沙盒配置行后该测试必红。
3. **取消与资源释放门**：慢查询 `analytics_cancel` 后在 1 秒内返回取消错误且会话不崩；`analytics_close` 后再查该 `analyticsId` 返回明确不存在错误。
4. **MongoDB / ES 嵌套文档门**：3 条结构不一致文档（缺字段 vs 字段为 `null`）装载后，`presentRatio` 与 `nullRatio` 区分正确。
5. **纯函数图表门**：`resultChart.test.ts` 覆盖 `stacked-bar`、`area`、`scatter`、`heatmap` 的比例尺、负值/空值处理、空表/单点边界；导出 SVG 含行内 `fill`/`stroke` 与背景色，暗黑模式下不透底。
6. **真界面与严格 CSP 门**：在真实运行的应用中验证 SQL、MongoDB 结果与本地 `.parquet` 文件完成「画像 → 二次 SQL → 散点/热力图 → 导出 PNG」，`dataomni.log` 零 CSP 报错、零白屏。

## 8. 当前版本不做（及可执行重估条件）

| 不做项 | 为什么当前不做 | 可执行重估条件 |
| --- | --- | --- |
| 多图表自由排版 Dashboard / 报表保存 | 偏离轻量原生数据库客户端定位，维护成本成倍上升 | 明确规划团队/协作看板阶段时再独立评估 |
| 双 Y 轴叠加图 | 视觉误导风险远大于收益，多列量纲不同时应分别看或做归一化 | 不重估 |
| DuckDB 原生联邦 Scanner（跨真库实时 JOIN） | 涉及额外扩展体积、认证/SSH 隧道转发与线上库负载风险；结果集模式已覆盖绝大多数分析诉求 | 有明确高频「单库结果超百万行且必须跨两个在线库实时 JOIN」的真实需求 |
| 引入 ECharts / Plotly / Vega-Lite | 显著增大前端包体、破坏现有纯函数单测与深浅色 Tailwind Token 统一性 | 当手写 SVG 在 2,000 点散点图出现明显帧率掉帧（实测渲染 > 150ms）时再评估 Canvas 渲染层 |
