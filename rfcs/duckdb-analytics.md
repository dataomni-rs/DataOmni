# 内置 DuckDB 统一结果集分析、列级画像与可视化升级（DA）

> 立项日期：2026-11-20。当前状态：**DA0–DA3 已全部实现（2026-11-20，跟代码走）**。任务状态维护在根目录 `TODOs.md`「v1.3.0 夯实根基路线 · 第一阶段（DA）」。
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

## 3. 动手前先做的实验（DA0，2026-11-20 实测完成）

每条实验均在内置 `duckdb = "1.20000.1"`（macOS ARM64，`test` 未优化 debug 构建）上跑完并反向验证，实测数据与拍板结论如下：

| 编号 | 实测结果（debug 构建） | 拍板结论 |
| --- | --- | --- |
| E1 | 20 列混合类型 `Appender` 装载 + 完整画像（`SUMMARIZE` + 9 列 10 桶直方图 + 8 列 Top-5）：<br>• 1 万行：装载 `180 ms`、画像 `1,105 ms`（`SUMMARIZE 586` + 直方图 `381` + Top-5 `137`），DuckDB 内存 `6.5 MiB`<br>• 5 万行：装载 `766 ms`、画像 `1,749 ms`，DuckDB 内存 `28.0 MiB`<br>• 10 万行：装载 `1,500 ms`、画像 `2,593 ms`（`SUMMARIZE 1,426` + 直方图 `657` + Top-5 `508`），DuckDB 内存 `54.5 MiB` | 默认装载上限取 `100,000` 行，单会话 `SET memory_limit = '512MB'`、`SET threads = 4` 充裕安全；画像耗时在 1–2.6 秒区间，必须走异步 `spawn_blocking` 并提供取消按钮 |
| E2 | 5,000 条 5 组异构嵌套 JSON（缺失键 vs 显式 `null`、对象/数组漂移、同字段 5 种类型漂移、空文档、Unicode）：<br>• 临时 NDJSON + `read_json_auto`：`481 ms`，需落盘临时文件，且无法区分「字段缺失」与「值为 `null`」<br>• 纯内存 Rust 单遍扫键 + `Appender`：**`120 ms`（快 4 倍）**、零临时文件，精确统计 `presentCount = 2000` / `explicitNullCount = 1000`（缺失 `3000`），类型漂移列稳定降级为 `VARCHAR` | MongoDB / Elasticsearch 文档统一走**纯内存单遍扫键 + `Appender`**，不落临时文件；保留精确 `presentRatio`，嵌套对象/数组序列化为 JSON 文本入 `VARCHAR` |
| E3 | 正向：`SET enable_external_access = false; SET lock_configuration = true;` 挡住全部 6 类逃逸（`read_text`、`read_csv_auto`、`glob`、`COPY TO`、`ATTACH`、重开 `enable_external_access`）；反向：撤掉锁后 `read_text('/etc/hosts')` 与 `COPY TO` 均成功。<br>本地文件模式实测：`CREATE VIEW` 在锁死外部访问后查询期报 `Permission Error`；`allowed_paths` 在 1.2 配置项接口报错；**先物化 `CREATE TABLE result_set AS SELECT * FROM ... LIMIT 100000` 再锁死外部访问**查询正常且彻底封死逃逸 | 两类模式均在物化建表后立即执行 `SET enable_external_access = false; SET lock_configuration = true;`；导出 Parquet 时由后端在沙盒内执行只读 SQL 取回结果，再经独立瞬态内存连接写目标文件，用户 SQL 永不接触未锁连接 |
| E4 | 20,000 行自连接（4 亿次组合）运行 120 ms 后调 `interrupt()`：**0 ms 延迟即中止**（`INTERRUPT Error`），紧接着同一连接 `SELECT COUNT(*) FROM result_set` 返回 `20000` | `analytics_cancel` 直接复用 `InterruptHandle::interrupt()`，中止后会话保持可用无需重建 |
| E5 | `Appender` 直接接受 `Value::BigInt`/`Value::HugeInt`/`Value::Blob` 及 `Value::Text` 写入 `DECIMAL(38,8)`/`DATE`/`TIME`/`TIMESTAMP`，`9007199254740993`、`i128::MAX`、`38` 位 `DECIMAL` 精确回读无损；超 `i128`/`DECIMAL(38,s)` 退回 `VARCHAR` 后 `SUMMARIZE` 照常运行不报错 | 映射规则定型：`bigint` 按范围选 `BIGINT` / `HUGEINT` / 超限退 `VARCHAR`；`decimal` 按最大标度拟合 `DECIMAL(38, scale)`，超限退 `VARCHAR`；`binary` 入 `BLOB` 仅算非空率与字节长 |

## 4. 架构设计：内存 DuckDB 沙盒（`services/local_analytics.rs`）

### 4.1 生命周期与资源上限
- 新增 `LocalAnalyticsRegistry`（托管在 Tauri `State`，按 `analytics_id: String` 索引），每个分析会话持有一条 `:memory:` `duckdb::Connection`、创建时间戳、元信息（来源类型、行数、列数）以及活跃查询的 `InterruptHandle`。
- **硬上限**（初值由 DA0 E1 实测拍板）：
  - 单会话 DuckDB 设 `SET memory_limit = '512MB'`、`SET threads = 4`；
  - 默认允许同时存活最多 8 个分析会话，超限时按 LRU 淘汰最久未使用的非活跃会话；
  - 前端标签页关闭、切换连接或关闭分析抽屉/对话框时主动调用 `analytics_close(analytics_id)` 释放内存。
- **两类明确隔离的打开模式**（已由 DA0 E2/E3 实测拍板）：
  1. **结果集模式**（`analytics_load_result` / `analytics_load_json_docs`）：通过 `Appender` 写入内存表 `result_set`（MongoDB/ES 走纯内存单遍扫键，保留精确 `presentCount` 与 `explicitNullCount`，零临时文件），建表后立即执行 `SET enable_external_access = false; SET lock_configuration = true;`，用户二次 SQL 只能访问内存表 `result_set`，物理杜绝读写磁盘。
  2. **本地文件只读探索模式**（`analytics_open_file`）：仅允许用户经系统文件选择器显式选中的 `.parquet` / `.csv` / `.tsv` / `.json` / `.ndjson` 文件，后端以 `CREATE TABLE result_set AS SELECT * FROM ... LIMIT <max_rows + 1>` 物化到内存（超出上限时截断并标记 `truncated = true`）后，**立即**执行 `SET enable_external_access = false; SET lock_configuration = true;` 锁死沙盒；导出 Parquet 时在沙盒内执行只读 SQL 取回数据，再由独立的瞬态内存连接写目标文件，用户 SQL 永不接触未锁连接。

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
