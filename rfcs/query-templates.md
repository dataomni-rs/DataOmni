# 本地参数化查询模板（E2）

2026-11-17。对应 `rfcs/roadmap-1.2.md` 第四阶段 `E2` 与 `TODOs.md`「4.4 连接和查询模板」。

## 1. 要回答的问题

日常排障、对账与运维里总有一批反复执行的 SQL，例如：

```sql
SELECT id, status, total_cents, created_at
FROM orders
WHERE customer_id = :customer_id
  AND created_at >= :since
ORDER BY created_at DESC
LIMIT 50;
```

此前这类语句只能散落在两处：

1. **查询历史（`queryHistoryStorage.ts`）**：记录的是「跑过的每一次执行」，会混入大量一次性试探语句，收藏后仍与执行流水混居；
2. **外部 `.sql` 文件**：每次需经系统文件框定位路径，无法按标题、SQL 正文或 `:param` 参数名在命令面板（`⌘K`）秒级检索。

`D2`（`rfcs/bind-parameters.md`）落地后，SQL 编辑器已原生支持 `:name` 驱动级参数绑定与填值弹窗。因此查询模板**无需发明任何自定义模板宏语法（如 `{{var}}` 或 `${var}`）**：一段含 `:name` 的 SQL 加上标题与可选库类型范围，本身就是完整的参数化查询模板。

## 2. 边界与「当前版本不做」

| 项目 | 决策 | 理由 / 重估条件 |
| --- | --- | --- |
| 参数语法 | 直接复用 `D2` 的 `:name` 词法与驱动真绑定 | 零新语法成本；插入后点执行即自动唤起 `QueryParametersDialog` |
| 存储位置 | 本机 `localStorage`（`dataomni.query-templates`，`version: 1`） | 与查询历史、工作区快照同级；纳入 `R1` 落盘兼容门 |
| 团队云同步 | **不做** | 需账号体系与远端服务，见 `TODOs.md` 4.4 |
| 非 SQL 控制台模板 | **当前版本不做** | Cypher / Mongo / Redis / ES 无 `:name` 绑定层；出现真实诉求再评估 |
| 参数默认值落盘 | **不做** | 同 `D2` §4.3：参数值常含客户号、邮箱等敏感数据，只留内存不落盘 |

## 3. 数据结构与 R1 落盘兼容契约

```ts
export interface QueryTemplate {
  id: string;
  title: string;
  sql: string;
  /** 限定适用的关系型数据库类型；缺省表示通用 SQL 模板 */
  dbType?: DatabaseType;
  updatedAt: string;
}
```

落盘快照样本固定在 `fixtures/on-disk-0.5/query-templates.json`：

1. **高版本拒写保护（`newerTemplatesOnDisk`）**：若读到 `snapshot.version > 1`，`loadQueryTemplates()` 返回 `[]` 且后续 `saveQueryTemplates()` 拒绝覆写，避免回退旧版时清空新版模板。
2. **逐条校验与未知方言降级**：数组中混入损坏条目时只跳过坏项；若 `dbType` 为当前版本不认识的字符串（如未来新增的库类型），降级为 `undefined`（通用模板）保留 SQL 正文，绝不丢弃用户手写语句。
3. **配额上限**：最多保留 `200` 条、序列化 JSON 不超过 `512_000` 字符（按 `updatedAt` 从旧到新裁剪），避免挤占同源 `localStorage` 中的工作区草稿与查询历史。

## 4. 交互入口

1. **SQL 编辑器工具栏（`SqlEditor.tsx`）**：提供紧凑的「SQL 模板」图标按钮，打开 `QueryTemplatesDialog`，支持将当前选区或全文一键保存为模板、按关键字与库类型筛选、通过 CodeMirror `view.dispatch` 插入（保留 `⌘Z` 撤销历史）或插入后直接执行（自动触发 `:name` 填值与风险确认闸门）。
2. **命令面板（`⌘K`，`App.tsx`）**：当前连接为关系型 SQL 库时，面板直接列出兼容模板并展示 `:param` 参数摘要，回车即以模板标题打开新查询标签；同时提供「SQL 查询模板…」全局管理动作。
