# RFC：个人版方言缺口收口与国产数据库务实适配方案（`DC0–DC7`）

- **状态**：已立项（2026-11-22，前置实验 `DC0-E1` 与 Phase 1 `DC1–DC4` 优先推进，Phase 2 `DC5–DC6` 待对应实库实验环境到位后执行）
- **目标版本**：`v1.3.1`（Phase 1 缺口收口）→ `v1.3.x / v1.4.0`（Phase 2 国产库与 Phase 3 运维补强）
- **定位**：在进入团队协作能力研发之前，集中补齐 DataOmni 单机个人版中高频、高价值且低膨胀的方言/编辑器/连接可用性缺口，并确立国产专有数据库（达梦 DM8、人大金仓 KingbaseES）的务实接入路径。

---

## 1. 架构师评估（当前版本限定）

### 1.1 为什么不「从零手写专有数据库客户端驱动」

对达梦 DM8、Oracle（TNS）、SQL Server（TDS）等闭源或超大型二进制协议数据库，从零手写网络协议驱动在当前版本**不做**：
- 单个闭源协议的握手、国密/TLS 协商、大对象（LOB/CLOB）、游标分页、类型编解码与错误码兼容通常需要数万行代码与长期持续逆向维护，极易沦为脆弱半成品，拖垮整个桌面端核心迭代。
- **真正高杠杆的做法**是按协议形态分层：
  1. **开放/半开放兼容协议的小型协议垫片（Protocol Shim）**：典型如既有的 `src-tauri/src/services/opengauss_relay.rs`（约 470 行），不改 `sqlx`，只拦截登录握手里的专有认证包；`DC1`（openGauss SM3）继续沿用这条成熟路径。
  2. **兼容 PostgreSQL / MySQL 线协议的国产库**：如 OceanBase（MySQL 模式）、TiDB、openGauss、KingbaseES（PG 模式），直接复用 `sqlx` 池与执行层，只在目录元数据、执行计划、DDL 差异处做最小方言修补。
  3. **专有协议国产库（达梦 DM8 及更长尾的 GBase / 神通等）**：先做 `DC0-E2` 实验，在「社区纯 Rust 驱动（如 `dameng` crate）放在独立 Cargo feature 后」与「可选 `odbc` Cargo feature（`odbc-api`，BYO 厂商 ODBC 驱动，零闭源库随包分发）」之间用实测数据二选一或组合落位，依托已完成的 `M1`（`compiled_drivers` + Cargo feature 隔离）保证默认包体积与跨平台构建不受牵连。

### 1.2 本轮要做什么（按收益与阻塞度分层）

| 阶段 | 任务 | 解决的真实问题 | 外部环境依赖 |
| --- | --- | --- | --- |
| Phase 0 | `DC0-E1` openGauss SM3 协议向量与 `sqlx` 报错核验 | 避免盲猜 `AUTH_REQ_SM3 = 13` 报文偏移、PBKDF2 散列与 `sqlx` 拒连文案 | 纯本地可先跑；有 openGauss SM3 容器可做真库复验 |
| Phase 0 | `DC0-E2` 达梦 DM8 接入路线实验（`dameng` crate vs `odbc` feature） | 实测编译兼容性、类型解码、参数绑定、跨平台分发与二进制增量 | 需 DM8 单机实例/镜像（`dm8_single.tar`）与 ODBC 驱动 |
| Phase 1 | `DC1` openGauss SM3 认证（原 `G3` 剩余） | 信创环境 `password_encryption_type = 3` 的 openGauss 用户此前无法直连 | 纯本地可 TDD；可补真库冒烟 |
| Phase 1 | `DC2` SQL 编辑器无表别名裸列补全（原 `R7`） | `SELECT <光标> FROM orders` / `WHERE <光标>` 不写 `o.` 补不出列 | 无 |
| Phase 1 | `DC3` Redis Sentinel 多哨兵高可用与 `READONLY` 切主重连（原 `G8` 剩余） | 单哨兵宕机无法连、主从切换后连接池持续报 `READONLY` | 无（本地 mock RESP 哨兵 + 主从可全覆盖） |
| Phase 1 | `DC4` OceanBase 表达式默认值消歧 | OceanBase `INFORMATION_SCHEMA` 不标 `DEFAULT_GENERATED`，改结构时 `DEFAULT (UUID())` 被误当成字符串字面量重述 | 无（基于 `SHOW CREATE TABLE` 语料；有 OB 容器可真库复验） |
| Phase 2 | `DC5` 达梦 DM8 完整接入（原 `M2`） | 补齐国产头部专有数据库：连接、目录、执行、分页、导出与冒烟 | 依赖 `DC0-E2` 拍板与 DM8 实例 |
| Phase 2 | `DC6` 人大金仓 KingbaseES 完整接入（原 `M2` / `B1b`） | 补齐国产 PG 系头部库：连接、目录、分页、导出与冒烟 | 依赖 KingbaseES 试用镜像/实例 |
| Phase 3 | `DC7` 关系库活跃会话与锁诊断面板（可选） | 日常排查慢查询、阻塞锁与一键终止会话（MySQL/PG/SQL Server/Oracle/ClickHouse） | 复用既有真库冒烟环境 |

