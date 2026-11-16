use crate::content::{Locale, PageCopy, SupportLevel};
use std::fmt::Write;

pub const SITE_ORIGIN: &str = "https://dataomni.dev";
pub const REPO_URL: &str = "https://github.com/dataomni-rs/DataOmni";

pub fn escape_html(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  for ch in input.chars() {
    match ch {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      '\'' => out.push_str("&#39;"),
      _ => out.push(ch),
    }
  }
  out
}

struct ReleaseAssets {
  release_tag_url: String,
  dmg_url: String,
  exe_url: String,
  msi_url: String,
  deb_url: String,
  rpm_url: String,
}

impl ReleaseAssets {
  fn for_version(version: &str) -> Self {
    let base = format!("{REPO_URL}/releases/download/v{version}");
    Self {
      release_tag_url: format!("{REPO_URL}/releases/tag/v{version}"),
      dmg_url: format!("{base}/DataOmni_{version}_aarch64.dmg"),
      exe_url: format!("{base}/DataOmni_{version}_x64-setup.exe"),
      msi_url: format!("{base}/DataOmni_{version}_x64_en-US.msi"),
      deb_url: format!("{base}/DataOmni_{version}_amd64.deb"),
      rpm_url: format!("{base}/DataOmni-{version}-1.x86_64.rpm"),
    }
  }
}

