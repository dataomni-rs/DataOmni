#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
  En,
  Zh,
}

impl Locale {
  pub fn html_lang(self) -> &'static str {
    match self {
      Self::En => "en",
      Self::Zh => "zh-CN",
    }
  }

  pub fn canonical_path(self) -> &'static str {
    match self {
      Self::En => "/",
      Self::Zh => "/zh/",
    }
  }

  pub fn alternate_path(self) -> &'static str {
    match self {
      Self::En => "/zh/",
      Self::Zh => "/",
    }
  }

  pub fn alternate_label(self) -> &'static str {
    match self {
      Self::En => "中文",
      Self::Zh => "EN",
    }
  }

  pub fn og_locale(self) -> &'static str {
    match self {
      Self::En => "en_US",
      Self::Zh => "zh_CN",
    }
  }

  pub fn og_alternate_locale(self) -> &'static str {
    match self {
      Self::En => "zh_CN",
      Self::Zh => "en_US",
    }
  }

  pub fn og_image_path(self) -> &'static str {
    match self {
      Self::En => "/assets/og-en.png",
      Self::Zh => "/assets/og-zh.png",
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustIcon {
  License,
  Offline,
  Keychain,
  Oracle,
}

pub struct TrustFact {
  pub icon: TrustIcon,
  pub label: &'static str,
  pub detail: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerKind {
  Relational,
  Workspace,
}

impl ServerKind {
  pub fn as_attr(self) -> &'static str {
    match self {
      Self::Relational => "relational",
      Self::Workspace => "workspace",
    }
  }
}

pub struct EngineBadge {
  pub name: &'static str,
  pub tag: &'static str,
  pub kind: ServerKind,
}

pub struct ScreenshotTab {
  pub id: &'static str,
  pub tab_label: &'static str,
  pub badge: &'static str,
  pub webp_path: &'static str,
  pub image_path: &'static str,
  pub width: u32,
  pub height: u32,
  pub alt: &'static str,
  pub caption_title: &'static str,
  pub caption_body: &'static str,
}

pub struct SpecItem {
  pub term: &'static str,
  pub tag: &'static str,
  pub detail: &'static str,
}

pub struct WorkflowStory {
  pub title: &'static str,
  pub lead: &'static str,
  pub bullets: &'static [&'static str],
}

pub struct McpClientPreset {
  pub id: &'static str,
  pub label: &'static str,
  pub command: &'static str,
}

pub struct CompareRow {
  pub dimension: &'static str,
  pub dataomni: &'static str,
  pub jvm_clients: &'static str,
  pub commercial_native: &'static str,
}

pub struct EcosystemCard {
  pub kind: ServerKind,
  pub title: &'static str,
  pub count_badge: &'static str,
  pub summary: &'static str,
  pub engines: &'static [&'static str],
  pub capabilities: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportLevel {
  Supported,
  UsableWithGaps,
}

pub struct ServerRow {
  pub server: &'static str,
  pub tested_version: &'static str,
  pub connection_type: &'static str,
  pub kind: ServerKind,
  pub level: SupportLevel,
  pub smoke_counts: &'static str,
  pub notes: &'static str,
}

pub struct FaqItem {
  pub question: &'static str,
  pub answer: &'static str,
}

pub struct PageCopy {
  pub locale: Locale,
  pub meta_title: &'static str,
  pub meta_description: &'static str,
  pub og_image_alt: &'static str,
  pub skip_to_content: &'static str,
  pub nav_workflows: &'static str,
  pub nav_compatibility: &'static str,
  pub nav_compare: &'static str,
  pub nav_faq: &'static str,
  pub nav_install: &'static str,
  pub theme_light: &'static str,
  pub theme_dark: &'static str,
  pub hero_title: &'static str,
  pub hero_lead: &'static str,
  pub primary_download_default: &'static str,
  pub primary_cta_macos: &'static str,
  pub primary_cta_windows: &'static str,
  pub primary_cta_linux: &'static str,
  pub github_cta: &'static str,
  pub release_notes_cta: &'static str,
  pub all_packages_label: &'static str,
  pub trust_facts: &'static [TrustFact],
  pub workbench_aria_label: &'static str,
  pub tabs: &'static [ScreenshotTab],
  pub ecosystem_strip_label: &'static str,
  pub engine_badges: &'static [EngineBadge],
  pub workflows_heading: &'static str,
  pub workflows_intro: &'static str,
  pub wf_native: WorkflowStory,
  pub wf_native_panel_bar: &'static str,
  pub wf_native_specs: &'static [SpecItem],
  pub wf_guardrails: WorkflowStory,
  pub sql_preview_bar: &'static str,
  pub sql_preview_note: &'static str,
  pub wf_schema: WorkflowStory,
  pub schema_panel_bar: &'static str,
  pub schema_panel_note: &'static str,
  pub wf_agents: WorkflowStory,
  pub agents_terminal_bar: &'static str,
  pub agents_cli_comment: &'static str,
  pub agents_mcp_cmd_note: &'static str,
  pub agents_mcp_aria_label: &'static str,
  pub mcp_presets: &'static [McpClientPreset],
  pub copy_label: &'static str,
  pub copied_label: &'static str,
  pub compare_heading: &'static str,
  pub compare_intro: &'static str,
  pub compare_col_dimension: &'static str,
  pub compare_col_dataomni: &'static str,
  pub compare_col_jvm: &'static str,
  pub compare_col_commercial: &'static str,
  pub compare_rows: &'static [CompareRow],
  pub compare_footnote: &'static str,
  pub compatibility_heading: &'static str,
  pub compatibility_intro: &'static str,
  pub ecosystem_cards: &'static [EcosystemCard],
  pub ledger_drawer_summary: &'static str,
  pub ledger_drawer_hint: &'static str,
  pub filter_all: &'static str,
  pub filter_relational: &'static str,
  pub filter_workspaces: &'static str,
  pub col_server: &'static str,
  pub col_version: &'static str,
  pub col_driver: &'static str,
  pub col_status: &'static str,
  pub col_smoke: &'static str,
  pub col_notes: &'static str,
  pub status_supported: &'static str,
  pub status_gaps: &'static str,
  pub servers: &'static [ServerRow],
  pub faq_heading: &'static str,
  pub faq_intro: &'static str,
  pub faq_items: &'static [FaqItem],
  pub install_heading: &'static str,
  pub install_intro: &'static str,
  pub macos_title: &'static str,
  pub macos_meta: &'static str,
  pub macos_note: &'static str,
  pub windows_title: &'static str,
  pub windows_meta: &'static str,
  pub windows_note: &'static str,
  pub linux_title: &'static str,
  pub linux_meta: &'static str,
  pub linux_note: &'static str,
  pub footer_summary: &'static str,
  pub footer_releases: &'static str,
  pub footer_issues: &'static str,
  pub footer_license: &'static str,
}

const EN_TRUST_FACTS: &[TrustFact] = &[
  TrustFact {
    icon: TrustIcon::License,
    label: "MIT open source",
    detail: "Free for personal & commercial teams, unlimited tabs",
  },
  TrustFact {
    icon: TrustIcon::Offline,
    label: "Zero telemetry",
    detail: "Direct local connections, no account or cloud sync",
  },
  TrustFact {
    icon: TrustIcon::Keychain,
    label: "OS keychain only",
    detail: "Zero plaintext passwords or API keys on disk",
  },
  TrustFact {
    icon: TrustIcon::Oracle,
    label: "Oracle OCI bundled",
    detail: "Connect immediately without separate driver setup",
  },
];

const ZH_TRUST_FACTS: &[TrustFact] = &[
  TrustFact {
    icon: TrustIcon::License,
    label: "MIT 开源免费",
    detail: "个人与企业商业环境零费用，不限连接与标签数",
  },
  TrustFact {
    icon: TrustIcon::Offline,
    label: "零遥测离线直连",
    detail: "不设云端账号，查询语句与表数据绝不出本机",
  },
  TrustFact {
    icon: TrustIcon::Keychain,
    label: "系统钥匙串存储",
    detail: "口令、私钥密码与 API Key 从不落盘明文",
  },
  TrustFact {
    icon: TrustIcon::Oracle,
    label: "内置 Oracle 驱动",
    detail: "随包集成 Instant Client，装完直连免配环境",
  },
];