### 1.3 当前版本明确不做与可执行重估条件

- **不从零重写专有二进制驱动（DM8 / TNS / TDS）**：重估条件——目标数据库公开稳定 wire protocol 规范且社区无可用 Rust/ODBC 方案。
- **不 fork `tiberius` 修 SQL Server `money` 超 `9×10¹¹` 精度**：网格浏览与导出已有保护；重估条件——上游 `tiberius` 支持精确 decimal 解码，或收到真实金融精度缺陷报告。
- **不做 Oracle Wallet TLS 与原生 `JSON`/`VECTOR` 二进制直解**：现已有 SSH 隧道与 `JSON_SERIALIZE` 绕法；重估条件——收到真实用户受阻反馈。
- **不把云账号/中心化团队服务直接混入开源个人版主仓**：与 `dataomni.dev/privacy/`「零遥测、无云账号」公开承诺保持一致；团队能力后续另立 RFC，优先评估 Git-native 无服务器协作目录（`.dataomni/`）或按 `rfcs/editions-and-branching.md` §5 独立后端拆分。

---

## 2. 前置实验设计（`DC0`）

### 2.1 `DC0-E1`：openGauss SM3 认证协议与 `sqlx` 拒连特征实验

待核实并钉死以下 4 项事实（任何一项不得凭猜测实现）：
1. **`sqlx-postgres` 收到 openGauss `AUTH_REQ_SM3 = 13` 时的精确报错文本**：用本地 mock `TcpListener` 对真实 `sqlx::postgres::PgConnection::connect` 发送 `R` 消息（码 `13`），记录确切错误串，确定 `opengauss_relay::is_opengauss_auth_rejection` 匹配规则（并反向验证普通密码错误 `28P01` 绝不误触发）。
2. **`AUTH_REQ_SM3 = 13` 报文体布局**：对照 openGauss 官方驱动源码（`openGauss-connector-go-pq` / `openGauss-connector-jdbc` / `openGauss-server`），确认消息体字段是 `[password_stored_method: u32][salt_hex: 64B][token_hex: 8B][iterations: u32]` 还是不带 `password_stored_method`（或两者兼有）。
3. **SM3 派生与签名算法**：确认 `PBKDF2` 的底层 PRF 是 `HMAC-SM3` 还是 `HMAC-SHA1`，以及 `ClientKey = HMAC-SM3(K, "Client Key")`、`StoredKey = SM3(ClientKey)`、`ClientSignature = HMAC-SM3(StoredKey, token)`、`ClientProof = hex(ClientSignature XOR ClientKey)` 的每一环字节输出，使用独立参考实现交叉核对测试向量。
4. **Rust 依赖审计**：确认 RustCrypto `sm3` crate 与现有 `hmac` / `pbkdf2` / `digest` 版本兼容、零 `unsafe` 风险且通过 `cargo audit`。

