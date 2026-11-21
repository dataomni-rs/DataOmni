use crate::content::Locale;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocSlug {
  Quickstart,
  Mcp,
  Cli,
  Databases,
  Changelog,
  Privacy,
}

pub const ALL_DOC_SLUGS: &[DocSlug] = &[
  DocSlug::Quickstart,
  DocSlug::Mcp,
  DocSlug::Cli,
  DocSlug::Databases,
  DocSlug::Changelog,
  DocSlug::Privacy,
];

impl DocSlug {
  pub const fn path(self, locale: Locale) -> &'static str {
    match (locale, self) {
      (Locale::En, Self::Quickstart) => "/docs/quickstart/",
      (Locale::Zh, Self::Quickstart) => "/zh/docs/quickstart/",
      (Locale::En, Self::Mcp) => "/docs/mcp/",
      (Locale::Zh, Self::Mcp) => "/zh/docs/mcp/",
      (Locale::En, Self::Cli) => "/docs/cli/",
      (Locale::Zh, Self::Cli) => "/zh/docs/cli/",
      (Locale::En, Self::Databases) => "/docs/databases/",
      (Locale::Zh, Self::Databases) => "/zh/docs/databases/",
      (Locale::En, Self::Changelog) => "/changelog/",
      (Locale::Zh, Self::Changelog) => "/zh/changelog/",
      (Locale::En, Self::Privacy) => "/privacy/",
      (Locale::Zh, Self::Privacy) => "/zh/privacy/",
    }
  }

  pub const fn output_rel_html(self, locale: Locale) -> &'static str {
    match (locale, self) {
      (Locale::En, Self::Quickstart) => "docs/quickstart/index.html",
      (Locale::Zh, Self::Quickstart) => "zh/docs/quickstart/index.html",
      (Locale::En, Self::Mcp) => "docs/mcp/index.html",
      (Locale::Zh, Self::Mcp) => "zh/docs/mcp/index.html",
      (Locale::En, Self::Cli) => "docs/cli/index.html",
      (Locale::Zh, Self::Cli) => "zh/docs/cli/index.html",
      (Locale::En, Self::Databases) => "docs/databases/index.html",
      (Locale::Zh, Self::Databases) => "zh/docs/databases/index.html",
      (Locale::En, Self::Changelog) => "changelog/index.html",
      (Locale::Zh, Self::Changelog) => "zh/changelog/index.html",
      (Locale::En, Self::Privacy) => "privacy/index.html",
      (Locale::Zh, Self::Privacy) => "zh/privacy/index.html",
    }
  }

  pub const fn nav_label(self, locale: Locale) -> &'static str {
    match (locale, self) {
      (Locale::En, Self::Quickstart) => "Quickstart",
      (Locale::Zh, Self::Quickstart) => "快速上手",
      (Locale::En, Self::Mcp) => "MCP Server",
      (Locale::Zh, Self::Mcp) => "只读 MCP",
      (Locale::En, Self::Cli) => "Agent CLI",
      (Locale::Zh, Self::Cli) => "命令行 CLI",
      (Locale::En, Self::Databases) => "Databases",
      (Locale::Zh, Self::Databases) => "数据库指南",
      (Locale::En, Self::Changelog) => "Changelog",
      (Locale::Zh, Self::Changelog) => "更新记录",
      (Locale::En, Self::Privacy) => "Privacy",
      (Locale::Zh, Self::Privacy) => "隐私安全",
    }
  }

  pub const fn is_root_route(self) -> bool {
    matches!(self, Self::Changelog | Self::Privacy)
  }

  pub const fn prev(self) -> Option<Self> {
    match self {
      Self::Quickstart => None,
      Self::Mcp => Some(Self::Quickstart),
      Self::Cli => Some(Self::Mcp),
      Self::Databases => Some(Self::Cli),
      Self::Changelog => Some(Self::Databases),
      Self::Privacy => Some(Self::Changelog),
    }
  }

  pub const fn next(self) -> Option<Self> {
    match self {
      Self::Quickstart => Some(Self::Mcp),
      Self::Mcp => Some(Self::Cli),
      Self::Cli => Some(Self::Databases),
      Self::Databases => Some(Self::Changelog),
      Self::Changelog => Some(Self::Privacy),
      Self::Privacy => None,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalloutTone {
  Note,
  Guardrail,
}

pub enum DocBlock {
  Paragraph(&'static str),
  Bullets(&'static [&'static str]),
  Steps(&'static [(&'static str, &'static str)]),
  Callout {
    tone: CalloutTone,
    title: &'static str,
    body: &'static str,
  },
  Code {
    label: &'static str,
    copy_text: Option<&'static str>,
    raw_code: &'static str,
  },
  Table {
    caption: &'static str,
    headers: &'static [&'static str],
    rows: &'static [&'static [&'static str]],
  },
  McpPresets,
  CompatibilityMatrix,
}

pub struct DocSection {
  pub id: &'static str,
  pub toc_label: &'static str,
  pub heading: &'static str,
  pub blocks: &'static [DocBlock],
}

pub struct DocPage {
  pub slug: DocSlug,
  pub locale: Locale,
  pub meta_title: &'static str,
  pub meta_description: &'static str,
  pub breadcrumb_group: &'static str,
  pub title: &'static str,
  pub lead: &'static str,
  pub toc_heading: &'static str,
  pub sections: &'static [DocSection],
}

pub fn doc_page(slug: DocSlug, locale: Locale) -> &'static DocPage {
  match (locale, slug) {
    (Locale::En, DocSlug::Quickstart) => &EN_QUICKSTART,
    (Locale::Zh, DocSlug::Quickstart) => &ZH_QUICKSTART,
    (Locale::En, DocSlug::Mcp) => &EN_MCP,
    (Locale::Zh, DocSlug::Mcp) => &ZH_MCP,
    (Locale::En, DocSlug::Cli) => &EN_CLI,
    (Locale::Zh, DocSlug::Cli) => &ZH_CLI,
    (Locale::En, DocSlug::Databases) => &EN_DATABASES,
    (Locale::Zh, DocSlug::Databases) => &ZH_DATABASES,
    (Locale::En, DocSlug::Changelog) => &EN_CHANGELOG,
    (Locale::Zh, DocSlug::Changelog) => &ZH_CHANGELOG,
    (Locale::En, DocSlug::Privacy) => &EN_PRIVACY,
    (Locale::Zh, DocSlug::Privacy) => &ZH_PRIVACY,
  }
}

// ============================================================================
// 1. QUICKSTART (/docs/quickstart/ & /zh/docs/quickstart/)
// ============================================================================

const EN_QUICKSTART_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "install",
    toc_label: "Install & first launch",
    heading: "Install and launch on macOS, Windows, or Linux",
    blocks: &[
      DocBlock::Paragraph(
        "Every installer ships as a single native Tauri 2 desktop application with Oracle Instant Client (Basic Light), DuckDB 2.0, and SQLite compiled or bundled inside. No Java runtime, Node.js runtime, or cloud account is required.",
      ),
      DocBlock::Table {
        caption: "Platform installers and first-launch behavior",
        headers: &["Platform", "Package", "First-launch & credential notes"],
        rows: &[
          &[
            "macOS 12+ (Apple Silicon)",
            "Homebrew Cask or .dmg",
            "Signed with an Apple Developer ID and notarized by Apple — drag DataOmni.app to /Applications, or install via Homebrew below (which also links dataomni onto PATH).",
          ],
          &[
            "Windows 10 / 11 (x64)",
            ".exe (per-user NSIS) or .msi",
            "Click \"More info\" -> \"Run anyway\" once if Windows SmartScreen prompts. Passwords are stored in Windows Credential Manager; SQLite and DuckDB file paths exceeding 260 characters work out of the box.",
          ],
          &[
            "Linux x86_64 (glibc 2.34+)",
            ".deb or .rpm",
            "Ubuntu 22.04+ and Fedora 42+. Saving passwords requires an unlocked Secret Service (such as gnome-keyring or KWallet); without one, uncheck \"Save password\" for session-only connections.",
          ],
        ],
      },
      DocBlock::Code {
        label: "Install on macOS via Homebrew Cask",
        copy_text: Some("brew install --cask dataomni-rs/tap/dataomni"),
        raw_code: "brew install --cask dataomni-rs/tap/dataomni",
      },
    ],
  },
  DocSection {
    id: "keychain",
    toc_label: "OS keychain & local storage",
    heading: "Where credentials and local data live",
    blocks: &[
      DocBlock::Paragraph(
        "DataOmni never writes database passwords, SSH passphrases, or AI API keys to disk in plaintext. Connection profiles store only non-secret metadata (host, port, database name, TLS mode, environment) plus an opaque reference to your operating system credential store. Moving from another desktop client? Use \"Import connections...\" on the Welcome screen or Command Palette (Cmd+K / Ctrl+K) to preview-import Navicat .ncx, DBeaver data-sources.json, or DataOmni JSON files — all third-party encrypted secrets are stripped and Agent access defaults to off.",
      ),
      DocBlock::Table {
        caption: "On-disk configuration and log directories",
        headers: &["Artifact", "macOS", "Linux", "Windows"],
        rows: &[
          &[
            "connections.json (no secrets)",
            "~/Library/Application Support/com.dataomni.app/",
            "~/.config/com.dataomni.app/",
            "%APPDATA%\\com.dataomni.app\\",
          ],
          &[
            "Passwords & SSH passphrases",
            "macOS Keychain",
            "Secret Service (gnome-keyring)",
            "Windows Credential Manager",
          ],
          &[
            "Diagnostic & CLI audit logs",
            "~/Library/Logs/com.dataomni.app/",
            "~/.local/share/com.dataomni.app/logs/",
            "%LOCALAPPDATA%\\com.dataomni.app\\logs\\",
          ],
        ],
      },
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "macOS Keychain & Developer ID team signing",
        body: "Official macOS builds (v1.2.1+) are signed with a consistent Apple Developer ID team, so routine upgrades preserve Keychain access without re-prompting. If upgrading once from an earlier ad-hoc build (<= 1.2.0), enter your login password and click \"Always Allow\" once so both the GUI and CLI/MCP share the team grant.",
      },
    ],
  },
  DocSection {
    id: "tls-and-ssh",
    toc_label: "TLS modes & SSH tunnels",
    heading: "TLS verification tiers and SSH jump hosts",
    blocks: &[
      DocBlock::Paragraph(
        "All network TLS connections run through rustls so verification rules behave identically across macOS, Windows, and Linux. When connecting through a bastion host, enable \"Connect via SSH tunnel\" to forward traffic directly in-process without spawning external ssh processes.",
      ),
      DocBlock::Table {
        caption: "Five TLS verification modes",
        headers: &["Mode", "Encryption", "Certificate check", "When to pick"],
        rows: &[
          &[
            "Disable",
            "Plaintext",
            "None",
            "Local Docker containers or legacy MySQL 5.7 instances without ECDHE ciphers.",
          ],
          &[
            "Prefer TLS",
            "Opportunistic",
            "None",
            "Encrypts if the server offers TLS, falls back to plaintext otherwise (default on SQL Server / TiDB / CockroachDB).",
          ],
          &[
            "Require TLS",
            "Mandatory",
            "None",
            "Rejects plaintext fallback while accepting self-signed server certificates (default on MySQL).",
          ],
          &[
            "Verify CA",
            "Mandatory",
            "Issuer CA",
            "Verifies the certificate chain against system roots or your custom CA PEM file.",
          ],
          &[
            "Verify Full",
            "Mandatory",
            "Issuer CA + Hostname",
            "Production-grade verification of both the CA chain and server DNS/IP SAN.",
          ],
        ],
      },
      DocBlock::Bullets(&[
        "Mutual TLS (mTLS): supply both Client Certificate Path and Client Private Key Path together (database X.509 pair, distinct from your SSH key).",
        "Strict SSH host-key verification: jump hosts are checked against ~/.ssh/known_hosts (%USERPROFILE%\\.ssh\\known_hosts on Windows). Unknown or changed host keys are refused and print the SHA256 fingerprint — ssh into a new bastion once first to trust its key.",
        "Layered diagnostics: click \"Diagnose\" in the connection form to isolate DNS resolution failures, refused TCP ports, TUN-proxy immediate drops, TLS alerts, or authentication errors.",
      ]),
    ],
  },
  DocSection {
    id: "guardrails",
    toc_label: "Safe edits & shortcuts",
    heading: "Environment badges, queued SQL preview, and shortcuts",
    blocks: &[
      DocBlock::Paragraph(
        "Mark every profile as Development, Test, Staging, or Production. Staging and Production display persistent amber and red badges across the sidebar, SQL tabs, and table grids, and enforce stricter confirmation dialogs before DROP, TRUNCATE, FLUSHDB, or UPDATE/DELETE without WHERE.",
      ),
      DocBlock::Steps(&[
        (
          "Edit cells in the grid without immediate network writes",
          "Modify cells, insert rows, or mark deletions in Table Data or editable query results. Changes accumulate in a local pending queue — clicking out of a cell never silently fires an UPDATE.",
        ),
        (
          "Inspect exact parameterized SQL",
          "Open the pending changes preview to review the generated INSERT, UPDATE, and DELETE statements. Original column values are bound into WHERE so concurrent edits by teammates are detected.",
        ),
        (
          "Commit in one verified transaction",
          "On Commit, DataOmni runs the batch inside a single transaction and checks that every statement affects exactly 1 row. Any mismatch triggers an automatic ROLLBACK and preserves your queued edits.",
        ),
      ]),
      DocBlock::Table {
        caption: "Essential keyboard shortcuts (use Ctrl on Windows / Linux)",
        headers: &["Action", "macOS", "Windows / Linux"],
        rows: &[
          &["Command palette (connections, tables, actions)", "Cmd + K", "Ctrl + K"],
          &["Execute current statement or selection", "Cmd + Enter", "Ctrl + Enter"],
          &["New query tab / Close current tab", "Cmd + T / Cmd + W", "Ctrl + T / Ctrl + W"],
          &["Close application window", "Cmd + Shift + W", "Ctrl + Shift + W"],
          &["Toggle object browser sidebar", "Cmd + B", "Ctrl + B"],
        ],
      },
    ],
  },
];

static EN_QUICKSTART: DocPage = DocPage {
  slug: DocSlug::Quickstart,
  locale: Locale::En,
  meta_title: "Quickstart & Connection Setup — DataOmni Docs",
  meta_description: "Install DataOmni on macOS, Windows, or Linux, store credentials in your OS keychain, configure SSH tunnels and TLS verification, and preview edits safely.",
  breadcrumb_group: "Docs",
  title: "Quickstart & connection setup",
  lead: "Go from download to a verified database connection in two minutes — with OS keychain storage, SSH jump-host verification, and preview-before-commit SQL guardrails.",
  toc_heading: "On this page",
  sections: EN_QUICKSTART_SECTIONS,
};