const ENGINE_BADGES: &[EngineBadge] = &[
  EngineBadge { name: "PostgreSQL", tag: "SQL", kind: ServerKind::Relational },
  EngineBadge { name: "MySQL", tag: "SQL", kind: ServerKind::Relational },
  EngineBadge { name: "Oracle", tag: "OCI", kind: ServerKind::Relational },
  EngineBadge { name: "SQL Server", tag: "TDS", kind: ServerKind::Relational },
  EngineBadge { name: "SQLite", tag: "File", kind: ServerKind::Relational },
  EngineBadge { name: "DuckDB", tag: "OLAP", kind: ServerKind::Relational },
  EngineBadge { name: "ClickHouse", tag: "HTTP", kind: ServerKind::Relational },
  EngineBadge { name: "MariaDB", tag: "SQL", kind: ServerKind::Relational },
  EngineBadge { name: "TiDB", tag: "SQL", kind: ServerKind::Relational },
  EngineBadge { name: "OceanBase", tag: "SQL", kind: ServerKind::Relational },
  EngineBadge { name: "CockroachDB", tag: "SQL", kind: ServerKind::Relational },
  EngineBadge { name: "openGauss", tag: "PG", kind: ServerKind::Relational },
  EngineBadge { name: "MongoDB", tag: "Doc", kind: ServerKind::Workspace },
  EngineBadge { name: "Redis", tag: "KV", kind: ServerKind::Workspace },
  EngineBadge { name: "Neo4j", tag: "Graph", kind: ServerKind::Workspace },
  EngineBadge { name: "Elasticsearch", tag: "Search", kind: ServerKind::Workspace },
  EngineBadge { name: "OpenSearch", tag: "Search", kind: ServerKind::Workspace },
];

const EN_TABS: &[ScreenshotTab] = &[
  ScreenshotTab {
    id: "overview",
    tab_label: "SQL workbench",
    badge: "DEV",
    webp_path: "/assets/shots/en/overview.webp",
    image_path: "/images/en/overview.png",
    width: 3188,
    height: 2572,
    alt: "DataOmni main SQL workbench showing object explorer, multi-tab SQL editor, and paginated query results",
    caption_title: "Multi-tab SQL workbench",
    caption_body: "Dialect-aware statement splitting, live schema completion, server-side cancellation, and environment badges on every tab.",
  },
  ScreenshotTab {
    id: "table-edit",
    tab_label: "Queued edits",
    badge: "TX",
    webp_path: "/assets/shots/en/table-edit.webp",
    image_path: "/images/en/table-edit.png",
    width: 3196,
    height: 2574,
    alt: "Table data grid showing queued cell edits ready to preview and commit in a transaction",
    caption_title: "Queued row edits with optimistic concurrency",
    caption_body: "Cell edits, inserts, and deletes queue locally, preview as exact SQL with original values in WHERE, and commit in one transaction.",
  },
  ScreenshotTab {
    id: "query",
    tab_label: "Plans & charts",
    badge: "SQL",
    webp_path: "/assets/shots/en/query.webp",
    image_path: "/images/en/query.png",
    width: 3190,
    height: 2580,
    alt: "SQL editor with query results and inline chart visualization",
    caption_title: "Execution plans, bind parameters, and instant charts",
    caption_body: "Bind :name parameters safely through the driver, inspect expandable plan trees, and plot bar or line charts from current results.",
  },
  ScreenshotTab {
    id: "er-diagram",
    tab_label: "ER & codegen",
    badge: "ER",
    webp_path: "/assets/shots/en/er-diagram.webp",
    image_path: "/images/en/er-diagram.png",
    width: 3198,
    height: 2578,
    alt: "Whole-database ER diagram with foreign-key links, table columns, and export controls",
    caption_title: "ER diagrams, schema diff, and deterministic ORM codegen",
    caption_body: "Diff two databases into a sync script, export SVG/PNG/PDF or Markdown data dictionaries, and generate Prisma, TypeORM, MikroORM, SeaORM, or sqlx models.",
  },
  ScreenshotTab {
    id: "mongodb",
    tab_label: "MongoDB",
    badge: "DOC",
    webp_path: "/assets/shots/en/mongodb.webp",
    image_path: "/images/en/mongodb.png",
    width: 3198,
    height: 2576,
    alt: "MongoDB workspace browsing documents with syntax-highlighted JSON and mongosh filter bar",
    caption_title: "MongoDB document workspace",
    caption_body: "Filter, sort, and edit documents in mongosh syntax, run aggregation pipelines, inspect indexes, and use the db.runCommand console.",
  },
  ScreenshotTab {
    id: "redis",
    tab_label: "Redis",
    badge: "KV",
    webp_path: "/assets/shots/en/redis.webp",
    image_path: "/images/en/redis.png",
    width: 3198,
    height: 2566,
    alt: "Redis workspace browsing keys via SCAN with decoded JSON value highlighting",
    caption_title: "Redis Standalone, Sentinel, and Cluster workspace",
    caption_body: "SCAN-based key browsing across shards, editors for all six data types, automatic JSON & Python pickle decoding, and a redis-cli console.",
  },
];

const ZH_TABS: &[ScreenshotTab] = &[
  ScreenshotTab {
    id: "overview",
    tab_label: "SQL 工作台",
    badge: "DEV",
    webp_path: "/assets/shots/zh/overview.webp",
    image_path: "/images/zh/overview.png",
    width: 3196,
    height: 2578,
    alt: "DataOmni 主界面展示对象树、多标签 SQL 编辑器与分页查询结果",
    caption_title: "多标签 SQL 工作台",
    caption_body: "按方言切分语句、基于实时目录补全表列与别名，超时与取消真正停掉服务端语句，每个标签醒目标注环境档位。",
  },
  ScreenshotTab {
    id: "table-edit",
    tab_label: "事务级改表",
    badge: "TX",
    webp_path: "/assets/shots/zh/table-edit.webp",
    image_path: "/images/zh/table-edit.png",
    width: 3202,
    height: 2574,
    alt: "表格数据页将多处单元格修改排入待提交队列并预览 SQL",
    caption_title: "变更入队预览，单事务核对影响行数",
    caption_body: "增删改先入待提交队列，WHERE 带上原值做乐观并发校验；预览将要发送的真实 SQL，单事务执行并逐条核对受影响行数。",
  },
  ScreenshotTab {
    id: "query",
    tab_label: "计划与图表",
    badge: "SQL",
    webp_path: "/assets/shots/zh/query.webp",
    image_path: "/images/zh/query.png",
    width: 3196,
    height: 2570,
    alt: "SQL 查询结果与图表可视化展示",
    caption_title: "绑定参数、执行计划树与结果图表",
    caption_body: "编辑器直接写 :name 走驱动真绑定；解析各方言执行计划并标出估算偏差超一个数量级的算子；一键绘制柱状图或折线图。",
  },
  ScreenshotTab {
    id: "er-diagram",
    tab_label: "ER 图与生成代码",
    badge: "ER",
    webp_path: "/assets/shots/zh/er-diagram.webp",
    image_path: "/images/zh/er-diagram.png",
    width: 3206,
    height: 2574,
    alt: "整库 ER 关系图，展示表字段、主外键连线与筛选工具栏",
    caption_title: "整库 ER 图、结构比对与确定性 ORM 代码生成",
    caption_body: "比对两个库生成同步脚本；导出 SVG/PNG/PDF、Markdown 数据字典，或确定性生成 Prisma、TypeORM、MikroORM、SeaORM、sqlx 代码。",
  },
  ScreenshotTab {
    id: "mongodb",
    tab_label: "MongoDB",
    badge: "DOC",
    webp_path: "/assets/shots/zh/mongodb.webp",
    image_path: "/images/zh/mongodb.png",
    width: 3194,
    height: 2578,
    alt: "MongoDB 专属工作区，按 mongosh 语法筛选与高亮浏览文档",
    caption_title: "MongoDB 文档工作区",
    caption_body: "支持 mongosh 写法筛选、排序与编辑文档，运行聚合管道、查看索引与校验规则，内置 db.runCommand 命令台，支持 SRV 与 X.509。",
  },
  ScreenshotTab {
    id: "redis",
    tab_label: "Redis",
    badge: "KV",
    webp_path: "/assets/shots/zh/redis.webp",
    image_path: "/images/zh/redis.png",
    width: 3196,
    height: 2572,
    alt: "Redis 专属工作区，基于 SCAN 浏览键并自动解码高亮 JSON 值",
    caption_title: "Redis 单机、Sentinel 与集群工作区",
    caption_body: "基于 SCAN 跨分片翻键，支持六种数据类型读写，自动解码并高亮 JSON 与 Python pickle 值，内置 redis-cli 风格命令台。",
  },
];