> **实测结论（2026-11-22 已完成）**：
> 1. **一手源码核对**（`opengauss-mirror/openGauss-server` `src/common/backend/libpq/sha2.cpp` `GsSm3Encrypt`、`fe-auth.cpp`、`fe-connect.cpp` 及 `openGauss-tools-chameleon/pg_chameleon/lib/sm3_auth.py`）：
>    - 认证请求码为独立码 **`AUTH_REQ_SM3 = 13`**（不是码 10），协议 `3.51` 下报文体与 SHA256 同构：`[password_stored_method: u32 = 3][salt_hex: 64B][token_hex: 8B][iterations: u32]`。
>    - **关键纠偏**：openGauss SM3 **并非** `PBKDF2-HMAC-SM3` 或 `HMAC-SM3`——`K = PBKDF2-HMAC-SHA1(password, salt, iterations, 32)`、`ClientKey = HMAC-SHA256(K, "Client Key")`、`ClientSignature = HMAC-SHA256(StoredKey, token)` 均沿用 SHA1/HMAC-SHA256，**仅 `StoredKey = SM3(ClientKey)` 这一步替换为 GB/T 32905-2016 SM3**。
> 2. **真实 `sqlx 0.8.6` 报错实测**：向 `sqlx::PgConnection::connect_with` 返回码 13 时，确切报错包含 `"unknown authentication method: 13"`；已由端到端 mock 测试 `sqlx_pool_open_completes_sm3_handshake_via_relay` 钉住。
> 3. **零新外部依赖拍板**：因只需对 32 字节 `ClientKey` 计算一次标准 SM3 摘要，在 `opengauss_relay.rs` 内实现约 60 行纯安全整数位运算 `sm3_digest`（无 `unsafe`、零新增 crates.io 依赖），并通过 GB/T 32905-2016 两条官方测试向量 + OpenSSL 3（Python `hashlib.new("sm3")`）独立交叉验证（同输入向量 `7fecc63e76dc024dc765342834035c72aca17d20a75c1ec26393fd2116c24f6e`）。

### 2.2 `DC0-E2`：达梦 DM8 接入路线实验（`dameng` crate vs 可选 `odbc` feature）

拿到 DM8 实验环境后，在隔离实验分支对比以下指标：
1. **协议与功能覆盖**：连接/TLS、参数绑定、元信息（`ALL_TABLES`/`ALL_TAB_COLUMNS`/`ALL_INDEXES`）、大字段（`CLOB`/`BLOB`）、中文 UTF-8/GBK 编码、事务与查询取消。
2. **分发与合规成本**：`dameng` 纯 Rust crate 是否有协议缺陷或维护停滞；`odbc-api` 方案是否能做到「默认静态编译/零闭源库打包，用户自行安装系统 unixODBC + 厂商驱动即可连 DM8/GBase/神通等」。
3. **决策标准**：若纯 Rust `dameng` 能稳定跑通全部核心查询/元数据/导入导出用例且许可证清晰，优先作为独立 Cargo feature `dameng` 落地；若存在协议硬伤，则以 `odbc` feature 承接 DM8 及长尾国产专有库。

