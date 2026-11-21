# 多数据模型深度分析与引擎原生诊断（AN，已随 v1.3.0 发布）

> 立项日期：2026-11-20。当前状态：**`AN0–AN10` 已全部实现并随 `v1.3.0` 正式发布（2026-11-21，跟代码走）**。任务状态维护在根目录 [`TODOs.md`](../TODOs.md)「v1.3.0 多数据模型深度分析与引擎原生诊断」。
> 原则不变：先跑 §3 的前置实验并把实测数据写回本文再动手；结论限定在当前版本（`v1.3.0` 底座之上）；「不做」必须写成可执行的重估条件；严守单机轻量排障定位，不做重型 BI 仪表盘，也不对线上库执行无界全表/全键扫描。

## 1. 要回答的问题

`v1.3.0` 的 `DA0–DA3`（见 [`rfcs/duckdb-analytics.md`](duckdb-analytics.md)）已经把内置内存 DuckDB 跑通，实现了「结果集二次 SQL / 列级画像 / 原生 SVG 图表 / 本地 CSV·Parquet·JSON 探索」。但实际使用后暴露出两个结构性短板：

1. **关系库「只看当前页 / 当前小结果集」的画像价值受限**：
   - 在表数据视图（每页默认 100 行）或加了窄条件的 6 行明细上做列画像，看到的不是表级规律；
   - 对纯 UUID / 高基数 ID 列平铺 Top-5 占比，噪音大于信息；
   - 日常高频的「把查出来的线上结果和本地 CSV / 另一条查询结果做对账（`LEFT JOIN` / `EXCEPT`）」目前还不能在同一个 DuckDB 会话内完成。
2. **NoSQL 与图数据库不能硬套关系型的「列级画像」**：
   - **Redis**：开发者要看的是**前缀命名空间内存占比、TTL 过期健康度、BigKey 排行**，而不是二维表列画像；
   - **MongoDB**：目前只展开当前页文档的顶层键，抓不到整集合的**嵌套路径 Schema 漂移（同字段混合类型）、数组膨胀与长期 0 命中的僵尸索引**；
   - **Elasticsearch**：ES 本身就是分布式聚合引擎，只把返回的前 20 条 hits 拉进 DuckDB 统计是舍近求远，缺的是**零文档回传（`size: 0`）的原生 `aggs` 下推、时间轴直方图与 Mapping/分片健康诊断**；
   - **Neo4j**：图数据库排查性能与数据质量看的是 **Label/关系体量、超级节点（Supernode）度分布、孤立节点与属性覆盖率**，不是扁平列画像。

核心解法：**双层分析架构——引擎原生诊断下推 + 本地 DuckDB 二次加工**。

```text
                       ┌─────────────────────────────────────────────┐
                       │     各数据模型专属的引擎原生分析（受限/可取消）     │
                       ├───────────┬───────────┬──────────┬──────────┤
                       │  关系库    │   Redis   │ MongoDB  │ ES / Neo4j│
                       │ 表级抽样   │ 非阻塞SCAN│ $sample  │ size:0   │
                       │ 继承过滤   │ 前缀/TTL  │ 嵌套漂移  │ aggs/映射 │
                       │ 语义画像   │ BigKey    │ 僵尸索引  │ 拓扑/度数 │
                       └─────┬─────┴─────┬─────┴────┬─────┴────┬─────┘
                             │           │          │          │
                             └───────────┴────┬─────┴──────────┘
                                              ▼
                 ┌────────────────────────────────────────────────────────┐
                 │  既有共用底座：ResultChartDialog (纯函数 SVG)            │
                 │            + LocalAnalyticsDialog (锁沙盒内存 DuckDB)   │
                 │            + 多表附加对账 (result_set × ref_1)          │
                 └────────────────────────────────────────────────────────┘
```

## 2. 已核对的代码事实