const EN_NATIVE_SPECS: &[SpecItem] = &[
  SpecItem {
    term: "Desktop shell",
    tag: "WebView",
    detail: "Tauri 2 + Rust + system WebView — no bundled JVM or Chromium bloat",
  },
  SpecItem {
    term: "Oracle & DuckDB",
    tag: "Bundled",
    detail: "Oracle Instant Client (Basic Light), DuckDB 2.0, and SQLite compiled right in",
  },
  SpecItem {
    term: "Auth & network",
    tag: "SSH · TLS",
    detail: "Built-in SSH jump-host tunnel, TLS/X.509 verification, and openGauss sha256 relay",
  },
  SpecItem {
    term: "OS Keychain",
    tag: "Keyring",
    detail: "macOS Keychain, Windows Credential Manager, and Linux Secret Service only",
  },
  SpecItem {
    term: "Actionable errors",
    tag: "Diagnostic",
    detail:
      "Separates DNS failure, port refusal, handshake drop, SSH host-key mismatch, and bad auth",
  },
];

const ZH_NATIVE_SPECS: &[SpecItem] = &[
  SpecItem {
    term: "原生桌面架构",
    tag: "WebView",
    detail: "Tauri 2 + Rust + 系统原生 WebView，告别数百 MB 的 JVM 或浏览器内核常驻开销",
  },
  SpecItem {
    term: "内置重型驱动",
    tag: "Bundled",
    detail: "安装包自带 Oracle Instant Client（Basic Light）、DuckDB 2.0 与 SQLite，免配环境变量",
  },
  SpecItem {
    term: "隧道与特殊认证",
    tag: "SSH · TLS",
    detail: "内置 SSH 跳板机隧道、TLS / X.509 双向校验，以及 openGauss 默认 sha256 口令本地转发口",
  },
  SpecItem {
    term: "凭据零明文",
    tag: "Keyring",
    detail: "仅写入 macOS 钥匙串、Windows 凭据管理器与 Linux Secret Service，配置文件零明文",
  },
  SpecItem {
    term: "诊断说人话",
    tag: "Layer ID",
    detail: "连不上时精准区分域名不存、端口拒绝、TLS 握手中断、SSH 指纹变更或账号口令错误",
  },
];

const MCP_PRESETS: &[McpClientPreset] = &[
  McpClientPreset {
    id: "claude",
    label: "Claude Code",
    command: "claude mcp add dataomni -- /Applications/DataOmni.app/Contents/MacOS/dataomni mcp",
  },
  McpClientPreset {
    id: "codex",
    label: "Codex CLI",
    command: "codex mcp add dataomni -- /Applications/DataOmni.app/Contents/MacOS/dataomni mcp",
  },
  McpClientPreset {
    id: "gemini",
    label: "Gemini CLI",
    command: "gemini mcp add dataomni /Applications/DataOmni.app/Contents/MacOS/dataomni mcp",
  },
  McpClientPreset {
    id: "json",
    label: "Cursor / Desktop (mcp.json)",
    command: "{\"mcpServers\":{\"dataomni\":{\"command\":\"/Applications/DataOmni.app/Contents/MacOS/dataomni\",\"args\":[\"mcp\"]}}}",
  },
];

const EN_COMPARE_ROWS: &[CompareRow] = &[
  CompareRow {
    dimension: "Runtime & memory",
    dataomni: "Native Tauri 2 + Rust using OS WebView; small footprint, fast cold start",
    jvm_clients: "Java / Eclipse RCP; higher idle RAM, very broad JDBC plugin catalog",
    commercial_native: "Native binaries with fast startup",
  },
  CompareRow {
    dimension: "License & limits",
    dataomni: "MIT open source — free for commercial use, no tab or connection limits",
    jvm_clients: "Apache 2.0 CE (NoSQL & enterprise features require paid PRO edition)",
    commercial_native:
      "Paid per-seat license or subscription (free tier caps open tabs/connections)",
  },
  CompareRow {
    dimension: "SQL + NoSQL in one app",
    dataomni: "7 SQL drivers + 5 compatible DBs + MongoDB, Redis, Neo4j & Elasticsearch workspaces",
    jvm_clients: "CE covers SQL; MongoDB, Redis & graph workspaces require PRO",
    commercial_native: "Varies by product edition or paid add-on tier",
  },
  CompareRow {
    dimension: "Oracle & openGauss setup",
    dataomni: "Oracle Instant Client bundled in installer; built-in openGauss sha256 relay",
    jvm_clients: "Downloads Maven JDBC jars on first connection",
    commercial_native: "Bundled or manual local OCI path configuration",
  },
  CompareRow {
    dimension: "Grid write safety",
    dataomni: "Queued edits, exact SQL preview, single-TX commit with original-value WHERE check",
    jvm_clients: "Manual transaction mode & script preview available",
    commercial_native: "Staged change preview; optimistic row checks vary by driver",
  },
  CompareRow {
    dimension: "Credential storage",
    dataomni: "OS Keychain / Credential Manager / Secret Service only — zero disk plaintext",
    jvm_clients: "Local encrypted file by default; OS keyring via extra integration",
    commercial_native: "OS keychain or vendor-encrypted local profile",
  },
  CompareRow {
    dimension: "AI Agent CLI & MCP",
    dataomni: "Built-in read-only cli & standard stdio mcp; dual read-only gates, PROD hidden",
    jvm_clients: "Requires external MCP server with separate DB credentials",
    commercial_native: "Requires external MCP server with separate DB credentials",
  },
];

const ZH_COMPARE_ROWS: &[CompareRow] = &[
  CompareRow {
    dimension: "运行架构与内存",
    dataomni: "Tauri 2 + Rust 原生二进制，复用系统 WebView，轻量秒开",
    jvm_clients: "Java / Eclipse RCP，常驻内存较高，但传统冷门 JDBC 插件极多",
    commercial_native: "原生编译构建，启动响应快",
  },
  CompareRow {
    dimension: "开源与商用限制",
    dataomni: "MIT 开源免费，个人与公司商业环境零费用，不限连接数与标签页",
    jvm_clients: "社区版 Apache 2.0（NoSQL 与高阶管理功能需商业版订阅）",
    commercial_native: "商业买断或按年订阅（免费试用通常限制同时打开的标签或连接数）",
  },
  CompareRow {
    dimension: "单包多模数据库",
    dataomni: "同一应用内建关系库 + MongoDB、Redis、Neo4j、Elasticsearch 专属工作区",
    jvm_clients: "社区版聚焦关系型 SQL，文档/键值/图工作区需商业版",
    commercial_native: "视具体产品线与授权档位而定",
  },
  CompareRow {
    dimension: "Oracle 与国产库免配",
    dataomni: "安装包内置 Oracle Instant Client；内置 openGauss sha256 认证转发口",
    jvm_clients: "首次连接需联网从 Maven 下载对应 JDBC 驱动包",
    commercial_native: "随包内置或需手动指定本机 OCI 客户端库目录",
  },
  CompareRow {
    dimension: "改表护栏与防覆盖",
    dataomni: "变更入队预览真实 SQL，单事务执行并在 WHERE 核对原值防并发覆盖",
    jvm_clients: "支持手动事务模式与保存前预览 SQL 脚本",
    commercial_native: "支持暂存区预览（Staging），乐观并发检查因驱动而异",
  },
  CompareRow {
    dimension: "密码存储方式",
    dataomni: "仅存系统钥匙串 / 凭据管理器 / Secret Service，磁盘零明文",
    jvm_clients: "默认存本地加密文件，接入系统钥匙串需额外插件配置",
    commercial_native: "存系统钥匙串或自有加密配置文件",
  },
  CompareRow {
    dimension: "本地 AI 工具接入",
    dataomni: "内置标准 stdio MCP 与只读 CLI，适配各类 AI 工具且物理隐藏 PROD",
    jvm_clients: "需额外部署第三方 MCP 服务并单独填写明文数据库账密",
    commercial_native: "无内置本地 MCP，需另配外部工具与账密",
  },
];

const EN_SQL_ENGINES: &[&str] = &[
  "PostgreSQL 16",
  "MySQL 8.4 & 5.7",
  "Oracle 23ai",
  "SQL Server 2022",
  "SQLite",
  "DuckDB 2.0",
  "ClickHouse",
  "MariaDB",
  "TiDB",
  "OceanBase",
  "CockroachDB",
  "openGauss",
];