> **实测结论（2026-11-22 已完成，真库环境：`cu` 上 `dataomni-dm8` / `DM Database Server 64 V8` `0x7000c`）**：
> 在隔离 scratch 工程对 crates.io 现有候选纯 Rust 驱动与官方 ODBC 驱动做同库实测：
> 1. **`tokio-dameng 0.1.0`（`rarnu/rust-dameng`，MIT，154 次下载）——淘汰**：
>    - 能建连并查 `V$VERSION`，但首次 `CREATE TABLE` 即报 `-6506: 试图在只读事务中修改数据`（登录包事务标志位缺陷），且引入 `native-tls`（Linux 侧拖入 `openssl-sys`），0.1.0 以来停更。
> 2. **`dm-pure 1.2.4`（`zhangtao103239/dm8_pure_rs`，MIT，248 次下载，0 `unsafe`，0 C 依赖）——存在 4 项实测协议硬伤，不满足数据安全门禁**：
>    - **跑通项**：macOS ARM64 / Linux / Windows 零驱动直连、`V$VERSION`/`ALL_USERS`/`ALL_TABLES`/`ALL_TAB_COLUMNS`/`V$SESSIONS`、500 行分批 `CMD_FETCH`、`DECIMAL` 精确字符串、中文 UTF-8/Emoji、`DATE`/`TIMESTAMP(6)`、行内 LOB、`LIMIT/OFFSET`、事务回滚、非 LOB 参数化查询（6/6 列）与错误密码拒连（`-2501`）。
>    - **实测硬伤**：
>      1. **非行内 `CLOB` 静默解码为空串 `""`**：实测 `LENGTH(C) = 12000` 读出 `0` 字符，连 `DBMS_METADATA.GET_DDL(...)` 也返回 `Str("")`（核心问题：`read_offrow_lob` 硬编码旧版 43 字节定位符且不匹配时静默 `Ok(Vec::new())`）——在带网格编辑与导出能力的客户端里属于高危静默丢数据缺陷；
>      2. **带 LOB 列的参数化 `SELECT` 静默截断后续列**：`SELECT ID, C, B, TAIL FROM ... WHERE ID = ?` 仅返回 2 列 `["ID", "C"]`，`B` 与 `TAIL` 静默丢失（`parse_cols_robust` 靠步进 2 字节启发式猜测列描述偏移）；
>      3. **`EXPLAIN SELECT ...` 阻塞至读超时**：报 `-6852: Resource temporarily unavailable (os error 35)`；
>      4. **不支持 TLS**（仅明文 TCP + 协议层 DH/DES/AES 加密），无法接入强制 TLS 的生产 DM8。
> 3. **可选 `odbc` Cargo feature（`odbc-api 30.1.0`，MIT，189 万次下载，2026-11 活跃维护，BYO 厂商官方驱动）——当前版本架构推荐**：
>    - 实测确认达梦容器 `/opt/dmdbms/bin/` 自带官方 `libdodbc.so`（与 Windows `dodbc.dll` 同属官方维护驱动），彻底避开逆向协议导致的大字段丢数据、截断列、`EXPLAIN` 挂死与 TLS/国密缺口；
>    - 一次落地 `odbc` feature（默认关或按需开，纳入 `M1` `compiled_drivers`，绝不随包分发任何闭源动态库），除 DM8 外还可统一承接 GBase 8s/8a、神通 Oscar、DB2、SAP HANA 等所有带官方 ODBC 驱动的专有库；
>    - **已知平台权衡（需诚实告知用户）**：Linux x64 与 Windows x64 有官方 DM8 ODBC 驱动可开箱对接；macOS ARM64 达梦官方暂未发布原生 `libdodbc.dylib`，因此 macOS ARM64 暂无法经 ODBC 直连 DM8（除非达梦发布 macOS ARM64 ODBC 驱动，或上游纯 Rust 驱动修复上述 4 项硬伤后重估）。

---

## 3. Phase 1 详细设计（`DC1–DC4`，零外部阻塞）

### 3.1 `DC1`：openGauss SM3 认证支持