| 事实 | 位置 | 对设计的影响 |
| --- | --- | --- |
| `LocalAnalyticsRegistry` 已有 10 万行 / 512 MB / 8 会话 LRU 上限、沙盒双锁（`enable_external_access=false` + `lock_configuration=true`）与 `interrupt()` 取消 | `src-tauri/src/services/local_analytics.rs` | 多表对账只需扩展会话内附加表接口；本地文件附加必须走「物化建表 → 重锁或经独立读连接导入」保证用户 SQL 永不触碰未锁连接 |
| `services/redis.rs` 已有 `scan_node` / `scan_cluster`（支持单机与 Cluster 按主节点遍历）、`describe_keys` 与 `with_deadline` | `src-tauri/src/services/redis.rs` | Redis 空间体检直接复用现有游标与集群遍历逻辑，严禁调用 `KEYS *`；需额外处理部分云 Redis 禁用 `MEMORY USAGE` 的降级 |
| `services/mongodb.rs` 已有 `aggregate_documents`、`collection_structure`（已区分视图无索引），`MongoCollectionViewer` 已接入过滤条与 `LocalAnalyticsDialog` | `src-tauri/src/services/mongodb.rs`、`src/components/MongoCollectionViewer.tsx` | `$sample`、`$collStats`、`$indexStats` 可统一走聚合/命令路径；视图与低权限账号须按 `collection_structure` 既有模式优雅降级 |
| `services/elasticsearch.rs` 已有 `EsPool::send`；代码注释明确记录 `_resolve/index` 仅需读权限，而 `_cat/indices` 需要 `monitor` 权限 | `src-tauri/src/services/elasticsearch.rs` | 读权限（`_mapping` + `size: 0` `_search` 聚合）与 `monitor` 权限（`_cat/indices` / `_cat/shards`）必须分离，后者返回 `403` 时只降级对应健康卡片 |
| `services/neo4j.rs` 已有 `Neo4jPool`、`single_column` 与 `CypherRequest` 只读执行，`CypherWorkbench` / `CypherGraphView` 已独立成工作区 | `src-tauri/src/services/neo4j.rs`、`src/components/CypherWorkbench.tsx` | Label/关系计数走 Neo4j O(1) count store，度分布与属性覆盖率必须拆分到受 `LIMIT` 与超时保护的采样步骤 |
| `ResultChartDialog` 与 `src/utils/resultChart.ts` 已支持 `bar` / `line` / `stacked-bar` / `area` / `scatter` / `heatmap` 与 SVG/PNG 导出 | `src/utils/resultChart.ts`、`src/components/ResultChartDialog.tsx` | Redis TTL 分布、Mongo 类型占比、ES `date_histogram` / `terms` 聚合结果直接转为统一表格契约复用现有 SVG 引擎，不新引图表库 |

## 3. 动手前先做的实验（AN0，2026-11-21 实测完成）

5 项边界与安全门实验已全部在本地 Rust `test`（debug 构建，无优化）上跑完并完成反向验证，实测数据与拍板结论如下：