const ZH_QUICKSTART_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "install",
    toc_label: "三平台安装与首次启动",
    heading: "在 macOS、Windows 与 Linux 上安装启动",
    blocks: &[
      DocBlock::Paragraph(
        "DataOmni 基于 Tauri 2 与 Rust 构建为单一原生桌面安装包，随包内置 Oracle Instant Client（Basic Light）、DuckDB 2.0 与 SQLite，无需安装 Java 运行时、Node.js 或注册任何云端账号。",
      ),
      DocBlock::Table {
        caption: "三平台安装包与首次启动说明",
        headers: &["平台", "安装包", "首次启动与凭据说明"],
        rows: &[
          &[
            "macOS 12+（Apple Silicon）",
            "Homebrew Cask 或 .dmg",
            "已由 Apple Developer ID 签名并通过 Apple 公证——直接将 DataOmni.app 拖入「应用程序」，或执行下方 Homebrew 命令安装（同时自动把 dataomni 软链进 PATH）。",
          ],
          &[
            "Windows 10 / 11（x64）",
            ".exe（NSIS）或 .msi",
            "首次启动若遇 SmartScreen 提示，点「更多信息 → 仍要运行」。口令存入 Windows 凭据管理器；SQLite 与 DuckDB 原生支持超过 260 字符的长路径。",
          ],
          &[
            "Linux x86_64（glibc 2.34+）",
            ".deb 或 .rpm",
            "支持 Ubuntu 22.04+、Fedora 42+。保存口令需桌面会话已解锁 Secret Service（如 gnome-keyring）；未安装时可取消勾选「保存密码」按当次会话输入。",
          ],
        ],
      },
      DocBlock::Code {
        label: "macOS 通过 Homebrew Cask 安装",
        copy_text: Some("brew install --cask dataomni-rs/tap/dataomni"),
        raw_code: "brew install --cask dataomni-rs/tap/dataomni",
      },
    ],
  },
  DocSection {
    id: "keychain",
    toc_label: "系统钥匙串与落盘目录",
    heading: "密码存在哪里，本地文件放在哪里",
    blocks: &[
      DocBlock::Paragraph(
        "数据库密码、SSH 私钥口令与 AI API Key 一律写入操作系统钥匙串，绝不以明文或私有可逆编码写进本地配置文件。磁盘上的 connections.json 只保留主机、端口、库名、TLS 档位与环境标记，可放心做常规备份。从其他工具迁移时，点击欢迎页或命令面板（Cmd+K / Ctrl+K）里的「导入连接…」，即可离线预览并勾选导入 Navicat .ncx、DBeaver data-sources.json 或 DataOmni JSON，第三方加密口令会被自动剥离且命令行访问重置为关闭。",
      ),
      DocBlock::Table {
        caption: "配置文件、凭据与日志存储位置",
        headers: &["内容", "macOS", "Linux", "Windows"],
        rows: &[
          &[
            "connections.json（不含口令）",
            "~/Library/Application Support/com.dataomni.app/",
            "~/.config/com.dataomni.app/",
            "%APPDATA%\\com.dataomni.app\\",
          ],
          &[
            "数据库口令与 SSH 口令",
            "macOS 系统钥匙串",
            "Secret Service（gnome-keyring）",
            "Windows 凭据管理器",
          ],
          &[
            "运行日志与 CLI 留痕",
            "~/Library/Logs/com.dataomni.app/",
            "~/.local/share/com.dataomni.app/logs/",
            "%LOCALAPPDATA%\\com.dataomni.app\\logs\\",
          ],
        ],
      },
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "macOS 钥匙串与团队签名升级说明",
        body: "官方 macOS 安装包（v1.2.1 起）采用固定 Apple Developer ID 团队签名，日常升级不会重弹钥匙串授权框。若从 1.2.0 及更早的未签名版本首次升上来，首次读取已存密码时输入开机密码并点「始终允许」，界面端与内置 CLI / MCP 即可共享同一团队授权。",
      },
    ],
  },
  DocSection {
    id: "tls-and-ssh",
    toc_label: "五档 TLS 与 SSH 隧道",
    heading: "TLS 证书校验档位与 SSH 跳板机隧道",
    blocks: &[
      DocBlock::Paragraph(
        "所有网络数据库统一经 rustls 建立加密连接，三平台行为完全一致。数据库仅监听内网时，勾选「通过 SSH 隧道连接」即可在进程内直接打通跳板机转发，无需手工维护外部 ssh 隧道进程。",
      ),
      DocBlock::Table {
        caption: "五档 TLS 模式含义对照",
        headers: &["模式", "是否加密", "证书校验", "适用场景"],
        rows: &[
          &[
            "禁用",
            "明文传输",
            "不校验",
            "本机 Docker 测试库，或无 ECDHE 加密套件的旧版 MySQL 5.7。",
          ],
          &[
            "优先使用 TLS",
            "协商加密",
            "不校验",
            "服务端支持则加密，不支持则退回明文（SQL Server / TiDB / CockroachDB 默认档）。",
          ],
          &[
            "要求 TLS",
            "强制加密",
            "不校验",
            "拒绝降级明文，但放行云厂商或内网自签证书（MySQL 新建连接默认档）。",
          ],
          &[
            "校验证书颁发机构",
            "强制加密",
            "校验 CA 链",
            "使用系统根证书或指定 CA PEM 文件验证服务端证书签发方。",
          ],
          &[
            "校验证书和主机名",
            "强制加密",
            "校验 CA + 主机名",
            "生产环境标准配置，同时核对证书链与连接地址域名 / IP。",
          ],
        ],
      },
      DocBlock::Bullets(&[
        "双向 TLS（mTLS）：服务端要求客户端证书时，「客户端证书路径」与「客户端私钥路径」必须成对填写（填数据库 TLS 证书，勿与 SSH 私钥混用）。",
        "SSH known_hosts 强校验：跳板机主机指纹严格核对 ~/.ssh/known_hosts（Windows 为 %USERPROFILE%\\.ssh\\known_hosts），未知或变更一律拒绝并回显 SHA256 指纹，不设「跳过校验」后门；首次连新跳板机前请先在终端 ssh 登录一次确认指纹。",
        "逐层网络诊断：连不上时点击表单里的「网络诊断」，直接区分 DNS 解析失败、端口未监听、本机 TUN 代理闪断、TLS 握手告警或账号口令错误。",
      ]),
    ],
  },
  DocSection {
    id: "guardrails",
    toc_label: "生产护栏与单事务提交",
    heading: "环境分级护栏、SQL 预览提交与常用快捷键",
    blocks: &[
      DocBlock::Paragraph(
        "新建连接时即可指定「开发 / 测试 / 预发 / 生产」环境。预发与生产连接在侧边栏、查询标签与表数据页全程悬挂醒目标识，并对 DROP、TRUNCATE、FLUSHDB 及不带 WHERE 的改删语句执行分级二次拦截。",
      ),
      DocBlock::Steps(&[
        (
          "表格改动先进本地待提交队列",
          "在表数据页或可编辑结果集里改单元格、新增行或标记删除，所有动作先进入待提交队列——鼠标点开单元格绝不会静默发出 UPDATE。",
        ),
        (
          "提交前审阅真实参数化 SQL",
          "展开待提交预览面板，逐条核对即将执行的 INSERT / UPDATE / DELETE 语句；WHERE 子句自动附带修改前原值，防止静默覆盖他人刚改过的数据。",
        ),
        (
          "单事务执行并逐条核对影响行数",
          "点击提交后，整批语句在同一个数据库事务内执行，逐条校验受影响行数恰好为 1；任一条不符立即整批 ROLLBACK，队列里的改动原样保留供刷新重试。",
        ),
      ]),
      DocBlock::Table {
        caption: "高频快捷键（Windows / Linux 将 Cmd 换为 Ctrl）",
        headers: &["操作", "macOS", "Windows / Linux"],
        rows: &[
          &["打开命令面板（搜连接、搜表、调动作）", "Cmd + K", "Ctrl + K"],
          &["执行当前语句或选中语句", "Cmd + Enter", "Ctrl + Enter"],
          &["新建查询标签 / 关闭当前标签", "Cmd + T / Cmd + W", "Ctrl + T / Ctrl + W"],
          &["关闭整个应用窗口", "Cmd + Shift + W", "Ctrl + Shift + W"],
          &["折叠或展开左侧对象树", "Cmd + B", "Ctrl + B"],
        ],
      },
    ],
  },
];

static ZH_QUICKSTART: DocPage = DocPage {
  slug: DocSlug::Quickstart,
  locale: Locale::Zh,
  meta_title: "快速上手与安全连接配置 — DataOmni 使用文档",
  meta_description: "在 macOS、Windows 与 Linux 上安装 DataOmni，使用系统钥匙串保存数据库口令，配置 SSH 跳板机与五档 TLS 校验，并按生产护栏安全提交事务修改。",
  breadcrumb_group: "文档",
  title: "快速上手与安全连接配置",
  lead: "两分钟完成安装、系统钥匙串存密、SSH 跳板机与 TLS 校验配置，掌握「改表先审 SQL、单事务核对回滚」的日常安全工作流。",
  toc_heading: "本页目录",
  sections: ZH_QUICKSTART_SECTIONS,
};

// ============================================================================
// 2. MCP SERVER (/docs/mcp/ & /zh/docs/mcp/)
// ============================================================================

const EN_MCP_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "enable",
    toc_label: "3-step setup",
    heading: "Enable read-only MCP access in three steps",
    blocks: &[
      DocBlock::Paragraph(
        "dataomni mcp runs inside the exact same binary as the desktop GUI, speaking JSON-RPC over stdio without opening a window. Sharing the binary identity means your AI agent uses your existing SSH tunnels, TLS certificates, and OS keychain secrets without ever seeing a password.",
      ),
      DocBlock::Steps(&[
        (
          "Save the database password to your OS keychain",
          "Open the connection in DataOmni and keep \"Save password to system credential store\" checked. Because dataomni mcp runs as a separate child process, session-only passwords entered in memory are unreachable.",
        ),
        (
          "Opt the non-production profile into Read-only agent access",
          "Verify the profile environment is Development, Test, or Staging, set \"Command line & agent access\" to Read-only, and click Save. DataOmni signs an HMAC-SHA256 capability (<id>#agent) into your OS keychain bound to the connection secret and endpoint.",
        ),
        (
          "Register dataomni mcp in your AI client",
          "Pick your AI coding assistant below to copy its exact registration command or mcpServers JSON block.",
        ),
      ]),
      DocBlock::McpPresets,
      DocBlock::Table {
        caption: "Binary path for dataomni mcp on each platform",
        headers: &["Platform", "Executable path", "Arguments"],
        rows: &[
          &["macOS", "/Applications/DataOmni.app/Contents/MacOS/dataomni", "mcp"],
          &["Linux (.deb / .rpm)", "/usr/bin/dataomni", "mcp"],
          &[
            "Windows (NSIS / MSI)",
            "%LOCALAPPDATA%\\DataOmni\\dataomni.exe or C:\\Program Files\\DataOmni\\dataomni.exe",
            "mcp",
          ],
        ],
      },
    ],
  },
  DocSection {
    id: "security-boundary",
    toc_label: "4-layer safety model",
    heading: "How production stays invisible and writes stay blocked",
    blocks: &[
      DocBlock::Callout {
        tone: CalloutTone::Guardrail,
        title: "Production profiles are physically invisible, not merely refused",
        body: "Connections marked Production force agent access off in the UI, omit any keychain grant, do not appear in tools/call connections, and return not_found if queried by name.",
      },
      DocBlock::Bullets(&[
        "Tamper-evident keychain grant: editing connections.json on disk to flip environment from production to development or enable agent access fails HMAC-SHA256 verification against the secret in your OS keychain.",
        "Live revocation without server restart: dataomni mcp re-reads connections.json and verifies the keychain grant on every tool call. Turning off agent access in the GUI hides the connection on the very next call.",
        "Two-barrier read-only execution: every SQL call accepts only a single read statement (SELECT, WITH without nested writes, SHOW, DESCRIBE, EXPLAIN without ANALYZE, VALUES, TABLE) AND executes inside a database read-only transaction (BEGIN READ ONLY / START TRANSACTION READ ONLY / SET TRANSACTION READ ONLY / readonly=1 / read-only file handle with DuckDB external access locked off) ending in ROLLBACK.",
        "No host filesystem side-channels: export, backup, and csv-preview are intentionally excluded from MCP so non-shell agents cannot write or read arbitrary local files through DataOmni.",
        "Statement-free audit trail: every call logs timestamp, connection name, tool, first SQL keyword, byte length, SHA-256 digest, outcome, row count, and duration to dataomni-cli.log — never logging literal query text.",
      ]),
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "Pair with a read-only database user for defense in depth",
        body: "Read-only transactions cannot block non-mutating server administration functions granted to superusers (such as PostgreSQL pg_terminate_backend or MySQL get_lock()). Always connect agents with a least-privilege read-only database account.",
      },
    ],
  },
  DocSection {
    id: "tools",
    toc_label: "Exposed MCP tools",
    heading: "All 13 read-only MCP tools",
    blocks: &[
      DocBlock::Paragraph(
        "Every tool carries readOnlyHint: true in tools/list and returns structured JSON with \"schema\": 1 (dictionary returns whole-database Markdown ready for LLM context). Unknown extra arguments or option-like positional values starting with -- are rejected as usage errors. Within a persistent dataomni mcp session, SSH tunnels are reused across tool calls and disconnected cleanly on EOF.",
      ),
      DocBlock::Table {
        caption: "MCP tools exposed by dataomni mcp",
        headers: &["Tool", "Arguments", "What it returns"],
        rows: &[
          &["connections", "(none)", "Lists non-production connections explicitly opened to agents (never leaks passwords or key paths)."],
          &["dictionary", "connection", "Whole-database Markdown data dictionary (tables, columns, types, PKs, FKs, and local semantics notes) — call this first before writing SQL."],
          &["schema", "connection, table?, schema?", "Lists tables and views, or columns, indexes, foreign keys, and table semantics for a specific table."],
          &["ddl", "connection, table, schema?", "Returns the database engine's own canonical object definition (SHOW CREATE TABLE, DBMS_METADATA, sqlite_master)."],
          &["query", "connection, sql, limit?, timeout_seconds?, params?", "Executes one read-only SQL statement with optional :name bind parameters (default 200 rows, max 10,000, reports truncated: true when capped)."],
          &["explain", "connection, sql, timeout_seconds?", "Returns the estimated execution plan without running ANALYZE or mutating state."],
          &["profile", "connection, sql, limit?, timeout_seconds?", "Profiles column distributions (type, null ratio, distinct count, min/max/quantiles, top-5 values) via built-in sandboxed DuckDB without pulling raw rows."],
          &["diff", "source, target, format?, timeout_seconds?", "Compares schemas across two open non-production connections (tables, columns, PKs, indexes, FKs) and returns a read-only JSON or Markdown report."],
          &["test", "connection", "Checks connectivity; on failure attaches DNS and TCP port diagnosis."],
          &["mongo", "connection, action, ...", "Read-only MongoDB operations: collections, find, count, aggregate ($out / $merge refused), explain, structure."],
          &["redis", "connection, action, ...", "Read-only Redis operations: keyspaces, scan, get, and command (verified via server COMMAND INFO readonly flag)."],
          &["neo4j", "connection, action, ...", "Read-only Cypher: labels and run (verified as read query via EXPLAIN plus Bolt read-mode routing)."],
          &["es", "connection, action, ...", "Read-only Elasticsearch / OpenSearch: indices and segment-checked GET/HEAD or search POST (_search, _count, _msearch)."],
        ],
      },
    ],
  },
  DocSection {
    id: "troubleshooting",
    toc_label: "Troubleshooting",
    heading: "Common MCP setup issues",
    blocks: &[
      DocBlock::Table {
        caption: "Diagnosing MCP errors",
        headers: &["Symptom", "Cause & fix"],
        rows: &[
          &[
            "connections returns [] or query says connection not found",
            "Either the profile is marked Production, \"Command line & agent access\" is still Off, or connections.json was edited manually without saving in the GUI. Open Edit Connection in DataOmni, set Read-only, and click Save.",
          ],
          &[
            "First tool call hangs on macOS",
            "macOS Keychain is waiting behind your editor window for authorization. Switch to the system prompt, enter your macOS login password, and click \"Always Allow\" — dataomni mcp keeps waiting rather than dropping the stdio session.",
          ],
          &[
            "SQL Server profile cannot enable agent access",
            "SQL Server lacks a session-level READ ONLY transaction guarantee; agent access is intentionally disabled for SQL Server in the current version.",
          ],
        ],
      },
    ],
  },
];