- **范围**：`src-tauri/Cargo.toml`、`src-tauri/src/services/opengauss_relay.rs`、`src-tauri/src/services/sqlx_pool.rs`、`src-tauri/tests/database_smoke.rs`（新增可选 `DATAOMNI_OPENGAUSS_SM3_TEST_URL` 冒烟用例）。
- **核心设计**：
  - 引入 RustCrypto `sm3`（匹配现有 `digest` / `hmac` / `pbkdf2` 主版本）。
  - `opengauss_relay` 扩展支持 `AUTH_REQUEST_SM3 = 13`（及 `DC0-E1` 核实的 stored method 与响应计算），`is_opengauss_auth_rejection` 同时覆盖 `AUTH_REQUEST_SHA256`（10）与 `AUTH_REQUEST_SM3`（13）在 `sqlx` 中产生的拒连文案（保留旧函数名或平滑重构）。
  - 单元测试：
    - 参考向量精确比对测试（SM3 正常计算）；
    - 短包 / 非法 hex / 不支持 stored method 拒收测试；
    - 本地 mock openGauss 服务端（发 `AUTH_REQUEST_SM3 = 13`、校验客户端回包、回 `AuthenticationOk` + `ReadyForQuery`）端到端测试，验证 `sqlx::PgPool` 经 `opengauss_relay` 真正连上并执行 `SELECT 1`。

### 3.2 `DC2`：SQL 编辑器无表别名裸列补全

- **问题**：当前 `SqlEditor.tsx` 只向 `@codemirror/lang-sql` 传 `schema` 与 `defaultSchema`，未传 `defaultTable`。用户输入 `SELECT <光标> FROM orders` 或 `SELECT * FROM orders WHERE <光标>` 时，如果不写 `orders.` 或 `o.`，补全列表里没有 `orders` 的列。
- **核心设计（纯函数优先，零重型 SQL 解析器依赖）**：
  - 在 `src/utils/sqlCompletionSchema.ts` 新增纯函数 `inferDefaultTableAtCursor(doc: string, cursorPos: number, relations: readonly CompletionRelation[], dbType: DatabaseType, defaultSchema?: string)`（及按需提取当前光标所在单条语句，复用 `getSqlStatementRanges` 语义或轻量词法扫描跳过字符串/注释）：
    1. 定位光标所在的那一条 SQL 语句（按顶层 `;` 切分，跳过单/双引号、反引号、`--` 与 `/* */` 注释）。
    2. 在该语句中提取 `FROM <table_ref>`、`UPDATE <table_ref>`、`INSERT INTO <table_ref>`、`DELETE FROM <table_ref>` 的**主表**（支持可选 `schema.table` 与引号标识符，大小写按方言规则匹配已知 `relations`）。
    3. 当语句里能唯一匹配到已知单表（或多个 `JOIN` 时也支持为所有引用表的列提供低侵入补充——先确认 `@codemirror/lang-sql` `defaultTable` 与自定义 `CompletionSource` 哪种体验最好且不污染点号前缀补全）：
       - 优先方案：新增轻量 `CompletionSource` 或动态 `defaultTable`——若采用 `sql()` 的 `defaultTable`，需随光标重建 `Compartment`（代价高）；**更干净、零重配置的方式**是挂一个极简的 CodeMirror `autocompletion` 自定义补全源 `bareColumnCompletionSource(relations, dbType, defaultSchema)`：
         - 当光标紧跟 `.`（成员访问 `alias.col` / `schema.table`）或处于字符串/注释内时立即返回 `null`（完全交给 `@codemirror/lang-sql` 处理）；
         - 提取当前语句中 `FROM` / `JOIN` / `UPDATE` / `INTO` 引用的已知表集合；
         - 仅将命中表的列作为候选返回（保留表内列序 `boost`，并在 `detail` 中标明 `table · dataType` 或单表时的 `dataType`），绝不把全库所有表的列一股脑倒出来污染补全！
  - **测试门禁**：覆盖 `SELECT | FROM orders`、`WHERE |`、`JOIN` 多表裸列、带引号/`schema.table` 表名、多语句分号隔离（不串到前一条语句的表）、`o.|` 点号访问退让返回 `null`、字符串字面量/注释内退让返回 `null`。

### 3.3 `DC3`：Redis Sentinel 多哨兵高可用与 `READONLY` 切主重连

- **现状缺口**：
  1. `RedisTarget` 只有一个 `(host, port)` 哨兵地址，第一台哨兵宕机即连不上；
  2. `RedisPool::connection` 缓存 `ConnectionManager`，主从切换后旧主降级为只读从库，所有写操作持续返回 `READONLY You can't write against a read only replica.`，直到用户手动断开重连。