| 编号 | 实测结果（debug 构建） | 拍板结论 |
| --- | --- | --- |
| E1 | 200,000 行表 `WHERE status = 'ok' LIMIT 5000`（无 `ORDER BY`）：`EXPLAIN` 无 `Top N` / `ORDER_BY` 算子，耗时 **`14.42 ms`**；对比 `ORDER BY random() LIMIT 5000`：出现全表扫描 + `Top N` 排序算子，耗时 **`77.12 ms`**（慢 5.3 倍） | 关系库表级抽样**严禁 `ORDER BY RAND()/random()`**，且默认不拼 `ORDER BY`：MySQL / PG / SQLite / DuckDB / ClickHouse 走 `LIMIT N`，SQL Server 走 `SELECT TOP (N)`，Oracle 走 `FETCH FIRST N ROWS ONLY`；明确标注「前 N 行样本（继承当前筛选）」 |
| E2 | 本地 RESP2 仿真服务实测 10,000 Key `SCAN`（每批 100）+ pipeline：<br>• **关键踩坑发现**：`redis-rs` 的 `pipe.query_async::<Vec<Value>>()` 只要流水线里**任意一条**返回服务端错误（如 `-NOPERM` 禁用 `MEMORY USAGE`），整批 pipeline 会直接报错并丢弃同批的 `TYPE`/`PTTL` 结果！<br>• 改为**首键单次探测 `MEMORY USAGE <key> SAMPLES 5`** 后：允许时 10,000 Key 耗时 **`225.75 ms`**；禁止时仅发 **`1` 次**探测即平滑降级（`memoryUsageAvailable = false`），10,000 Key 耗时 **`153.70 ms`**，零失败批次 | Redis 体检在第一批拿到 Key 后先**独立探测 1 次 `MEMORY USAGE`**，成功才拼入后续批量 pipeline；失败则立即降级为元素数/字节长估算口径，绝不让 `NOPERM` 冲毁整批 `TYPE`/`PTTL` |
| E3 | 5,000 条含 12 层嵌套、1,200 个动态时间戳键、显式 `Bson::Null` vs 缺失键、混合类型与数组的异构文档纯内存展平：耗时 **`87.46 ms`**，严格钉在 `max_depth = 8` / `max_paths = 300` 上限（记录 `truncated = 8,656`），零 OOM 且精确区分缺失与 `null` | MongoDB 嵌套 Schema 展平定型为纯内存 Rust 单遍遍历（深度上限 `8`、路径上限 `300`、每数组最多抽样前 `20` 个文档元素防大数组放大），超时通过 `maxTimeMS` 下推 |
| E4 | 本地 mock HTTP 服务（`.no_proxy()`）验证：`_mapping` 递归识别 **`1,053` 个字段**触发 `> 1,000` 膨胀预警；`size: 0` 原生 `date_histogram` / `terms` / `extended_stats` / `percentiles` 响应解析通过；`_cat/indices` 返回 HTTP `403` 时独立降级为 `permissionDenied: true`，只读聚合零受阻 | ES 分析严格拆分为「只读路径（`_mapping` + `size: 0` `_search`）」与「可选监控路径（`_cat/indices` / `_cat/shards`）」，后者 `403` 只折起健康卡片 |
| E5 | 向已执行 `enable_external_access = false; lock_configuration = true;` 的 DuckDB 会话通过**独立瞬态读连接物化 + `Appender`** 附加 10,000 行 CSV 为 `ref_1`：附加耗时 **`202.26 ms`**，跨表 `LEFT JOIN` + `EXCEPT` 耗时 **`55.88 ms`**；正向安全门：附加后 `read_text` / `COPY TO` / `ATTACH` / `SET enable_external_access = true` 全部被拒；反向验证：未锁连接执行 `read_text` 成功 | `analytics_attach_file` 采用「瞬态读连接读盘物化 → `Appender` 写入已锁沙盒会话」双连接隔离方案，用户 SQL 所在会话从创建到销毁全程永不解锁 |

## 4. 分阶段模块设计

### 阶段 1（P0）：结构化数据深度分析升级

#### AN1：表级抽样画像与按列语义智能视图
- **表级抽样画像（解决「当前页 100 行不具代表性」）**：
  - 在 `TableDataViewer` 工具栏「分析 / 画像」提供显式范围切换：`当前页（N 行）` / `表抽样（默认 5,000 行，可选 1,000 / 5,000 / 20,000 行）`；
  - 抽样查询**自动继承当前 `TableFilterBar` 筛选条件**，走只读查询路径 + 显式超时 + 中途取消；
  - 界面标题与导出口径强制标注「抽样样本（前 N 行，按当前筛选）」，绝不伪装为整表精确统计。
- **按列语义分流呈现（纯函数决策落在 `src/utils/analyticsProfile.ts`）**：
  - **唯一 / 高基数 ID 列**（`distinctRatio >= 0.98` 且文本长度集中或整型主键）：默认折叠无意义的 Top-5 列表，卡片首屏只突出空值/重复告警与一键查重入口；
  - **低基数枚举列**：突出分布占比条与长尾小众值标记（识别脏状态码）；
  - **时间/日期列**：在后端画像增加按合理粒度自动分桶的迷你趋势点集（`timeBins`，最多 16 桶），在卡片内绘制极简内联 SVG Sparkline；
  - **数值列**：保留 P25/P50/P75/P95 与直方图，新增「查看离群值（`> P95` 或 `< 0`）」一键生成过滤 SQL。
- **反哺原库**：在画像卡片点击 Top 值、空值或离群区间时，除在内存 `result_set` 过滤外，提供「复制原库 `WHERE` 条件」或「带筛选回到表视图」动作（按当前连接方言转义标识符与字面量）。