static EN_MCP: DocPage = DocPage {
  slug: DocSlug::Mcp,
  locale: Locale::En,
  meta_title: "Read-Only MCP Server Guide — DataOmni Docs",
  meta_description: "Connect Claude Code, Codex, Cursor, Windsurf, Copilot, Gemini CLI, or Claude Desktop to DataOmni via stdio MCP with physical production isolation.",
  breadcrumb_group: "Docs",
  title: "Read-only MCP server",
  lead: "Give Claude Code, Codex, Cursor, Windsurf, Copilot, Gemini CLI, or Claude Desktop direct read-only access to your configured SQL and NoSQL databases — zero passwords in config files, zero write paths, and production profiles physically hidden.",
  toc_heading: "On this page",
  sections: EN_MCP_SECTIONS,
};

const ZH_MCP_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "enable",
    toc_label: "三步接入流程",
    heading: "三步开启只读 MCP 服务",
    blocks: &[
      DocBlock::Paragraph(
        "dataomni mcp 直接内置于桌面客户端同一个可执行文件中，不起界面窗口，通过标准输入输出（stdio）提供原生的 MCP 协议服务。由于与桌面界面共享同一代码身份，AI 工具可直接复用已配好的 SSH 跳板机、TLS 证书与系统钥匙串口令，全程无需在任何 JSON 配置里粘贴数据库密码。",
      ),
      DocBlock::Steps(&[
        (
          "将连接密码存入系统钥匙串",
          "在 DataOmni 中编辑目标连接，确保勾选「将密码保存到系统凭据库」。MCP 作为独立子进程运行，无法读取仅在单次会话内存里输入的临时密码。",
        ),
        (
          "将非生产连接设为「只读」Agent 访问",
          "确认连接环境为「开发 / 测试 / 预发」，把「命令行与 Agent 访问」改为「只读」并保存。应用会用连接口令与端点指纹计算 HMAC-SHA256 开放凭证（<id>#agent）写入系统钥匙串。",
        ),
        (
          "在你的 AI 编程工具中注册服务",
          "点选下方你正在使用的 AI 工具，一键复制对应的注册命令或 mcpServers 配置片段。",
        ),
      ]),
      DocBlock::McpPresets,
      DocBlock::Table {
        caption: "各平台 dataomni 可执行文件路径",
        headers: &["平台", "可执行文件路径", "启动参数"],
        rows: &[
          &["macOS", "/Applications/DataOmni.app/Contents/MacOS/dataomni", "mcp"],
          &["Linux（.deb / .rpm）", "/usr/bin/dataomni", "mcp"],
          &[
            "Windows（NSIS / MSI）",
            "%LOCALAPPDATA%\\DataOmni\\dataomni.exe 或 C:\\Program Files\\DataOmni\\dataomni.exe",
            "mcp",
          ],
        ],
      },
    ],
  },
  DocSection {
    id: "security-boundary",
    toc_label: "四层安全隔离机制",
    heading: "生产库物理隐身与双重只读防线",
    blocks: &[
      DocBlock::Callout {
        tone: CalloutTone::Guardrail,
        title: "生产环境（PROD）连接物理不可见，而非仅仅报错拦截",
        body: "环境设为「生产」的连接在界面上强制锁定关闭 Agent 访问、不签发钥匙串凭证、不出现在 connections 列表里；即便按名字强行调用也直接返回 not_found。",
      },
      DocBlock::Bullets(&[
        "防篡改钥匙串开放凭证：即便本地进程直接手工修改磁盘上的 connections.json 把生产库改成开发库或打开只读开关，也会因算不出钥匙串口令对应的 HMAC-SHA256 凭证而被当场忽略。",
        "改配置即时生效（免重启服务）：dataomni mcp 不在内存里缓存权限或口令；在 DataOmni 界面里关掉某个连接的访问，AI 工具下一次调用立刻看不见它。",
        "词法白名单 + 数据库只读事务双保险：每次只收单条读语句（SELECT、无嵌套写的 WITH、SHOW、DESCRIBE、不带 ANALYZE 的 EXPLAIN、VALUES、TABLE），并在数据库级只读事务或只读句柄（含锁死 DuckDB 外部文件访问）内执行，最后一律 ROLLBACK。",
        "隔离宿主机文件系统：MCP 工具表刻意剔除了 export、backup 与 csv-preview，防止原本不能跑 shell 的客户端借道读写用户磁盘上的任意文件。",
        "脱敏调用留痕：每次工具调用都在日志目录的 dataomni-cli.log 写入时间、连接名、工具名、首关键字、长度与 SHA-256 摘要、结果、行数及耗时，绝不记录可能含敏感数据的语句原文。",
      ]),
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "纵深防御建议：为 Agent 搭配只读数据库账号",
        body: "数据库的只读事务无法拦截高权限账号自带的非改表管理函数（如 PostgreSQL 超级用户的 pg_terminate_backend 或 MySQL 的 get_lock()）。建议同时在服务端使用最小权限只读账号。",
      },
    ],
  },
  DocSection {
    id: "tools",
    toc_label: "13 个只读工具清单",
    heading: "内置的 13 个只读 MCP 工具",
    blocks: &[
      DocBlock::Paragraph(
        "所有工具在 tools/list 中均声明 readOnlyHint: true，回包统一采用顶层 \"schema\": 1 的 JSON（dictionary 直接返回适合喂给大模型的整库 Markdown 数据字典）。多余未知字段或以 -- 开头的位置参数均按 usage 错误拦截。常驻 dataomni mcp 会话内自动复用同一条 SSH 隧道，EOF 退出时优雅断开。",
      ),
      DocBlock::Table {
        caption: "dataomni mcp 工具与入参速查",
        headers: &["工具名", "入参", "作用与输出"],
        rows: &[
          &["connections", "无", "列出已开放给 Agent 的非生产连接（不含口令与私钥路径）。"],
          &["dictionary", "connection", "导出整库 Markdown 数据字典（表、列、类型、主外键与本地语义备注）——建议让 Agent 写 SQL 前先调它。"],
          &["schema", "connection, table?, schema?", "列出整库表与视图，或指定单表的列、索引、外键与表级语义说明。"],
          &["ddl", "connection, table, schema?", "读取数据库自身的原始建表定义（SHOW CREATE TABLE、DBMS_METADATA、sqlite_master 等）。"],
          &["query", "connection, sql, limit?, timeout_seconds?, params?", "执行单条只读 SQL，支持命名 :name 绑定参数（默认最多 200 行，上限 10000 行，超限标记 truncated: true）。"],
          &["explain", "connection, sql, timeout_seconds?", "获取估算执行计划（不带 ANALYZE，不实际执行语句）。"],
          &["profile", "connection, sql, limit?, timeout_seconds?", "经内置沙箱 DuckDB 计算列级数据画像（推断类型、空值率、基数、最小/最大、分位数、Top-5 高频值），无需拉回全量明细行。"],
          &["diff", "source, target, format?, timeout_seconds?", "对比两个已开放非生产库的表、列、主键、索引与外键结构差异，返回只读 JSON 或 Markdown 报告。"],
          &["test", "connection", "测试连接可用性；失败时附带 DNS 解析与端口连通性诊断。"],
          &["mongo", "connection, action, ...", "MongoDB 只读操作：collections、find、count、aggregate（拒绝 $out / $merge）、explain、structure。"],
          &["redis", "connection, action, ...", "Redis 只读操作：keyspaces、scan、get，以及经服务端 COMMAND INFO 确认为 readonly 的 command。"],
          &["neo4j", "connection, action, ...", "Neo4j 只读 Cypher：labels 与 run（先经 EXPLAIN 确认类型为 r，再走只读路由事务）。"],
          &["es", "connection, action, ...", "Elasticsearch / OpenSearch 只读查询：indices 与按路径段白名单校验的 GET/HEAD 及搜索类 POST（_search、_count、_msearch）。"],
        ],
      },
    ],
  },
  DocSection {
    id: "troubleshooting",
    toc_label: "常见排错",
    heading: "接入常见问题排错",
    blocks: &[
      DocBlock::Table {
        caption: "MCP 常见现象与解决办法",
        headers: &["现象", "原因与处理方式"],
        rows: &[
          &[
            "connections 返回空数组，或查库报找不到连接",
            "该连接处于「生产」环境、「命令行与 Agent 访问」仍为关闭，或只手工改了 JSON 未经界面保存签发凭证。请在 DataOmni 编辑连接设为「只读」并保存。",
          ],
          &[
            "macOS 首次调用一直等待无响应",
            "系统钥匙串授权弹窗正等在桌面后方。切过去输入开机密码并点击「始终允许」，MCP 进程会继续完成本次调用而不退出。",
          ],
          &[
            "SQL Server 连接无法开启 Agent 访问",
            "SQL Server 缺少会话级只读事务兜底机制，当前版本刻意不在 CLI / MCP 开放 SQL Server。",
          ],
        ],
      },
    ],
  },
];

static ZH_MCP: DocPage = DocPage {
  slug: DocSlug::Mcp,
  locale: Locale::Zh,
  meta_title: "只读 MCP 服务配置指南 — DataOmni 使用文档",
  meta_description: "通过标准输入输出将 Claude Code、Codex、Cursor、Windsurf、Copilot、Gemini CLI 与 Claude Desktop 接入 DataOmni 只读 MCP 服务，确保生产环境物理不可见。",
  breadcrumb_group: "文档",
  title: "只读 MCP 服务配置指南",
  lead: "让 Claude Code、Codex、Cursor、Windsurf、Copilot、Gemini CLI 或 Claude Desktop 直接安全读取你配好的关系库与四大 NoSQL 工作区——配置文件零密码、写语句硬拦截、生产库彻底隐身。",
  toc_heading: "本页目录",
  sections: ZH_MCP_SECTIONS,
};

// ============================================================================
// 3. AGENT & TERMINAL CLI (/docs/cli/ & /zh/docs/cli/)
// ============================================================================