const EN_WORKSPACE_ENGINES: &[&str] = &[
  "MongoDB 8.0",
  "Redis 7.4 (Standalone / Sentinel / Cluster)",
  "Neo4j 5.26 / 2026",
  "Elasticsearch 8 / 9",
  "OpenSearch 3",
];

const ZH_SQL_ENGINES: &[&str] = &[
  "PostgreSQL 16",
  "MySQL 8.4 / 5.7",
  "Oracle 23ai",
  "SQL Server 2022",
  "SQLite",
  "DuckDB 2.0",
  "ClickHouse",
  "MariaDB",
  "TiDB",
  "OceanBase",
  "CockroachDB",
  "openGauss",
];

const ZH_WORKSPACE_ENGINES: &[&str] = &[
  "MongoDB 8.0",
  "Redis 7.4（单机 / Sentinel / 集群）",
  "Neo4j 5.26 / 2026",
  "Elasticsearch 8 / 9",
  "OpenSearch 3",
];

const EN_ECOSYSTEM_CARDS: &[EcosystemCard] = &[
  EcosystemCard {
    kind: ServerKind::Relational,
    title: "Relational & Analytical SQL",
    count_badge: "13 tested servers",
    summary: "One full-featured SQL workbench across OLTP databases, embedded analytics, and distributed SQL compatible servers.",
    engines: EN_SQL_ENGINES,
    capabilities: &[
      "Dialect-aware statement splitting (including SSMS GO batches and stored procedures) plus live catalog completion",
      "Queued grid edits & table designer with exact DDL/DML preview and optimistic concurrency checks",
      "Visual execution plan trees flagging >10x row-estimation skews, bind parameters (:name), and streaming CSV/SQL/Markdown export",
      "Cross-connection schema diff sync scripts, whole-DB ER diagrams, and deterministic ORM codegen",
    ],
  },
  EcosystemCard {
    kind: ServerKind::Workspace,
    title: "Document, KV, Graph & Search Workspaces",
    count_badge: "5 dedicated workspaces",
    summary: "Purpose-built native workspaces instead of forcing non-relational engines into fake SQL tables.",
    engines: EN_WORKSPACE_ENGINES,
    capabilities: &[
      "MongoDB: mongosh filter/sort/projection bar, document edits, aggregation pipeline runner, index inspector, and db.runCommand console",
      "Redis: non-blocking SCAN across cluster shards, editors for all 6 types, automatic JSON & Python pickle decoding, and redis-cli console",
      "Neo4j: Bolt 5 Cypher workbench, interactive node/relationship graph canvas, visual property editing, and EXPLAIN / PROFILE plans",
      "Elasticsearch & OpenSearch: Kibana Dev Tools request console, ES|QL & SQL tabular views, mapping viewer, and _seq_no optimistic-lock edits",
    ],
  },
];

const ZH_ECOSYSTEM_CARDS: &[EcosystemCard] = &[
  EcosystemCard {
    kind: ServerKind::Relational,
    title: "关系型与分析型 SQL 工作台",
    count_badge: "13 种实测服务端",
    summary: "一套完整的 SQL 工作台，覆盖主流事务库、本地嵌入式分析库，以及国内外分布式协议兼容数据库。",
    engines: ZH_SQL_ENGINES,
    capabilities: &[
      "按方言精准切分语句（含 SQL Server GO 分批与存储过程体），基于实时目录补全表、列与表别名",
      "表格数据增删改与可视化改表结构均先生成精确 SQL 预览，带原值乐观并发检查与单事务行数核对",
      "可视化执行计划树（自动标出估算偏差超 10 倍的算子）、驱动级 :name 参数绑定、流式导入导出与在线备份",
      "双库结构比对生成同步脚本、整库 ER 关系图导出，以及不依赖大模型的确定性 ORM 代码生成",
    ],
  },
  EcosystemCard {
    kind: ServerKind::Workspace,
    title: "文档、键值、图与搜索专属工作区",
    count_badge: "5 类原生工作区",
    summary: "不为非关系型数据库套用勉强的 SQL 外壳，而是按各自原生的查询心智打造专属工作区。",
    engines: ZH_WORKSPACE_ENGINES,
    capabilities: &[
      "MongoDB：支持 mongosh 语法筛选、排序与修改文档，运行聚合管道、查看索引与校验规则，内置 db.runCommand 命令台",
      "Redis：基于 SCAN 安全跨分片翻键，支持 6 种数据类型可视化编辑，自动解码高亮 JSON 与 Python pickle，内置命令行",
      "Neo4j：Bolt 5 协议直连，Cypher 编辑器 + 交互式节点关系图画布，界面直接改属性与连关系，支持 PROFILE 计划",
      "Elasticsearch / OpenSearch：Kibana Dev Tools 风格多请求控制台，hits / ES|QL / 聚合自动转表格，单文档修改带乐观锁保护",
    ],
  },
];

const EN_SERVERS: &[ServerRow] = &[
  ServerRow {
    server: "PostgreSQL",
    tested_version: "16",
    connection_type: "PostgreSQL",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "39 / 0 / 0",
    notes: "Full SQL workbench, actual execution plans (EXPLAIN ANALYZE), bind parameters, schema diff, pg_dump backup.",
  },
  ServerRow {
    server: "MySQL",
    tested_version: "8.4",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "42 / 0 / 0",
    notes: "Multi-result CALL support, JSON editor, TLS verification, schema diff, mysqldump backup.",
  },
  ServerRow {
    server: "MySQL 5.7",
    tested_version: "5.7.44",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "27+3 / 0 / 0*",
    notes: "TLS must be set to Disable (no ECDHE ciphers in 5.7); column renames restate definitions via CHANGE COLUMN.",
  },
  ServerRow {
    server: "SQLite",
    tested_version: "Bundled",
    connection_type: "SQLite",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "18 / 0 / 0",
    notes: "Built in, Windows >260-char paths supported, VACUUM INTO live backup, schema diff and ORM codegen.",
  },
  ServerRow {
    server: "MariaDB",
    tested_version: "11.4",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "43 / 0 / 0",
    notes: "Dedicated preset in connection form; JSON columns are LONGTEXT aliases and edit as text.",
  },
  ServerRow {
    server: "TiDB",
    tested_version: "8.5",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "40 / 0 / 2*",
    notes: "Parses EXPLAIN FORMAT='tidb_json'; warns when DROP is non-transactional; splits combined column + table renames.",
  },
  ServerRow {
    server: "SQL Server",
    tested_version: "2022",
    connection_type: "SQL Server",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "33 / 0 / 0",
    notes: "SSMS-compatible GO batching, multiple result sets, estimated & actual XML execution plans, schema diff & codegen.",
  },
  ServerRow {
    server: "Oracle",
    tested_version: "23ai Free (23.26)",
    connection_type: "Oracle (bundled OCI)",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "31 / 1 / 0",
    notes: "Instant Client bundled; connect by Service Name or SID; server-side cancel; use SSH tunnel when encryption is needed.",
  },
  ServerRow {
    server: "DuckDB",
    tested_version: "2.0.0 (built in)",
    connection_type: "DuckDB",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "19 / 0 / 0",
    notes: "Streams Parquet/JSON without loading whole files into RAM; Windows long paths supported; EXPORT DATABASE backup.",
  },
  ServerRow {
    server: "OceanBase",
    tested_version: "4.4.2 CE (MySQL)",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "39 / 0 / 3*",
    notes: "Parses OceanBase CHILD_n JSON plans; inspect SQL preview before altering columns that use expression defaults.",
  },
  ServerRow {
    server: "CockroachDB",
    tested_version: "25.2",
    connection_type: "PostgreSQL",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "39 / 0 / 0",
    notes: "Reads triggers via SHOW CREATE TRIGGER; parses EXPLAIN (VERBOSE) & EXPLAIN ANALYZE; runs schema changes per statement.",
  },
  ServerRow {
    server: "openGauss",
    tested_version: "5.0.3 (PG mode)",
    connection_type: "PostgreSQL",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "40 / 0 / 0",
    notes: "Local protocol relay authenticates default sha256-password users seamlessly over configured TLS.",
  },
  ServerRow {
    server: "ClickHouse",
    tested_version: "25.8",
    connection_type: "ClickHouse (HTTP)",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "17 / 0 / 0",
    notes: "HTTP/HTTPS interface; single-row MergeTree edits pin _part block + _part_offset because ClickHouse has no row locks.",
  },
  ServerRow {
    server: "MongoDB",
    tested_version: "8.0",
    connection_type: "MongoDB",
    kind: ServerKind::Workspace,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "34 / 0 / 0",
    notes: "Dedicated workspace with mongosh filter/edit syntax, aggregations, db.runCommand console, SRV (Atlas), TLS & X.509.",
  },
  ServerRow {
    server: "Redis",
    tested_version: "7.4",
    connection_type: "Redis",
    kind: ServerKind::Workspace,
    level: SupportLevel::Supported,
    smoke_counts: "14 / 0 / 0",
    notes: "Standalone, Sentinel, and Cluster; SCAN browsing, 6 data type editors, JSON & pickle decoding, redis-cli console.",
  },
  ServerRow {
    server: "Neo4j",
    tested_version: "5.26 LTS / 2026.09",
    connection_type: "Neo4j (Bolt 5)",
    kind: ServerKind::Workspace,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "10 / 0 / 0",
    notes: "Cypher editor, interactive graph view, visual node & relationship editing, EXPLAIN and PROFILE plans.",
  },
  ServerRow {
    server: "Elasticsearch",
    tested_version: "8.19 / 9.5",
    connection_type: "HTTP / API Key",
    kind: ServerKind::Workspace,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "7 / 0 / 0",
    notes: "Kibana Dev Tools console, hits/ES|QL/SQL/aggregation tables, mapping viewer, optimistic-concurrency document edits.",
  },
  ServerRow {
    server: "OpenSearch",
    tested_version: "3.8",
    connection_type: "Elasticsearch",
    kind: ServerKind::Workspace,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "6 / 1 / 0",
    notes: "Shares the Elasticsearch workspace over basic auth + TLS (no API key auth; read-only accounts cannot list tree).",
  },
];