#### AN2：DuckDB 工作台多表联动对账
- **后端最小命令扩展**（`src-tauri/src/services/local_analytics.rs` + `commands/analytics_commands.rs`）：
  - `analytics_attach_result(analyticsId, tableName, columns, rows)` → `{ tableName, rowCount, columnCount, truncated }`
  - `analytics_attach_file(analyticsId, tableName, path)` → `{ tableName, rowCount, columnCount, fileKind, truncated }`
  - 安全约束：因为已有会话已执行 `lock_configuration = true` 无法重新解锁读盘，`analytics_attach_file` 先在独立的瞬态只读连接读取并按上限物化出列/行，再经 `Appender` 写入目标沙盒会话（复用与 `export_parquet_from_session` 相同的双连接隔离模式），用户 SQL 所在连接从头到尾永不具备外部访问能力；单会话附加表总数上限 4 张，共享 512 MB 内存限额。
- **前端体验**（`LocalAnalyticsDialog`）：
  - 顶部增加「附加对比表」按钮：支持选本地 `.csv` / `.parquet` / `.json`，或直接选当前工作区已打开的另一张查询结果标签；
  - CodeMirror SQL 补全自动纳入所有已附加的表名与列名；
  - 附加第二张表后自动亮出 2 个高频对账模板：`LEFT JOIN 关联补全`、`NOT EXISTS / EXCEPT 差异核对`。

---

### 阶段 2（P1）：Redis 与 MongoDB 引擎专属分析

#### AN3：Redis 非阻塞 Keyspace 内存、TTL 与 BigKey 体检
- **后端命令与安全红线**（`src-tauri/src/services/redis.rs` + `commands/redis_commands.rs`）：
  - `redis_analyze_keyspace(connectionId, database, pattern?, sampleLimit?, separator?, timeoutSeconds?)`
  - 取消命令：`redis_cancel_analysis(requestId)`（`AtomicBool` 协作取消，每轮 `SCAN` 后检查）
  - **硬护栏**：
    - 严禁调用 `KEYS *`、`DEBUG OBJECT` 或无上限 `HGETALL`/`SMEMBERS`；
    - `sampleLimit` 默认 `5,000`，硬上限 `20,000`；默认时间上限 `15s`，超时立即返回已采集样本并标记 `partial: true`（写明已扫描 Key 数与估算覆盖率）；
    - 按批（如 100 Key 一批）通过 pipeline 批量发 `TYPE`、`PTTL`、`MEMORY USAGE <key> SAMPLES 5` 以及按类型取长度（`STRLEN`/`HLEN`/`LLEN`/`SCARD`/`ZCARD`/`XLEN`）；
    - 若服务端返回 `unknown command` 或 `NOPERM`（常见于托管 Redis ACL 禁用 `MEMORY USAGE`），自动关闭后续批次的 `MEMORY USAGE` 调用，在结果中标注 `memoryUsageAvailable: false` 并回退按元素数/字符串长度排序。
- **前端纯函数聚合与呈现**（`src/utils/redisAnalytics.ts` + `RedisKeyspaceAnalyticsDialog.tsx`）：
  - **前缀树内存剖析**：按可配置分隔符（默认 `:`，可选 `/`、`.`）聚合 1–3 级前缀，展示每前缀的 Key 数、占比、估算内存、主要类型、无过期（`TTL = -1`）占比；
  - **TTL 健康度分布**：固定 5 档（`永不过期 (-1)` / `< 1h` / `1h–24h` / `1d–7d` / `> 7d`），点击任一分档筛选对应样本 Key；
  - **BigKey Top 50**：支持按「估算内存」或「元素数量」切换排序，点击行直接定位到 `RedisKeyBrowser` 详情查看/处理；
  - **单 Key 深度桥接**：在 `RedisValueTable` 中，对 `Hash` / `ZSet`（含 Score 分布）或 JSON 值列表提供「导入 DuckDB 二次分析」入口。