const EN_CLI_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "path-setup",
    toc_label: "PATH & SKILL.md setup",
    heading: "Headless CLI setup and ready-to-use Agent SKILL.md",
    blocks: &[
      DocBlock::Paragraph(
        "Run dataomni <command> directly from your terminal, scripts, or coding agents (Claude Code, Codex, Cursor, Antigravity) without launching or keeping the desktop window open. The explicit prefix dataomni cli <command> remains 100% compatible, while bare dataomni (or dataomni gui) opens the desktop client. Every command shares the exact same keychain grant, production invisibility, and read-only enforcement as dataomni mcp.",
      ),
      DocBlock::Steps(&[
        (
          "Put dataomni on your PATH",
          "Homebrew Cask installs already link $(brew --prefix)/bin/dataomni on macOS, Linux packages install /usr/bin/dataomni, and manual macOS .dmg installs can symlink the binary inside DataOmni.app.",
        ),
        (
          "Open a non-production connection to Read-only in DataOmni",
          "In DataOmni, edit any Development / Test / Staging connection with a saved password, set \"Command line & agent access\" to Read-only, and click Save. Production connections are permanently invisible.",
        ),
        (
          "Install SKILL.md for terminal AI agents",
          "Generate SKILL.md straight from your local binary (always matching your installed version) or curl the hosted copy from https://dataomni.dev/SKILL.md.",
        ),
      ]),
      DocBlock::Code {
        label: "PATH symlink & SKILL.md installation (Claude Code / Codex / Agents)",
        copy_text: Some("mkdir -p ~/.claude/skills/dataomni && dataomni guide > ~/.claude/skills/dataomni/SKILL.md"),
        raw_code: "# macOS manual .dmg symlink (Homebrew Cask already links dataomni automatically):\nln -sf /Applications/DataOmni.app/Contents/MacOS/dataomni ~/.local/bin/dataomni\n\n# Verify headless CLI without launching the GUI\ndataomni --help\ndataomni connections\n\n# Option 1 (recommended): export SKILL.md directly from the installed binary\nmkdir -p ~/.claude/skills/dataomni\ndataomni guide > ~/.claude/skills/dataomni/SKILL.md\n\n# Option 2: download the hosted SKILL.md directly from dataomni.dev\ncurl -fsSL https://dataomni.dev/SKILL.md -o ~/.claude/skills/dataomni/SKILL.md",
      },
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "Windows PowerShell & CMD exit-code note",
        body: "Because dataomni.exe uses the Windows GUI subsystem, interactive shells return immediately unless stdout is piped. Pipe output in scripts (dataomni connections | Out-String) or run cmd /c start /wait to capture $LASTEXITCODE. AI agents already invoke subprocesses via pipes and are unaffected.",
      },
    ],
  },
  DocSection {
    id: "sql-commands",
    toc_label: "SQL, profile, diff & export",
    heading: "Relational inspection, parameterized SQL, DuckDB profiling, diff & export",
    blocks: &[
      DocBlock::Paragraph(
        "Pass connection names or IDs from dataomni connections. Always inspect schema or dictionary before writing SQL so you use real column names. Pass - in place of SQL to read multi-line statements from stdin. Export and backup write only to brand-new target paths and never overwrite existing files.",
      ),
      DocBlock::Table {
        caption: "Core dataomni CLI subcommands",
        headers: &["Subcommand", "Behavior & limits"],
        rows: &[
          &["dataomni connections", "Lists open non-production connections, database types, and SSH tunnel flags."],
          &["dataomni dictionary <conn>", "Outputs a whole-database Markdown data dictionary (tables, columns, types, PKs, FKs, and local semantics notes)."],
          &["dataomni schema <conn> [<table> [--schema S]]", "Lists all tables, views, and routines — or columns, primary key, indexes, foreign keys, and table semantics for one table."],
          &["dataomni ddl <conn> <table> [--schema S]", "Prints the database's native CREATE definition (SHOW CREATE TABLE, sqlite_master, DBMS_METADATA; views only on PostgreSQL)."],
          &["dataomni query <conn> \"<sql>\" [flags]", "Runs one read-only statement (default --limit 200, max 10000; --timeout 30s..600s). Supports safe prepared :name bindings via --param K=V, --param-null K, and --params-json JSON."],
          &["dataomni explain <conn> \"<sql>\"", "Returns structured plan tree (plan.roots) plus raw database text (plan.raw) without running ANALYZE."],
          &["dataomni profile <conn> \"<sql>\" | --file <path>", "Computes column-level profiles (inferred type, null ratio, distinct count, min/max/quantiles, top-5 values) via built-in sandboxed DuckDB (default 10,000 rows, max 100,000; works on live DBs or local Parquet/CSV/JSON files)."],
          &["dataomni diff <src> <dst> [--format json|markdown]", "Read-only schema diff across two open non-production connections (tables, columns, PKs, indexes, FKs) using bulk metadata queries; never runs DDL."],
          &["dataomni export <conn> \"<sql>\" --out <file>", "Streams full read-only results to a new CSV, JSON (--format json), or Apache Parquet (--format parquet) file (default 300s timeout; supports --delimiter, --no-header, --null, --bom)."],
          &["dataomni backup <conn> --out <path> [--database D]", "Runs a read-only logical/file backup (SQLite VACUUM INTO, DuckDB EXPORT DATABASE, pg_dump, mysqldump, mongodump)."],
          &["dataomni csv-preview <file>", "Sniffs delimiter and UTF-8 / GB18030 encoding on a local CSV file, previews rows, and flags ragged lines."],
          &["dataomni test <conn>", "Verifies read-only connectivity; attaches DNS / TCP port diagnosis on network failure."],
          &["dataomni guide / version / --help", "Prints the built-in SKILL.md agent guide, CLI version JSON, or command reference."],
        ],
      },
      DocBlock::Code {
        label: "End-to-end terminal & agent workflow examples",
        copy_text: Some("dataomni schema shop_dev orders"),
        raw_code: "# 1. Inspect real columns before writing SQL\ndataomni schema shop_dev\ndataomni schema shop_dev orders\ndataomni ddl shop_dev orders\n\n# 2. Run parameterized read-only SQL (parameter values are bound natively and never logged)\ndataomni query shop_dev \"SELECT id, status, total FROM orders WHERE customer_id = :cid AND deleted_at IS :del\" \\\n  --param cid=1042 --param-null del --limit 50\n\n# 3. Profile distributions via built-in DuckDB without pulling raw rows\ndataomni profile shop_dev \"SELECT * FROM orders\" --limit 10000\n\n# 4. Export to Apache Parquet and compare staging vs dev schemas\ndataomni export shop_dev - --out /tmp/orders.parquet --format parquet < report.sql\ndataomni profile --file /tmp/orders.parquet\ndataomni diff shop_dev shop_staging --format markdown",
      },
    ],
  },
  DocSection {
    id: "workspace-commands",
    toc_label: "Mongo, Redis, Neo4j & ES",
    heading: "Dedicated NoSQL workspace commands",
    blocks: &[
      DocBlock::Paragraph(
        "Non-relational engines use purpose-built read-only subcommands with engine-native syntax and server/parser read-only verification:",
      ),
      DocBlock::Code {
        label: "Read-only MongoDB, Redis, Neo4j, and Elasticsearch calls",
        copy_text: None,
        raw_code: "# MongoDB (mongosh filter/pipeline syntax; $out and $merge refused at parse time)\ndataomni mongo app_mongo collections\ndataomni mongo app_mongo find shop.orders '{ status: \"paid\" }' --sort '{ createdAt: -1 }' --limit 20\ndataomni mongo app_mongo aggregate shop.orders '[{ $group: { _id: \"$status\", n: { $sum: 1 } } }]'\n\n# Redis (--db N goes before the operation; command checks server COMMAND INFO readonly)\ndataomni redis cache_dev keyspaces\ndataomni redis cache_dev --db 0 scan \"session:*\" --limit 50\ndataomni redis cache_dev --db 0 get \"session:42\"\n\n# Neo4j (EXPLAIN verifies query type == r before running in a read-mode Bolt transaction)\ndataomni neo4j graph_dev labels\ndataomni neo4j graph_dev run \"MATCH (u:User)-[:BOUGHT]->(p) RETURN u.id, p.sku LIMIT 25\"\n\n# Elasticsearch / OpenSearch (GET/HEAD or search POST such as _search, _count, _msearch)\ndataomni es search_dev indices\ndataomni es search_dev request POST /orders/_search '{\"size\":10,\"query\":{\"match_all\":{}}}'",
      },
    ],
  },
  DocSection {
    id: "formatting",
    toc_label: "Formatting & highlighting",
    heading: "Terminal pretty-printing, ANSI highlighting & global display flags",
    blocks: &[
      DocBlock::Paragraph(
        "Standard output and standard error detect TTY independently. In interactive terminals, JSON is automatically 2-space pretty-printed and syntax-highlighted with ANSI colors, and Markdown outputs (dictionary and diff --format markdown) receive subtle heading, inline-code, and table-rule highlighting. Whenever stdout or stderr is piped, redirected, or captured by an AI agent, output switches automatically to compact single-line plain JSON with zero ANSI escape codes.",
      ),
      DocBlock::Table {
        caption: "Global display flags and environment variables",
        headers: &["Flag / Env", "Effect"],
        rows: &[
          &["(default in TTY)", "2-space pretty-printed JSON + ANSI syntax highlighting (Markdown lightly highlighted)."],
          &["(default in pipe / script / agent)", "Compact single-line plain JSON, zero ANSI codes — works cleanly with jq and JSON parsers."],
          &["--pretty", "Force 2-space pretty-printed JSON even when stdout is piped (e.g. | less -R or > file.json)."],
          &["--compact / -c", "Force compact single-line JSON even in an interactive terminal (ideal for copy-paste)."],
          &["--color auto|always|never", "Override ANSI color detection explicitly; --no-color is a shorthand for --color never."],
          &["NO_COLOR / CLICOLOR / CLICOLOR_FORCE / TERM=dumb", "Standard terminal color conventions respected automatically under --color auto."],
        ],
      },
      DocBlock::Code {
        label: "Combining formatting flags with less and jq",
        copy_text: Some("dataomni schema shop_dev --pretty --color always | less -R"),
        raw_code: "# Page long highlighted output in less -R\ndataomni schema shop_dev --pretty --color always | less -R\n\n# Copy compact monochrome JSON in a terminal\ndataomni connections -c --no-color\n\n# Pipe directly into jq (automatically compact & uncolored)\ndataomni schema shop_dev | jq '.objects | length'",
      },
    ],
  },
  DocSection {
    id: "exit-codes",
    toc_label: "JSON schema, SSH & exit codes",
    heading: "JSON contract (schema: 1), SSH tunnel reuse & exit codes 0–4",
    blocks: &[
      DocBlock::Paragraph(
        "All JSON responses include top-level \"schema\": 1. Errors are written to stderr as {\"schema\": 1, \"error\": {\"kind\": \"...\", \"message\": \"...\"}} with deterministic exit codes so agents know whether to fix syntax or stop immediately:",
      ),
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "High-frequency queries over SSH jump hosts: prefer dataomni mcp",
        body: "Each standalone dataomni <command> opens and gracefully closes its own SSH tunnel. Running 10+ standalone CLI commands back-to-back in a few seconds can trigger jump-host SSH rate limits (such as OpenSSH PerSourcePenalties or MaxStartups). For multi-step agent loops over SSH tunnels, use dataomni mcp — it reuses a single persistent SSH tunnel for the whole session.",
      },
      DocBlock::Table {
        caption: "Deterministic CLI exit codes",
        headers: &["Exit code", "Category", "Agent / caller action"],
        rows: &[
          &["0", "Success", "Parse stdout JSON (or Markdown from dictionary / diff --format markdown)."],
          &["1", "Database error or timeout", "Read error.message and fix the SQL / column names or tighten filters."],
          &["2", "Usage or not_found", "Fix CLI flags, or ask the user to open the connection to agents in DataOmni."],
          &["3", "Refused by read-only gate", "Stop immediately — write statements, DDL, multi-statements, and $out/$merge are never allowed and no CLI flag overrides this."],
          &["4", "Unavailable / keychain waiting", "Network unreachable (inspect error.diagnosis) or macOS Keychain prompt waiting 10s for \"Always Allow\"."],
        ],
      },
    ],
  },
];

static EN_CLI: DocPage = DocPage {
  slug: DocSlug::Cli,
  locale: Locale::En,
  meta_title: "Agent & Terminal CLI Reference — DataOmni Docs",
  meta_description: "Reference for headless dataomni CLI and SKILL.md: read-only SQL, DuckDB profile, schema diff, Parquet export, NoSQL, TTY highlighting, and exit codes.",
  breadcrumb_group: "Docs",
  title: "Agent & terminal CLI reference",
  lead: "Run headless read-only SQL, whole-database Markdown dictionaries, DuckDB column profiling, schema diffs, Parquet exports, and MongoDB / Redis / Neo4j / Elasticsearch queries straight from your terminal or AI agent.",
  toc_heading: "On this page",
  sections: EN_CLI_SECTIONS,
};