const ZH_SERVERS: &[ServerRow] = &[
  ServerRow {
    server: "PostgreSQL",
    tested_version: "16",
    connection_type: "PostgreSQL",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "39 / 0 / 0",
    notes: "完整 SQL 工作台、真实执行计划（EXPLAIN ANALYZE）、绑定参数、结构比对、pg_dump 备份。",
  },
  ServerRow {
    server: "MySQL",
    tested_version: "8.4",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "42 / 0 / 0",
    notes: "支持存储过程多结果集、JSON 专用编辑器、TLS 证书校验、结构比对、mysqldump 备份。",
  },
  ServerRow {
    server: "MySQL 5.7",
    tested_version: "5.7.44",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "27+3 / 0 / 0*",
    notes: "TLS 需选「禁用」（5.7 无 ECDHE 套件）；改列名自动走 CHANGE COLUMN 重述列定义。",
  },
  ServerRow {
    server: "SQLite",
    tested_version: "随应用内置",
    connection_type: "SQLite",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "18 / 0 / 0",
    notes: "开箱即用，支持 Windows 超 260 字符路径、VACUUM INTO 在线备份、结构比对与 ORM 代码生成。",
  },
  ServerRow {
    server: "MariaDB",
    tested_version: "11.4",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "43 / 0 / 0",
    notes: "新建连接有独立入口；JSON 列本质为 LONGTEXT 别名，按文本显示与编辑。",
  },
  ServerRow {
    server: "TiDB",
    tested_version: "8.5",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "40 / 0 / 2*",
    notes: "解析 EXPLAIN FORMAT='tidb_json'；DROP 单独提示不受事务保护；同改列与表名自动拆成两条语句。",
  },
  ServerRow {
    server: "SQL Server",
    tested_version: "2022",
    connection_type: "SQL Server",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "33 / 0 / 0",
    notes: "支持 GO 分批、多结果集逐个展示、估算与真实执行计划（STATISTICS XML）、结构比对与代码生成。",
  },
  ServerRow {
    server: "Oracle",
    tested_version: "23ai Free (23.26)",
    connection_type: "Oracle（内置 OCI）",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "31 / 1 / 0",
    notes: "内置 Instant Client 免安装；支持按服务名或 SID 连接、服务端取消语句；需加密时走 SSH 隧道。",
  },
  ServerRow {
    server: "DuckDB",
    tested_version: "2.0.0（内置）",
    connection_type: "DuckDB",
    kind: ServerKind::Relational,
    level: SupportLevel::Supported,
    smoke_counts: "19 / 0 / 0",
    notes: "流式直查大 Parquet / JSON 文件不撑内存；支持 Windows 超长路径与 EXPORT DATABASE 备份。",
  },
  ServerRow {
    server: "OceanBase",
    tested_version: "4.4.2 CE（MySQL）",
    connection_type: "MySQL",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "39 / 0 / 3*",
    notes: "支持用户@租户连接与 CHILD_n 执行计划树；含表达式默认值的列在改结构前请先核对预览语句。",
  },
  ServerRow {
    server: "CockroachDB",
    tested_version: "25.2",
    connection_type: "PostgreSQL",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "39 / 0 / 0",
    notes: "触发器走 SHOW CREATE TRIGGER 读取；支持 EXPLAIN (VERBOSE) 与 ANALYZE；改结构逐条提交。",
  },
  ServerRow {
    server: "openGauss",
    tested_version: "5.0.3（PG 模式）",
    connection_type: "PostgreSQL",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "40 / 0 / 0",
    notes: "内置本地协议转发口，默认只存 sha256 口令的用户可直接按原 TLS 配置登录。",
  },
  ServerRow {
    server: "ClickHouse",
    tested_version: "25.8",
    connection_type: "ClickHouse（HTTP）",
    kind: ServerKind::Relational,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "17 / 0 / 0",
    notes: "走 HTTP/HTTPS 接口；MergeTree 单行修改额外锁定 _part 块与 _part_offset，防止无行锁下误改新写入的同名行。",
  },
  ServerRow {
    server: "MongoDB",
    tested_version: "8.0",
    connection_type: "MongoDB",
    kind: ServerKind::Workspace,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "34 / 0 / 0",
    notes: "专属文档工作区：mongosh 语法筛选与编辑、聚合管道、db.runCommand 命令台，支持 SRV、TLS 与 X.509。",
  },
  ServerRow {
    server: "Redis",
    tested_version: "7.4",
    connection_type: "Redis",
    kind: ServerKind::Workspace,
    level: SupportLevel::Supported,
    smoke_counts: "14 / 0 / 0",
    notes: "支持单机、Sentinel 与集群；跨主节点 SCAN 翻键、六种类型编辑、JSON 与 pickle 解码高亮、命令行。",
  },
  ServerRow {
    server: "Neo4j",
    tested_version: "5.26 LTS / 2026.09",
    connection_type: "Neo4j（Bolt 5）",
    kind: ServerKind::Workspace,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "10 / 0 / 0",
    notes: "Cypher 编辑器、结果图视图、界面直接编辑节点与建关系、EXPLAIN 与 PROFILE 执行计划。",
  },
  ServerRow {
    server: "Elasticsearch",
    tested_version: "8.19 / 9.5",
    connection_type: "HTTP / API Key",
    kind: ServerKind::Workspace,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "7 / 0 / 0",
    notes: "Kibana Dev Tools 控制台、hits / ES|QL / SQL / 聚合结果表格、映射查看与带乐观锁的单文档增删改。",
  },
  ServerRow {
    server: "OpenSearch",
    tested_version: "3.8",
    connection_type: "Elasticsearch",
    kind: ServerKind::Workspace,
    level: SupportLevel::UsableWithGaps,
    smoke_counts: "6 / 1 / 0",
    notes: "共用 Elasticsearch 工作区（支持账号口令与 TLS，无 API Key，只读账号列不出对象树）。",
  },
];