#### AN4：MongoDB `$sample` 递归嵌套 Schema 漂移与索引体检
- **后端命令与安全红线**（`src-tauri/src/services/mongodb.rs` + `commands/mongodb_commands.rs`）：
  - `mongo_analyze_collection(connectionId, database, collection, filter?, sampleSize?, timeoutSeconds?)`
  - **采样策略**：
    - `sampleSize` 默认 `1,000`，硬上限 `10,000`；若用户传入当前 `filter`，构造 `[ { $match: filter }, { $sample: { size } } ]`，否则直接 `[ { $sample: { size } } ]`；`maxTimeMS` 绑定超时；
    - 纯内存递归遍历 BSON，点路径规范化（数组元素统一记为 `items[]` / `items[].price`），硬设**最大嵌套深度 8**、**最大唯一路径数 300**（超出记入 `truncatedPathsCount` 防动态键爆炸）；
    - 每条路径输出：`presentCount`（存在文档数）、`missingCount`、`typeBreakdown: { bsonType, count, ratio }[]`（精确区分 `null` / `int32` / `int64` / `double` / `string` / `objectId` / `date` / `array` / `document`）、标量样例 Top-5、以及数组路径的 `arrayLen: { min, p50, p95, max }`。
  - **存储与索引体检（同接口并行返回，单项失败局部降级）**：
    - 视图跳过存储/索引统计；普通集合并行查 `$collStats`（storageStats）与 `$indexStats`，产出文档均长、数据/索引体积比，以及 `accesses.ops == 0` 的候选僵尸索引（同步展示 `since` 统计起始时间，避免误判刚重启实例）。
- **前端呈现**（`src/utils/mongoAnalytics.ts` + `MongoSchemaAnalyticsDialog.tsx`）：
  - 在 `MongoCollectionViewer` 工具栏增加「Schema 与索引分析」按钮；
  - 嵌套路径表格：直观高亮**混合类型漂移**（超过 1 种非 null 类型时标橙色提示）与**大数组预警**；
  - 点击任意路径下的某类型（例如发现 `age` 里有 3% 是 `string`），一键生成 `{ "age": { "$type": "string" } }` 回填过滤框排查脏文档；
  - 同时把 `MongoConsole` / 聚合管道结果接入 `ResultChartDialog` 一键出图。

---

### 阶段 3（P2–P3）：Elasticsearch 原生聚合分析与 Neo4j 图拓扑体检

#### AN5：Elasticsearch 原生 `aggs` 下推探索与 Mapping / 分片健康诊断
- **后端隔离的两类接口**（`src-tauri/src/services/elasticsearch.rs` + `commands/elasticsearch_commands.rs`）：
  1. **只读索引分析（仅需读权限）**：
     - `es_index_analytics_meta(connectionId, index)`：调用 `GET /<index>/_mapping`，递归统计总字段数（逼近/超过默认 1,000 字段上限时预警 mapping explosion），分类列出时间字段（`date` / `date_nanos`）、可聚合分类字段（`keyword` / `boolean` / `ip`）、数值字段及 `text + .keyword` 双存储清单；
     - `es_run_field_aggs(connectionId, index, query?, spec)`：向 `POST /<index>/_search` 下推 `size: 0` + `track_total_hits: false`（或受控上限）的原生聚合：
       - 时间轴：`date_histogram`（自动根据时间跨度选 `minute` / `hour` / `day` / `week`，或手动切换）；
       - 分类字段：`terms`（Top 20）+ `missing` 缺失统计；
       - 数值字段：`extended_stats` + `percentiles`（`[25, 50, 75, 95, 99]`）。
  2. **可选集群/分片健康（需 `monitor` 权限，独立降级）**：
     - `es_index_health(connectionId, index)`：调用 `_cat/indices/<index>?format=json` 与 `_cat/shards/<index>?format=json`，汇总健康灯、主/副本分片数、单分片大小分布（过小 < 100 MB 或过大 > 50 GB 提醒）与 `docs.deleted / docs.count` 软删除率；返回 `403` 时返回结构化 `permissionDenied: true`，界面友好折叠而不阻塞原生聚合。