const ZH_CLI_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "path-setup",
    toc_label: "安装 PATH 与 SKILL.md",
    heading: "无头命令行接入与一键安装 Agent SKILL.md",
    blocks: &[
      DocBlock::Paragraph(
        "在终端、巡检脚本或具备命令执行能力的 AI Agent（Claude Code、Codex、Cursor、Antigravity）中可直接运行 dataomni <子命令>——完全无头执行，不拉起也不依赖桌面界面窗口。旧前缀 dataomni cli <子命令> 保持 100% 兼容，无参数运行 dataomni（或 dataomni gui）仍正常打开桌面端。所有命令与 dataomni mcp 共用同一套钥匙串开放凭证、生产库隐身与只读防线。",
      ),
      DocBlock::Steps(&[
        (
          "确认 dataomni 已在 PATH 中",
          "macOS 通过 Homebrew Cask 安装会自动软链 $(brew --prefix)/bin/dataomni；Linux 包默认安装到 /usr/bin/dataomni；手动拖 .dmg 安装可自行软链应用包内二进制。",
        ),
        (
          "在 DataOmni 中为非生产连接开启「只读」访问",
          "编辑任意开发 / 测试 / 预发连接，勾选「保存密码」并将「命令行与 Agent 访问」设为「只读」后保存。生产环境（PROD）连接永久不可见。",
        ),
        (
          "一键安装给 AI Agent 用的 SKILL.md",
          "推荐直接用本机二进制导出与当前版本严格一致的 SKILL.md，也可直接从 https://dataomni.dev/SKILL.md 下载托管版本。",
        ),
      ]),
      DocBlock::Code {
        label: "macOS 软链配置与 SKILL.md 安装（Claude Code / Codex / Agent）",
        copy_text: Some("mkdir -p ~/.claude/skills/dataomni && dataomni guide > ~/.claude/skills/dataomni/SKILL.md"),
        raw_code: "# macOS 手动拖 .dmg 安装时补建软链（Homebrew Cask 安装已自动处理）：\nln -sf /Applications/DataOmni.app/Contents/MacOS/dataomni ~/.local/bin/dataomni\n\n# 验证无头命令行（不会弹出桌面窗口）\ndataomni --help\ndataomni connections\n\n# 方式一（推荐）：直接由本地二进制导出与当前版本严格一致的 SKILL.md\nmkdir -p ~/.claude/skills/dataomni\ndataomni guide > ~/.claude/skills/dataomni/SKILL.md\n\n# 方式二：直接从官网拉取最新托管版 SKILL.md\ncurl -fsSL https://dataomni.dev/SKILL.md -o ~/.claude/skills/dataomni/SKILL.md",
      },
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "Windows 终端退出码小贴士",
        body: "Windows 发布版主程序采用 GUI 子系统，在 PowerShell / CMD 直接手敲命令时终端不会等待进程结束；若脚本里需要读取 $LASTEXITCODE，请接管道（如 dataomni connections | Out-String）或用 cmd /c start /wait。AI Agent 本身经管道调用子进程，完全不受影响。",
      },
    ],
  },
  DocSection {
    id: "sql-commands",
    toc_label: "查询、画像、比对与导出",
    heading: "关系库结构、参数化查询、DuckDB 画像、跨库 Diff 与导出",
    blocks: &[
      DocBlock::Paragraph(
        "所有命令均支持传连接名或连接 ID；写 SQL 前建议先查 schema 或 dictionary 确认真实列名。长 SQL 可用 - 代替语句参数从标准输入读取。export 与 backup 只允许写入全新的目标路径，绝不覆盖磁盘上已有文件。",
      ),
      DocBlock::Table {
        caption: "dataomni 核心子命令速查",
        headers: &["子命令", "功能与默认边界"],
        rows: &[
          &["dataomni connections", "列出当前已对命令行开放的非生产连接、引擎类型与 SSH 隧道标记。"],
          &["dataomni dictionary <连接>", "输出整库 Markdown 数据字典（表、列、类型、主外键与本地语义备注），最适合写 SQL 前先读一遍。"],
          &["dataomni schema <连接> [<表> [--schema S]]", "列出整库表、视图与存储过程，或读取指定单表的列、主键、索引、外键及表级语义说明。"],
          &["dataomni ddl <连接> <表> [--schema S]", "获取数据库自身保存的对象定义原文（SHOW CREATE TABLE、sqlite_master、DBMS_METADATA；PostgreSQL 仅视图有原文）。"],
          &["dataomni query <连接> \"<SQL>\" [参数]", "执行单条只读语句（默认 --limit 200，上限 10000；--timeout 30–600 秒）。支持安全预处理绑定参数：--param K=V、--param-null K、--params-json JSON（参数值永不进日志）。"],
          &["dataomni explain <连接> \"<SQL>\"", "返回结构化执行计划树（plan.roots）及数据库原始 EXPLAIN 文本（禁止 ANALYZE，不实际执行语句）。"],
          &["dataomni profile <连接> \"<SQL>\" | --file <路径>", "通过内置沙箱 DuckDB 计算列级数据画像（推断类型、空值率、基数、最小/最大、分位数、Top-5 高频值；默认采样 1 万行、上限 10 万行，同时支持离线分析本地 Parquet/CSV/JSON 文件）。"],
          &["dataomni diff <源连接> <目标连接> [--format json|markdown]", "批量元数据只读比对两个已开放非生产库的表、列、主键、索引与外键差异，只出报告绝不执行 DDL。"],
          &["dataomni export <连接> \"<SQL>\" --out <文件>", "在只读事务内流式导出完整结果到新 CSV、JSON（--format json）或 Apache Parquet（--format parquet）文件（默认 300 秒；支持 --delimiter / --no-header / --null / --bom）。"],
          &["dataomni backup <连接> --out <路径> [--database D]", "按应用内置逻辑做只读备份（SQLite VACUUM INTO、DuckDB EXPORT DATABASE、pg_dump、mysqldump、mongodump）。"],
          &["dataomni csv-preview <文件>", "离线嗅探本地 CSV 分隔符与编码（UTF-8 / GB18030），预览数据并标出字段数异常行。"],
          &["dataomni test <连接>", "测试只读连通性；网络失败时在 error.diagnosis 返回域名解析与端口探测步骤。"],
          &["dataomni guide / version / --help", "打印内置 Agent SKILL.md 指南、版本 JSON 或完整命令行帮助。"],
        ],
      },
      DocBlock::Code {
        label: "端到端查库、画像、导出与比对示例",
        copy_text: Some("dataomni schema shop_dev orders"),
        raw_code: "# 1. 先查真实表结构与原生建表语句，避免猜错列名\ndataomni schema shop_dev\ndataomni schema shop_dev orders\ndataomni ddl shop_dev orders\n\n# 2. 命名绑定参数安全查询（参数走驱动预处理，值永不写入审计日志）\ndataomni query shop_dev \"SELECT id, status, total FROM orders WHERE customer_id = :cid AND deleted_at IS :del\" \\\n  --param cid=1042 --param-null del --limit 50\n\n# 3. 用内置 DuckDB 做列级统计画像，无需拉回全量明细\ndataomni profile shop_dev \"SELECT * FROM orders\" --limit 10000\n\n# 4. 导出 Apache Parquet、离线画像本地文件、跨环境对比表结构\ndataomni export shop_dev - --out /tmp/orders.parquet --format parquet < report.sql\ndataomni profile --file /tmp/orders.parquet\ndataomni diff shop_dev shop_staging --format markdown",
      },
    ],
  },
  DocSection {
    id: "workspace-commands",
    toc_label: "四大 NoSQL 只读命令",
    heading: "MongoDB、Redis、Neo4j 与 Elasticsearch 只读子命令",
    blocks: &[
      DocBlock::Paragraph(
        "四类非关系型数据库各自提供符合原生习惯的只读子命令，每一种都有独立的服务端/词法双重只读校验：",
      ),
      DocBlock::Code {
        label: "四大非关系型工作区 CLI 示例",
        copy_text: None,
        raw_code: "# MongoDB（沿用 mongosh 写法，解析期直接拒绝含 $out / $merge 的聚合管道）\ndataomni mongo app_mongo collections\ndataomni mongo app_mongo find shop.orders '{ status: \"paid\" }' --sort '{ createdAt: -1 }' --limit 20\ndataomni mongo app_mongo aggregate shop.orders '[{ $group: { _id: \"$status\", n: { $sum: 1 } } }]'\n\n# Redis（--db N 放在操作名前；command 先查服务端 COMMAND INFO 确认带 readonly 旗标）\ndataomni redis cache_dev keyspaces\ndataomni redis cache_dev --db 0 scan \"session:*\" --limit 50\ndataomni redis cache_dev --db 0 get \"session:42\"\n\n# Neo4j（先发 EXPLAIN 确认服务端判定查询类型为 r，再在只读路由事务内运行）\ndataomni neo4j graph_dev labels\ndataomni neo4j graph_dev run \"MATCH (u:User)-[:BOUGHT]->(p) RETURN u.id, p.sku LIMIT 25\"\n\n# Elasticsearch / OpenSearch（按路径段白名单放行 GET/HEAD 及 _search、_count、_msearch 等查询）\ndataomni es search_dev indices\ndataomni es search_dev request POST /orders/_search '{\"size\":10,\"query\":{\"match_all\":{}}}'",
      },
    ],
  },
  DocSection {
    id: "formatting",
    toc_label: "终端格式化与语法高亮",
    heading: "TTY 自动美化高亮与全局显示开关",
    blocks: &[
      DocBlock::Paragraph(
        "标准输出与标准错误各自独立检测是否为交互终端（TTY）：人在终端直接运行时自动 2 空格缩进并启用 ANSI 语法高亮，dictionary 与 diff --format markdown 也会做标题、行内代码与表格分隔线轻度高亮；一旦进入管道、重定向或 AI Agent 子进程捕获，则自动退回单行紧凑无色 JSON，零 ANSI 转义符，完美兼容 jq 与脚本解析。",
      ),
      DocBlock::Table {
        caption: "全局输出格式开关与环境变量",
        headers: &["参数 / 环境变量", "效果"],
        rows: &[
          &["交互终端（TTY）默认", "JSON 自动 2 空格缩进 + ANSI 语法高亮；Markdown 轻度高亮。"],
          &["管道 / 脚本 / Agent 默认", "单行紧凑纯 JSON，零 ANSI 控制符，直接喂给 jq 或程序解析。"],
          &["--pretty", "在管道或重定向下强制 2 空格缩进美化（适合配合 | less -R 或导出易读 JSON）。"],
          &["--compact / -c", "在交互终端下强制单行紧凑输出（方便整行复制）。"],
          &["--color auto|always|never", "显式控制 ANSI 色彩；--no-color 等价于 --color never。"],
          &["NO_COLOR / CLICOLOR / CLICOLOR_FORCE / TERM=dumb", "在 --color auto 下自动遵循标准终端色彩环境变量规范。"],
        ],
      },
      DocBlock::Code {
        label: "搭配 less -R 分页与 jq 管道的常用写法",
        copy_text: Some("dataomni schema shop_dev --pretty --color always | less -R"),
        raw_code: "# 长输出带颜色交给 less 分页浏览\ndataomni schema shop_dev --pretty --color always | less -R\n\n# 终端内强制单行无色输出\ndataomni connections -c --no-color\n\n# 直接管道给 jq（自动紧凑无色，无需加任何额外参数）\ndataomni schema shop_dev | jq '.objects | length'",
      },
    ],
  },
  DocSection {
    id: "exit-codes",
    toc_label: "JSON 契约、SSH 与退出码",
    heading: "JSON 输出契约（schema: 1）、SSH 隧道与退出码 0–4",
    blocks: &[
      DocBlock::Paragraph(
        "所有 JSON 结果顶层均包含 \"schema\": 1。报错统一写向标准错误 {\"schema\": 1, \"error\": {\"kind\": \"...\", \"message\": \"...\"}}，并通过 0–4 退出码明确区分「修正 SQL 重试」与「权限红线切勿重试」：",
      ),
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "经 SSH 跳板机高频连续查库：建议使用 dataomni mcp",
        body: "每次独立执行 dataomni <子命令> 都会建立并优雅关闭一条短生命周期 SSH 隧道；若在数秒内连续发起 10 次以上独立 CLI 调用，可能触发跳板机侧的 SSH 频控保护（如 OpenSSH PerSourcePenalties 或 MaxStartups）。对于需要多轮连续查库的 AI Agent，请改用 dataomni mcp，单次常驻会话复用同一条 SSH 隧道。",
      },
      DocBlock::Table {
        caption: "CLI 退出码（0–4）精确语义",
        headers: &["退出码", "类别", "调用方 / Agent 应对动作"],
        rows: &[
          &["0", "执行成功", "正常解析 stdout JSON（或 dictionary / diff --format markdown 的 Markdown）。"],
          &["1", "数据库执行报错或超时", "阅读 error.message，修正表名、列名、语法或收窄查询条件。"],
          &["2", "参数用法错误或连接不存在", "修正命令行参数；连接报 not_found 说明未创建或未对 Agent 开放。"],
          &["3", "被只读权限门拒绝", "立即停止换花样重试——属于写操作、DDL 或多条语句，命令行没有任何参数可放宽该限制。"],
          &["4", "连接不可达或缺凭据授权", "检查网络（看 error.diagnosis）或提示用户在系统钥匙串弹窗点「始终允许」（CLI 最多等 10 秒）。"],
        ],
      },
    ],
  },
];

static ZH_CLI: DocPage = DocPage {
  slug: DocSlug::Cli,
  locale: Locale::Zh,
  meta_title: "命令行与 Agent 接口参考 — DataOmni 使用文档",
  meta_description: "dataomni 无头命令行与 SKILL.md 参考：只读参数化 SQL、DuckDB 画像、跨库结构比对、Parquet 导出、四大 NoSQL、终端高亮与 0–4 退出码。",
  breadcrumb_group: "文档",
  title: "命令行与 Agent 接口参考",
  lead: "无需启动桌面窗口，直接在终端或 Claude Code、Codex 等 AI Agent 中复用已保存的连接，运行只读参数化 SQL、整库数据字典、DuckDB 列画像、结构比对、Parquet 导出与四大 NoSQL 查询。",
  toc_heading: "本页目录",
  sections: ZH_CLI_SECTIONS,
};

// ============================================================================
// 4. DATABASES & MATRIX (/docs/databases/ & /zh/docs/databases/)
// ============================================================================

const EN_DATABASES_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "relational-guides",
    toc_label: "Relational & analytical notes",
    heading: "Connection notes for relational and analytical engines",
    blocks: &[
      DocBlock::Paragraph(
        "DataOmni ships seven native relational drivers (PostgreSQL, MySQL, SQLite, SQL Server, Oracle, DuckDB, and ClickHouse) plus dedicated connection presets and dialect adaptations for MariaDB, TiDB, OceanBase, CockroachDB, and openGauss.",
      ),
      DocBlock::Table {
        caption: "Dialect-specific connection and editing details",
        headers: &["Engine", "Connection & dialect specifics"],
        rows: &[
          &[
            "Oracle (19c / 21c / 23ai)",
            "Oracle Instant Client (Basic Light) is bundled inside every installer (Linux .deb/.rpm pulls libaio). Put the Easy Connect Service Name (e.g. FREEPDB1) in Database; encrypt via SSH tunnel; terminate PL/SQL blocks with a standalone / line; empty grid strings follow Oracle semantics and store NULL.",
          ],
          &[
            "PostgreSQL, CockroachDB & openGauss",
            "Full PG 16/17 support including schema diff, ER codegen, and pg_dump. CockroachDB defaults to port 26257 + Prefer TLS. openGauss accounts using default sha256 password storage authenticate seamlessly via DataOmni's built-in local PBKDF2-HMAC-SHA1 relay while keeping TLS intact.",
          ],
          &[
            "MySQL, MariaDB, TiDB & OceanBase",
            "New MySQL connections default to Require TLS — switch to Disable or Prefer TLS for local unencrypted Docker servers or MySQL 5.7 (no ECDHE ciphers). For OceanBase (MySQL mode), select MySQL on port 2881 with username formatted as user@tenant (e.g. root@test).",
          ],
          &[
            "SQL Server (2019 / 2022)",
            "Pure-Rust TDS over rustls (defaults to Prefer TLS for self-signed certs). Supports SSMS GO batch separators, estimated and actual SHOWPLAN_XML trees, and live @@TRANCOUNT state tracking.",
          ],
          &[
            "SQLite & DuckDB 2.0",
            "Open local files (.db, .sqlite, .duckdb, .ddb) or :memory: directly, including Windows paths > 260 characters. DuckDB bundles parquet and json extensions so SELECT * FROM 'events.parquet' streams without loading the whole file; disconnect in DataOmni before opening the same .duckdb file in another process.",
          ],
          &[
            "ClickHouse",
            "Connects over HTTP (8123 plain / 8443 TLS, not native port 9000). Supports EXPLAIN indexes = 1 granule inspection and guarded single-row MergeTree edits verified via _part and _part_offset before and after mutation.",
          ],
        ],
      },
    ],
  },
  DocSection {
    id: "workspace-guides",
    toc_label: "Document, KV, Graph & Search",
    heading: "Dedicated workspaces beyond SQL tables",
    blocks: &[
      DocBlock::Paragraph(
        "Instead of pretending documents, keys, graphs, or inverted indices are flat SQL tables, DataOmni switches into a tailored workspace when you connect to MongoDB, Redis, Neo4j, or Elasticsearch / OpenSearch:",
      ),
      DocBlock::Bullets(&[
        "MongoDB (7.0 / 8.0 / Atlas SRV): browse collections, filter and aggregate with mongosh object syntax, edit documents in Extended JSON while preserving ObjectId / Decimal128 / Date types, inspect sampled field schemas, and run mongodump backups.",
        "Redis (7.x / Valkey / Sentinel / Cluster): cursor-based SCAN keyspace browser (never fires KEYS *), dedicated editors for string, hash, list, set, zset, and stream types, live Pub/Sub subscription tabs, and command console.",
        "Neo4j (5.x / 2025+ Bolt 4.4–5.8): interactive force-directed node/relationship canvas, property table view, Cypher parameter binding, and visual PROFILE / EXPLAIN operator trees.",
        "Elasticsearch 8.x & OpenSearch 2.x: multi-request REST console (GET /path + JSON body), index health/mappings/aliases explorer, visual query builder, and bulk/delete confirmation guardrails.",
      ]),
    ],
  },
  DocSection {
    id: "matrix",
    toc_label: "18-server smoke matrix",
    heading: "Live-database smoke test compatibility matrix",
    blocks: &[
      DocBlock::Paragraph(
        "Every entry below is backed by automated integration tests against real database instances in src-tauri/tests/*_smoke.rs. Counts report passed / skipped / fixture-unsupported cases:",
      ),
      DocBlock::CompatibilityMatrix,
    ],
  },
];

static EN_DATABASES: DocPage = DocPage {
  slug: DocSlug::Databases,
  locale: Locale::En,
  meta_title: "Database Guides & Compatibility Matrix — DataOmni Docs",
  meta_description: "Connection guides and real-database smoke test matrix across 17 engines: PostgreSQL, MySQL, Oracle, SQL Server, DuckDB, ClickHouse, MongoDB, and Redis.",
  breadcrumb_group: "Docs",
  title: "Database guides & compatibility matrix",
  lead: "Practical connection specifics — bundled Oracle Instant Client, openGauss sha256 relay, DuckDB Parquet streaming, ClickHouse HTTP mutations — plus the full 18-server smoke-test ledger.",
  toc_heading: "On this page",
  sections: EN_DATABASES_SECTIONS,
};