const EN_FAQ: &[FaqItem] = &[
  FaqItem {
    question: "Is DataOmni really free for commercial work?",
    answer: "Yes. DataOmni is released under the permissive MIT License. Individuals, startups, and enterprise teams can use it without seat fees, mandatory accounts, or artificial caps on open tabs and saved connections.",
  },
  FaqItem {
    question: "Does DataOmni collect telemetry or upload my queries, schemas, or passwords?",
    answer: "No. Connections go straight from your machine (or your configured SSH jump host) to your database. Passwords, SSH key passphrases, and AI API keys live exclusively in your OS keychain (macOS Keychain, Windows Credential Manager, or Linux Secret Service) — never in plaintext JSON. The only background network call is a once-per-day GitHub Releases version check, which you can disable in Settings → About & Diagnostics. AI features are off by default and can even be compiled out via Cargo's --no-default-features for air-gapped intranets.",
  },
  FaqItem {
    question: "Why does Oracle work out of the box without installing Instant Client manually?",
    answer: "Every official DataOmni installer bundles Oracle Instant Client (Basic Light) under Oracle's redistributable license (included in the package). On macOS and Windows you just enter your host and Service Name or SID; on Linux, .deb and .rpm packages declare the standard system libaio dependency.",
  },
  FaqItem {
    question: "Are installers signed, and why might Windows SmartScreen warn on first launch?",
    answer: "All installers are built reproducibly on GitHub Actions from public source. macOS releases (v1.2.1+) are signed with an Apple Developer ID and notarized by Apple, so both Homebrew Cask (brew install --cask dataomni-rs/tap/dataomni) and the .dmg open directly without Gatekeeper prompts. Windows builds do not yet carry a paid EV code-signing certificate — click \"More info\" → \"Run anyway\" once on first launch.",
  },
  FaqItem {
    question: "Which AI tools work with DataOmni MCP & CLI, and how is PROD protected?",
    answer: "DataOmni implements standard stdio MCP (JSON-RPC 2.0) alongside structured CLI JSON output, so it works out of the box with Claude Code, Codex, Cursor, Windsurf, GitHub Copilot, Gemini CLI, Zed, and Claude Desktop. Agent access is off by default on every connection; enabling read-only access binds an HMAC capability to your keychain secret so editing connections.json cannot bypass it. Production (PROD) connections are unconditionally hidden, and every query must pass both a dialect-specific read-only lexer gate and a database read-only transaction ending in ROLLBACK.",
  },
  FaqItem {
    question: "Will upgrading (or rolling back) wipe my saved connections, open tabs, or query history?",
    answer: "No. DataOmni enforces an automated on-disk compatibility gate (fixtures/on-disk-0.5): unknown future fields or driver types in connections.json are preserved verbatim on save, and older builds refuse to overwrite workspace or history snapshots written by a newer schema version.",
  },
];

const ZH_FAQ: &[FaqItem] = &[
  FaqItem {
    question: "DataOmni 完全免费吗？在公司商业项目里用有没有授权风险？",
    answer: "完全免费。DataOmni 采用宽松的 MIT 开源许可证发布，个人开发者与企业商业环境均可自由使用，不设账号登录，也没有「免费版最多开几个标签页或存几条连接」的人为限制。",
  },
  FaqItem {
    question: "数据库口令、查询语句或表里的数据会上传到外部服务器吗？",
    answer: "绝不会。所有连接均从你的本机（或你配置的 SSH 跳板机）直连目标数据库，没有任何遥测埋点或云同步服务。数据库口令、SSH 私钥密码与 AI API Key 只存入操作系统钥匙串（macOS 钥匙串、Windows 凭据管理器、Linux Secret Service），磁盘上的 connections.json 从不落盘明文。唯一可选的联网请求是每天最多一次查询 GitHub Releases 检查新版本，可在「设置 → 关于与诊断」中随时关闭；内网或信创环境还可通过 --no-default-features 编译完全剔除 AI 模块的构建。",
  },
  FaqItem {
    question: "为什么连 Oracle 不用像别的工具那样自己下 Instant Client 配环境变量？",
    answer: "DataOmni 在全平台安装包内直接随包集成了允许分发的 Oracle Instant Client（Basic Light，随包附带官方许可原文）。macOS 与 Windows 装完填上服务名或 SID 即可直连；Linux 下 .deb / .rpm 包已声明系统 libaio 依赖。",
  },
  FaqItem {
    question: "安装包有代码签名吗？Windows 首次弹出 SmartScreen 怎么处理？",
    answer: "全平台安装包均由 GitHub Actions 从公开源码自动构建。macOS 安装包（v1.2.1 起）已由 Apple Developer ID 签名并通过 Apple 官方公证，无论是通过 Homebrew（brew install --cask dataomni-rs/tap/dataomni）还是下载 .dmg 拖入「应用程序」均可直接打开。Windows 暂未购买代码签名证书，首次启动在弹窗点「更多信息」→「仍要运行」即可，之后打开不再提示。",
  },
  FaqItem {
    question: "除了 Claude Code，还能适配 Codex、Cursor 等其他 AI 工具吗？会不会误触生产库？",
    answer: "全部原生适配。dataomni mcp 实现的是标准 stdio MCP 协议，dataomni cli 输出标准结构化 JSON，可直接接入 Claude Code、Codex、Cursor、Windsurf、GitHub Copilot、Gemini CLI、Zed 与 Claude Desktop 等任意支持 MCP 或命令行的工具。每条连接的 Agent 访问默认关闭；开启只读时由钥匙串口令签发 HMAC 凭证，篡改本地 JSON 无法越权；生产环境（PROD）连接在架构层彻底隐身，且所有查询均经过「方言词法白名单 + 数据库只读事务 ROLLBACK」双保险。",
  },
  FaqItem {
    question: "以后升级新版本（或者临时退回旧版本）会丢连接配置、工作区标签和历史记录吗？",
    answer: "不会。仓库有专门的落盘兼容门禁（fixtures/on-disk-0.5）：遇到不认识的新增连接类型或字段会原样保留写回；若旧版本读到新版本写出的工作区快照或查询历史，会自动跳过覆盖，做到升级不丢东西、降级不毁配置。",
  },
];