- **前端呈现**（`src/utils/esAnalytics.ts` + 接入 `EsConsole` / `EsIndexStructureView`）：
  - 选定索引后顶部可直接展示时间轴直方图（类 Kibana Discover 轻量化版本），点击时间桶自动补全 `range` 过滤条件；
  - 侧边字段列表点一下字段直接下推 `aggs` 并将返回桶转成标准表格行喂给 `ResultChartDialog` 原生 SVG 渲染（柱图/折线/导出 PNG）；
  - 既有「查询结果进 DuckDB」保留为「明细二次清洗 & 导出 Parquet」入口，与原生聚合清晰分工。

#### AN6：Neo4j 图谱拓扑与超级节点体检
- **后端分层查询**（`src-tauri/src/services/neo4j.rs` + `commands/neo4j_commands.rs`）：
  - `neo4j_analyze_graph(connectionId, database, sampleNodesPerLabel?, timeoutSeconds?)`
  - **第一层（O(1) 级安全概览）**：通过 Neo4j count store 快速统计每个 `Label` 的节点总数与每个 `Relationship Type` 的边总数；
  - **第二层（受限采样深度体检，默认按需触发、带 `LIMIT` 与超时保护）**：
    - 每 Label 采样最多 `1,000` 个节点（可调上限 `5,000`），用 `COUNT { (n)--() }`（兼容 Neo4j 5，旧版回退 `size((n)--())` 或显式检测版本）统计入度/出度分位数（P50 / P95 / Max）、孤立节点占比、Top 20 高连接度节点（Supernode 定位，仅返回 elementId/id、Labels 与度数）及各属性键存在率。
- **前端呈现**（`src/utils/neo4jAnalytics.ts` + 接入 `CypherWorkbench`）：
  - 概览页展示 Label / 关系体量柱状占比（复用 `ResultChartDialog` 渲染）；
  - 超级节点与稀疏属性列表支持一键生成只读排查 Cypher 语句（如 `MATCH (n) WHERE elementId(n) = ... RETURN n LIMIT 50`）或直接在 `CypherGraphView` 展开局部邻域。

## 5. 与 CLI / MCP 的共用边界

- `AN1` 的按列语义提示与 `AN2` 的多表逻辑位于无 `AppHandle` 依赖的 `services/local_analytics.rs`；
- `AN3`（Redis 体检）、`AN4`（MongoDB Schema 分析）、`AN5`（ES 聚合/Mapping 摘要）与 `AN6`（Neo4j 拓扑统计）的纯分析逻辑均落在 `services/` 层且返回可序列化 JSON 结构；
- `v1.4` 初期优先交付 GUI 交互闭环；待结构稳定且有真实 Agent 排障诉求时，可零重复代码暴露到 `dataomni cli analyze` / 只读 MCP（继续严守 `PROD` 物理不可见红线）。

## 6. 验收门与反向验证

每阶段落地均须「先写测试确认变红，再写实现」：

1. **AN1 抽样与语义门**：7 类方言抽样 SQL 生成单测覆盖带/不带 `WHERE` 与标识符转义（断言绝不含 `ORDER BY RAND()`）；高基数 ID 列折叠与时间分桶直方图纯函数单测全覆盖。
2. **AN2 沙盒隔离反向门**：附加本地 CSV/Parquet 与第二份结果集后 `LEFT JOIN` / `EXCEPT` 精确通过；反向验证：附加后在用户 SQL 执行 `read_text('/etc/hosts')`、`COPY TO`、`ATTACH` 全部失败（撤掉锁后该测试必红）。
3. **AN3 Redis 安全与降级门**：单测校验前缀树聚合、TTL 5 档边界（`-1`、`-2`、`0`、边界秒数）与 BigKey 排序；集成/mock 验证 `MEMORY USAGE` 报 `NOPERM` 时不中断扫描且正确标注降级口径，触发取消后 1 轮 `SCAN` 内退出。
4. **AN4 MongoDB 漂移与防爆门**：含 12 层嵌套、动态键、显式 `null` vs 缺失键、混合类型与数组的 BSON 夹具精确验证存在率、类型占比、深度/路径截断计数；视图及无 `$collStats` 权限时只降级存储卡片。
5. **AN5 ES 权限分层门**：ES 8 与 OpenSearch 响应夹具验证 `date_histogram` / `terms` / `percentiles` 转表格与 Mapping 字段上限预警；反向验证 `_cat/indices` 返回 `403` 时读权限聚合仍完整成功。
6. **AN6 Neo4j 拓扑门**：Cypher 响应夹具验证 Label/关系占比、度分位数、孤立节点率与超级节点 Cypher 生成，且严格拒绝生成任何写 Cypher。