const ZH_DATABASES_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "relational-guides",
    toc_label: "关系库与分析库连接要点",
    heading: "关系型与分析型数据库连接要点",
    blocks: &[
      DocBlock::Paragraph(
        "DataOmni 内置七套原生关系型驱动（PostgreSQL、MySQL、SQLite、SQL Server、Oracle、DuckDB、ClickHouse），并针对 MariaDB、TiDB、OceanBase、CockroachDB 与 openGauss 做了专门的连接预设与目录方言适配。",
      ),
      DocBlock::Table {
        caption: "各关系型引擎连接与使用细节",
        headers: &["数据库", "连接配置与方言注意事项"],
        rows: &[
          &[
            "Oracle（19c / 21c / 23ai）",
            "安装包已随包集成 Oracle Instant Client（Basic Light），免配环境变量（Linux deb/rpm 自动依赖 libaio）。「数据库」一栏填 Easy Connect 服务名（如 FREEPDB1）；加密传输请勾 SSH 隧道；PL/SQL 块以单独一行 / 结尾；遵循 Oracle 语义将空字符串存为 NULL。",
          ],
          &[
            "PostgreSQL、CockroachDB 与 openGauss",
            "完整支持 PG 16/17 结构比对、ER 代码生成与 pg_dump。CockroachDB 独立入口默认填 26257 端口与「优先使用 TLS」。openGauss 默认只存 sha256 口令的账号由内置本地转发口（PBKDF2-HMAC-SHA1）自动完成握手并保留 TLS 校验。",
          ],
          &[
            "MySQL、MariaDB、TiDB 与 OceanBase",
            "新建 MySQL 连接默认「要求 TLS」——连本地未开加密的实例或旧版 MySQL 5.7（无 ECDHE 套件）请改为「禁用」或「优先使用 TLS」。连 OceanBase（MySQL 模式）选 MySQL、端口填 2881、用户名按 用户@租户（如 root@test）填写。",
          ],
          &[
            "SQL Server（2019 / 2022）",
            "纯 Rust TDS 驱动配 rustls（默认「优先使用 TLS」适配默认自签证书）。支持直接运行带 GO 分批的 SSMS 脚本、估算与真实 SHOWPLAN_XML 执行计划树，以及 @@TRANCOUNT 实时事务状态同步。",
          ],
          &[
            "SQLite 与 DuckDB 2.0",
            "可通过「打开数据库文件…」直连本地文件或 :memory:，完整支持 Windows 超 260 字符长路径。DuckDB 内置 parquet 与 json 扩展，可直接流式查询 SELECT * FROM 'events.parquet'；注意连接期间 DuckDB 独占文件锁，需在外部程序打开前请先点断开。",
          ],
          &[
            "ClickHouse",
            "连接 HTTP 接口（明文默认 8123，TLS 默认 8443，勿填原生协议 9000 端口）。支持 EXPLAIN indexes = 1 跳数索引粒度分析，以及基于 MergeTree _part + _part_offset 前后双重核对的单行安全修改。",
          ],
        ],
      },
    ],
  },
  DocSection {
    id: "workspace-guides",
    toc_label: "四大非关系型专用工作区",
    heading: "文档、键值、图与搜索专用工作区",
    blocks: &[
      DocBlock::Paragraph(
        "不把非关系型数据硬塞进二维 SQL 表格——连上 MongoDB、Redis、Neo4j 或 Elasticsearch / OpenSearch 后，工作台会自动切换为贴合各自数据模型的专用界面：",
      ),
      DocBlock::Bullets(&[
        "MongoDB（7.0 / 8.0 / Atlas SRV）：集合浏览、mongosh 语法筛选与聚合管道、保留 ObjectId / Decimal128 / Date 类型的 Extended JSON 文档编辑、采样字段结构统计与 mongodump 备份。",
        "Redis（7.x / Valkey / Sentinel / Cluster）：基于游标的分页 SCAN 键浏览（绝不发 KEYS * 阻塞主线程），String / Hash / List / Set / ZSet / Stream 六种类型专用编辑器、实时 Pub/Sub 订阅与命令台。",
        "Neo4j（5.x / 2025+ Bolt 4.4–5.8）：节点与关系力导向交互画布、属性表格切换、Cypher 参数绑定，以及可视化 PROFILE / EXPLAIN 算子计划树。",
        "Elasticsearch 8.x & OpenSearch 2.x：多请求 REST 控制台（GET /path + JSON 请求体）、索引健康/映射/别名面板、可视化条件构造器与批量写删二次确认。",
      ]),
    ],
  },
  DocSection {
    id: "matrix",
    toc_label: "18 种服务端真库兼容矩阵",
    heading: "18 种服务端真库冒烟测试兼容性矩阵",
    blocks: &[
      DocBlock::Paragraph(
        "下表每一行均由 src-tauri/tests/*_smoke.rs 连真实数据库实例自动化跑出，计数依次代表「通过 / 跳过 / 夹具不支持」：",
      ),
      DocBlock::CompatibilityMatrix,
    ],
  },
];

static ZH_DATABASES: DocPage = DocPage {
  slug: DocSlug::Databases,
  locale: Locale::Zh,
  meta_title: "数据库连接指南与兼容性矩阵 — DataOmni 使用文档",
  meta_description: "涵盖 17 种数据库的连接配置要点与真库冒烟测试矩阵：内置 Oracle Instant Client、openGauss sha256 转发、DuckDB 直查 Parquet，以及四大非关系型专用工作区。",
  breadcrumb_group: "文档",
  title: "数据库连接指南与兼容性矩阵",
  lead: "汇总 Oracle 内置驱动免配、openGauss sha256 转发、DuckDB 流式查 Parquet、ClickHouse HTTP 接口等实战连接细节，并附完整 18 项真库冒烟兼容台账。",
  toc_heading: "本页目录",
  sections: ZH_DATABASES_SECTIONS,
};

// ============================================================================
// 5. CHANGELOG (/changelog/ & /zh/changelog/)
// ============================================================================

const EN_CHANGELOG_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "v1-3-0",
    toc_label: "v1.3.0 (2026-11-21)",
    heading: "v1.3.0 — 2026-11-21",
    blocks: &[
      DocBlock::Paragraph(
        "Introduces the built-in sandboxed DuckDB analytics engine, multi-engine read-only diagnostics across five data models, top-level CLI with TTY highlighting, and restrained in-app AI assistance.",
      ),
      DocBlock::Bullets(&[
        "Sandboxed in-memory DuckDB analytics & native charts: profile columns (inferred type, null/empty/padded whitespace counts, cardinality, quantiles, histograms, time sparklines, top-5 values) across SQL results, bounded table samples (1k / 5k / 20k rows inheriting active filters), MongoDB collections, and Elasticsearch hits; attach up to 4 local CSV/Parquet/JSON files for JOIN/EXCEPT reconciliation, run secondary SQL / PIVOT queries, export Apache Parquet, and explore local files ad-hoc (capped at 100k rows / 512 MB with enable_external_access = false and lock_configuration = true). Native SVG charts expand with stacked bar, area, scatter, 24×24 heatmap, Pearson correlation bipolar heatmap, 2D Markdown export, explicit DuckDB aggregation past 200 rows, and themed SVG / 2× PNG export.",
        "Multi-engine read-only diagnostics: non-blocking Redis SCAN prefix memory / TTL / BigKey inspection, MongoDB $sample nested schema drift & $indexStats unused index detection, Elasticsearch size: 0 native aggregation templates & mapping/shard health checks, and Neo4j O(1) label/relationship overviews with bounded supernode topology analysis.",
        "Top-level headless CLI & MCP ergonomics: call dataomni <command> directly (keeping dataomni cli as an alias) with automatic pretty/colored TTY JSON & Markdown and compact pipe JSON; adds profile (with --file on CLI and physical local-file isolation on MCP), Parquet export, typed --param / --param-null / --params-json bindings, bulk 3-query schema diff, per-connection local semantics (semantics/<id>.md), shared SSH tunnels in MCP sessions, and an in-app CLI & Agent setup guide with read-only audit log viewer.",
        "Restrained in-app AI (behind the ai Cargo feature, zero raw rows sent): adds cancellation, connection testing, multi-preset configs, proxy-free local Ollama support, and a STAGING/PROD local-model guard; adds inspectable SQL generation/rewrite, error diagnosis, EXPLAIN bottleneck reading, ER semantics drafting, and redacted-stats-only profile health insights.",
        "Connection & Keychain polish: saved password status indicator with explicit preserve/clear controls when editing connections, plus a fix for macOS Keychain -25244 overwrite errors.",
      ]),
    ],
  },
  DocSection {
    id: "v1-2-2",
    toc_label: "v1.2.2 (2026-11-19)",
    heading: "v1.2.2 — 2026-11-19",
    blocks: &[
      DocBlock::Paragraph(
        "Delivers Ed25519-signed in-app upgrades with install-channel awareness, optional compile-time driver decoupling, and bilingual privacy & compliance documentation.",
      ),
      DocBlock::Bullets(&[
        "Ed25519-signed in-app upgrades & channel guardrails: standalone macOS (.dmg) and Windows (.msi / -setup.exe) installs now support user-confirmed in-app upgrades via tauri-plugin-updater and latest.json (three explicit steps: banner Upgrade -> review notes & verify Ed25519 signature -> Restart & install now, never silent background download or forced restart), automatically guarding open transactions, active imports/exports, and SQL drafts; Homebrew Cask installs expose a one-click brew upgrade --cask dataomni copy button while blocking package-managed self-overwrite.",
        "Compile-time Oracle driver feature split: decoupled the ODPI-C Oracle driver behind a default-enabled oracle Cargo feature (default = [\"ai\", \"oracle\"]), enabling zero-dynamic-library minimal builds with --no-default-features while preserving full cross-version connections.json compatibility.",
        "Release automation & privacy transparency: upgraded all GitHub Actions workflows to native Node 24 majors (zero deprecation warnings), added automated notarized Homebrew Tap verification on release publish, and launched bilingual Privacy Policy & Security Architecture pages (/privacy/ and /zh/privacy/).",
      ]),
    ],
  },
  DocSection {
    id: "v1-2-1",
    toc_label: "v1.2.1 (2026-11-18)",
    heading: "v1.2.1 — 2026-11-18",
    blocks: &[
      DocBlock::Paragraph(
        "Brings official Apple Developer ID code signing, Apple notarization, and Homebrew Cask installation to macOS builds.",
      ),
      DocBlock::Bullets(&[
        "macOS signing & notarization: both DataOmni.app and the outer .dmg are now signed with an Apple Developer ID, notarized via Apple notarytool, and stapled with offline tickets — drag to /Applications and launch directly without Gatekeeper quarantine workarounds, and preserve Keychain team authorization across future updates.",
        "Official Homebrew Cask tap: install or upgrade via brew install --cask dataomni-rs/tap/dataomni, which installs DataOmni.app and automatically links the read-only dataomni CLI / MCP binary onto $(brew --prefix)/bin/dataomni.",
        "Bundle & CI verification gates: aligned macOS LSMinimumSystemVersion to 12.0 (Monterey) and added an automated release gate asserting byte-for-byte SHA-256 preservation of bundled Oracle Instant Client libraries alongside stapler and spctl verification.",
      ]),
    ],
  },
  DocSection {
    id: "v1-2-0",
    toc_label: "v1.2.0 (2026-11-18)",
    heading: "v1.2.0 — 2026-11-18",
    blocks: &[
      DocBlock::Paragraph(
        "Focuses on frictionless migration, daily SQL ergonomics, backend pool consolidation, and full multi-engine smoke verification.",
      ),
      DocBlock::Bullets(&[
        "External connection import: preview and selectively import connection profiles from Navicat (.ncx XML, including InitialDatabase, Oracle Service Name / SID, MariaDB presets, TLS CA, and SSH tunnels) and DBeaver (data-sources.json, including JDBC URL fallback, SSH handlers, and Production environment mapping); all third-party encrypted passwords are stripped and Agent access resets to off.",
        "Local parameterized query templates: save and search reusable SQL snippets from the editor toolbar or Command Palette (Cmd+K / Ctrl+K), reusing native :name bind parameters without custom template macros, with optional dialect filtering, Undo-preserving insertion, and one-click Insert & run.",
        "Website docs & Help links: multi-page bilingual documentation on dataomni.dev plus direct Website / Docs and Report Issue links in Settings -> About and the macOS Help menu.",
        "Runtime & pool hardening: removed the legacy tauri-plugin-sql dependency in favor of synchronous SqlxRegistry pooling, added immediate draft flush hooks on window hide/unload, and unified connection-lost reconnection banners across table browsing, inline commits, and background import/export tasks.",
        "Expanded real-database verification: wired DuckDB into round-trip schema diff sync verification (scripts/schema-diff-verify.sh) and verified :name bind parameters plus constraint-vs-index catalog metadata on Oracle 23ai Free (31/31 smoke tests green).",
      ]),
    ],
  },
  DocSection {
    id: "v1-1-0",
    toc_label: "v1.1.0 (2026-11-16)",
    heading: "v1.1.0 — 2026-11-16",
    blocks: &[
      DocBlock::Paragraph(
        "Expands structural engineering workflows and safe AI integration on top of the v1.0 desktop foundation, alongside strict WebView security hardening and DuckDB 2.0.",
      ),
      DocBlock::Bullets(&[
        "Read-only MCP server (dataomni mcp): built-in stdio MCP protocol server exposing 11 read-only tools across SQL and MongoDB / Redis / Neo4j / Elasticsearch, sharing the OS keychain HMAC-SHA256 grant and physical Production profile invisibility.",
        "Cross-connection schema diff & sync DDL: compare tables, columns, primary keys, indexes, and foreign keys between two connections and generate reviewable sync scripts verified end-to-end across PostgreSQL, MySQL, SQLite, SQL Server, and DuckDB.",
        "Editor :name bind parameters: typed parameter bar (text, number, boolean, date/time, NULL, unset) mapped to native driver placeholders ($1, ?, @p1, :name) while ignoring comments, string literals, PostgreSQL $$...$$, and Oracle q'[...]'.",
        "Deterministic ER diagram codegen: generate Prisma, TypeORM, MikroORM, SeaORM, and Rust sqlx models directly from live database catalogs offline, with zero LLM non-determinism.",
        "Security & runtime hardening: enforced strict WebView Content Security Policy, moved SQLite fully onto the backend pool, blocked file operations inside app_config_dir, upgraded bundled DuckDB to 2.0 (supporting Windows file paths > 260 characters), and eliminated root UI re-renders on editor keystrokes.",
      ]),
    ],
  },
  DocSection {
    id: "v1-0-0",
    toc_label: "v1.0.0 (2026-11-05)",
    heading: "v1.0.0 — 2026-11-05",
    blocks: &[
      DocBlock::Paragraph(
        "First stable MIT-licensed desktop release across macOS (Apple Silicon), Windows (x64), and Linux (x86_64).",
      ),
      DocBlock::Bullets(&[
        "Unified multi-engine coverage: PostgreSQL, MySQL, SQLite, SQL Server, Oracle (Instant Client Basic Light bundled), DuckDB, ClickHouse, MariaDB, TiDB, OceanBase, CockroachDB, and openGauss, plus dedicated workspaces for MongoDB, Redis, Neo4j, and Elasticsearch / OpenSearch.",
        "Zero-surprise editing & guardrails: local edit queue with exact parameterized SQL preview, single-transaction optimistic concurrency verification, environment badges (DEV / TEST / STAGING / PROD), and destructive-statement confirmation.",
        "Credentials & networking: OS keychain storage (zero plaintext secrets on disk), built-in SSH jump-host tunnels with strict ~/.ssh/known_hosts verification, five TLS modes over rustls, and layered connection diagnostics.",
        "Data workflows & Agent CLI: streaming CSV/JSON/SQL export, encoding-aware CSV import, native backups, visual execution plans, whole-database ER diagrams, and the read-only dataomni cli interface.",
      ]),
    ],
  },
  DocSection {
    id: "compatibility-policy",
    toc_label: "Upgrade compatibility",
    heading: "Upgrade & on-disk compatibility guarantee",
    blocks: &[
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "Forward- and backward-safe connections.json",
        body: "DataOmni preserves unrecognized fields and newer connection entries verbatim when reading and saving connections.json (pinned by fixtures/on-disk-0.5/ conformance tests). Upgrading across 1.x releases — or rolling back to an earlier 1.x build — never drops saved connections or keychain references.",
      },
    ],
  },
];