- **核心设计**：
  - **配置模型（完全向后兼容 `connections.json`）**：
    - `src-tauri/src/models/mod.rs` 与 `src/utils/redisConnection.ts` 新增可选 `options.sentinelNodes`（逗号/换行分隔的备用哨兵列表 `host:port`，未填时退回单哨兵 `(profile.host, profile.port)`；填了时将主 `(profile.host, profile.port)` 与去重后的备用节点合并为有序候选列表，支持 `[ipv6]:port` 与默认端口 `26379`）。
    - `ConnectionForm.tsx` 在 Sentinel 模式下增加可选「备用哨兵节点」输入框（支持 `{...PLAIN_TEXT_INPUT}`，留空即与旧配置完全一致）。
    - `fixtures/on-disk-0.5/` 无需破坏兼容，新增向后兼容单测。
  - **多哨兵故障转移（`services/redis.rs`）**：
    - `master_address` 按序遍历全部哨兵候选节点，对单节点设置合理短超时（如 3s，受总连接超时约束）；遇到网络不可达/超时或哨兵返回空时继续尝试下一台哨兵；保留「无口令哨兵自动回退不带凭证询问」逻辑；全部失败时返回清晰汇总错误。
    - 顺带核对 `sentinel_master` 模式下单机重连：若主从已切换，`open()` 重新问哨兵即可连到新主。
  - **`READONLY` 自动切主单次重试**：
    - 在 `RedisPool` 新增 `refresh_sentinel_connection(database)`（加锁清掉该库号旧连接并重新向哨兵查询主节点建连）。
    - 在 `services/redis.rs` 统一的命令执行入口检测服务端错误码 `READONLY`（仅当 `sentinel_master.is_some()`）：自动调用一次 `refresh_sentinel_connection`，若新主建连成功则**单次**重试该命令；严禁无限循环重试，且绝不对非 Sentinel 连接或非 `READONLY` 错误盲目重试。
  - **测试门禁（纯本地 mock RESP 服务端）**：
    - Mock 哨兵 1 拒连 / 超时 + 哨兵 2 正常返回主节点地址 → 成功建连；
    - Mock 旧主对写命令返回 `-READONLY You can't write against a read only replica.\r\n`，哨兵此时返回新主地址 → 自动刷新连接并单次重试写命令成功；
    - 反向门：新主仍返回 `READONLY` 时只重试 1 次即报错返回，不死循环；
    - `sentinelNodes` 解析（空格、换行、IPv6、默认端口、去重、非法端口）前后端纯函数单测。

### 3.4 `DC4`：OceanBase MySQL 模式表达式默认值消歧

- **现状根因**（见归档 `docs/archive/TODOs-2026-10-06.md` B1）：OceanBase 4.x 在 `INFORMATION_SCHEMA.COLUMNS` 中不为 `DEFAULT (UUID())` 等表达式默认值写入 `EXTRA = DEFAULT_GENERATED`，导致表达式与同名字符串字面量在 `INFORMATION_SCHEMA` 里无法区分，修改表结构或比对同步时会被错误重述为 `DEFAULT 'UUID()'`。
- **最小真实缝设计**：
  - 不新增「按服务端换目录 SQL」的重型抽象，也不依赖高权限内部表 `oceanbase.__all_column`。
  - 在前端纯函数 `src/utils/tableDdl.ts`（并视需要在 Rust `cli/diff.rs` 同构）新增 `extractMysqlExpressionDefaultColumns(createTableDdl: string): Set<string>` 与 `enrichOceanBaseExpressionDefaults(columns, ddl, serverVersion)`：
    - 仅当 `serverVersion` 含 `OceanBase` 且已取得 `SHOW CREATE TABLE` 原文时生效；
    - 词法级扫描建表语句体内的列定义行，跳过引号字符串（含 `''` 与 `\\` 转义）和 `COMMENT '...'`，精确识别列级 `DEFAULT (<expr>)`（括号嵌套平衡，排除 `DEFAULT '...'` 字符串字面量），返回需要补标 `DEFAULT_GENERATED` 的列名集合；
    - 对命中的列在 `column_extra` 补上 `DEFAULT_GENERATED`（幂等不重复追加），下游 `columnDefaultSql`、`TableStructureEditor` 与 `SchemaCompareDialog` 自动按既有逻辑正确处理，拒绝将其误重述为字符串。
  - **测试门禁**：真实 OceanBase `SHOW CREATE TABLE` 语料覆盖 `DEFAULT (uuid())`、`DEFAULT (concat('a', 'b'))`、`DEFAULT (1 + 2)`、字符串 `DEFAULT '(uuid())'`、`DEFAULT 'UUID()'`、`COMMENT 'DEFAULT (fake)'`、反引号含空格列名、非 OceanBase 服务端完全透传不修改。