pub fn render_page(copy: &PageCopy, version: &str) -> Result<String, std::fmt::Error> {
  let assets = ReleaseAssets::for_version(version);
  let canonical_url = format!("{SITE_ORIGIN}{}", copy.locale.canonical_path());
  let og_image_url = match copy.locale {
    Locale::En => format!("{SITE_ORIGIN}/images/en/overview.png"),
    Locale::Zh => format!("{SITE_ORIGIN}/images/zh/overview.png"),
  };
  let mcp_add_cmd =
    "claude mcp add dataomni -- /Applications/DataOmni.app/Contents/MacOS/dataomni mcp";
  let macos_quarantine_cmd = "xattr -dr com.apple.quarantine /Applications/DataOmni.app";
  let linux_install_cmd = format!("sudo apt install ./DataOmni_{version}_amd64.deb");

  let mut html = String::with_capacity(40_960);

  writeln!(html, "<!DOCTYPE html>")?;
  writeln!(html, "<html lang=\"{}\">", copy.locale.html_lang())?;
  writeln!(html, "<head>")?;
  writeln!(html, "  <meta charset=\"utf-8\">")?;
  writeln!(html, "  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">")?;
  writeln!(html, "  <title>{}</title>", escape_html(copy.meta_title))?;
  writeln!(
    html,
    "  <meta name=\"description\" content=\"{}\">",
    escape_html(copy.meta_description)
  )?;
  writeln!(html, "  <link rel=\"canonical\" href=\"{}\">", escape_html(&canonical_url))?;
  writeln!(html, "  <link rel=\"alternate\" hreflang=\"en\" href=\"{SITE_ORIGIN}/\">")?;
  writeln!(html, "  <link rel=\"alternate\" hreflang=\"zh-CN\" href=\"{SITE_ORIGIN}/zh/\">")?;
  writeln!(html, "  <link rel=\"alternate\" hreflang=\"x-default\" href=\"{SITE_ORIGIN}/\">")?;
  writeln!(html, "  <meta property=\"og:type\" content=\"website\">")?;
  writeln!(html, "  <meta property=\"og:site_name\" content=\"DataOmni\">")?;
  writeln!(html, "  <meta property=\"og:title\" content=\"{}\">", escape_html(copy.meta_title))?;
  writeln!(
    html,
    "  <meta property=\"og:description\" content=\"{}\">",
    escape_html(copy.meta_description)
  )?;
  writeln!(html, "  <meta property=\"og:url\" content=\"{}\">", escape_html(&canonical_url))?;
  writeln!(html, "  <meta property=\"og:image\" content=\"{}\">", escape_html(&og_image_url))?;
  writeln!(html, "  <meta name=\"twitter:card\" content=\"summary_large_image\">")?;
  writeln!(
    html,
    "  <link rel=\"icon\" type=\"image/png\" sizes=\"32x32\" href=\"/assets/favicon-32.png\">"
  )?;
  writeln!(html, "  <link rel=\"apple-touch-icon\" href=\"/assets/icon-256.png\">")?;
  writeln!(html, "  <script>")?;
  writeln!(
    html,
    "    try {{ const s = localStorage.getItem('dataomni-site-theme'); const d = s ? s === 'dark' : window.matchMedia('(prefers-color-scheme: dark)').matches; if (d) document.documentElement.setAttribute('data-theme', 'dark'); }} catch {{}}"
  )?;
  writeln!(html, "  </script>")?;
  writeln!(html, "  <link rel=\"stylesheet\" href=\"/assets/style.css\">")?;
  writeln!(html, "</head>")?;
  writeln!(html, "<body>")?;
  writeln!(
    html,
    "  <a class=\"skip-link\" href=\"#main\">{}</a>",
    escape_html(copy.skip_to_content)
  )?;

  // Top utility bar
  writeln!(html, "  <header class=\"topbar\">")?;
  writeln!(html, "    <div class=\"shell topbar-inner\">")?;
  writeln!(html, "      <a class=\"brand\" href=\"{}\">", copy.locale.canonical_path())?;
  writeln!(
    html,
    "        <img class=\"brand-icon\" src=\"/assets/icon-256.png\" alt=\"DataOmni icon\" width=\"28\" height=\"28\">"
  )?;
  writeln!(html, "        <span>DataOmni</span>")?;
  writeln!(html, "        <span class=\"version-pill\">v{}</span>", escape_html(version))?;
  writeln!(html, "      </a>")?;
  writeln!(html, "      <nav class=\"topnav\" aria-label=\"Primary\">")?;
  writeln!(html, "        <a href=\"#guardrails\">{}</a>", escape_html(copy.nav_guardrails))?;
  writeln!(html, "        <a href=\"#agents\">{}</a>", escape_html(copy.nav_agents))?;
  writeln!(html, "        <a href=\"#compatibility\">{}</a>", escape_html(copy.nav_compatibility))?;
  writeln!(html, "        <a href=\"#install\">{}</a>", escape_html(copy.nav_install))?;
  writeln!(html, "      </nav>")?;
  writeln!(html, "      <div class=\"topbar-actions\">")?;
  writeln!(
    html,
    "        <a class=\"control-btn\" href=\"{}\">{}</a>",
    copy.locale.alternate_path(),
    escape_html(copy.locale.alternate_label())
  )?;
  writeln!(
    html,
    "        <button id=\"theme-toggle\" class=\"control-btn\" type=\"button\" aria-pressed=\"false\" data-label-light=\"{}\" data-label-dark=\"{}\">{}</button>",
    escape_html(copy.theme_light),
    escape_html(copy.theme_dark),
    escape_html(copy.theme_dark)
  )?;
  writeln!(
    html,
    "        <a class=\"control-btn\" href=\"{REPO_URL}\" rel=\"noopener\">GitHub</a>"
  )?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </div>")?;
  writeln!(html, "  </header>")?;

  writeln!(html, "  <main id=\"main\">")?;

  // Hero split
  writeln!(html, "    <section class=\"hero\">")?;
  writeln!(html, "      <div class=\"shell hero-grid\">")?;
  writeln!(html, "        <div>")?;
  writeln!(html, "          <h1 class=\"hero-title\">{}</h1>", escape_html(copy.hero_title))?;
  writeln!(html, "          <p class=\"hero-lead\">{}</p>", escape_html(copy.hero_lead))?;
  writeln!(html, "          <div class=\"download-box\">")?;
  writeln!(html, "            <div class=\"download-primary-row\">")?;
  writeln!(
    html,
    "              <a id=\"primary-download-btn\" class=\"btn-primary\" href=\"{}\">{} v{}</a>",
    escape_html(&assets.release_tag_url),
    escape_html(copy.primary_download_default),
    escape_html(version)
  )?;
  writeln!(
    html,
    "              <a class=\"btn-secondary\" href=\"{}\">{} v{}</a>",
    escape_html(&assets.release_tag_url),
    escape_html(copy.release_notes_cta),
    escape_html(version)
  )?;
  writeln!(html, "            </div>")?;
  writeln!(html, "            <div class=\"download-matrix-inline\">")?;
  writeln!(html, "              <span>{}</span>", escape_html(copy.all_packages_label))?;
  writeln!(
    html,
    "              <a href=\"{}\" data-os=\"macos\" data-os-primary=\"macos\" data-primary-cta=\"{}\">macOS .dmg</a>",
    escape_html(&assets.dmg_url),
    escape_html(copy.primary_cta_macos)
  )?;
  writeln!(
    html,
    "              <a href=\"{}\" data-os=\"windows\" data-os-primary=\"windows\" data-primary-cta=\"{}\">Windows .exe</a>",
    escape_html(&assets.exe_url),
    escape_html(copy.primary_cta_windows)
  )?;
  writeln!(
    html,
    "              <a href=\"{}\" data-os=\"windows\">Windows .msi</a>",
    escape_html(&assets.msi_url)
  )?;
  writeln!(
    html,
    "              <a href=\"{}\" data-os=\"linux\" data-os-primary=\"linux\" data-primary-cta=\"{}\">Linux .deb</a>",
    escape_html(&assets.deb_url),
    escape_html(copy.primary_cta_linux)
  )?;
  writeln!(
    html,
    "              <a href=\"{}\" data-os=\"linux\">Linux .rpm</a>",
    escape_html(&assets.rpm_url)
  )?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </div>")?;
  writeln!(html, "        </div>")?;

  writeln!(html, "        <dl class=\"spec-sheet\">")?;
  for item in copy.specs {
    writeln!(html, "          <div class=\"spec-row\">")?;
    writeln!(html, "            <dt>{}</dt>", escape_html(item.term))?;
    writeln!(html, "            <dd>{}</dd>", escape_html(item.detail))?;
    writeln!(html, "          </div>")?;
  }
  writeln!(html, "        </dl>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Interactive Workbench Frame
  writeln!(
    html,
    "    <section class=\"workbench-section\" aria-label=\"{}\">",
    escape_html(copy.workbench_aria_label)
  )?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"workbench-frame\">")?;
  writeln!(html, "          <div class=\"workbench-bar\">")?;
  writeln!(
    html,
    "            <div class=\"workbench-tabs\" role=\"tablist\" aria-label=\"{}\">",
    escape_html(copy.workbench_aria_label)
  )?;
  for (index, tab) in copy.tabs.iter().enumerate() {
    let selected = if index == 0 { "true" } else { "false" };
    let tabindex = if index == 0 { "0" } else { "-1" };
    writeln!(
      html,
      "              <button id=\"tab-{id}\" class=\"workbench-tab\" type=\"button\" role=\"tab\" aria-selected=\"{selected}\" aria-controls=\"panel-{id}\" tabindex=\"{tabindex}\" data-tab-target=\"{id}\" data-caption-title=\"{ct}\" data-caption-body=\"{cb}\"><span>{label}</span><span class=\"tab-badge\">{badge}</span></button>",
      id = escape_html(tab.id),
      selected = selected,
      tabindex = tabindex,
      ct = escape_html(tab.caption_title),
      cb = escape_html(tab.caption_body),
      label = escape_html(tab.tab_label),
      badge = escape_html(tab.badge),
    )?;
  }
  writeln!(html, "            </div>")?;
  writeln!(html, "          </div>")?;

  writeln!(html, "          <div class=\"workbench-stage\">")?;
  for (index, tab) in copy.tabs.iter().enumerate() {
    let hidden_attr = if index == 0 { "" } else { " hidden" };
    writeln!(
      html,
      "            <div id=\"panel-{id}\" class=\"workbench-panel\" role=\"tabpanel\" aria-labelledby=\"tab-{id}\" data-tab-panel=\"{id}\"{hidden_attr}>",
      id = escape_html(tab.id),
      hidden_attr = hidden_attr,
    )?;
    if index == 0 {
      writeln!(
        html,
        "              <img class=\"workbench-shot\" src=\"{src}\" alt=\"{alt}\" width=\"{w}\" height=\"{h}\" fetchpriority=\"high\">",
        src = escape_html(tab.image_path),
        alt = escape_html(tab.alt),
        w = tab.width,
        h = tab.height,
      )?;
    } else {
      writeln!(
        html,
        "              <img class=\"workbench-shot\" src=\"data:image/gif;base64,R0lGODlhAQABAAAAACw=\" data-deferred-src=\"{src}\" alt=\"{alt}\" width=\"{w}\" height=\"{h}\" decoding=\"async\">",
        src = escape_html(tab.image_path),
        alt = escape_html(tab.alt),
        w = tab.width,
        h = tab.height,
      )?;
    }
    writeln!(html, "            </div>")?;
  }
  writeln!(html, "          </div>")?;

  if let Some(first_tab) = copy.tabs.first() {
    writeln!(html, "          <div class=\"workbench-caption\">")?;
    writeln!(
      html,
      "            <div><strong id=\"workbench-caption-title\">{}</strong> — <span id=\"workbench-caption-body\">{}</span></div>",
      escape_html(first_tab.caption_title),
      escape_html(first_tab.caption_body)
    )?;
    writeln!(html, "          </div>")?;
  }
  writeln!(html, "        </div>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Guardrails section
  writeln!(html, "    <section id=\"guardrails\" class=\"section\">")?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"section-header\">")?;
  writeln!(html, "          <h2>{}</h2>", escape_html(copy.guardrails_heading))?;
  writeln!(html, "          <p>{}</p>", escape_html(copy.guardrails_intro))?;
  writeln!(html, "        </div>")?;
  writeln!(html, "        <div class=\"split-grid\">")?;
  writeln!(html, "          <ul class=\"feature-list\">")?;
  for item in copy.guardrails_items {
    writeln!(html, "            <li class=\"feature-item\">")?;
    writeln!(html, "              <h3>{}</h3>", escape_html(item.title))?;
    writeln!(html, "              <p>{}</p>", escape_html(item.body))?;
    writeln!(html, "            </li>")?;
  }
  writeln!(html, "          </ul>")?;
  writeln!(html, "          <div class=\"code-panel\">")?;
  writeln!(html, "            <div class=\"code-panel-bar\">")?;
  writeln!(html, "              <span>{}</span>", escape_html(copy.sql_preview_bar))?;
  writeln!(html, "              <span class=\"env-pill-ok\">1 TRANSACTION</span>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "            <pre><code><span class=\"code-comment\">-- Previewed before execution; bound via driver parameters</span>\nBEGIN;\n\nUPDATE \"public\".\"products\"\n   SET \"price\" = $1          <span class=\"code-comment\">-- 489</span>\n WHERE \"id\" = $2             <span class=\"code-comment\">-- 1 (primary key)</span>\n   AND \"price\" = $3;         <span class=\"code-comment\">-- 429 (optimistic check)</span>\n\n<span class=\"code-comment\">-- Verify affected_rows == 1, then:</span>\nCOMMIT;</code></pre>")?;
  writeln!(
    html,
    "            <div class=\"code-note\">{}</div>",
    escape_html(copy.sql_preview_note)
  )?;
  writeln!(html, "          </div>")?;
  writeln!(html, "        </div>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Agents CLI & MCP section
  writeln!(html, "    <section id=\"agents\" class=\"section\">")?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"section-header\">")?;
  writeln!(html, "          <h2>{}</h2>", escape_html(copy.agents_heading))?;
  writeln!(html, "          <p>{}</p>", escape_html(copy.agents_intro))?;
  writeln!(html, "        </div>")?;
  writeln!(html, "        <div class=\"split-grid\">")?;
  writeln!(html, "          <ul class=\"feature-list\">")?;
  for item in copy.agents_items {
    writeln!(html, "            <li class=\"feature-item\">")?;
    writeln!(html, "              <h3>{}</h3>", escape_html(item.title))?;
    writeln!(html, "              <p>{}</p>", escape_html(item.body))?;
    writeln!(html, "            </li>")?;
  }
  writeln!(html, "          </ul>")?;
  writeln!(html, "          <div class=\"code-panel\">")?;
  writeln!(html, "            <div class=\"code-panel-bar\">")?;
  writeln!(html, "              <span>{}</span>", escape_html(copy.agents_terminal_bar))?;
  writeln!(html, "              <span class=\"env-pill-prod\">PROD HIDDEN</span>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "            <pre><code><span class=\"code-comment\">$ dataomni cli connections</span>\n[{{\"name\":\"shop_dev\",\"db_type\":\"postgres\",\"access\":\"read_only\"}}]\n\n<span class=\"code-comment\">$ dataomni cli query shop_dev \"SELECT id, total FROM orders LIMIT 2\"</span>\n{{\"columns\":[\"id\",\"total\"],\"rows\":[[1,1467],[2,1032.5]],\"truncated\":false}}\n\n<span class=\"code-comment\">$ dataomni cli query shop_dev \"DELETE FROM orders\"</span>\nrefused: statement is not read-only (exit code 3)\n\n<span class=\"code-comment\">$ dataomni cli guide &gt; ~/.claude/skills/dataomni/SKILL.md</span></code></pre>")?;
  writeln!(
    html,
    "            <div class=\"code-note\">{}</div>",
    escape_html(copy.agents_mcp_cmd_note)
  )?;
  writeln!(html, "            <div class=\"copy-cmd-row\">")?;
  writeln!(html, "              <code>{}</code>", escape_html(mcp_add_cmd))?;
  writeln!(
    html,
    "              <button class=\"copy-btn\" type=\"button\" data-copy-text=\"{}\" data-default-label=\"{}\" data-copied-label=\"{}\">{}</button>",
    escape_html(mcp_add_cmd),
    escape_html(copy.copy_label),
    escape_html(copy.copied_label),
    escape_html(copy.copy_label)
  )?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </div>")?;
  writeln!(html, "        </div>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Smoke-tested Compatibility Ledger
  writeln!(html, "    <section id=\"compatibility\" class=\"section\">")?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"section-header\">")?;
  writeln!(html, "          <h2>{}</h2>", escape_html(copy.compatibility_heading))?;
  writeln!(html, "          <p>{}</p>", escape_html(copy.compatibility_intro))?;
  writeln!(html, "        </div>")?;
  writeln!(html, "        <div class=\"ledger-controls\">")?;
  writeln!(
    html,
    "          <div class=\"filter-group\" role=\"group\" aria-label=\"Database category\">"
  )?;
  writeln!(
    html,
    "            <button class=\"filter-btn\" type=\"button\" aria-pressed=\"true\" data-ledger-filter=\"all\">{}</button>",
    escape_html(copy.filter_all)
  )?;
  writeln!(
    html,
    "            <button class=\"filter-btn\" type=\"button\" aria-pressed=\"false\" data-ledger-filter=\"relational\">{}</button>",
    escape_html(copy.filter_relational)
  )?;
  writeln!(
    html,
    "            <button class=\"filter-btn\" type=\"button\" aria-pressed=\"false\" data-ledger-filter=\"workspace\">{}</button>",
    escape_html(copy.filter_workspaces)
  )?;
  writeln!(html, "          </div>")?;
  writeln!(html, "        </div>")?;
  writeln!(html, "        <div class=\"ledger-wrap\">")?;
  writeln!(html, "          <table class=\"ledger-table\">")?;
  writeln!(html, "            <thead>")?;
  writeln!(html, "              <tr>")?;
  writeln!(html, "                <th scope=\"col\">{}</th>", escape_html(copy.col_server))?;
  writeln!(html, "                <th scope=\"col\">{}</th>", escape_html(copy.col_version))?;
  writeln!(html, "                <th scope=\"col\">{}</th>", escape_html(copy.col_driver))?;
  writeln!(html, "                <th scope=\"col\">{}</th>", escape_html(copy.col_status))?;
  writeln!(html, "                <th scope=\"col\">{}</th>", escape_html(copy.col_smoke))?;
  writeln!(html, "                <th scope=\"col\">{}</th>", escape_html(copy.col_notes))?;
  writeln!(html, "              </tr>")?;
  writeln!(html, "            </thead>")?;
  writeln!(html, "            <tbody>")?;
  for row in copy.servers {
    let (status_class, status_label) = match row.level {
      SupportLevel::Supported => ("status-ok", copy.status_supported),
      SupportLevel::UsableWithGaps => ("status-gap", copy.status_gaps),
    };
    writeln!(html, "              <tr data-ledger-kind=\"{}\">", row.kind.as_attr())?;
    writeln!(html, "                <td class=\"ledger-server\">{}</td>", escape_html(row.server))?;
    writeln!(
      html,
      "                <td class=\"ledger-mono\">{}</td>",
      escape_html(row.tested_version)
    )?;
    writeln!(html, "                <td>{}</td>", escape_html(row.connection_type))?;
    writeln!(
      html,
      "                <td><span class=\"{status_class}\">{}</span></td>",
      escape_html(status_label)
    )?;
    writeln!(
      html,
      "                <td class=\"ledger-mono\">{}</td>",
      escape_html(row.smoke_counts)
    )?;
    writeln!(html, "                <td class=\"ledger-note\">{}</td>", escape_html(row.notes))?;
    writeln!(html, "              </tr>")?;
  }
  writeln!(html, "            </tbody>")?;
  writeln!(html, "          </table>")?;
  writeln!(html, "        </div>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Install section
  writeln!(html, "    <section id=\"install\" class=\"section\">")?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"section-header\">")?;
  writeln!(
    html,
    "          <h2>{} v{}</h2>",
    escape_html(copy.install_heading),
    escape_html(version)
  )?;
  writeln!(html, "          <p>{}</p>", escape_html(copy.install_intro))?;
  writeln!(html, "        </div>")?;
  writeln!(html, "        <div class=\"install-grid\">")?;

  // macOS card
  writeln!(html, "          <div class=\"install-card\" data-os=\"macos\">")?;
  writeln!(html, "            <div>")?;
  writeln!(html, "              <h3>{}</h3>", escape_html(copy.macos_title))?;
  writeln!(html, "              <p class=\"install-meta\">{}</p>", escape_html(copy.macos_meta))?;
  writeln!(html, "              <div class=\"install-links\">")?;
  writeln!(
    html,
    "                <a class=\"btn-primary\" href=\"{}\">DataOmni_{}_aarch64.dmg</a>",
    escape_html(&assets.dmg_url),
    escape_html(version)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "            <div>")?;
  writeln!(html, "              <p class=\"install-note\">{}</p>", escape_html(copy.macos_note))?;
  writeln!(html, "              <div class=\"install-cmd\">")?;
  writeln!(html, "                <code>{}</code>", escape_html(macos_quarantine_cmd))?;
  writeln!(
    html,
    "                <button class=\"copy-btn\" type=\"button\" data-copy-text=\"{}\" data-default-label=\"{}\" data-copied-label=\"{}\">{}</button>",
    escape_html(macos_quarantine_cmd),
    escape_html(copy.copy_label),
    escape_html(copy.copied_label),
    escape_html(copy.copy_label)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </div>")?;

  // Windows card
  writeln!(html, "          <div class=\"install-card\" data-os=\"windows\">")?;
  writeln!(html, "            <div>")?;
  writeln!(html, "              <h3>{}</h3>", escape_html(copy.windows_title))?;
  writeln!(html, "              <p class=\"install-meta\">{}</p>", escape_html(copy.windows_meta))?;
  writeln!(html, "              <div class=\"install-links\">")?;
  writeln!(
    html,
    "                <a class=\"btn-primary\" href=\"{}\">NSIS setup.exe</a>",
    escape_html(&assets.exe_url)
  )?;
  writeln!(
    html,
    "                <a class=\"btn-secondary\" href=\"{}\">MSI installer</a>",
    escape_html(&assets.msi_url)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "            <div>")?;
  writeln!(html, "              <p class=\"install-note\">{}</p>", escape_html(copy.windows_note))?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </div>")?;

  // Linux card
  writeln!(html, "          <div class=\"install-card\" data-os=\"linux\">")?;
  writeln!(html, "            <div>")?;
  writeln!(html, "              <h3>{}</h3>", escape_html(copy.linux_title))?;
  writeln!(html, "              <p class=\"install-meta\">{}</p>", escape_html(copy.linux_meta))?;
  writeln!(html, "              <div class=\"install-links\">")?;
  writeln!(
    html,
    "                <a class=\"btn-primary\" href=\"{}\">.deb (amd64)</a>",
    escape_html(&assets.deb_url)
  )?;
  writeln!(
    html,
    "                <a class=\"btn-secondary\" href=\"{}\">.rpm (x86_64)</a>",
    escape_html(&assets.rpm_url)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "            <div>")?;
  writeln!(html, "              <p class=\"install-note\">{}</p>", escape_html(copy.linux_note))?;
  writeln!(html, "              <div class=\"install-cmd\">")?;
  writeln!(html, "                <code>{}</code>", escape_html(&linux_install_cmd))?;
  writeln!(
    html,
    "                <button class=\"copy-btn\" type=\"button\" data-copy-text=\"{}\" data-default-label=\"{}\" data-copied-label=\"{}\">{}</button>",
    escape_html(&linux_install_cmd),
    escape_html(copy.copy_label),
    escape_html(copy.copied_label),
    escape_html(copy.copy_label)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </div>")?;

  writeln!(html, "        </div>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;
  writeln!(html, "  </main>")?;

  // Footer
  writeln!(html, "  <footer class=\"site-footer\">")?;
  writeln!(html, "    <div class=\"shell footer-inner\">")?;
  writeln!(html, "      <span>{}</span>", escape_html(copy.footer_summary))?;
  writeln!(html, "      <div class=\"footer-links\">")?;
  writeln!(
    html,
    "        <a href=\"{REPO_URL}/blob/main/docs/user-manual.md\">{}</a>",
    escape_html(copy.footer_manual)
  )?;
  writeln!(
    html,
    "        <a href=\"{REPO_URL}/blob/main/rfcs/agent-cli.md\">{}</a>",
    escape_html(copy.footer_cli_rfc)
  )?;
  writeln!(
    html,
    "        <a href=\"{REPO_URL}/releases\">{}</a>",
    escape_html(copy.footer_releases)
  )?;
  writeln!(html, "        <a href=\"{REPO_URL}/issues\">{}</a>", escape_html(copy.footer_issues))?;
  writeln!(
    html,
    "        <a href=\"{REPO_URL}/blob/main/LICENSE\">{}</a>",
    escape_html(copy.footer_license)
  )?;
  writeln!(
    html,
    "        <a href=\"{}\">{}</a>",
    copy.locale.alternate_path(),
    escape_html(copy.locale.alternate_label())
  )?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </div>")?;
  writeln!(html, "  </footer>")?;

  writeln!(html, "  <script src=\"/assets/site.js\" defer></script>")?;
  writeln!(html, "</body>")?;
  writeln!(html, "</html>")?;

  Ok(html)
}

pub fn render_not_found() -> &'static str {
  "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
  <title>404 — DataOmni</title>\n\
  <link rel=\"icon\" type=\"image/png\" sizes=\"32x32\" href=\"/assets/favicon-32.png\">\n\
  <link rel=\"stylesheet\" href=\"/assets/style.css\">\n\
</head>\n\
<body>\n\
  <main class=\"shell hero\">\n\
    <h1 class=\"hero-title\">Page not found (404)</h1>\n\
    <p class=\"hero-lead\">The requested path does not exist on dataomni.dev.</p>\n\
    <p><a class=\"btn-primary\" href=\"/\">English home</a> <a class=\"btn-secondary\" href=\"/zh/\">中文首页</a></p>\n\
  </main>\n\
</body>\n\
</html>\n"
}

pub fn render_robots_txt() -> String {
  format!("User-agent: *\nAllow: /\nSitemap: {SITE_ORIGIN}/sitemap.xml\n")
}

pub fn render_sitemap_xml() -> String {
  format!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n\
  <url><loc>{SITE_ORIGIN}/</loc></url>\n\
  <url><loc>{SITE_ORIGIN}/zh/</loc></url>\n\
</urlset>\n"
  )
}