static EN_CHANGELOG: DocPage = DocPage {
  slug: DocSlug::Changelog,
  locale: Locale::En,
  meta_title: "Changelog & Release Notes — DataOmni",
  meta_description: "Release history, new capabilities, security hardening, and upgrade notes for DataOmni v1.3.0 through v1.0.0 across macOS, Windows, and Linux.",
  breadcrumb_group: "Releases",
  title: "Changelog",
  lead: "User-facing release highlights, security hardening milestones, and on-disk configuration compatibility guarantees across DataOmni releases.",
  toc_heading: "Releases",
  sections: EN_CHANGELOG_SECTIONS,
};

const ZH_CHANGELOG_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "v1-3-0",
    toc_label: "v1.3.0（2026-11-21）",
    heading: "v1.3.0 — 2026-11-21",
    blocks: &[
      DocBlock::Paragraph(
        "正式引入受控内存 DuckDB 统一分析引擎、五大数据库原生只读深度体检、顶层高亮 CLI / MCP 体验增强，以及零原始行外发的克制型应用内 AI 助手。",
      ),
      DocBlock::Bullets(&[
        "内置内存 DuckDB 分析沙盒与图表升级：支持 SQL 查询结果、继承当前筛选的 1,000 / 5,000 / 20,000 行有界表抽样、MongoDB 集合与 Elasticsearch 命中结果的列级画像（空值/空串/首尾空白、基数、分位数、直方图、时间趋势、体检摘要条、一键回表筛选与脱敏 Markdown 体检报告）；工作台支持附加最多 4 张本地 CSV/Parquet/JSON 参考表进行 JOIN/EXCEPT 对账、二次 SQL / PIVOT 透视与 Parquet 导出，并支持侧边栏免连接探索本地数据文件（10 万行 / 512 MB 上限并锁死 enable_external_access = false）。原生 SVG 图表新增堆叠柱图、面积图、散点图、24×24 交叉热力图、Pearson 相关性双极热力图、二维 Markdown 复制及带主题底色的 SVG / 2× PNG 导出。",
        "五大引擎原生只读体检：新增 Redis 非阻塞 SCAN 前缀内存/TTL/BigKey 诊断、MongoDB $sample 嵌套 Schema 漂移与 $indexStats 僵尸索引识别、Elasticsearch size: 0 原生聚合模板与 Mapping/分片体检、Neo4j O(1) 图概览与有界超级节点拓扑分析。",
        "顶层无头 CLI 与 MCP 增强：升级为顶层直调 dataomni <command>（兼容保留 dataomni cli），交互终端自动漂亮缩进与 ANSI 高亮、管道输出纯 JSON；新增 profile（CLI 支持 --file、MCP 物理隔离本地文件）、Parquet 导出、--param / --param-null / --params-json 绑定参数、3 次批量元数据查询 schema diff、连接级本地语义备注（semantics/<id>.md）、MCP 常驻会话复用 SSH 隧道，以及设置页「命令行与 Agent」一键接入引导与只读审计日志。",
        "克制型应用内 AI 增强（ai Cargo feature 后，零数据行外发）：新增请求取消、连通测试、多预设配置、本地 Ollama 无 Key 直连与「STAGING/PROD 仅允许本地模型」路由守卫；提供 SQL 自然语言生成/改写、报错诊断、EXPLAIN 瓶颈解读、ER 业务语义批量草拟及基于脱敏统计摘要的数据画像体检解读（均可展开核对发包原文、绝不自动执行）。",
        "连接体验与钥匙串修复：编辑连接时清晰标识已保存密码状态（支持保留原密码或显式清除），并修复 macOS Keychain 覆盖写入时的 -25244 错误。",
      ]),
    ],
  },
  DocSection {
    id: "v1-2-2",
    toc_label: "v1.2.2（2026-11-19）",
    heading: "v1.2.2 — 2026-11-19",
    blocks: &[
      DocBlock::Paragraph(
        "正式补齐经 Ed25519 签名校验的应用内平滑升级与安装渠道防冲突引导、可选驱动编译期解耦，以及双语隐私安全合规声明页。",
      ),
      DocBlock::Bullets(&[
        "应用内签名平滑升级与渠道识别：macOS（.dmg 独立安装）与 Windows（.msi / -setup.exe）接入 tauri-plugin-updater 与 Minisign Ed25519 签名校验（latest.json），采用「横幅点升级 → 查看说明并下载验签 → 人工确认立即重启安装」三步显式交互，绝不后台静默下载或强制重启，并在重启前自动串联未提交事务确认、活跃导入导出提醒与草稿落盘；Homebrew Cask 副本自动提供一键复制 brew upgrade --cask dataomni，拦截包管理器副本自覆盖。",
        "Oracle 驱动 Cargo feature 解耦：将 ODPI-C 动态库驱动拆分为默认开启的 oracle 特性（default = [\"ai\", \"oracle\"]），支持 --no-default-features 构建不依赖外部动态库的精简二进制，同时保持跨版本 connections.json 无损读写兼容。",
        "发布流水线提效与隐私合规：GitHub Actions 全量升至原生 Node 24 主版本（0 弃用告警），新增正式版公开后自动复核公证票据与 brew style 的 Tap 同步工作流，并上线双语隐私政策与数据安全声明页（/privacy/ 与 /zh/privacy/）。",
      ]),
    ],
  },
  DocSection {
    id: "v1-2-1",
    toc_label: "v1.2.1（2026-11-18）",
    heading: "v1.2.1 — 2026-11-18",
    blocks: &[
      DocBlock::Paragraph(
        "正式补齐 macOS 安装包的 Apple Developer ID 代码签名、Apple 官方公证与 Homebrew Cask 一键安装。",
      ),
      DocBlock::Bullets(&[
        "macOS 签名公证与票据钉合：DataOmni.app 与外层 .dmg 双层均完成 Apple Developer ID 签名、notarytool 公证与 stapler 离线票据钉合，下载拖入「应用程序」即可直接打开，无需再手动清理隔离属性，且后续同团队证书版本升级不再重弹系统钥匙串授权。",
        "官方 Homebrew Tap 上线：支持执行 brew install --cask dataomni-rs/tap/dataomni 一键安装与升级，装入应用的同时自动将内置只读命令行与 MCP 服务软链至 $(brew --prefix)/bin/dataomni。",
        "打包元数据与构建门禁：将 macOS Info.plist 的最低系统版本修正为 12.0（Monterey），并在发布流水线新增包内 Oracle Instant Client 全部库文件逐字节 SHA-256 校验与 stapler / spctl 公证自动核验。",
      ]),
    ],
  },
  DocSection {
    id: "v1-2-0",
    toc_label: "v1.2.0（2026-11-18）",
    heading: "v1.2.0 — 2026-11-18",
    blocks: &[
      DocBlock::Paragraph(
        "聚焦平滑迁移、日常高频查询体验、后端连接池收口与多引擎真库回归巩固。",
      ),
      DocBlock::Bullets(&[
        "主流客户端连接导入：欢迎页与命令面板新增对 Navicat（.ncx XML，支持 InitialDatabase、Oracle 服务名 / SID、MariaDB 预设、TLS CA、SSH 隧道）与 DBeaver（data-sources.json，支持 JDBC URL 兜底、SSH 通道与生产环境映射）的离线预览导入；前后端双重剥离加密口令与私钥密码，并将命令行与 Agent 访问重置为关闭。",
        "本地参数化查询模板：编辑器工具栏与命令面板（Cmd+K / Ctrl+K）新增常用 SQL 模板管理，直接复用 :name 绑定参数词法（零自定义宏语法），支持按方言筛选、保留撤销栈插入以及一键「插入并执行」。",
        "官网实用文档与直达入口：上线 dataomni.dev 多页双语文档体系，并在设置「关于与诊断」及 macOS Help 菜单补齐官网文档与问题反馈直达入口。",
        "连接池与交互稳态加固：后端彻底移除残留的 tauri-plugin-sql 依赖并收归同步 SqlxRegistry；补齐窗口隐藏与退出时的查询草稿立即落盘钩子；表数据浏览、就地提交与后台导入导出遇到连接断开或 SSH 隧道中断时统一激活断线重连横幅。",
        "真库回归补齐：DuckDB 接入 scripts/schema-diff-verify.sh 结构比对往返脚本验收；Oracle 23ai Free 补齐 :name 绑定参数与约束索引区分真库冒烟用例（31/31 全绿）。",
      ]),
    ],
  },
  DocSection {
    id: "v1-1-0",
    toc_label: "v1.1.0（2026-11-16）",
    heading: "v1.1.0 — 2026-11-16",
    blocks: &[
      DocBlock::Paragraph(
        "在 v1.0 稳定桌面底座之上补齐结构工程与只读 MCP 生态能力，并完成 WebView 严格安全边界收口与 DuckDB 2.0 升级。",
      ),
      DocBlock::Bullets(&[
        "内置只读 MCP 服务（dataomni mcp）：通过标准输入输出暴露 11 个关系库与四大 NoSQL 只读工具，复用系统钥匙串 HMAC-SHA256 开放凭证，生产环境连接物理不可见。",
        "跨库表结构比对与同步脚本：直接比对两个连接的表、列、主键、索引与外键差异，生成可审阅的方言 DDL 同步脚本（已通过 PostgreSQL、MySQL、SQLite、SQL Server 与 DuckDB 真库往返验收）。",
        "编辑器 :name 绑定参数栏：支持文本、数字、布尔、日期时间、NULL 与未设置档位，自动映射各方言原生占位符（$1、?、@p1、:name），准确跳过注释、字符串、PostgreSQL $$...$$ 与 Oracle q'[...]'。",
        "ER 图确定性代码生成：直接根据实时目录元数据离线生成 Prisma、TypeORM、MikroORM、SeaORM 与 Rust sqlx 模型代码，零大模型幻觉。",
        "安全与运行期加固：启用严格 WebView CSP，SQLite 全量迁入后端连接池，拦截针对应用配置目录的文件读写，内置 DuckDB 升至 2.0（打通 Windows 超 260 字符长路径），消除编辑器打字时的根组件重渲染。",
      ]),
    ],
  },
  DocSection {
    id: "v1-0-0",
    toc_label: "v1.0.0（2026-11-05）",
    heading: "v1.0.0 — 2026-11-05",
    blocks: &[
      DocBlock::Paragraph(
        "首个跨 macOS（Apple Silicon）、Windows（x64）与 Linux（x86_64）三平台的 MIT 开源正式版。",
      ),
      DocBlock::Bullets(&[
        "一站式多引擎覆盖：内置 PostgreSQL、MySQL、SQLite、SQL Server、Oracle（随包集成 Instant Client Basic Light）、DuckDB、ClickHouse 及 MariaDB / TiDB / OceanBase / CockroachDB / openGauss 支持，并为 MongoDB、Redis、Neo4j、Elasticsearch / OpenSearch 提供专用工作区。",
        "防误改编辑与环境护栏：表格增删改本地排队并预览精确参数化 SQL，单事务内逐行校验受影响行数防覆盖，全界面环境标识（开发 / 测试 / 预发 / 生产）与高危语句分级拦截。",
        "凭据与内网连通：全部敏感口令仅存系统钥匙串（磁盘零明文），内置强制校验 ~/.ssh/known_hosts 的 SSH 跳板机隧道、基于 rustls 的五档 TLS 模式与逐层连接诊断。",
        "数据工程与命令行接口：流式 CSV / JSON / SQL 导出、自动识别编码的 CSV 导入、多引擎备份恢复、可视化执行计划树、整库 ER 图，以及面向终端与 AI Agent 的只读 dataomni cli。",
      ]),
    ],
  },
  DocSection {
    id: "compatibility-policy",
    toc_label: "升级与落盘兼容承诺",
    heading: "升级与本地配置兼容承诺",
    blocks: &[
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "connections.json 前后向无损兼容",
        body: "DataOmni 读写 connections.json 时会原样保留不认识的新字段与高版本连接条目（由 fixtures/on-disk-0.5/ 回归样本钉死）。在 1.x 各版本间升级或临时退回旧版本，已保存的连接与系统钥匙串引用均不会丢失。",
      },
    ],
  },
];

static ZH_CHANGELOG: DocPage = DocPage {
  slug: DocSlug::Changelog,
  locale: Locale::Zh,
  meta_title: "版本更新记录与发布说明 — DataOmni",
  meta_description: "DataOmni v1.3.0、v1.2.2、v1.2.1、v1.1.0 与 v1.0.0 正式版面向用户的能力更新、真库回归覆盖、安全加固细节与本地配置文件前后向兼容性说明，覆盖三平台。",
  breadcrumb_group: "版本发布",
  title: "版本更新记录",
  lead: "记录 DataOmni 每个正式版本面向用户的核心能力演进、安全加固里程碑与本地配置兼容保障。",
  toc_heading: "版本列表",
  sections: ZH_CHANGELOG_SECTIONS,
};