pub static EN_COPY: PageCopy = PageCopy {
  locale: Locale::En,
  meta_title: "DataOmni — Native Rust GUI for SQL, MongoDB, Redis & Oracle",
  meta_description: "MIT open-source database client (Tauri 2 + Rust) for PostgreSQL, MySQL, Oracle, MongoDB, Redis, Neo4j & ES. Previewed SQL, OS keychain, and read-only MCP.",
  og_image_alt: "DataOmni native desktop database client for SQL, MongoDB, Redis, Neo4j, and Elasticsearch",
  skip_to_content: "Skip to main content",
  nav_workflows: "Workflows",
  nav_compatibility: "Databases",
  nav_compare: "Compare",
  nav_faq: "FAQ",
  nav_install: "Download",
  theme_light: "Light",
  theme_dark: "Dark",
  hero_title: "One native client for your SQL, document, KV, graph, and search databases.",
  hero_lead: "Built with Tauri 2 and Rust — no 1.5 GB JVM, no tab caps, and zero plaintext passwords on disk. Preview every grid edit as exact SQL before commit, connect to Oracle out of the box, and give Claude Code, Codex, Cursor, or any MCP client strictly read-only access while keeping PROD invisible.",
  primary_download_default: "Download DataOmni",
  primary_cta_macos: "Download for macOS (.dmg)",
  primary_cta_windows: "Download for Windows (.exe)",
  primary_cta_linux: "Download for Linux (.deb)",
  github_cta: "View on GitHub",
  release_notes_cta: "Release notes",
  all_packages_label: "All installers:",
  trust_facts: EN_TRUST_FACTS,
  workbench_aria_label: "DataOmni live workspace views",
  tabs: EN_TABS,
  ecosystem_strip_label: "17 engines verified against real database instances:",
  engine_badges: ENGINE_BADGES,
  workflows_heading: "Built around how engineers actually debug, edit, and ship data",
  workflows_intro: "Every workflow in DataOmni answers a concrete failure mode of traditional database GUIs — heavy runtimes, silent cell overwrites, opaque errors, and unsafe credential sharing.",
  wf_native: WorkflowStory {
    title: "Native Rust speed, with Oracle and enterprise quirks solved out of the box",
    lead: "Stop juggling separate tools for Postgres, Oracle, MongoDB, and Redis — or spending half an afternoon configuring Instant Client paths and SSH tunnels.",
    bullets: &[
      "Single native Tauri 2 binary backed by Rust drivers; streams large DuckDB Parquet files and paginated results without hogging memory.",
      "Oracle Instant Client (Basic Light) is bundled right inside the installer, alongside a built-in local relay for openGauss default sha256 accounts.",
      "Connection failures tell you exactly which layer broke: DNS lookup, refused TCP port, TLS handshake, SSH known_hosts fingerprint, or credentials.",
    ],
  },
  wf_native_panel_bar: "Runtime & zero-ceremony stack",
  wf_native_specs: EN_NATIVE_SPECS,
  wf_guardrails: WorkflowStory {
    title: "Every write previews as exact SQL — zero silent blur commits",
    lead: "Grid editors become dangerous when clicking away silently fires an UPDATE or overwrites a row changed by another session.",
    bullets: &[
      "Edits, inserts, and deletes queue locally so you can inspect the exact parameterized SQL before anything touches the server.",
      "Generated UPDATE and DELETE statements include original column values in WHERE; if affected_rows disagrees, the transaction rolls back.",
      "Connections carry explicit DEV, TEST, STAGING, or PROD badges, enforcing stricter confirmation before DROP, TRUNCATE, or unguarded writes.",
    ],
  },
  sql_preview_bar: "Queued edit preview · 1 cell modified",
  sql_preview_note: "If another session changed price from 429 in the meantime, affected_rows is 0 and the entire batch rolls back.",
  wf_schema: WorkflowStory {
    title: "Schema diff, visual execution plans, and deterministic ORM codegen",
    lead: "Move from ad-hoc queries to structural engineering without relying on black-box plugins or probabilistic AI output.",
    bullets: &[
      "Compare tables, columns, primary keys, indexes, and foreign keys across two connections to generate a reviewable DDL sync script.",
      "Inspect expandable execution plan trees across PostgreSQL, MySQL, TiDB, OceanBase, SQL Server, Oracle, and CockroachDB with >10x skew highlights.",
      "Export whole-database ER diagrams (SVG, PNG, PDF, or Markdown dictionary) and generate Prisma, TypeORM, MikroORM, SeaORM, or sqlx models deterministically.",
    ],
  },
  schema_panel_bar: "Schema diff & deterministic Rust / TypeScript codegen",
  schema_panel_note: "Generated directly from live catalog metadata — 100% deterministic, offline, and repeatable.",
  wf_agents: WorkflowStory {
    title: "Standard MCP & CLI for every AI coding tool, with PROD physically hidden",
    lead: "Connect Claude Code, Codex, Cursor, Windsurf, Copilot, Gemini CLI, or Claude Desktop directly to your configured connections and SSH tunnels — without ever pasting a password into a config file.",
    bullets: &[
      "Off by default per connection. Enabling read-only access binds an HMAC capability to your OS keychain secret — editing JSON on disk cannot unlock it.",
      "Production (PROD) connections are structurally invisible to both dataomni cli and dataomni mcp.",
      "Every query passes two independent barriers: a dialect-specific read-only lexer gate plus a database read-only transaction that always ends in ROLLBACK.",
    ],
  },
  agents_terminal_bar: "dataomni mcp & dataomni cli",
  agents_cli_comment: "# Export full CLI schema & usage for any terminal agent",
  agents_mcp_cmd_note: "Standard stdio MCP — pick your AI tool and copy:",
  agents_mcp_aria_label: "MCP client configuration presets",
  mcp_presets: MCP_PRESETS,
  copy_label: "Copy",
  copied_label: "Copied",
  compare_heading: "How DataOmni compares",
  compare_intro: "An honest look at architectural trade-offs across desktop database clients.",
  compare_col_dimension: "Dimension",
  compare_col_dataomni: "DataOmni",
  compare_col_jvm: "JVM Suites (e.g. DBeaver)",
  compare_col_commercial: "Commercial Native (e.g. TablePlus / Navicat)",
  compare_rows: EN_COMPARE_ROWS,
  compare_footnote: "Where others still win: if your workflow depends on DB2, Teradata, Snowflake, or dozens of legacy JDBC drivers, DBeaver or long-standing commercial suites offer broader long-tail coverage. If your daily stack is PostgreSQL, MySQL, Oracle, SQL Server, SQLite, DuckDB, ClickHouse, and MongoDB / Redis / Neo4j / Elasticsearch — and you want native speed, explicit SQL guardrails, MIT licensing, and safe local MCP — DataOmni is purpose-built for you.",
  compatibility_heading: "One client across 18 real-server-tested databases",
  compatibility_intro: "Every supported database is verified by automated smoke suites against real server instances (src-tauri/tests/*_smoke.rs), never assumed from wire-protocol similarity.",
  ecosystem_cards: EN_ECOSYSTEM_CARDS,
  ledger_drawer_summary: "Inspect full 18-server smoke-test compatibility ledger (versions, test counts & known gaps)",
  ledger_drawer_hint: "Counts show passed / skipped / fixture-unsupported",
  filter_all: "All 18 servers",
  filter_relational: "Relational & SQL (13)",
  filter_workspaces: "Document, KV, Graph & Search (5)",
  col_server: "Server",
  col_version: "Tested",
  col_driver: "Connection type",
  col_status: "Status",
  col_smoke: "Smoke (P/S/F)",
  col_notes: "Verified behavior & known gaps",
  status_supported: "Supported",
  status_gaps: "Usable, with gaps",
  servers: EN_SERVERS,
  faq_heading: "Frequently asked questions",
  faq_intro: "Straight answers on licensing, credential security, code signing, and agent safety.",
  faq_items: EN_FAQ,
  install_heading: "Download DataOmni",
  install_intro: "Oracle Instant Client (Basic Light) is bundled in every package. macOS builds are signed with an Apple Developer ID and notarized by Apple; Windows builds require a one-time SmartScreen confirmation.",
  macos_title: "macOS (Apple Silicon)",
  macos_meta: "macOS 12+ · arm64 · Signed & Notarized (.dmg / Homebrew)",
  macos_note: "Signed & notarized by Apple — drag to /Applications, or install via Homebrew (also links dataomni CLI):",
  windows_title: "Windows (x64)",
  windows_meta: "Windows 10/11 · per-user NSIS (.exe) or machine-wide MSI",
  windows_note: "On first launch, click \"More info\" → \"Run anyway\" in Windows SmartScreen. ARM64 Windows runs the x64 build under emulation.",
  linux_title: "Linux (x64)",
  linux_meta: "glibc 2.34+ (Ubuntu 22.04+, Fedora 42+) · .deb & .rpm",
  linux_note: "Saving passwords requires an unlocked Secret Service keyring (such as gnome-keyring). Install via apt or dnf:",
  footer_summary: "DataOmni — MIT-licensed native desktop database client.",
  footer_releases: "Releases & SHA-256",
  footer_issues: "Issues",
  footer_license: "MIT License",
};

