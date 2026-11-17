# 从外部客户端导入连接配置（E1）

2026-11-17。状态：实现中，跟代码走（`src/utils/externalConnections.ts`、`src/components/ConnectionImportDialog.tsx`、`src-tauri/src/services/connection_service.rs`）。任务在 `TODOs.md`「v1.2 路线」的 E1。

## 1. 要回答的问题

从 Navicat 或 DBeaver 切换到 DataOmni 时，用户手里通常已经有十几到几十条开发、测试、预发或生产库连接（常带 SSH 跳板机与私钥路径）。逐条手填主机、端口、服务名与隧道参数既枯燥又容易抄错，而现有的「导入连接…」只认 DataOmni 自己导出的 `dataomni-connections` JSON。

## 2. 支持的格式与映射规则

统一从同一个「导入连接…」（欢迎页与 `⌘K` 命令面板均可唤起，文件筛选 `json` / `ncx` / `xml`）进入，经现有的 `read_text_file`（含 `app_config_dir` 保留路径拦截、8 MB 上限与 UTF-8 / UTF-16 BOM 解码）读入文本后，由前端纯函数 `parseConnectionImportFile` 识别三类文件：

| 来源 | 识别特征 | 支持的数据库与关键字段映射 |
| --- | --- | --- |
| **DataOmni JSON** | JSON 顶层 `"format": "dataomni-connections"`（`version <= 1`） | 原样保留全部字段与 `options`（`version > 1` 报 `DATAOMNI_IMPORT_NEWER_VERSION`）。 |
| **Navicat `.ncx`** | XML 根元素 `<Connections>` 下的 `<Connection>` 节点 | `MYSQL`、`MARIADB`（记 `options.server = "mariadb"`）、`PGSQL` / `POSTGRESQL`、`ORA` / `ORACLE`（`OraServiceNameType="SID"` 或 `OraConnMode="SID"` 记 `options.connectBy = "sid"`）、`MSSQL` / `SQLSERVER`、`SQLITE`（`DatabaseFileName`）、`MONGODB` / `MONGO`、`REDIS`；支持 `SSH="true"` 跳板机（主机、端口、用户名、私钥路径）与 `SSL="true"` 证书路径。 |
| **DBeaver `data-sources.json`** | JSON 顶层含对象型的 `"connections"`（且无 `format` 字段） | 按 `provider` / `driver` 识别 `postgresql`、`cockroachdb`（`options.server = "cockroachdb"`）、`opengauss`（走 PostgreSQL）、`mysql`、`mariadb`、`tidb`、`oceanbase`（走 MySQL）、`oracle`（`@dbeaver-sid-service@ = "SID"` 记 `connectBy = "sid"`）、`sqlserver` / `mssql`、`sqlite`、`duckdb`、`clickhouse`、`mongodb`、`redis`、`neo4j`、`elasticsearch` / `opensearch`；当 `host` / `port` / `database` 缺省时回退解析 `configuration.url`（`jdbc:*` 等）；`connectionType = "prod"` 自动映射为 `environment = "production"`（保留红色生产护栏），`"test"` 映射为 `"testing"`；解析 `handlers.ssh_tunnel`（`enabled = true`）。 |

遇到 DataOmni 当前不支持的引擎（如 `Snowflake`、`DB2`、`Sybase`），不静默丢弃，也不让整份文件报错作废——收集进 `unsupported` 列表，在预览对话框里明确列出连接名与源驱动类型。

## 3. 安全边界（不重估）

1. **绝不解密或导入第三方加密口令**：Navicat `.ncx` 里的十六进制加密 `Password` / `SSH_Password` 与 DBeaver 的 `credentials-config.json` 一律忽略。
2. **默认不保存空口令、默认关闭 Agent 访问**：外部导入的连接统一设为 `password: ""`、`save_password: false`、`credential_ref: undefined`、`agent_access: "off"`，SSH 隧道 `secret: ""`。导入后首次点击连接时，直接走现有的会话口令输入框（也可进入编辑页勾选保存到系统钥匙串）。
3. **后端二次清洗**：新增 `import_connection_profiles` 命令复用 `ConnectionService::import_profiles`，在入库落盘前再次强制清空 `password`、`credential_ref`、`ssh_tunnel.secret` / `secret_ref` 并将 `agent_access` 置为 `Off`。

## 4. 预览与去重

- 解析成功后弹出 `ConnectionImportDialog`，显示识别到的来源格式、可导入连接勾选列表（连接名、类型、目标地址、`SSH` / `PROD` 标记）、因本机已存在相同 `(name, db_type, host, port, database, username)` 而置灰跳过的条目，以及因引擎不支持而跳过的条目。
- 对话框顶部常驻提示：口令未包含在导入内，首次连接时会询问。

## 5. 验收门与反向验证

1. `src/utils/externalConnections.test.ts`：覆盖 Navicat `.ncx`（含 SSH 私钥、Oracle SID、不支持的 DB2）、DBeaver `data-sources.json`（含 JDBC URL 回退、`prod` 环境映射、SSH handler）、DataOmni JSON（含高版本拒绝）、损坏的 XML/JSON 报错，以及夹带明文/密文 `Password` 时输出 `password === ""` 且 `agent_access === "off"`。反向验证：去掉 `agent_access = 'off'` 或口令清空逻辑，测试变红。
2. `src-tauri/src/services/connection_service.rs`：`import_profiles` 对传入的 `password`、`credential_ref`、`agent_access: Read` 全部剥离并按六元组去重。
3. `src/components/ConnectionImportDialog.test.tsx` + 无头 Chrome 渲染截图核对中英双语布局。