// ============================================================================
// 6. PRIVACY & SECURITY (/privacy/ & /zh/privacy/)
// ============================================================================

const EN_PRIVACY_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "zero-telemetry",
    toc_label: "Zero telemetry & no account",
    heading: "Zero telemetry, no cloud account, and direct connections",
    blocks: &[
      DocBlock::Paragraph(
        "DataOmni is a local-first, MIT-licensed desktop database client. Neither the desktop application nor the dataomni.dev documentation website collects analytics, usage telemetry, crash dumps, advertising identifiers, or personal profiles.",
      ),
      DocBlock::Bullets(&[
        "No user registration or cloud sync: DataOmni requires no account, login, license server, or cloud workspace. All connection profiles, query tabs, and templates stay on your own machine.",
        "Direct database connections: Every SQL, MongoDB, Redis, Neo4j, and Elasticsearch session connects directly from your workstation (or through your own SSH jump host) to your database server, bypassing system HTTP proxies so database traffic never routes through third-party relays.",
        "Cookie-free static website: https://dataomni.dev is static HTML, CSS, and progressive-enhancement JavaScript served via GitHub Pages with zero analytics scripts, tracking pixels, or cookies (only a local dataomni-site-theme preference in browser localStorage when you toggle light or dark mode).",
      ]),
    ],
  },
  DocSection {
    id: "credentials-and-local-data",
    toc_label: "Keychain & on-disk data",
    heading: "OS keychain custody and local on-disk artifacts",
    blocks: &[
      DocBlock::Paragraph(
        "Database passwords, SSH private-key passphrases, optional AI API keys (#ai), and the Agent grant signing key (#agent-grant) are stored exclusively in your operating system credential manager — macOS Keychain, Windows Credential Manager, or Linux Secret Service (gnome-keyring / KWallet). Plaintext secrets are never written to disk.",
      ),
      DocBlock::Table {
        caption: "Local data stored on your workstation",
        headers: &["Artifact", "What is stored", "Secret & privacy guarantee"],
        rows: &[
          &[
            "connections.json",
            "Connection name, host, port, database, TLS mode, environment tag, and SSH jump-host metadata.",
            "Zero passwords or passphrases; third-party encrypted secrets in imported Navicat .ncx or DBeaver files are stripped on both frontend and backend.",
          ],
          &[
            "OS Keychain entries",
            "Database password, SSH passphrase, optional AI API key, and local HMAC-SHA256 Agent grant key.",
            "Managed by OS access controls; macOS builds ship with Hardened Runtime + Apple Developer ID notarization so injected libraries cannot impersonate the app.",
          ],
          &[
            "Workspace drafts & history",
            "Unsaved editor tabs, local :name query templates, and recent local query execution metadata.",
            "Stored only in your local app config/storage directory; never uploaded or synchronized externally.",
          ],
          &[
            "agent-audit.jsonl & dataomni.log",
            "Timestamp, connection name, command kind, row count, and SHA-256 digest of Agent statements; operational diagnostics.",
            "Audit entries record statement SHA-256 digests rather than raw SQL text or returned row values.",
          ],
        ],
      },
    ],
  },
  DocSection {
    id: "network-requests",
    toc_label: "Outbound network disclosure",
    heading: "Complete disclosure of outbound network requests",
    blocks: &[
      DocBlock::Paragraph(
        "DataOmni initiates network connections only for the explicit functions listed below. Every non-database network feature can be turned off in settings or compiled out entirely.",
      ),
      DocBlock::Table {
        caption: "All outbound network connections made by DataOmni",
        headers: &["Feature", "Destination", "Default & how to disable"],
        rows: &[
          &[
            "Database & SSH sessions",
            "Only the host, port, and SSH jump host you configure.",
            "Initiated strictly when you test, open, or query a connection.",
          ],
          &[
            "New-version check & user-confirmed upgrade",
            "GET https://api.github.com/repos/dataomni-rs/DataOmni/releases/latest (at most once per 24 hours), plus latest.json and signed installer download from github.com/dataomni-rs/DataOmni/releases only when you click Upgrade.",
            "Sends only standard HTTP headers and User-Agent (DataOmni/<version>), zero device IDs; verifies Ed25519 signature before staging. Toggle off anytime in Settings -> About & Diagnostics.",
          ],
          &[
            "Optional AI schema design",
            "Direct HTTPS request from the Rust backend to the OpenAI-compatible or Anthropic endpoint you configure.",
            "Disabled until you configure an API key and click generate; sends only your prompt and current DDL/schema draft (never table rows). Strip physically at build time with cargo build --no-default-features.",
          ],
        ],
      },
      DocBlock::Code {
        label: "Build an air-gapped binary with zero AI network code",
        copy_text: Some("cargo build --manifest-path src-tauri/Cargo.toml --release --no-default-features"),
        raw_code: "$ cargo build --manifest-path src-tauri/Cargo.toml --release --no-default-features",
      },
    ],
  },
  DocSection {
    id: "agent-and-webview-boundaries",
    toc_label: "Agent & WebView isolation",
    heading: "Read-only Agent isolation and WebView security boundaries",
    blocks: &[
      DocBlock::Callout {
        tone: CalloutTone::Guardrail,
        title: "Production connections are physically invisible to AI Agents",
        body: "Every connection defaults to Agent access OFF. Enabling CLI or MCP access signs an HMAC-SHA256 grant with a key stored in your OS keychain — editing connections.json on disk cannot forge access. Connections marked PROD return not_found to both dataomni cli and dataomni mcp, and permitted non-PROD sessions enforce read-only transactions plus statement risk gates.",
      },
      DocBlock::Bullets(&[
        "Strict WebView Content Security Policy: Bundled with default-src 'self' and script-src 'self'; the frontend WebView makes zero external fetch/WebSocket calls, holds zero sql:* plugin permissions, and is blocked from reading or writing the application config directory.",
        "Controlled external links: Opening documentation or GitHub release pages uses fixed backend-validated HTTPS targets rather than exposing arbitrary URL execution to the WebView.",
      ]),
    ],
  },
  DocSection {
    id: "audit-and-contact",
    toc_label: "Open-source audit & contact",
    heading: "Verifiable open-source builds and privacy contact",
    blocks: &[
      DocBlock::Paragraph(
        "All application code, SQL/NoSQL drivers, static website generator code, and GitHub Actions release/notarization workflows are open source under the MIT License at https://github.com/dataomni-rs/DataOmni. Updates to this policy are tracked in version control alongside each release. For privacy, compliance, or security questions, open an issue or security advisory on the official GitHub repository.",
      ),
    ],
  },
];

static EN_PRIVACY: DocPage = DocPage {
  slug: DocSlug::Privacy,
  locale: Locale::En,
  meta_title: "Privacy Policy & Security Architecture — DataOmni",
  meta_description: "DataOmni collects zero telemetry, requires no account, stores passwords only in your OS keychain, isolates PROD from AI agents, and connects directly to databases.",
  breadcrumb_group: "Privacy & Security",
  title: "Privacy Policy & Security Architecture",
  lead: "Last updated: November 19, 2026. DataOmni is built so your credentials, queries, and query results stay strictly between your workstation and your databases.",
  toc_heading: "On this page",
  sections: EN_PRIVACY_SECTIONS,
};

const ZH_PRIVACY_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "zero-telemetry",
    toc_label: "零遥测与无账号",
    heading: "零遥测采集、无云端账号与数据库直连",
    blocks: &[
      DocBlock::Paragraph(
        "DataOmni 是基于 MIT 许可证开源的纯本地桌面数据库客户端。无论桌面端应用还是官网 dataomni.dev，均不采集任何行为统计、遥测埋点、崩溃上传、广告标识符或个人画像数据。",
      ),
      DocBlock::Bullets(&[
        "无需注册登录与云同步：应用不设账号体系、授权服务器或云端配置同步，所有连接配置、未保存草稿与查询模板均只保存在本机。",
        "数据库全链路直连：所有关系型数据库与 MongoDB、Redis、Neo4j、Elasticsearch 会话均由本机（或您自行配置的 SSH 跳板机）直接连接目标库，显式绕过系统 HTTP 代理，查询流量绝不经过任何第三方中转服务。",
        "官网零追踪脚本与零 Cookie：https://dataomni.dev 为托管于 GitHub Pages 的纯静态页面，不加载任何第三方统计脚本、追踪像素或 Cookie（仅在您手动切换深浅色外观时于浏览器 localStorage 记录主题偏好）。",
      ]),
    ],
  },
  DocSection {
    id: "credentials-and-local-data",
    toc_label: "系统钥匙串与本地文件",
    heading: "操作系统钥匙串托管与本地落盘边界",
    blocks: &[
      DocBlock::Paragraph(
        "数据库密码、SSH 私钥口令、可选 AI 接口密钥（#ai）以及 Agent 开放凭证签名密钥（#agent-grant）一律仅存入操作系统凭据管理器（macOS 钥匙串、Windows 凭据管理器、Linux Secret Service），磁盘配置文件不留任何明文口令。",
      ),
      DocBlock::Table {
        caption: "本机存储内容与安全承诺一览",
        headers: &["本地数据项", "记录内容", "安全与隐私保障"],
        rows: &[
          &[
            "connections.json",
            "连接名称、主机、端口、库名、TLS 档位、环境标识与 SSH 跳板机非敏感元数据。",
            "零明文或密文口令；从 Navicat .ncx 或 DBeaver 导入连接时，前后端双重剥离原客户端加密口令。",
          ],
          &[
            "系统钥匙串条目",
            "数据库登录密码、SSH 私钥口令、可选 AI Key 与本地 HMAC-SHA256 授权密钥。",
            "受操作系统级访问控制保护；macOS 版开启 Hardened Runtime 并经 Apple Developer ID 公证，阻断动态库注入冒领凭据。",
          ],
          &[
            "工作区草稿与查询模板",
            "未保存的查询标签草稿、本地 :name 参数化 SQL 模板与本机历史执行摘要。",
            "仅保存在本机应用数据目录，绝不向外部上传或同步。",
          ],
          &[
            "agent-audit.jsonl 与日志",
            "Agent 调用时间、连接名、命令类别、返回行数与语句 SHA-256 摘要；本地运行诊断日志。",
            "审计留痕只记语句的 SHA-256 摘要，不落盘 SQL 原文与查询结果行数据。",
          ],
        ],
      },
    ],
  },
  DocSection {
    id: "network-requests",
    toc_label: "出网请求完整清单",
    heading: "出网网络请求完整披露与关闭方式",
    blocks: &[
      DocBlock::Paragraph(
        "DataOmni 仅在以下三类明确场景发起网络请求；所有非数据库本身的联网能力均可在设置中一键关闭，或在编译期物理剔除。",
      ),
      DocBlock::Table {
        caption: "DataOmni 全部出网行为清单",
        headers: &["功能项", "通信目标", "默认行为与关闭方式"],
        rows: &[
          &[
            "数据库与 SSH 隧道连接",
            "仅连接您在配置表单中填写的目标数据库与 SSH 跳板机地址。",
            "仅在您主动测试连接、打开工作区或执行查询时建立。",
          ],
          &[
            "新版本检查与确认升级",
            "GET https://api.github.com/repos/dataomni-rs/DataOmni/releases/latest（最多每 24 小时一次）；仅当您主动点击「升级」时拉取同仓库 Releases 下的 latest.json 与签名安装包。",
            "仅发送标准请求头与版本号（DataOmni/<version>），不含任何设备标识，且强制校验 Ed25519 签名；可在「设置 → 关于与诊断」随时关闭。",
          ],
          &[
            "可选 AI 辅助建表设计",
            "由 Rust 后端直连您自行配置的 OpenAI 兼容或 Anthropic 接口地址。",
            "未填 Key 且未点击生成时零请求；请求仅含您的自然语言描述与当前表结构草稿，绝不上传表内业务数据。内网环境可通过 --no-default-features 编译期彻底剥离。",
          ],
        ],
      },
      DocBlock::Code {
        label: "构建不含任何 AI 出网代码的纯净内网二进制",
        copy_text: Some("cargo build --manifest-path src-tauri/Cargo.toml --release --no-default-features"),
        raw_code: "$ cargo build --manifest-path src-tauri/Cargo.toml --release --no-default-features",
      },
    ],
  },
  DocSection {
    id: "agent-and-webview-boundaries",
    toc_label: "Agent 与前端隔离",
    heading: "只读 Agent 物理隔离与 WebView 安全边界",
    blocks: &[
      DocBlock::Callout {
        tone: CalloutTone::Guardrail,
        title: "生产环境（PROD）对命令行与 MCP 物理不可见",
        body: "所有连接默认关闭命令行与 MCP 访问；开启时由系统钥匙串中的密钥签发 HMAC-SHA256 凭证，直接篡改磁盘上的 connections.json 无法伪造提权。标为 PROD 的生产连接对 dataomni cli 与 dataomni mcp 一律返回 not_found，已授权的非生产连接全程叠加只读事务与语句风险门禁。",
      },
      DocBlock::Bullets(&[
        "严格 WebView 内容安全策略（CSP）：启用 default-src 'self' 与 script-src 'self'，前端零外部网络调用、零 sql:* 插件权限，且后端文件读写命令显式拦截针对应用配置目录的访问。",
        "受控外部链接跳转：设置页与帮助菜单仅允许打开后端枚举锁定的官方 HTTPS 页面，不向前端暴露任意 URL 打开接口。",
      ]),
    ],
  },
  DocSection {
    id: "audit-and-contact",
    toc_label: "开源审计与联络方式",
    heading: "可验证的开源构建与安全联络方式",
    blocks: &[
      DocBlock::Paragraph(
        "DataOmni 的前后端源码、数据库驱动封装、静态官网生成器与 GitHub Actions 签名公证流水线全部在 https://github.com/dataomni-rs/DataOmni 以 MIT 许可证公开，可供企业安全团队逐行审计。本声明随代码库版本同步演进；如有隐私合规或安全问题，欢迎通过官方 GitHub Issues 或 Security Advisories 联系维护团队。",
      ),
    ],
  },
];

static ZH_PRIVACY: DocPage = DocPage {
  slug: DocSlug::Privacy,
  locale: Locale::Zh,
  meta_title: "隐私政策与数据安全声明 — DataOmni",
  meta_description: "DataOmni 坚持零遥测、无云端账号与数据库直连架构：口令与密钥仅存操作系统钥匙串，生产环境对 AI Agent 物理不可见，可选版本检查与 AI 模块均可关闭或编译期物理剔除。",
  breadcrumb_group: "隐私与安全",
  title: "隐私政策与数据安全声明",
  lead: "最后更新：2026 年 11 月 19 日。DataOmni 从架构层确保数据库凭据、SQL 脚本与查询结果始终只留在您的工作站与目标数据库之间。",
  toc_heading: "本页目录",
  sections: ZH_PRIVACY_SECTIONS,
};
