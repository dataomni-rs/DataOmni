use crate::content::Locale;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocSlug {
  Quickstart,
  Mcp,
  Cli,
  Databases,
  Changelog,
}

pub const ALL_DOC_SLUGS: &[DocSlug] =
  &[DocSlug::Quickstart, DocSlug::Mcp, DocSlug::Cli, DocSlug::Databases, DocSlug::Changelog];

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
    }
  }

  pub const fn is_changelog(self) -> bool {
    matches!(self, Self::Changelog)
  }

  pub const fn prev(self) -> Option<Self> {
    match self {
      Self::Quickstart => None,
      Self::Mcp => Some(Self::Quickstart),
      Self::Cli => Some(Self::Mcp),
      Self::Databases => Some(Self::Cli),
      Self::Changelog => Some(Self::Databases),
    }
  }

  pub const fn next(self) -> Option<Self> {
    match self {
      Self::Quickstart => Some(Self::Mcp),
      Self::Mcp => Some(Self::Cli),
      Self::Cli => Some(Self::Databases),
      Self::Databases => Some(Self::Changelog),
      Self::Changelog => None,
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
    heading: "All 11 read-only MCP tools",
    blocks: &[
      DocBlock::Paragraph(
        "Every tool carries readOnlyHint: true in tools/list and returns structured JSON with \"schema\": 1 (dictionary returns whole-database Markdown ready for LLM context). Unknown extra arguments or option-like positional values starting with -- are rejected as usage errors.",
      ),
      DocBlock::Table {
        caption: "MCP tools exposed by dataomni mcp",
        headers: &["Tool", "Arguments", "What it returns"],
        rows: &[
          &["connections", "(none)", "Lists non-production connections explicitly opened to agents (never leaks passwords or key paths)."],
          &["dictionary", "connection", "Whole-database Markdown data dictionary (tables, columns, types, PKs, FKs) — call this first before writing SQL."],
          &["schema", "connection, table?", "Lists tables and views, or columns, indexes, and foreign keys for a specific table."],
          &["ddl", "connection, table", "Returns the database engine's own canonical object definition (SHOW CREATE TABLE, DBMS_METADATA, sqlite_master)."],
          &["query", "connection, sql, limit?, timeout_seconds?", "Executes one read-only SQL statement (default 200 rows, max 10,000, reports truncated: true when capped)."],
          &["explain", "connection, sql", "Returns the estimated execution plan without running ANALYZE or mutating state."],
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
    toc_label: "11 个只读工具清单",
    heading: "内置的 11 个只读 MCP 工具",
    blocks: &[
      DocBlock::Paragraph(
        "所有工具在 tools/list 中均声明 readOnlyHint: true，回包统一采用顶层 \"schema\": 1 的 JSON（dictionary 直接返回适合喂给大模型的整库 Markdown 数据字典）。多余未知字段或以 -- 开头的位置参数均按 usage 错误拦截。",
      ),
      DocBlock::Table {
        caption: "dataomni mcp 工具与入参速查",
        headers: &["工具名", "入参", "作用与输出"],
        rows: &[
          &["connections", "无", "列出已开放给 Agent 的非生产连接（不含口令与私钥路径）。"],
          &["dictionary", "connection", "导出整库 Markdown 数据字典（表、列、类型、主外键）——建议让 Agent 写 SQL 前先调它。"],
          &["schema", "connection, table?", "列出整库表与视图，或指定单表的列、索引与外键结构。"],
          &["ddl", "connection, table", "读取数据库自身的原始建表定义（SHOW CREATE TABLE、DBMS_METADATA、sqlite_master 等）。"],
          &["query", "connection, sql, limit?, timeout_seconds?", "执行单条只读 SQL（默认最多 200 行，上限 10000 行，超限标记 truncated: true）。"],
          &["explain", "connection, sql", "获取估算执行计划（不带 ANALYZE，不实际执行语句）。"],
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
    toc_label: "PATH & Skill setup",
    heading: "Put dataomni on PATH and export the Agent Skill",
    blocks: &[
      DocBlock::Paragraph(
        "Terminal-native coding agents (such as Claude Code or Codex) and shell scripts can invoke dataomni cli directly without configuring an MCP server. Every command shares the exact same keychain grant, production invisibility, and read-only enforcement as dataomni mcp.",
      ),
      DocBlock::Code {
        label: "macOS symlink & Agent Skill export",
        copy_text: Some("ln -sf /Applications/DataOmni.app/Contents/MacOS/dataomni ~/.local/bin/dataomni"),
        raw_code: "# Homebrew Cask already links $(brew --prefix)/bin/dataomni; for manual .dmg installs:\nln -sf /Applications/DataOmni.app/Contents/MacOS/dataomni ~/.local/bin/dataomni\n\n# Print the built-in SKILL.md guide for terminal agents\ndataomni cli guide",
      },
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "Windows PowerShell & CMD exit-code note",
        body: "Because dataomni.exe uses the Windows GUI subsystem, interactive shells return immediately unless stdout is piped. Pipe output in scripts (dataomni cli connections | Out-String) or run cmd /c start /wait to capture $LASTEXITCODE. AI agents already invoke subprocesses via pipes and are unaffected.",
      },
    ],
  },
  DocSection {
    id: "sql-commands",
    toc_label: "SQL, export & backup",
    heading: "Relational inspection, query, export, and backup commands",
    blocks: &[
      DocBlock::Paragraph(
        "Pass connection names or IDs. For long SQL queries, pass - in place of the statement to read from standard input. Export and backup commands write only to brand-new target paths and never overwrite existing files.",
      ),
      DocBlock::Table {
        caption: "Core dataomni cli subcommands",
        headers: &["Subcommand", "Behavior & limits"],
        rows: &[
          &["dataomni cli connections", "Lists open non-production connections and their database types."],
          &["dataomni cli dictionary <conn>", "Outputs a whole-database Markdown data dictionary (tables, columns, types, PKs, FKs)."],
          &["dataomni cli schema <conn> [<table>]", "Lists all tables and views, or columns, indexes, and foreign keys for one table."],
          &["dataomni cli ddl <conn> <table>", "Prints the database's native CREATE definition (views on PostgreSQL; tables & views elsewhere)."],
          &["dataomni cli query <conn> \"<sql>\"", "Runs one read-only statement (default --limit 200, max 10000; default --timeout 30s)."],
          &["dataomni cli explain <conn> \"<sql>\"", "Returns the estimated execution plan without running ANALYZE."],
          &["dataomni cli export <conn> \"<sql>\" --out <file>", "Streams full read-only results to a new CSV or JSON (--format json) file (default 300s timeout)."],
          &["dataomni cli backup <conn> --out <path>", "Runs a read-only logical/file backup (VACUUM INTO, DuckDB EXPORT, pg_dump, mysqldump, mongodump)."],
          &["dataomni cli csv-preview <file>", "Sniffs delimiter and UTF-8 / GB18030 encoding on a local CSV file and flags malformed rows."],
          &["dataomni cli test <conn>", "Verifies read-only connectivity; attaches DNS / TCP port diagnosis on failure."],
        ],
      },
      DocBlock::Code {
        label: "Typical terminal / agent workflow",
        copy_text: Some("dataomni cli dictionary shop_dev"),
        raw_code: "$ dataomni cli dictionary shop_dev\n$ dataomni cli query shop_dev \"SELECT status, COUNT(*) FROM orders GROUP BY status\" --limit 50\n$ dataomni cli export shop_dev - --out /tmp/orders_summary.json --format json < report.sql",
      },
    ],
  },
  DocSection {
    id: "workspace-commands",
    toc_label: "Mongo, Redis, Neo4j & ES",
    heading: "Dedicated NoSQL workspace commands",
    blocks: &[
      DocBlock::Paragraph(
        "Non-relational engines use purpose-built read-only subcommands rather than shoehorning everything into SQL strings:",
      ),
      DocBlock::Code {
        label: "Read-only MongoDB, Redis, Neo4j, and Elasticsearch calls",
        copy_text: None,
        raw_code: "# MongoDB (mongosh filter/pipeline syntax; $out and $merge refused at parse time)\ndataomni cli mongo app_mongo collections\ndataomni cli mongo app_mongo find shop.orders '{ status: \"paid\" }' --limit 20\n\n# Redis (--db N goes before the operation; command checks COMMAND INFO readonly)\ndataomni cli redis cache_dev --db 0 scan \"session:*\"\ndataomni cli redis cache_dev get \"session:42\"\n\n# Neo4j (EXPLAIN verifies query type == r before running in a read transaction)\ndataomni cli neo4j graph_dev labels\ndataomni cli neo4j graph_dev run \"MATCH (u:User)-[:BOUGHT]->(p) RETURN u.id, p.sku LIMIT 25\"\n\n# Elasticsearch / OpenSearch (GET/HEAD or search POST such as _search, _count, _msearch)\ndataomni cli es search_dev indices\ndataomni cli es search_dev request POST /orders/_search '{\"size\":10,\"query\":{\"match_all\":{}}}'",
      },
    ],
  },
  DocSection {
    id: "exit-codes",
    toc_label: "JSON schema & exit codes",
    heading: "JSON contract (schema: 1) and exit codes 0–4",
    blocks: &[
      DocBlock::Paragraph(
        "Standard output always emits JSON with top-level \"schema\": 1 (pass --format table for human terminal tables; dictionary always emits Markdown). Errors go to standard error as {\"schema\": 1, \"error\": {\"kind\": \"...\", \"message\": \"...\"}} with deterministic exit codes so agents know whether to fix syntax or stop:",
      ),
      DocBlock::Table {
        caption: "Deterministic CLI exit codes",
        headers: &["Exit code", "Category", "Agent / caller action"],
        rows: &[
          &["0", "Success", "Parse stdout JSON (or Markdown from dictionary)."],
          &["1", "Database error or timeout", "Read error.message and fix the SQL / query syntax or tighten filters."],
          &["2", "Usage or not_found", "Fix CLI flags, or ask the user to open the connection to agents in DataOmni."],
          &["3", "Refused by read-only gate", "Stop immediately — write statements, multi-statements, and $out/$merge are never allowed and no CLI flag overrides this."],
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
  meta_description: "Reference for dataomni cli: read-only SQL, whole-database Markdown dictionaries, MongoDB, Redis, Neo4j, Elasticsearch commands, JSON output, and exit codes.",
  breadcrumb_group: "Docs",
  title: "Agent & terminal CLI reference",
  lead: "Run structured read-only SQL, whole-database Markdown dictionaries, exports, backups, and MongoDB / Redis / Neo4j / Elasticsearch queries straight from your terminal or shell-capable AI agent.",
  toc_heading: "On this page",
  sections: EN_CLI_SECTIONS,
};

const ZH_CLI_SECTIONS: &[DocSection] = &[
  DocSection {
    id: "path-setup",
    toc_label: "配置 PATH 与导出 Skill",
    heading: "配置 PATH 与一键生成 Agent Skill",
    blocks: &[
      DocBlock::Paragraph(
        "对于原生具备终端执行能力的工具（如 Claude Code、Codex 或你自己的巡检脚本），直接调用 dataomni cli 甚至无需注册常驻 MCP 服务——不占上下文工具表，输出结构化 JSON，且与 MCP 共用同一套钥匙串开放凭证、生产库隐身与只读事务防线。",
      ),
      DocBlock::Code {
        label: "macOS 软链到 PATH 并导出 Agent 使用说明",
        copy_text: Some("ln -sf /Applications/DataOmni.app/Contents/MacOS/dataomni ~/.local/bin/dataomni"),
        raw_code: "# Homebrew Cask 安装已自动软链 $(brew --prefix)/bin/dataomni；手动拖 .dmg 安装时可执行：\nln -sf /Applications/DataOmni.app/Contents/MacOS/dataomni ~/.local/bin/dataomni\n\n# 输出完整 SKILL.md，可直接存进 Claude Code / Codex 的 skills 目录\ndataomni cli guide",
      },
      DocBlock::Callout {
        tone: CalloutTone::Note,
        title: "Windows 终端退出码小贴士",
        body: "Windows 发布版主程序采用 GUI 子系统，在 PowerShell / CMD 直接手敲命令时终端不会等待进程结束；若脚本里需要读取 $LASTEXITCODE，请接管道（如 dataomni cli connections | Out-String）或用 cmd /c start /wait。AI Agent 本身经管道调用子进程，完全不受影响。",
      },
    ],
  },
  DocSection {
    id: "sql-commands",
    toc_label: "关系库查询、导出与备份",
    heading: "关系库结构读取、只读查询、导出与备份",
    blocks: &[
      DocBlock::Paragraph(
        "支持按连接名称或连接 ID 调用；长 SQL 可用 - 代替语句参数从标准输入读取。export 与 backup 只允许写入全新的目标路径，绝不覆盖磁盘上已有文件。",
      ),
      DocBlock::Table {
        caption: "dataomni cli 核心子命令速查",
        headers: &["子命令", "功能与默认边界"],
        rows: &[
          &["dataomni cli connections", "列出当前已对 Agent 开放的非生产连接及数据库类型。"],
          &["dataomni cli dictionary <连接>", "输出整库 Markdown 数据字典（表、列、类型、主外键），最适合写 SQL 前先读一遍。"],
          &["dataomni cli schema <连接> [<表>]", "列出整库表与视图，或读取指定单表的列、索引与外键。"],
          &["dataomni cli ddl <连接> <表>", "获取数据库自身保存的对象定义原文（PostgreSQL 仅视图有原文，其余库含表与视图）。"],
          &["dataomni cli query <连接> \"<SQL>\"", "执行单条只读语句（默认 --limit 200，上限 10000；默认 --timeout 30 秒）。"],
          &["dataomni cli explain <连接> \"<SQL>\"", "获取估算执行计划（禁止 ANALYZE，不实际执行语句）。"],
          &["dataomni cli export <连接> \"<SQL>\" --out <文件>", "在只读事务内流式导出完整结果到新 CSV 或 JSON（--format json）文件（默认 300 秒）。"],
          &["dataomni cli backup <连接> --out <路径>", "按应用内置逻辑做只读备份（SQLite VACUUM INTO、DuckDB EXPORT、pg_dump、mysqldump、mongodump）。"],
          &["dataomni cli csv-preview <文件>", "离线嗅探本地 CSV 分隔符与编码（UTF-8 / GB18030），标出字段数异常行。"],
          &["dataomni cli test <连接>", "测试连接；失败时在 error.diagnosis 返回域名解析与端口探测结果。"],
        ],
      },
      DocBlock::Code {
        label: "典型排障与导出命令示例",
        copy_text: Some("dataomni cli dictionary shop_dev"),
        raw_code: "$ dataomni cli dictionary shop_dev\n$ dataomni cli query shop_dev \"SELECT status, COUNT(*) FROM orders GROUP BY status\" --limit 50\n$ dataomni cli export shop_dev - --out /tmp/orders_summary.json --format json < report.sql",
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
        raw_code: "# MongoDB（沿用 mongosh 写法，解析期直接拒绝含 $out / $merge 的聚合管道）\ndataomni cli mongo app_mongo collections\ndataomni cli mongo app_mongo find shop.orders '{ status: \"paid\" }' --limit 20\n\n# Redis（--db N 放在操作名前；command 先查服务端 COMMAND INFO 确认带 readonly 旗标）\ndataomni cli redis cache_dev --db 0 scan \"session:*\"\ndataomni cli redis cache_dev get \"session:42\"\n\n# Neo4j（先发 EXPLAIN 确认服务端判定查询类型为 r，再在只读路由事务内运行）\ndataomni cli neo4j graph_dev labels\ndataomni cli neo4j graph_dev run \"MATCH (u:User)-[:BOUGHT]->(p) RETURN u.id, p.sku LIMIT 25\"\n\n# Elasticsearch / OpenSearch（按路径段白名单放行 GET/HEAD 及 _search、_count、_msearch 等查询）\ndataomni cli es search_dev indices\ndataomni cli es search_dev request POST /orders/_search '{\"size\":10,\"query\":{\"match_all\":{}}}'",
      },
    ],
  },
  DocSection {
    id: "exit-codes",
    toc_label: "JSON 输出与退出码 0–4",
    heading: "JSON 输出结构（schema: 1）与退出码契约",
    blocks: &[
      DocBlock::Paragraph(
        "标准输出默认打印顶层带 \"schema\": 1 的 JSON（传 --format table 可切为终端对齐表格；dictionary 固定输出 Markdown）。报错统一写向标准错误 {\"schema\": 1, \"error\": {\"kind\": \"...\", \"message\": \"...\"}}，并通过 0–4 退出码明确区分「改写 SQL 重试」与「权限红线切勿重试」：",
      ),
      DocBlock::Table {
        caption: "CLI 退出码（0–4）精确语义",
        headers: &["退出码", "类别", "调用方 / Agent 应对动作"],
        rows: &[
          &["0", "执行成功", "正常解析 stdout JSON（或 dictionary 的 Markdown）。"],
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
  meta_description: "dataomni cli 完整命令参考：整库 Markdown 数据字典、只读 SQL 与执行计划、MongoDB / Redis / Neo4j / Elasticsearch 查询、JSON 输出契约及 0–4 退出码说明。",
  breadcrumb_group: "文档",
  title: "命令行与 Agent 接口参考",
  lead: "在终端脚本或 Claude Code、Codex 等命令行 Agent 中直接复用已保存的数据库连接，跑通整库数据字典、只读 SQL、流式导出与四大 NoSQL 查询。",
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
  meta_description: "Release history, new capabilities, security hardening, and upgrade compatibility notes for DataOmni v1.2.0, v1.1.0, and v1.0.0 on macOS, Windows, and Linux.",
  breadcrumb_group: "Releases",
  title: "Changelog",
  lead: "User-facing release highlights, security hardening milestones, and on-disk configuration compatibility guarantees across DataOmni releases.",
  toc_heading: "Releases",
  sections: EN_CHANGELOG_SECTIONS,
};

const ZH_CHANGELOG_SECTIONS: &[DocSection] = &[
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
  meta_description: "DataOmni v1.2.0、v1.1.0 与 v1.0.0 正式版面向用户的能力更新、真库回归覆盖、安全加固细节与本地配置文件前后向兼容性说明，覆盖三平台。",
  breadcrumb_group: "版本发布",
  title: "版本更新记录",
  lead: "记录 DataOmni 每个正式版本面向用户的核心能力演进、安全加固里程碑与本地配置兼容保障。",
  toc_heading: "版本列表",
  sections: ZH_CHANGELOG_SECTIONS,
};