---

## 4. Phase 2–3 概要规划（待外部环境 / 下一阶段）

- **`DC5`（达梦 DM8）**：已按 `DC0-E2` 推荐路线落地为可选 Cargo feature `odbc = ["dep:odbc-api"]`（默认关闭，纳入 `M1` `compiled_drivers` 动态收窄，零闭源驱动随包分发）。后端新增 `services/dameng.rs`（自动初始化临时 `DM_SVC_PATH` `CHAR_CODE=(PG_UTF8)` 根治中文 GBK 乱码、`LongVarchar` 走 `get_wide_text` 规避 DM8 off-row CLOB indicator panic、`Binary` 走 `get_binary`）、全套目录/元数据/补全/ER/会话目标/活跃会话（`V$SESSIONS` + `SP_CLOSE_SESSION`）查询、流式导出、事务批量写入、只读 CLI/MCP 及 `tests/dameng_smoke.rs`；前端接入 `DatabaseType.Dameng`、连接表单可选 ODBC 驱动路径与全方言纯函数。真库实测 `cu` DM8 12 条目录/元数据/会话查询与 60,000 字 off-row CLOB / 含 LOB 参数化查询 / `GET_DDL` / 事务回滚全通。
- **`DC6`（人大金仓 KingbaseES）**：已完成 V9（PG 模式）42/42 真库冒烟、9 个系统 schema 过滤与前端快捷入口预设（PR `#69`）。
- **`DC7`（活跃会话与锁诊断面板）**：已完成 MySQL / PostgreSQL / SQL Server / Oracle / ClickHouse / Dameng DM8 统一 10 列投影查询与按需刷新、二次确认终止面板（PR `#70` + `DC5` 扩展）。

---

## 5. 需外部配合的实验环境清单（不阻塞 Phase 1）

| 实验 / 任务 | 所需环境 | 说明 |
| --- | --- | --- |
| `DC1` 真库复验（可选） | openGauss 5.x/6.x 实例或 Docker 容器，开启 `password_encryption_type = 3` 且建好一个 SM3 口令测试账号 | Phase 1 本地可先用参考向量 + mock 服务端完成 TDD，有真库后跑一次 `DATAOMNI_OPENGAUSS_SM3_TEST_URL` 冒烟确认 |
| `DC0-E2` & `DC5`（必需） | 达梦 DM8 单机实例或官方开发者 Docker 镜像（`dm8_single.tar`，默认端口 5236）；如评估 ODBC 路线，附带对应架构 `libdodbc` 驱动 | 官网 `eco.dameng.com` 需个人账号下载，无法由 CI 直接公开拉取 |
| `DC6`（必需） | 人大金仓 KingbaseES V8/V9（PG 兼容模式，默认 54321）测试实例或带开发试用 license 的镜像 | 官网下载需实名/申请试用 license |
| `DC4` 真库复验（可选） | OceanBase CE 4.x 容器（`oceanbase/oceanbase-ce`，`MODE=mini`） | 用于现场核对带 `DEFAULT (uuid())` 列的 `SHOW CREATE TABLE` 与修改列注释预览 |