pub static ZH_COPY: PageCopy = PageCopy {
  locale: Locale::Zh,
  meta_title: "DataOmni — 原生 Rust 数据库客户端 | SQL · MongoDB · Redis",
  meta_description: "MIT 开源跨平台数据库客户端（Tauri 2 + Rust），连齐 PostgreSQL、MySQL、Oracle、ClickHouse、MongoDB、Redis、Neo4j 与 ES。改表先审 SQL、钥匙串存密、内置只读 MCP。",
  og_image_alt: "DataOmni 原生跨平台桌面数据库客户端界面预览（支持关系库、MongoDB、Redis、Neo4j 与 ES）",
  skip_to_content: "跳到主要内容",
  nav_workflows: "核心体验",
  nav_compatibility: "支持引擎",
  nav_compare: "选型对比",
  nav_faq: "常见问题",
  nav_install: "下载安装",
  theme_light: "浅色",
  theme_dark: "深色",
  hero_title: "一个原生轻量客户端，连齐关系库与 MongoDB、Redis、Neo4j、ES。",
  hero_lead: "基于 Tauri 2 与 Rust 原生构建——告别上 GB 的 JVM 内存占用、标签页数量限制与明文落盘密码。装完直连 Oracle，每一次改表先审真实 SQL 再进单事务提交，还能为 Claude Code、Codex、Cursor 等各类 AI 工具开启物理隔离生产库的只读 MCP。",
  primary_download_default: "免费下载 DataOmni",
  primary_cta_macos: "下载 macOS 版（Apple Silicon .dmg）",
  primary_cta_windows: "下载 Windows 版（x64 .exe）",
  primary_cta_linux: "下载 Linux 版（x64 .deb）",
  github_cta: "GitHub 源码",
  release_notes_cta: "版本说明",
  all_packages_label: "全平台安装包：",
  trust_facts: ZH_TRUST_FACTS,
  workbench_aria_label: "DataOmni 真实工作台界面切换预览",
  tabs: ZH_TABS,
  ecosystem_strip_label: "全部经真实数据库实例自动化冒烟验证的 17 种引擎：",
  engine_badges: ENGINE_BADGES,
  workflows_heading: "针对数据库客户端最痛的四个瞬间重新设计",
  workflows_intro: "不堆砌用不上的花哨噱头。DataOmni 的每一处设计，都对应日常查库、改数据、对表结构与配合 AI 排障时的真实痛点。",
  wf_native: WorkflowStory {
    title: "原生 Rust 秒开，连 Oracle 与国产兼容库也免折腾",
    lead: "不必再为了查 Postgres、Oracle、MongoDB 和 Redis 同时开四五个客户端，也不用花半天配 Instant Client 环境变量。",
    bullets: &[
      "Tauri 2 + Rust 原生构建，直接复用操作系统 WebView；DuckDB 流式直查大体积 Parquet / JSON 文件，不把整份数据塞进内存。",
      "安装包随包内置 Oracle Instant Client（Basic Light），并内置 openGauss 默认 sha256 口令的本地协议转发口，开箱即连。",
      "连不上时绝不只报一句笼统的超时：精准区分域名不存在、端口拒绝、TLS 握手中断、SSH known_hosts 指纹变更或凭据错误。",
    ],
  },
  wf_native_panel_bar: "原生运行栈与开箱即用能力",
  wf_native_specs: ZH_NATIVE_SPECS,
  wf_guardrails: WorkflowStory {
    title: "改表绝不「失焦暗自提交」，每一笔变更先审真实 SQL",
    lead: "桌面客户端最危险的设计，莫过于鼠标点开单元格就悄悄发出 UPDATE，或者静默覆盖掉同事刚改过的数据。",
    bullets: &[
      "表格增删改与表结构变更一律先进入待提交队列，完整展示即将执行的精确参数化 SQL，确认无误才发往服务端。",
      "自动在 WHERE 子句带上修改前的原值做乐观并发校验；单事务内逐条核对受影响行数，一旦不符立即整批回滚。",
      "连接自带文字与颜色双重环境档位（DEV / TEST / STAGING / PROD），对 DROP、TRUNCATE 与无条件改删执行分级拦截确认。",
    ],
  },
  sql_preview_bar: "待提交变更预览 · 已修改 1 个单元格",
  sql_preview_note: "若提交前其他会话已将 price 改离 429，受影响行数为 0，整批变更立即回滚，绝不静默覆盖。",
  wf_schema: WorkflowStory {
    title: "双库结构比对、执行计划偏差定位与确定性 ORM 代码生成",
    lead: "把核对测试库与预发库表结构、排查慢查询、同步代码模型这些高频机械活，收进干净的确定性工作流。",
    bullets: &[
      "直接比对两个连接的表、列、主键、索引与外键差异，勾选后生成可审阅的 DDL 同步脚本送入编辑器执行。",
      "可视化解析 PostgreSQL、MySQL、TiDB、OceanBase、SQL Server、Oracle 与 CockroachDB 执行计划树，自动高亮估算偏差超 10 倍的算子。",
      "一键生成整库 ER 图（导出 SVG / PNG / PDF 与 Markdown 数据字典），并确定性生成 Prisma、TypeORM、MikroORM、SeaORM 与 sqlx 代码，不依赖大模型。",
    ],
  },
  schema_panel_bar: "双库结构差异同步 & 确定性模型代码生成",
  schema_panel_note: "直接从实时表目录元数据确定性推导——完全离线、零幻觉、每次输出一致。",
  wf_agents: WorkflowStory {
    title: "标准 MCP 与 CLI 适配各类 AI 工具，生产库在架构层彻底隐身",
    lead: "支持 Claude Code、Codex、Cursor、Windsurf、GitHub Copilot、Gemini CLI 与 Claude Desktop——直接复用已配好的开发库与 SSH 隧道查结构、跑诊断查询，全程无需在任何配置文件里粘贴数据库密码。",
    bullets: &[
      "每条连接默认关闭 Agent 访问；手动开启只读时用钥匙串口令签发 HMAC 凭证，只改磁盘上的 connections.json 无法越权。",
      "标记为生产环境（PROD）的连接在任何参数下对 dataomni cli 与 dataomni mcp 均不可见，从根源杜绝误连线上库。",
      "每条查询先过各方言只读词法闸门（拦截写操作、DDL 与 pg_sleep 等副作用函数），再放进数据库只读事务执行并以 ROLLBACK 收尾。",
    ],
  },
  agents_terminal_bar: "dataomni mcp 与 dataomni cli",
  agents_cli_comment: "# 导出完整调用规范，供任意支持命令行的 AI 工具直接读取",
  agents_mcp_cmd_note: "标准 stdio MCP 协议，点选对应工具即可复制接入配置：",
  agents_mcp_aria_label: "MCP 客户端配置预设切换",
  mcp_presets: MCP_PRESETS,
  copy_label: "复制",
  copied_label: "已复制",
  compare_heading: "客观选型对比",
  compare_intro: "不同技术路线各有取舍，把架构事实摊开，方便你判断 DataOmni 是否适合手头的工作流。",
  compare_col_dimension: "对比维度",
  compare_col_dataomni: "DataOmni",
  compare_col_jvm: "JVM 系综合工具（如 DBeaver）",
  compare_col_commercial: "商业原生客户端（如 TablePlus / Navicat）",
  compare_rows: ZH_COMPARE_ROWS,
  compare_footnote: "坦诚说明边界：如果你的业务重度依赖 DB2、Teradata、Snowflake 或数十种冷门遗留 JDBC 数据源，DBeaver 或老牌商业套件的插件覆盖面依然更广；如果你的主力栈是 PostgreSQL、MySQL、Oracle、SQL Server、SQLite、DuckDB、ClickHouse 以及 MongoDB / Redis / Neo4j / ES，且看重原生轻量、MIT 免费商用、改表防覆盖与安全的本地 AI MCP，DataOmni 就是专门为你打磨的。",
  compatibility_heading: "18 种服务端全部经真库实测验收",
  compatibility_intro: "不靠「协议兼容」做纸面宣传。每一种列出的服务端，都连上真实数据库跑完自动化冒烟套件（src-tauri/tests/*_smoke.rs），并如实记录已知边界。",
  ecosystem_cards: ZH_ECOSYSTEM_CARDS,
  ledger_drawer_summary: "展开全部 18 种服务端真库实测兼容明细（实测版本、冒烟用例数与已知边界）",
  ledger_drawer_hint: "计数依次为：通过 / 跳过 / 夹具不支持",
  filter_all: "全部 18 种服务端",
  filter_relational: "关系型与 SQL（13）",
  filter_workspaces: "文档 / 键值 / 图 / 搜索（5）",
  col_server: "服务端",
  col_version: "实测版本",
  col_driver: "连接类型",
  col_status: "结论",
  col_smoke: "冒烟用例（过/跳/挂）",
  col_notes: "已验证能力与已知边界",
  status_supported: "完整支持",
  status_gaps: "可用，有已知边界",
  servers: ZH_SERVERS,
  faq_heading: "常见问题",
  faq_intro: "关于开源商用授权、数据与密码隐私、首次打开系统提示以及 AI 工具安全边界的直接解答。",
  faq_items: ZH_FAQ,
  install_heading: "下载安装 DataOmni",
  install_intro: "各平台安装包均已内置 Oracle Instant Client（Basic Light）。macOS 安装包已由 Apple Developer ID 签名并通过 Apple 公证；Windows 首次启动按下方提示放行一次即可。",
  macos_title: "macOS（Apple Silicon）",
  macos_meta: "macOS 12+ · arm64 · 已签名公证（.dmg / Homebrew）",
  macos_note: "已由 Apple Developer ID 签名公证——直接拖入「应用程序」，或通过 Homebrew 安装（自动软链 CLI）：",
  windows_title: "Windows（x64）",
  windows_meta: "Windows 10/11 · 按用户安装 NSIS（.exe）或按机器安装 MSI",
  windows_note: "首次运行遇到 SmartScreen 提示时，点「更多信息」→「仍要运行」。ARM64 Windows 可经系统转译运行 x64 包。",
  linux_title: "Linux（x64）",
  linux_meta: "glibc 2.34+（Ubuntu 22.04+、Fedora 42+）· .deb 与 .rpm",
  linux_note: "保存数据库口令需要桌面会话已解锁 Secret Service（如 gnome-keyring）。安装命令：",
  footer_summary: "DataOmni — MIT 开源的原生桌面数据库客户端。",
  footer_releases: "版本发布与 SHA-256",
  footer_issues: "问题反馈",
  footer_license: "MIT 许可证",
};