## 7. 当前版本不做（及可执行重估条件）

| 不做项 | 为什么当前不做 | 可执行重估条件 |
| --- | --- | --- |
| 拖拽式多图表 Dashboard / 定时巡检报告 | 偏离轻量原生数据库客户端定位，维护与状态同步成本成倍上升 | 明确规划定时任务/团队协作阶段时再单独立项 |
| Redis 全量 RDB 文件离线解析器 | RDB 二进制格式随 Redis 版本演进且需服务端磁盘访问，客户端通常只有网络连接；`SCAN` 抽样已覆盖 90% 线上排查 | 有高频「离线分析数 GB `.rdb` 备份文件」需求且实测抽样偏差无法满足时再评估独立解析模块 |
| 在线上关系库直接执行无保护的全表 `ORDER BY RAND()` 或全表 `PERCENTILE_CONT` | 大表上极易打满线上 CPU/IO，风险远大于收益 | 不重估；大表全量分析统一走「抽样 / 导出 Parquet 后交本地 DuckDB」 |
| 为 NoSQL 分析引入独立重型图表库（ECharts / D3 / Vega） | 破坏现有纯函数单测与深浅色 Tailwind Token 统一性 | 与 `rfcs/duckdb-analytics.md` 同一把尺子：手写 SVG 实测帧渲染耗时 `> 150ms` 时再评估 Canvas 层 |
| CLI / MCP 立即新增全部 NoSQL 分析子命令 | 等 GUI 真实使用一轮沉淀出最稳定的 JSON 字段契约，避免过早固化 CLI 输出格式 | `AN3–AN5` 合入稳定版并收集到外部 Agent 调用 Redis/Mongo/ES 诊断的真实反馈时开放 |

---

## 8. 高价值低膨胀增强（`AN7–AN10`，2026-11-21 增补）

在 `AN0–AN6` 合入 `main`（PR `#64`）并经过真实数据实测后，聚焦以下 4 项零重型依赖、复用既有 DuckDB 沙盒、`ResultChartDialog` 与 `ai_complete` 的高信噪比增强：

1. **`AN7` 画像体检增强**：
   - 在 `local_analytics.rs` 文本列单次扫描中顺带统计 `empty_string_count`（`""`）与 `padded_whitespace_count`（含首尾空格的非空字符串），零额外表扫描；
   - 在 `DataProfileDialog` 与 `LocalAnalyticsDialog` 列级画像上方新增可点击的「异常体检摘要条」（仅存在异常时出现，点击快速聚焦问题列）；
   - 将回表筛选算子从 `eq`/`is-null` 扩展到 `lt`/`gt`，让数值列 `< 0` / `> P95` 离群项与文本列空串支持一键回表筛选。
2. **`AN8` 画像体检 Markdown 报告导出**：
   - 纯函数 `formatProfileMarkdownReport` 将当前列画像（样本口径、异常摘要、各列关键分布指标）生成结构化 Markdown，支持一键复制，便于粘贴至工单、IM 或 PR。
3. **`AN9` 数值列相关性矩阵**：
   - 纯函数 `buildCorrelationMatrixSql` 针对内存 DuckDB 会话中最多 12 个数值列生成只读 `CORR(a, b)` 矩阵查询，直接复用 `ResultChartDialog`（新增 `initialChartType="heatmap"`）绘制相关系数热力图并支持 SVG/PNG 导出。
4. **`AN10` 基于脱敏统计摘要的 AI 体检解读**：
   - 严格留在 Cargo feature `ai` 之后（复用 `ai_available`、`ai_complete`、`ai_cancel` 与敏感环境路由守卫）；
   - **隐私硬门禁**：提示词仅发送聚合统计指标（列名、类型、空值率、唯一率、数值分位数、空串/空格计数、时间跨度），**主动剥离 `topValues` 样本值与所有原始行数据**，支持展开查看发包原文，且绝不自动执行任何 SQL。
