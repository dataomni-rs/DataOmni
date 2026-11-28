use crate::content::{
  Locale, PageCopy, ServerKind, SupportLevel, TrustIcon, WorkflowStory, EN_COPY, ZH_COPY,
};
use crate::docs::{doc_page, CalloutTone, CodeLang, DocBlock, DocPage, DocSlug, ALL_DOC_SLUGS};
use std::fmt::Write;

pub const SITE_ORIGIN: &str = "https://dataomni.dev";
pub const REPO_URL: &str = "https://github.com/dataomni-rs/DataOmni";
pub const TAP_REPO_URL: &str = "https://github.com/dataomni-rs/homebrew-tap";
pub const INDEXNOW_KEY: &str = "da7a0f91c8e24f5bb3109a6e7d2c84f1";
pub const ROBOTS_INDEX_DIRECTIVE: &str =
  "index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1";

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

fn escape_json_str(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  for ch in input.chars() {
    match ch {
      '\\' => out.push_str("\\\\"),
      '"' => out.push_str("\\\""),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      '<' => out.push_str("\\u003c"),
      '>' => out.push_str("\\u003e"),
      '&' => out.push_str("\\u0026"),
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

fn trust_icon_svg(icon: TrustIcon) -> &'static str {
  match icon {
    TrustIcon::License => {
      "<svg viewBox=\"0 0 20 20\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.65\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M10 2.5 16 5v4.6c0 4-2.6 6.9-6 8-3.4-1.1-6-4-6-8V5l6-2.5Z\"/><path d=\"m7.5 10.1 1.8 1.8 3.4-3.6\"/></svg>"
    }
    TrustIcon::Offline => {
      "<svg viewBox=\"0 0 20 20\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.65\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><rect x=\"3\" y=\"4\" width=\"14\" height=\"9.5\" rx=\"1.8\"/><path d=\"M6.8 16.5h6.4M10 13.5v3\"/><path d=\"m7.6 8.8 1.6 1.6 3.2-3.2\"/></svg>"
    }
    TrustIcon::Keychain => {
      "<svg viewBox=\"0 0 20 20\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.65\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><circle cx=\"7.2\" cy=\"10\" r=\"3.4\"/><path d=\"M10.6 10h6.4M14.6 10v2.4M17 10v1.8\"/></svg>"
    }
    TrustIcon::Oracle => {
      "<svg viewBox=\"0 0 20 20\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.65\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><ellipse cx=\"10\" cy=\"5.2\" rx=\"5.8\" ry=\"2.4\"/><path d=\"M4.2 5.2v4.8c0 1.3 2.6 2.4 5.8 2.4s5.8-1.1 5.8-2.4V5.2\"/><path d=\"M4.2 10v4.8c0 1.3 2.6 2.4 5.8 2.4s5.8-1.1 5.8-2.4V10\"/></svg>"
    }
  }
}

fn render_workflow_story(html: &mut String, story: &WorkflowStory) -> Result<(), std::fmt::Error> {
  writeln!(html, "            <div class=\"workflow-story\">")?;
  writeln!(html, "              <h3>{}</h3>", escape_html(story.title))?;
  writeln!(html, "              <p class=\"workflow-lead\">{}</p>", escape_html(story.lead))?;
  writeln!(html, "              <ul class=\"workflow-bullets\">")?;
  for bullet in story.bullets {
    writeln!(html, "                <li>{}</li>", escape_html(bullet))?;
  }
  writeln!(html, "              </ul>")?;
  writeln!(html, "            </div>")?;
  Ok(())
}

fn render_guardrails_sql() -> &'static str {
  concat!(
    "<span class=\"code-comment\">-- Previewed before execution; bound via driver parameters</span>\n",
    "<span class=\"tok-kw\">BEGIN</span>;\n\n",
    "<span class=\"tok-kw\">UPDATE</span> <span class=\"tok-ident\">&quot;public&quot;.&quot;products&quot;</span>\n",
    "   <span class=\"tok-kw\">SET</span> <span class=\"tok-ident\">&quot;price&quot;</span> = <span class=\"tok-param\">$1</span>          <span class=\"code-comment\">-- 489</span>\n",
    " <span class=\"tok-kw\">WHERE</span> <span class=\"tok-ident\">&quot;id&quot;</span> = <span class=\"tok-param\">$2</span>             <span class=\"code-comment\">-- 1 (primary key)</span>\n",
    "   <span class=\"tok-kw\">AND</span> <span class=\"tok-ident\">&quot;price&quot;</span> = <span class=\"tok-param\">$3</span>;         <span class=\"code-comment\">-- 429 (optimistic check)</span>\n\n",
    "<span class=\"code-comment\">-- Verify affected_rows == 1, then:</span>\n",
    "<span class=\"tok-kw\">COMMIT</span>;"
  )
}

fn render_schema_codegen() -> &'static str {
  concat!(
    "<span class=\"code-comment\">-- 1. Schema diff sync script (staging -&gt; dev)</span>\n",
    "<span class=\"tok-kw\">ALTER TABLE</span> <span class=\"tok-ident\">&quot;public&quot;.&quot;orders&quot;</span>\n",
    "  <span class=\"tok-kw\">ADD COLUMN</span> <span class=\"tok-ident\">&quot;currency&quot;</span> <span class=\"tok-type\">varchar</span>(<span class=\"tok-num\">3</span>) <span class=\"tok-kw\">NOT NULL DEFAULT</span> <span class=\"tok-str\">&#39;USD&#39;</span>;\n",
    "<span class=\"tok-kw\">CREATE INDEX</span> <span class=\"tok-ident\">&quot;idx_orders_customer_created&quot;</span>\n",
    "  <span class=\"tok-kw\">ON</span> <span class=\"tok-ident\">&quot;public&quot;.&quot;orders&quot;</span> (<span class=\"tok-ident\">&quot;customer_id&quot;</span>, <span class=\"tok-ident\">&quot;created_at&quot;</span>);\n\n",
    "<span class=\"code-comment\">// 2. Deterministic ER -&gt; sqlx / Prisma / SeaORM / TypeORM</span>\n",
    "<span class=\"tok-attr\">#[derive(</span><span class=\"tok-type\">Debug</span>, <span class=\"tok-type\">Clone</span>, <span class=\"tok-attr\">sqlx::</span><span class=\"tok-type\">FromRow</span><span class=\"tok-attr\">)]</span>\n",
    "<span class=\"tok-kw\">pub struct</span> <span class=\"tok-type\">Order</span> {\n",
    "  <span class=\"tok-kw\">pub</span> <span class=\"tok-prop\">id</span>: <span class=\"tok-type\">i64</span>,\n",
    "  <span class=\"tok-kw\">pub</span> <span class=\"tok-prop\">customer_id</span>: <span class=\"tok-type\">i64</span>,\n",
    "  <span class=\"tok-kw\">pub</span> <span class=\"tok-prop\">currency</span>: <span class=\"tok-type\">String</span>,\n",
    "  <span class=\"tok-kw\">pub</span> <span class=\"tok-prop\">total_cents</span>: <span class=\"tok-type\">i64</span>,\n",
    "}"
  )
}

fn render_agents_cli(cli_comment: &str) -> String {
  format!(
    concat!(
      "<span class=\"tok-prompt\">$</span> <span class=\"tok-cmd\">dataomni cli connections</span>\n",
      "[{{<span class=\"tok-prop\">&quot;name&quot;</span>:<span class=\"tok-str\">&quot;shop_dev&quot;</span>,<span class=\"tok-prop\">&quot;db_type&quot;</span>:<span class=\"tok-str\">&quot;postgres&quot;</span>,<span class=\"tok-prop\">&quot;access&quot;</span>:<span class=\"tok-str\">&quot;read_only&quot;</span>}}]\n\n",
      "<span class=\"tok-prompt\">$</span> <span class=\"tok-cmd\">dataomni cli query</span> shop_dev <span class=\"tok-str\">&quot;SELECT id, total FROM orders LIMIT 2&quot;</span>\n",
      "{{<span class=\"tok-prop\">&quot;columns&quot;</span>:[<span class=\"tok-str\">&quot;id&quot;</span>,<span class=\"tok-str\">&quot;total&quot;</span>],\n",
      " <span class=\"tok-prop\">&quot;rows&quot;</span>:[[<span class=\"tok-num\">1</span>,<span class=\"tok-num\">1467</span>],[<span class=\"tok-num\">2</span>,<span class=\"tok-num\">1032.5</span>]],<span class=\"tok-prop\">&quot;truncated&quot;</span>:<span class=\"tok-num\">false</span>}}\n\n",
      "<span class=\"tok-prompt\">$</span> <span class=\"tok-cmd\">dataomni cli query</span> shop_dev <span class=\"tok-str\">&quot;DELETE FROM orders&quot;</span>\n",
      "<span class=\"tok-err\">refused: statement is not read-only (exit code 3)</span>\n\n",
      "<span class=\"code-comment\">{}</span>\n",
      "<span class=\"tok-prompt\">$</span> <span class=\"tok-cmd\">dataomni cli guide</span>"
    ),
    escape_html(cli_comment)
  )
}

fn render_smoke_counts(html: &mut String, counts: &str) -> Result<(), std::fmt::Error> {
  let parts: Vec<&str> = counts.split('/').map(str::trim).collect();
  if let [passed, skipped, unsupported] = parts.as_slice() {
    let classify_secondary = |val: &str| {
      if val == "0" || val == "0*" {
        "smoke-zero"
      } else {
        "smoke-warn"
      }
    };
    write!(
      html,
      "<span class=\"smoke-triplet\"><span class=\"smoke-pass\">{}</span><span class=\"smoke-sep\">/</span><span class=\"{}\">{}</span><span class=\"smoke-sep\">/</span><span class=\"{}\">{}</span></span>",
      escape_html(passed),
      classify_secondary(skipped),
      escape_html(skipped),
      classify_secondary(unsupported),
      escape_html(unsupported),
    )
  } else {
    write!(html, "{}", escape_html(counts))
  }
}

fn render_organization_node(json: &mut String) -> Result<(), std::fmt::Error> {
  write!(
    json,
    "{{\"@type\":\"Organization\",\"@id\":\"{SITE_ORIGIN}/#organization\",\"name\":\"DataOmni\",\"url\":\"{SITE_ORIGIN}/\",\"logo\":{{\"@type\":\"ImageObject\",\"url\":\"{SITE_ORIGIN}/assets/icon-256.png\",\"width\":256,\"height\":256}},\"sameAs\":[\"{REPO_URL}\",\"{TAP_REPO_URL}\"]}}"
  )
}

fn render_structured_data(
  copy: &PageCopy,
  version: &str,
  released_at: &str,
  canonical_url: &str,
  og_image_url: &str,
  release_tag_url: &str,
) -> Result<String, std::fmt::Error> {
  let mut json = String::with_capacity(4608);
  write!(json, "{{\"@context\":\"https://schema.org\",\"@graph\":[")?;

  // 0. Brand Organization entity with logo and verified repositories
  render_organization_node(&mut json)?;
  json.push(',');

  // 1. WebSite entity (Google SERP Site Name signal)
  write!(
    json,
    "{{\"@type\":\"WebSite\",\"@id\":\"{SITE_ORIGIN}/#website\",\"name\":\"DataOmni\",\"alternateName\":\"DataOmni Database Client\",\"url\":\"{SITE_ORIGIN}/\",\"publisher\":{{\"@id\":\"{SITE_ORIGIN}/#organization\"}},\"inLanguage\":[\"en\",\"zh-CN\"]}},"
  )?;

  // 2. SoftwareApplication entity with screenshots, repository, and free MIT offer
  write!(
    json,
    "{{\"@type\":\"SoftwareApplication\",\"@id\":\"{SITE_ORIGIN}/#software\",\"name\":\"DataOmni\",\"softwareVersion\":\"{}\",\"datePublished\":\"2026-11-17\",\"dateModified\":\"{}\",\"applicationCategory\":\"DeveloperApplication\",\"applicationSubCategory\":\"Database Management\",\"operatingSystem\":\"macOS 12+, Windows 10/11, Linux (x86_64)\",\"license\":\"https://opensource.org/licenses/MIT\",\"isAccessibleForFree\":true,\"inLanguage\":\"{}\",\"url\":\"{}\",\"downloadUrl\":\"{}\",\"releaseNotes\":\"{}\",\"sameAs\":[\"{REPO_URL}\",\"{TAP_REPO_URL}\"],\"image\":\"{}\",\"screenshot\":[",
    escape_json_str(version),
    escape_json_str(released_at),
    escape_json_str(copy.locale.html_lang()),
    escape_json_str(canonical_url),
    escape_json_str(release_tag_url),
    escape_json_str(release_tag_url),
    escape_json_str(og_image_url),
  )?;
  for (index, tab) in copy.tabs.iter().enumerate() {
    if index > 0 {
      json.push(',');
    }
    let shot_url = format!("{SITE_ORIGIN}{}", tab.webp_path);
    write!(
      json,
      "{{\"@type\":\"ImageObject\",\"url\":\"{}\",\"caption\":\"{}\",\"width\":{},\"height\":{}}}",
      escape_json_str(&shot_url),
      escape_json_str(tab.alt),
      tab.width,
      tab.height,
    )?;
  }
  write!(
    json,
    "],\"description\":\"{}\",\"author\":{{\"@id\":\"{SITE_ORIGIN}/#organization\"}},\"offers\":{{\"@type\":\"Offer\",\"price\":\"0\",\"priceCurrency\":\"USD\"}}}},",
    escape_json_str(copy.meta_description),
  )?;

  // 3. FAQPage entity for AI Overviews & Generative Engine citation
  write!(
    json,
    "{{\"@type\":\"FAQPage\",\"@id\":\"{}#faq-schema\",\"inLanguage\":\"{}\",\"mainEntity\":[",
    escape_json_str(canonical_url),
    escape_json_str(copy.locale.html_lang()),
  )?;
  for (index, item) in copy.faq_items.iter().enumerate() {
    if index > 0 {
      json.push(',');
    }
    write!(
      json,
      "{{\"@type\":\"Question\",\"name\":\"{}\",\"acceptedAnswer\":{{\"@type\":\"Answer\",\"text\":\"{}\"}}}}",
      escape_json_str(item.question),
      escape_json_str(item.answer),
    )?;
  }
  write!(json, "]}}]}}")?;

  Ok(json)
}

pub fn render_page(
  copy: &PageCopy,
  version: &str,
  released_at: &str,
) -> Result<String, std::fmt::Error> {
  let assets = ReleaseAssets::for_version(version);
  let canonical_url = format!("{SITE_ORIGIN}{}", copy.locale.canonical_path());
  let og_image_url = format!("{SITE_ORIGIN}{}", copy.locale.og_image_path());
  let structured_data = render_structured_data(
    copy,
    version,
    released_at,
    &canonical_url,
    &og_image_url,
    &assets.release_tag_url,
  )?;
  let macos_brew_cmd = "brew install --cask dataomni-rs/tap/dataomni";
  let linux_install_cmd = format!("sudo apt install ./DataOmni_{version}_amd64.deb");

  let mut html = String::with_capacity(68_608);

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
  writeln!(html, "  <meta name=\"robots\" content=\"{ROBOTS_INDEX_DIRECTIVE}\">")?;
  writeln!(html, "  <link rel=\"canonical\" href=\"{}\">", escape_html(&canonical_url))?;
  writeln!(html, "  <link rel=\"alternate\" hreflang=\"en\" href=\"{SITE_ORIGIN}/\">")?;
  writeln!(html, "  <link rel=\"alternate\" hreflang=\"zh-CN\" href=\"{SITE_ORIGIN}/zh/\">")?;
  writeln!(html, "  <link rel=\"alternate\" hreflang=\"x-default\" href=\"{SITE_ORIGIN}/\">")?;
  writeln!(
    html,
    "  <link rel=\"alternate\" type=\"text/plain\" href=\"/llms.txt\" title=\"DataOmni LLM Documentation\">"
  )?;
  writeln!(
    html,
    "  <link rel=\"alternate\" type=\"text/markdown\" href=\"/SKILL.md\" title=\"DataOmni CLI Agent Skill (SKILL.md)\">"
  )?;
  writeln!(html, "  <meta property=\"og:type\" content=\"website\">")?;
  writeln!(html, "  <meta property=\"og:site_name\" content=\"DataOmni\">")?;
  writeln!(html, "  <meta property=\"og:locale\" content=\"{}\">", copy.locale.og_locale())?;
  writeln!(
    html,
    "  <meta property=\"og:locale:alternate\" content=\"{}\">",
    copy.locale.og_alternate_locale()
  )?;
  writeln!(html, "  <meta property=\"og:title\" content=\"{}\">", escape_html(copy.meta_title))?;
  writeln!(
    html,
    "  <meta property=\"og:description\" content=\"{}\">",
    escape_html(copy.meta_description)
  )?;
  writeln!(html, "  <meta property=\"og:url\" content=\"{}\">", escape_html(&canonical_url))?;
  writeln!(html, "  <meta property=\"og:image\" content=\"{}\">", escape_html(&og_image_url))?;
  writeln!(html, "  <meta property=\"og:image:type\" content=\"image/png\">")?;
  writeln!(html, "  <meta property=\"og:image:width\" content=\"1200\">")?;
  writeln!(html, "  <meta property=\"og:image:height\" content=\"630\">")?;
  writeln!(
    html,
    "  <meta property=\"og:image:alt\" content=\"{}\">",
    escape_html(copy.og_image_alt)
  )?;
  writeln!(html, "  <meta name=\"twitter:card\" content=\"summary_large_image\">")?;
  writeln!(html, "  <meta name=\"twitter:title\" content=\"{}\">", escape_html(copy.meta_title))?;
  writeln!(
    html,
    "  <meta name=\"twitter:description\" content=\"{}\">",
    escape_html(copy.meta_description)
  )?;
  writeln!(html, "  <meta name=\"twitter:image\" content=\"{}\">", escape_html(&og_image_url))?;
  writeln!(
    html,
    "  <meta name=\"twitter:image:alt\" content=\"{}\">",
    escape_html(copy.og_image_alt)
  )?;
  writeln!(html, "  <link rel=\"icon\" href=\"/favicon.ico\" sizes=\"any\">")?;
  writeln!(
    html,
    "  <link rel=\"icon\" type=\"image/png\" sizes=\"96x96\" href=\"/assets/favicon-96.png\">"
  )?;
  writeln!(
    html,
    "  <link rel=\"icon\" type=\"image/png\" sizes=\"32x32\" href=\"/assets/favicon-32.png\">"
  )?;
  writeln!(
    html,
    "  <link rel=\"apple-touch-icon\" sizes=\"256x256\" href=\"/assets/icon-256.png\">"
  )?;
  writeln!(html, "  <script type=\"application/ld+json\">{structured_data}</script>")?;
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
  let docs_root = DocSlug::Quickstart.path(copy.locale);
  let changelog_path = DocSlug::Changelog.path(copy.locale);
  let docs_label = match copy.locale {
    Locale::En => "Docs",
    Locale::Zh => "使用文档",
  };
  let docs_short_label = match copy.locale {
    Locale::En => "Docs",
    Locale::Zh => "文档",
  };
  let changelog_label = DocSlug::Changelog.nav_label(copy.locale);

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
  writeln!(html, "        <a href=\"#workflows\">{}</a>", escape_html(copy.nav_workflows))?;
  writeln!(html, "        <a href=\"#compatibility\">{}</a>", escape_html(copy.nav_compatibility))?;
  writeln!(html, "        <a href=\"#compare\">{}</a>", escape_html(copy.nav_compare))?;
  writeln!(html, "        <a href=\"#faq\">{}</a>", escape_html(copy.nav_faq))?;
  writeln!(html, "        <a href=\"{docs_root}\">{}</a>", escape_html(docs_label))?;
  writeln!(html, "        <a href=\"{changelog_path}\">{}</a>", escape_html(changelog_label))?;
  writeln!(html, "        <a href=\"#install\">{}</a>", escape_html(copy.nav_install))?;
  writeln!(html, "      </nav>")?;
  writeln!(html, "      <div class=\"topbar-actions\">")?;
  writeln!(
    html,
    "        <a class=\"control-btn mobile-docs-btn\" href=\"{docs_root}\">{}</a>",
    escape_html(docs_short_label)
  )?;
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

  // Section 1: Hero + Above-the-fold Interactive Workbench + Engine Strip
  writeln!(html, "    <section class=\"hero\">")?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"hero-top-grid\">")?;
  writeln!(html, "          <div class=\"hero-copy\">")?;
  writeln!(html, "            <h1 class=\"hero-title\">{}</h1>", escape_html(copy.hero_title))?;
  writeln!(html, "            <p class=\"hero-lead\">{}</p>", escape_html(copy.hero_lead))?;
  writeln!(html, "            <div class=\"download-box\">")?;
  writeln!(html, "              <div class=\"download-primary-row\">")?;
  writeln!(
    html,
    "                <a id=\"primary-download-btn\" class=\"btn-primary\" href=\"{}\">{} v{}</a>",
    escape_html(&assets.release_tag_url),
    escape_html(copy.primary_download_default),
    escape_html(version)
  )?;
  writeln!(
    html,
    "                <a class=\"btn-secondary\" href=\"{REPO_URL}\" rel=\"noopener\">{}</a>",
    escape_html(copy.github_cta)
  )?;
  writeln!(
    html,
    "                <a class=\"hero-release-link\" href=\"{}\">{} v{}</a>",
    escape_html(&assets.release_tag_url),
    escape_html(copy.release_notes_cta),
    escape_html(version)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "              <div class=\"download-matrix-inline\">")?;
  writeln!(html, "                <span>{}</span>", escape_html(copy.all_packages_label))?;
  writeln!(
    html,
    "                <a href=\"{}\" data-os=\"macos\" data-os-primary=\"macos\" data-primary-cta=\"{}\">macOS .dmg</a>",
    escape_html(&assets.dmg_url),
    escape_html(copy.primary_cta_macos)
  )?;
  writeln!(
    html,
    "                <a href=\"{}\" data-os=\"windows\" data-os-primary=\"windows\" data-primary-cta=\"{}\">Windows .exe</a>",
    escape_html(&assets.exe_url),
    escape_html(copy.primary_cta_windows)
  )?;
  writeln!(
    html,
    "                <a href=\"{}\" data-os=\"windows\">Windows .msi</a>",
    escape_html(&assets.msi_url)
  )?;
  writeln!(
    html,
    "                <a href=\"{}\" data-os=\"linux\" data-os-primary=\"linux\" data-primary-cta=\"{}\">Linux .deb</a>",
    escape_html(&assets.deb_url),
    escape_html(copy.primary_cta_linux)
  )?;
  writeln!(
    html,
    "                <a href=\"{}\" data-os=\"linux\">Linux .rpm</a>",
    escape_html(&assets.rpm_url)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </div>")?;

  writeln!(html, "          <dl class=\"trust-grid\">")?;
  for (index, fact) in copy.trust_facts.iter().enumerate() {
    let delay_ms = (index as i32) * -2100;
    writeln!(html, "            <div class=\"trust-item\" style=\"--beam-delay: {delay_ms}ms\">")?;
    writeln!(html, "              <div class=\"trust-head\">")?;
    writeln!(
      html,
      "                <span class=\"trust-icon\" aria-hidden=\"true\">{}</span>",
      trust_icon_svg(fact.icon)
    )?;
    writeln!(html, "                <dt>{}</dt>", escape_html(fact.label))?;
    writeln!(html, "              </div>")?;
    writeln!(html, "              <dd>{}</dd>", escape_html(fact.detail))?;
    writeln!(html, "            </div>")?;
  }
  writeln!(html, "          </dl>")?;
  writeln!(html, "        </div>")?;

  // Interactive Workbench Stage directly in Hero
  writeln!(html, "        <div class=\"workbench-frame\">")?;
  writeln!(html, "          <div class=\"workbench-bar\">")?;
  writeln!(
    html,
    "            <span class=\"win-dots workbench-dots\" aria-hidden=\"true\"><i></i><i></i><i></i></span>"
  )?;
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
      writeln!(html, "              <picture>")?;
      writeln!(
        html,
        "                <source type=\"image/webp\" srcset=\"{webp}\">",
        webp = escape_html(tab.webp_path),
      )?;
      writeln!(
        html,
        "                <img class=\"workbench-shot\" src=\"{src}\" alt=\"{alt}\" width=\"{w}\" height=\"{h}\" fetchpriority=\"high\">",
        src = escape_html(tab.image_path),
        alt = escape_html(tab.alt),
        w = tab.width,
        h = tab.height,
      )?;
      writeln!(html, "              </picture>")?;
    } else {
      writeln!(html, "              <picture>")?;
      writeln!(
        html,
        "                <source type=\"image/webp\" data-deferred-srcset=\"{webp}\">",
        webp = escape_html(tab.webp_path),
      )?;
      writeln!(
        html,
        "                <img class=\"workbench-shot\" src=\"data:image/gif;base64,R0lGODlhAQABAAAAACw=\" data-deferred-src=\"{src}\" alt=\"{alt}\" width=\"{w}\" height=\"{h}\" fetchpriority=\"low\" decoding=\"async\">",
        src = escape_html(tab.image_path),
        alt = escape_html(tab.alt),
        w = tab.width,
        h = tab.height,
      )?;
      writeln!(html, "              </picture>")?;
      writeln!(
        html,
        "              <noscript><img class=\"workbench-shot\" src=\"{webp}\" alt=\"{alt}\" width=\"{w}\" height=\"{h}\" loading=\"lazy\"></noscript>",
        webp = escape_html(tab.webp_path),
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

  // Database Ecosystem Badge Strip
  writeln!(html, "        <div class=\"engine-strip\">")?;
  writeln!(
    html,
    "          <p class=\"engine-strip-label\">{}</p>",
    escape_html(copy.ecosystem_strip_label)
  )?;
  writeln!(html, "          <ul class=\"engine-badges\">")?;
  for badge in copy.engine_badges {
    let kind_class = match badge.kind {
      ServerKind::Relational => "engine-pill",
      ServerKind::Workspace => "engine-pill engine-pill-workspace",
    };
    writeln!(
      html,
      "            <li class=\"{kind_class}\"><span class=\"engine-name\">{}</span><span class=\"engine-tag\">{}</span></li>",
      escape_html(badge.name),
      escape_html(badge.tag)
    )?;
  }
  writeln!(html, "          </ul>")?;
  writeln!(html, "        </div>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Section 2: Four Core Product Workflows
  writeln!(html, "    <section id=\"workflows\" class=\"section\">")?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"section-header\">")?;
  writeln!(html, "          <h2>{}</h2>", escape_html(copy.workflows_heading))?;
  writeln!(html, "          <p>{}</p>", escape_html(copy.workflows_intro))?;
  writeln!(html, "        </div>")?;
  writeln!(html, "        <div class=\"workflow-stack\">")?;

  // Workflow 1: Native Rust & Bundled Stack
  writeln!(html, "          <article class=\"workflow-row\">")?;
  render_workflow_story(&mut html, &copy.wf_native)?;
  writeln!(html, "            <div class=\"workflow-proof\">")?;
  writeln!(html, "              <div class=\"spec-panel\">")?;
  writeln!(html, "                <div class=\"spec-panel-bar\">")?;
  writeln!(
    html,
    "                  <span class=\"panel-bar-title\"><span class=\"win-dots\" aria-hidden=\"true\"><i></i><i></i><i></i></span><span>{}</span></span>",
    escape_html(copy.wf_native_panel_bar)
  )?;
  writeln!(html, "                  <span class=\"env-pill-ok\">TAURI 2 + RUST</span>")?;
  writeln!(html, "                </div>")?;
  writeln!(html, "                <dl class=\"spec-sheet\">")?;
  for item in copy.wf_native_specs {
    writeln!(html, "                  <div class=\"spec-row\">")?;
    writeln!(
      html,
      "                    <dt><span class=\"spec-term\">{}</span><span class=\"spec-tag\">{}</span></dt>",
      escape_html(item.term),
      escape_html(item.tag)
    )?;
    writeln!(html, "                    <dd>{}</dd>", escape_html(item.detail))?;
    writeln!(html, "                  </div>")?;
  }
  writeln!(html, "                </dl>")?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </article>")?;

  // Workflow 2: Reviewable Write Guardrails (preserves #guardrails anchor)
  writeln!(
    html,
    "          <article id=\"guardrails\" class=\"workflow-row workflow-row-reverse\">"
  )?;
  render_workflow_story(&mut html, &copy.wf_guardrails)?;
  writeln!(html, "            <div class=\"workflow-proof\">")?;
  writeln!(html, "              <div class=\"code-panel\">")?;
  writeln!(html, "                <div class=\"code-panel-bar\">")?;
  writeln!(
    html,
    "                  <span class=\"panel-bar-title\"><span class=\"win-dots\" aria-hidden=\"true\"><i></i><i></i><i></i></span><span>{}</span></span>",
    escape_html(copy.sql_preview_bar)
  )?;
  writeln!(html, "                  <span class=\"env-pill-ok\">1 TRANSACTION</span>")?;
  writeln!(html, "                </div>")?;
  writeln!(html, "                <pre><code>{}</code></pre>", render_guardrails_sql())?;
  writeln!(
    html,
    "                <div class=\"code-note\">{}</div>",
    escape_html(copy.sql_preview_note)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </article>")?;

  // Workflow 3: Schema Diff & Deterministic Codegen
  writeln!(html, "          <article class=\"workflow-row\">")?;
  render_workflow_story(&mut html, &copy.wf_schema)?;
  writeln!(html, "            <div class=\"workflow-proof\">")?;
  writeln!(html, "              <div class=\"code-panel\">")?;
  writeln!(html, "                <div class=\"code-panel-bar\">")?;
  writeln!(
    html,
    "                  <span class=\"panel-bar-title\"><span class=\"win-dots\" aria-hidden=\"true\"><i></i><i></i><i></i></span><span>{}</span></span>",
    escape_html(copy.schema_panel_bar)
  )?;
  writeln!(html, "                  <span class=\"env-pill-ok\">DETERMINISTIC</span>")?;
  writeln!(html, "                </div>")?;
  writeln!(html, "                <pre><code>{}</code></pre>", render_schema_codegen())?;
  writeln!(
    html,
    "                <div class=\"code-note\">{}</div>",
    escape_html(copy.schema_panel_note)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </article>")?;

  // Workflow 4: Production-Isolated CLI & MCP (preserves #agents anchor)
  writeln!(html, "          <article id=\"agents\" class=\"workflow-row workflow-row-reverse\">")?;
  render_workflow_story(&mut html, &copy.wf_agents)?;
  writeln!(html, "            <div class=\"workflow-proof\">")?;
  writeln!(html, "              <div class=\"code-panel\">")?;
  writeln!(html, "                <div class=\"code-panel-bar\">")?;
  writeln!(
    html,
    "                  <span class=\"panel-bar-title\"><span class=\"win-dots\" aria-hidden=\"true\"><i></i><i></i><i></i></span><span>{}</span></span>",
    escape_html(copy.agents_terminal_bar)
  )?;
  writeln!(html, "                  <span class=\"env-pill-prod\">PROD HIDDEN</span>")?;
  writeln!(html, "                </div>")?;
  writeln!(
    html,
    "                <pre><code>{}</code></pre>",
    render_agents_cli(copy.agents_cli_comment)
  )?;
  render_mcp_switcher(&mut html, copy, "mcp-switcher")?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </article>")?;

  writeln!(html, "        </div>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Section 3: Database Ecosystem Cards + Collapsible 18-Server Smoke-Test Ledger
  writeln!(html, "    <section id=\"compatibility\" class=\"section\">")?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"section-header\">")?;
  writeln!(html, "          <h2>{}</h2>", escape_html(copy.compatibility_heading))?;
  writeln!(html, "          <p>{}</p>", escape_html(copy.compatibility_intro))?;
  writeln!(html, "        </div>")?;
  writeln!(html, "        <div class=\"ecosystem-grid\">")?;
  for card in copy.ecosystem_cards {
    let card_kind_class = match card.kind {
      ServerKind::Relational => "ecosystem-card ecosystem-card-sql",
      ServerKind::Workspace => "ecosystem-card ecosystem-card-workspace",
    };
    writeln!(html, "          <div class=\"{card_kind_class}\">")?;
    writeln!(html, "            <div class=\"ecosystem-card-head\">")?;
    writeln!(html, "              <h3>{}</h3>", escape_html(card.title))?;
    writeln!(
      html,
      "              <span class=\"ecosystem-count\">{}</span>",
      escape_html(card.count_badge)
    )?;
    writeln!(html, "            </div>")?;
    writeln!(html, "            <p class=\"ecosystem-summary\">{}</p>", escape_html(card.summary))?;
    writeln!(html, "            <ul class=\"eco-engine-chips\">")?;
    for engine in card.engines {
      writeln!(html, "              <li class=\"eco-engine-chip\">{}</li>", escape_html(engine))?;
    }
    writeln!(html, "            </ul>")?;
    writeln!(html, "            <ul class=\"ecosystem-bullets\">")?;
    for cap in card.capabilities {
      writeln!(html, "              <li>{}</li>", escape_html(cap))?;
    }
    writeln!(html, "            </ul>")?;
    writeln!(html, "          </div>")?;
  }
  writeln!(html, "        </div>")?;

  writeln!(html, "        <details class=\"ledger-drawer\">")?;
  writeln!(html, "          <summary class=\"ledger-drawer-summary\">")?;
  writeln!(
    html,
    "            <span class=\"ledger-summary-title\">{}</span>",
    escape_html(copy.ledger_drawer_summary)
  )?;
  writeln!(
    html,
    "            <span class=\"ledger-summary-hint\">{}</span>",
    escape_html(copy.ledger_drawer_hint)
  )?;
  writeln!(html, "          </summary>")?;
  writeln!(html, "          <div class=\"ledger-drawer-body\">")?;
  writeln!(html, "            <div class=\"ledger-controls\">")?;
  writeln!(
    html,
    "              <div class=\"filter-group\" role=\"group\" aria-label=\"Database category\">"
  )?;
  writeln!(
    html,
    "                <button class=\"filter-btn\" type=\"button\" aria-pressed=\"true\" data-ledger-filter=\"all\">{}</button>",
    escape_html(copy.filter_all)
  )?;
  writeln!(
    html,
    "                <button class=\"filter-btn\" type=\"button\" aria-pressed=\"false\" data-ledger-filter=\"relational\">{}</button>",
    escape_html(copy.filter_relational)
  )?;
  writeln!(
    html,
    "                <button class=\"filter-btn\" type=\"button\" aria-pressed=\"false\" data-ledger-filter=\"workspace\">{}</button>",
    escape_html(copy.filter_workspaces)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "            <div class=\"ledger-wrap\">")?;
  writeln!(html, "              <table class=\"ledger-table\">")?;
  writeln!(html, "                <thead>")?;
  writeln!(html, "                  <tr>")?;
  writeln!(html, "                    <th scope=\"col\">{}</th>", escape_html(copy.col_server))?;
  writeln!(html, "                    <th scope=\"col\">{}</th>", escape_html(copy.col_version))?;
  writeln!(html, "                    <th scope=\"col\">{}</th>", escape_html(copy.col_driver))?;
  writeln!(html, "                    <th scope=\"col\">{}</th>", escape_html(copy.col_status))?;
  writeln!(html, "                    <th scope=\"col\">{}</th>", escape_html(copy.col_smoke))?;
  writeln!(html, "                    <th scope=\"col\">{}</th>", escape_html(copy.col_notes))?;
  writeln!(html, "                  </tr>")?;
  writeln!(html, "                </thead>")?;
  writeln!(html, "                <tbody>")?;
  for row in copy.servers {
    let (status_class, status_label) = match row.level {
      SupportLevel::Supported => ("status-ok", copy.status_supported),
      SupportLevel::UsableWithGaps => ("status-gap", copy.status_gaps),
    };
    writeln!(html, "                  <tr data-ledger-kind=\"{}\">", row.kind.as_attr())?;
    writeln!(
      html,
      "                    <td class=\"ledger-server\">{}</td>",
      escape_html(row.server)
    )?;
    writeln!(
      html,
      "                    <td class=\"ledger-mono\">{}</td>",
      escape_html(row.tested_version)
    )?;
    writeln!(
      html,
      "                    <td><span class=\"driver-chip\">{}</span></td>",
      escape_html(row.connection_type)
    )?;
    writeln!(
      html,
      "                    <td><span class=\"{status_class}\">{}</span></td>",
      escape_html(status_label)
    )?;
    write!(html, "                    <td class=\"ledger-mono\">")?;
    render_smoke_counts(&mut html, row.smoke_counts)?;
    writeln!(html, "</td>")?;
    writeln!(
      html,
      "                    <td class=\"ledger-note\">{}</td>",
      escape_html(row.notes)
    )?;
    writeln!(html, "                  </tr>")?;
  }
  writeln!(html, "                </tbody>")?;
  writeln!(html, "              </table>")?;
  writeln!(html, "            </div>")?;
  writeln!(html, "          </div>")?;
  writeln!(html, "        </details>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Section 4: Honest Selection Comparison
  writeln!(html, "    <section id=\"compare\" class=\"section\">")?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"section-header\">")?;
  writeln!(html, "          <h2>{}</h2>", escape_html(copy.compare_heading))?;
  writeln!(html, "          <p>{}</p>", escape_html(copy.compare_intro))?;
  writeln!(html, "        </div>")?;
  writeln!(html, "        <div class=\"compare-wrap\">")?;
  writeln!(html, "          <table class=\"compare-table\">")?;
  writeln!(html, "            <thead>")?;
  writeln!(html, "              <tr>")?;
  writeln!(
    html,
    "                <th scope=\"col\">{}</th>",
    escape_html(copy.compare_col_dimension)
  )?;
  writeln!(
    html,
    "                <th scope=\"col\" class=\"compare-featured-head\"><span class=\"compare-head-badge\"><span class=\"compare-head-dot\" aria-hidden=\"true\"></span>{}</span></th>",
    escape_html(copy.compare_col_dataomni)
  )?;
  writeln!(html, "                <th scope=\"col\">{}</th>", escape_html(copy.compare_col_jvm))?;
  writeln!(
    html,
    "                <th scope=\"col\">{}</th>",
    escape_html(copy.compare_col_commercial)
  )?;
  writeln!(html, "              </tr>")?;
  writeln!(html, "            </thead>")?;
  writeln!(html, "            <tbody>")?;
  for row in copy.compare_rows {
    writeln!(html, "              <tr>")?;
    writeln!(
      html,
      "                <th scope=\"row\" class=\"compare-dim\">{}</th>",
      escape_html(row.dimension)
    )?;
    writeln!(
      html,
      "                <td class=\"compare-featured-cell\"><div class=\"compare-cell-inner\"><span class=\"compare-check\" aria-hidden=\"true\">✓</span><span>{}</span></div></td>",
      escape_html(row.dataomni)
    )?;
    writeln!(html, "                <td>{}</td>", escape_html(row.jvm_clients))?;
    writeln!(html, "                <td>{}</td>", escape_html(row.commercial_native))?;
    writeln!(html, "              </tr>")?;
  }
  writeln!(html, "            </tbody>")?;
  writeln!(html, "          </table>")?;
  writeln!(html, "        </div>")?;
  writeln!(
    html,
    "        <p class=\"compare-footnote\">{}</p>",
    escape_html(copy.compare_footnote)
  )?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Section 5: Accessible Native FAQ Accordion
  writeln!(html, "    <section id=\"faq\" class=\"section\" aria-labelledby=\"faq-heading\">")?;
  writeln!(html, "      <div class=\"shell\">")?;
  writeln!(html, "        <div class=\"section-header\">")?;
  writeln!(html, "          <h2 id=\"faq-heading\">{}</h2>", escape_html(copy.faq_heading))?;
  writeln!(html, "          <p>{}</p>", escape_html(copy.faq_intro))?;
  writeln!(html, "        </div>")?;
  writeln!(html, "        <div class=\"faq-list\">")?;
  for (index, item) in copy.faq_items.iter().enumerate() {
    let open_attr = if index == 0 { " open" } else { "" };
    writeln!(html, "          <details class=\"faq-item\"{open_attr}>")?;
    writeln!(
      html,
      "            <summary class=\"faq-summary\">{}</summary>",
      escape_html(item.question)
    )?;
    writeln!(html, "            <div class=\"faq-answer\">")?;
    writeln!(html, "              <p>{}</p>", escape_html(item.answer))?;
    writeln!(html, "            </div>")?;
    writeln!(html, "          </details>")?;
  }
  writeln!(html, "        </div>")?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </section>")?;

  // Section 6: Download & Install
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
  writeln!(html, "                <code>{}</code>", escape_html(macos_brew_cmd))?;
  writeln!(
    html,
    "                <button class=\"copy-btn\" type=\"button\" data-copy-text=\"{}\" data-default-label=\"{}\" data-copied-label=\"{}\">{}</button>",
    escape_html(macos_brew_cmd),
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
  render_footer(&mut html, copy, copy.locale.alternate_path())?;

  writeln!(html, "  <script src=\"/assets/site.js\" defer></script>")?;
  writeln!(html, "</body>")?;
  writeln!(html, "</html>")?;

  Ok(html)
}

fn alternate_locale(locale: Locale) -> Locale {
  match locale {
    Locale::En => Locale::Zh,
    Locale::Zh => Locale::En,
  }
}

fn render_footer(
  html: &mut String,
  copy: &PageCopy,
  alternate_href: &str,
) -> Result<(), std::fmt::Error> {
  writeln!(html, "  <footer class=\"site-footer\">")?;
  writeln!(html, "    <div class=\"shell footer-inner\">")?;
  writeln!(html, "      <span>{}</span>", escape_html(copy.footer_summary))?;
  writeln!(html, "      <div class=\"footer-links\">")?;
  for slug in ALL_DOC_SLUGS {
    writeln!(
      html,
      "        <a href=\"{}\">{}</a>",
      slug.path(copy.locale),
      escape_html(slug.nav_label(copy.locale))
    )?;
  }
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
    escape_html(alternate_href),
    escape_html(copy.locale.alternate_label())
  )?;
  writeln!(html, "      </div>")?;
  writeln!(html, "    </div>")?;
  writeln!(html, "  </footer>")
}

fn expand_version(raw: &str, version: &str) -> String {
  if raw.contains("{VERSION}") {
    raw.replace("{VERSION}", version)
  } else {
    raw.to_string()
  }
}

fn push_span(out: &mut String, class: &str, text: &str) {
  let _ = write!(out, "<span class=\"{class}\">{}</span>", escape_html(text));
}

fn is_pure_number(token: &str) -> bool {
  !token.is_empty() && token.bytes().all(|b| b.is_ascii_digit())
}

fn is_subcommand_word(token: &str) -> bool {
  !token.is_empty()
    && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    && token.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
}

fn subcommand_budget_for(cmd: &str) -> u8 {
  match cmd {
    "claude" | "codex" | "gemini" => 2,
    "dataomni" | "brew" | "apt" | "dnf" | "cargo" => 1,
    _ => 0,
  }
}

fn highlight_json(raw: &str) -> String {
  let bytes = raw.as_bytes();
  let mut out = String::with_capacity(raw.len() + 128);
  let mut i = 0;

  while i < bytes.len() {
    match bytes[i] {
      b'"' => {
        let start = i;
        i += 1;
        while i < bytes.len() {
          if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
          } else if bytes[i] == b'"' {
            i += 1;
            break;
          } else {
            i += 1;
          }
        }
        let mut look = i;
        while look < bytes.len() && bytes[look].is_ascii_whitespace() {
          look += 1;
        }
        let class = if look < bytes.len() && bytes[look] == b':' { "tok-prop" } else { "tok-str" };
        push_span(&mut out, class, &raw[start..i]);
      }
      b'-' | b'0'..=b'9' => {
        let start = i;
        i += 1;
        while i < bytes.len()
          && (bytes[i].is_ascii_digit() || matches!(bytes[i], b'.' | b'e' | b'E' | b'+' | b'-'))
        {
          i += 1;
        }
        push_span(&mut out, "tok-num", &raw[start..i]);
      }
      b't' if raw[i..].starts_with("true") => {
        push_span(&mut out, "tok-kw", "true");
        i += 4;
      }
      b'f' if raw[i..].starts_with("false") => {
        push_span(&mut out, "tok-kw", "false");
        i += 5;
      }
      b'n' if raw[i..].starts_with("null") => {
        push_span(&mut out, "tok-kw", "null");
        i += 4;
      }
      _ => {
        if let Some(ch) = raw[i..].chars().next() {
          out.push_str(&escape_html(&raw[i..i + ch.len_utf8()]));
          i += ch.len_utf8();
        } else {
          break;
        }
      }
    }
  }

  out
}

fn highlight_shell_line(
  line: &str,
  lang: CodeLang,
  in_continuation: bool,
  out: &mut String,
) -> bool {
  let trimmed_start = line.trim_start();
  if trimmed_start.is_empty() {
    out.push_str(line);
    return false;
  }
  if trimmed_start.starts_with('#') {
    push_span(out, "code-comment", line);
    return false;
  }

  let leading_ws = line.len() - trimmed_start.len();
  out.push_str(&line[..leading_ws]);

  let mut rest = trimmed_start;
  if lang == CodeLang::Shell {
    if let Some(after_prompt) = rest.strip_prefix("$ ") {
      push_span(out, "tok-prompt", "$");
      out.push(' ');
      rest = after_prompt;
    }
  } else if lang == CodeLang::PowerShell {
    if let Some(after_prompt) = rest.strip_prefix("PS> ") {
      push_span(out, "tok-prompt", "PS>");
      out.push(' ');
      rest = after_prompt;
    }
  }

  let bytes = rest.as_bytes();
  let mut i = 0;
  let mut expect_cmd = !in_continuation;
  let mut active_cmd: Option<&str> = None;
  let mut subcmd_budget: u8 = 0;
  let mut next_continuation = false;

  while i < bytes.len() {
    let b = bytes[i];
    if b.is_ascii_whitespace() {
      let start = i;
      while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
      out.push_str(&rest[start..i]);
      continue;
    }

    // Inline comment preceded by whitespace
    if b == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
      push_span(out, "code-comment", &rest[i..]);
      break;
    }

    // Single-quoted string
    if b == b'\'' {
      let start = i;
      i += 1;
      while i < bytes.len() {
        if bytes[i] == b'\'' {
          i += 1;
          if lang == CodeLang::PowerShell && i < bytes.len() && bytes[i] == b'\'' {
            i += 1;
            continue;
          }
          break;
        }
        i += 1;
      }
      push_span(out, "tok-str", &rest[start..i]);
      expect_cmd = false;
      continue;
    }

    // Double-quoted string
    if b == b'"' {
      let start = i;
      i += 1;
      while i < bytes.len() {
        if (lang == CodeLang::Shell && bytes[i] == b'\\' && i + 1 < bytes.len())
          || (lang == CodeLang::PowerShell
            && (bytes[i] == b'`' || bytes[i] == b'\\')
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'"')
        {
          i += 2;
        } else if bytes[i] == b'"' {
          i += 1;
          break;
        } else {
          i += 1;
        }
      }
      push_span(out, "tok-str", &rest[start..i]);
      expect_cmd = false;
      continue;
    }

    // Line-continuation operator at end of line (`\` in Shell, `` ` `` in PowerShell)
    if ((lang == CodeLang::Shell && b == b'\\') || (lang == CodeLang::PowerShell && b == b'`'))
      && rest[i + 1..].trim().is_empty()
    {
      push_span(out, "tok-kw", &rest[i..i + 1]);
      next_continuation = true;
      i += 1;
      continue;
    }

    // Shell / PowerShell operators
    if rest[i..].starts_with("&&") || rest[i..].starts_with("||") || rest[i..].starts_with(">>") {
      let op = &rest[i..i + 2];
      push_span(out, "tok-kw", op);
      if op != ">>" {
        expect_cmd = true;
        active_cmd = None;
        subcmd_budget = 0;
      }
      i += 2;
      continue;
    }
    if matches!(b, b'|' | b'>' | b'<' | b';' | b'(' | b')' | b'+') {
      let op = &rest[i..i + 1];
      push_span(out, "tok-kw", op);
      if matches!(b, b'|' | b';' | b'(') {
        expect_cmd = true;
        active_cmd = None;
        subcmd_budget = 0;
      }
      i += 1;
      continue;
    }

    // Read a regular word token
    let start = i;
    while i < bytes.len() {
      let ch = bytes[i];
      if ch.is_ascii_whitespace()
        || matches!(ch, b'|' | b'>' | b'<' | b';' | b'(' | b')' | b'"' | b'\'' | b'+')
        || (ch == b'&' && i + 1 < bytes.len() && bytes[i + 1] == b'&')
      {
        break;
      }
      i += 1;
    }
    let word = &rest[start..i];
    if word.is_empty() {
      if let Some(ch) = rest[i..].chars().next() {
        out.push_str(&escape_html(&rest[i..i + ch.len_utf8()]));
        i += ch.len_utf8();
      }
      continue;
    }

    if word == "--" {
      push_span(out, "tok-kw", word);
      subcmd_budget = 0;
      continue;
    }

    // PowerShell static method call like [Environment]::SetEnvironmentVariable
    if lang == CodeLang::PowerShell {
      if let Some((type_part, method_part)) = word.split_once("::") {
        if type_part.starts_with('[') && type_part.ends_with(']') {
          push_span(out, "tok-type", type_part);
          out.push_str("::");
          push_span(out, "tok-cmd", method_part);
          expect_cmd = false;
          continue;
        }
      }
    }

    // Environment / shell variables ($binDir, $env:TEMP, %LOCALAPPDATA%)
    if word.starts_with('$') || (word.starts_with('%') && word.len() > 2 && word.ends_with('%')) {
      push_span(out, "tok-param", word);
      if expect_cmd {
        expect_cmd = false;
      }
      continue;
    }

    // Assignment operator `=` in PowerShell (`$binDir = ...`)
    if word == "=" {
      push_span(out, "tok-kw", word);
      continue;
    }

    if expect_cmd {
      if word == "sudo" {
        push_span(out, "tok-kw", word);
      } else {
        push_span(out, "tok-cmd", word);
        active_cmd = Some(word);
        subcmd_budget = subcommand_budget_for(word);
        expect_cmd = false;
      }
      continue;
    }

    if word == "-" {
      push_span(out, "tok-kw", word);
      continue;
    }

    if word.starts_with('-')
      || (lang == CodeLang::PowerShell
        && active_cmd == Some("msiexec")
        && word.starts_with('/')
        && word.len() <= 4)
    {
      push_span(out, "tok-attr", word);
      continue;
    }

    if subcmd_budget > 0 && is_subcommand_word(word) {
      push_span(out, "tok-type", word);
      subcmd_budget -= 1;
      continue;
    }

    if is_pure_number(word) {
      push_span(out, "tok-num", word);
      continue;
    }

    // Key=value bindings like cid=1042
    if let Some((key, val)) = word.split_once('=') {
      if !key.is_empty()
        && !val.is_empty()
        && key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
      {
        push_span(out, "tok-prop", key);
        out.push('=');
        if is_pure_number(val) {
          push_span(out, "tok-num", val);
        } else {
          push_span(out, "tok-str", val);
        }
        continue;
      }
    }

    out.push_str(&escape_html(word));
  }

  next_continuation
}

pub fn highlight_code(raw: &str, lang: CodeLang) -> String {
  if lang == CodeLang::Json {
    return highlight_json(raw);
  }
  let mut out = String::with_capacity(raw.len() + 256);
  let mut in_continuation = false;
  for (index, line) in raw.lines().enumerate() {
    if index > 0 {
      out.push('\n');
    }
    in_continuation = highlight_shell_line(line, lang, in_continuation, &mut out);
  }
  out
}

fn render_mcp_switcher(
  html: &mut String,
  copy: &PageCopy,
  switcher_class: &str,
) -> Result<(), std::fmt::Error> {
  let default_cmd = copy.mcp_presets.first().map_or("dataomni mcp", |p| p.macos_command);
  let os_options = [("macos", "macOS"), ("windows", "Windows"), ("linux", "Linux")];

  writeln!(html, "            <div class=\"{switcher_class}\" data-mcp-switcher>")?;
  writeln!(html, "              <div class=\"mcp-switcher-head\">")?;
  writeln!(
    html,
    "                <span class=\"mcp-switcher-note\">{}</span>",
    escape_html(copy.agents_mcp_cmd_note)
  )?;
  writeln!(
    html,
    "                <div class=\"mcp-os-tabs\" role=\"group\" aria-label=\"{}\">",
    escape_html(copy.agents_mcp_os_aria_label)
  )?;
  for (index, (os_id, os_label)) in os_options.iter().enumerate() {
    let pressed = if index == 0 { "true" } else { "false" };
    writeln!(
      html,
      "                  <button class=\"mcp-os-tab\" type=\"button\" aria-pressed=\"{pressed}\" data-mcp-os-btn=\"{os_id}\" data-os-sync=\"{os_id}\">{os_label}</button>"
    )?;
  }
  writeln!(html, "                </div>")?;
  writeln!(html, "              </div>")?;
  writeln!(html, "              <div class=\"mcp-client-row\">")?;
  writeln!(
    html,
    "                <div class=\"mcp-client-tabs\" role=\"group\" aria-label=\"{}\">",
    escape_html(copy.agents_mcp_aria_label)
  )?;
  for (index, preset) in copy.mcp_presets.iter().enumerate() {
    let pressed = if index == 0 { "true" } else { "false" };
    writeln!(
      html,
      "                  <button class=\"mcp-client-tab\" type=\"button\" aria-pressed=\"{pressed}\" data-mcp-preset=\"{id}\">{label}</button>",
      pressed = pressed,
      id = escape_html(preset.id),
      label = escape_html(preset.label),
    )?;
  }
  writeln!(html, "                </div>")?;
  writeln!(html, "              </div>")?;
  writeln!(html, "              <div class=\"copy-cmd-row\">")?;
  writeln!(html, "                <div class=\"mcp-cmd-stage\">")?;
  for (preset_idx, preset) in copy.mcp_presets.iter().enumerate() {
    let per_os = [
      (
        "macos",
        preset.macos_command,
        if preset.id == "json" { CodeLang::Json } else { CodeLang::Shell },
      ),
      (
        "windows",
        preset.windows_command,
        if preset.id == "json" { CodeLang::Json } else { CodeLang::PowerShell },
      ),
      (
        "linux",
        preset.linux_command,
        if preset.id == "json" { CodeLang::Json } else { CodeLang::Shell },
      ),
    ];
    for (os_idx, (os_id, raw_cmd, lang)) in per_os.iter().enumerate() {
      let is_default = preset_idx == 0 && os_idx == 0;
      let hidden_attr = if is_default { "" } else { " hidden" };
      writeln!(
        html,
        "                  <code class=\"mcp-cmd-code\" data-mcp-os=\"{os_id}\" data-mcp-client=\"{client}\" data-raw-cmd=\"{raw}\"{hidden_attr}>{highlighted}</code>",
        client = escape_html(preset.id),
        raw = escape_html(raw_cmd),
        highlighted = highlight_code(raw_cmd, *lang),
      )?;
    }
  }
  writeln!(html, "                </div>")?;
  writeln!(
    html,
    "                <button class=\"copy-btn\" type=\"button\" data-mcp-copy-btn data-copy-text=\"{}\" data-default-label=\"{}\" data-copied-label=\"{}\">{}</button>",
    escape_html(default_cmd),
    escape_html(copy.copy_label),
    escape_html(copy.copied_label),
    escape_html(copy.copy_label)
  )?;
  writeln!(html, "              </div>")?;
  writeln!(
    html,
    "              <p class=\"mcp-os-note\" data-mcp-os-note=\"windows\" hidden>{}</p>",
    escape_html(copy.agents_mcp_windows_note)
  )?;
  writeln!(html, "            </div>")?;
  Ok(())
}

fn render_doc_structured_data(
  page: &DocPage,
  version: &str,
  released_at: &str,
  canonical_url: &str,
  og_image_url: &str,
) -> Result<String, std::fmt::Error> {
  let home_url = format!("{SITE_ORIGIN}{}", page.locale.canonical_path());
  let home_name = match page.locale {
    Locale::En => "DataOmni",
    Locale::Zh => "DataOmni 首页",
  };
  let docs_url = format!("{SITE_ORIGIN}{}", DocSlug::Quickstart.path(page.locale));

  let mut json = String::with_capacity(3072);
  write!(json, "{{\"@context\":\"https://schema.org\",\"@graph\":[")?;
  render_organization_node(&mut json)?;
  write!(
    json,
    ",{{\"@type\":\"WebSite\",\"@id\":\"{SITE_ORIGIN}/#website\",\"name\":\"DataOmni\",\"alternateName\":\"DataOmni Database Client\",\"url\":\"{SITE_ORIGIN}/\",\"publisher\":{{\"@id\":\"{SITE_ORIGIN}/#organization\"}},\"inLanguage\":[\"en\",\"zh-CN\"]}},"
  )?;
  write!(
    json,
    "{{\"@type\":\"SoftwareApplication\",\"@id\":\"{SITE_ORIGIN}/#software\",\"name\":\"DataOmni\",\"softwareVersion\":\"{}\",\"applicationCategory\":\"DeveloperApplication\",\"operatingSystem\":\"macOS 12+, Windows 10/11, Linux (x86_64)\",\"license\":\"https://opensource.org/licenses/MIT\",\"isAccessibleForFree\":true,\"url\":\"{SITE_ORIGIN}/\",\"author\":{{\"@id\":\"{SITE_ORIGIN}/#organization\"}},\"offers\":{{\"@type\":\"Offer\",\"price\":\"0\",\"priceCurrency\":\"USD\"}}}},",
    escape_json_str(version),
  )?;
  write!(json, "{{\"@type\":\"BreadcrumbList\",\"itemListElement\":[")?;
  write!(
    json,
    "{{\"@type\":\"ListItem\",\"position\":1,\"name\":\"{}\",\"item\":\"{}\"}},",
    escape_json_str(home_name),
    escape_json_str(&home_url)
  )?;
  if page.slug.is_root_route() {
    write!(
      json,
      "{{\"@type\":\"ListItem\",\"position\":2,\"name\":\"{}\",\"item\":\"{}\"}}",
      escape_json_str(page.title),
      escape_json_str(canonical_url)
    )?;
  } else {
    write!(
      json,
      "{{\"@type\":\"ListItem\",\"position\":2,\"name\":\"{}\",\"item\":\"{}\"}},{{\"@type\":\"ListItem\",\"position\":3,\"name\":\"{}\",\"item\":\"{}\"}}",
      escape_json_str(page.breadcrumb_group),
      escape_json_str(&docs_url),
      escape_json_str(page.title),
      escape_json_str(canonical_url)
    )?;
  }
  write!(json, "]}},")?;
  let schema_type = if page.slug == DocSlug::Privacy { "WebPage" } else { "TechArticle" };
  let title_prop = if page.slug == DocSlug::Privacy { "name" } else { "headline" };
  write!(
    json,
    "{{\"@type\":\"{schema_type}\",\"{title_prop}\":\"{}\",\"description\":\"{}\",\"url\":\"{}\",\"inLanguage\":\"{}\",\"image\":\"{}\",\"datePublished\":\"2026-11-17\",\"dateModified\":\"{}\",\"author\":{{\"@id\":\"{SITE_ORIGIN}/#organization\"}},\"publisher\":{{\"@id\":\"{SITE_ORIGIN}/#organization\"}},\"isPartOf\":{{\"@id\":\"{SITE_ORIGIN}/#website\"}},\"about\":{{\"@id\":\"{SITE_ORIGIN}/#software\"}}}}",
    escape_json_str(page.title),
    escape_json_str(page.meta_description),
    escape_json_str(canonical_url),
    escape_json_str(page.locale.html_lang()),
    escape_json_str(og_image_url),
    escape_json_str(released_at),
  )?;
  write!(json, "]}}")?;
  Ok(json)
}

pub fn render_doc_page(
  page: &DocPage,
  version: &str,
  released_at: &str,
) -> Result<String, std::fmt::Error> {
  let copy = match page.locale {
    Locale::En => &EN_COPY,
    Locale::Zh => &ZH_COPY,
  };
  let alt_locale = alternate_locale(page.locale);
  let en_path = page.slug.path(Locale::En);
  let zh_path = page.slug.path(Locale::Zh);
  let canonical_path = page.slug.path(page.locale);
  let alternate_path = page.slug.path(alt_locale);
  let canonical_url = format!("{SITE_ORIGIN}{canonical_path}");
  let og_image_url = format!("{SITE_ORIGIN}{}", page.locale.og_image_path());
  let structured_data =
    render_doc_structured_data(page, version, released_at, &canonical_url, &og_image_url)?;
  let install_href = format!("{}#install", page.locale.canonical_path());

  let mut html = String::with_capacity(40_960);
  writeln!(html, "<!DOCTYPE html>")?;
  writeln!(html, "<html lang=\"{}\">", page.locale.html_lang())?;
  writeln!(html, "<head>")?;
  writeln!(html, "  <meta charset=\"utf-8\">")?;
  writeln!(html, "  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">")?;
  writeln!(html, "  <title>{}</title>", escape_html(page.meta_title))?;
  writeln!(
    html,
    "  <meta name=\"description\" content=\"{}\">",
    escape_html(page.meta_description)
  )?;
  writeln!(html, "  <meta name=\"robots\" content=\"{ROBOTS_INDEX_DIRECTIVE}\">")?;
  writeln!(html, "  <link rel=\"canonical\" href=\"{}\">", escape_html(&canonical_url))?;
  writeln!(html, "  <link rel=\"alternate\" hreflang=\"en\" href=\"{SITE_ORIGIN}{en_path}\">")?;
  writeln!(html, "  <link rel=\"alternate\" hreflang=\"zh-CN\" href=\"{SITE_ORIGIN}{zh_path}\">")?;
  writeln!(
    html,
    "  <link rel=\"alternate\" hreflang=\"x-default\" href=\"{SITE_ORIGIN}{en_path}\">"
  )?;
  writeln!(
    html,
    "  <link rel=\"alternate\" type=\"text/plain\" href=\"/llms.txt\" title=\"DataOmni LLM Documentation\">"
  )?;
  writeln!(
    html,
    "  <link rel=\"alternate\" type=\"text/markdown\" href=\"/SKILL.md\" title=\"DataOmni CLI Agent Skill (SKILL.md)\">"
  )?;
  writeln!(html, "  <meta property=\"og:type\" content=\"article\">")?;
  writeln!(html, "  <meta property=\"og:site_name\" content=\"DataOmni\">")?;
  writeln!(html, "  <meta property=\"og:locale\" content=\"{}\">", page.locale.og_locale())?;
  writeln!(
    html,
    "  <meta property=\"og:locale:alternate\" content=\"{}\">",
    page.locale.og_alternate_locale()
  )?;
  writeln!(html, "  <meta property=\"og:title\" content=\"{}\">", escape_html(page.meta_title))?;
  writeln!(
    html,
    "  <meta property=\"og:description\" content=\"{}\">",
    escape_html(page.meta_description)
  )?;
  writeln!(html, "  <meta property=\"og:url\" content=\"{}\">", escape_html(&canonical_url))?;
  writeln!(html, "  <meta property=\"og:image\" content=\"{}\">", escape_html(&og_image_url))?;
  writeln!(html, "  <meta property=\"og:image:type\" content=\"image/png\">")?;
  writeln!(html, "  <meta property=\"og:image:width\" content=\"1200\">")?;
  writeln!(html, "  <meta property=\"og:image:height\" content=\"630\">")?;
  writeln!(
    html,
    "  <meta property=\"og:image:alt\" content=\"{}\">",
    escape_html(copy.og_image_alt)
  )?;
  writeln!(html, "  <meta name=\"twitter:card\" content=\"summary_large_image\">")?;
  writeln!(html, "  <meta name=\"twitter:title\" content=\"{}\">", escape_html(page.meta_title))?;
  writeln!(
    html,
    "  <meta name=\"twitter:description\" content=\"{}\">",
    escape_html(page.meta_description)
  )?;
  writeln!(html, "  <meta name=\"twitter:image\" content=\"{}\">", escape_html(&og_image_url))?;
  writeln!(
    html,
    "  <meta name=\"twitter:image:alt\" content=\"{}\">",
    escape_html(copy.og_image_alt)
  )?;
  writeln!(html, "  <link rel=\"icon\" href=\"/favicon.ico\" sizes=\"any\">")?;
  writeln!(
    html,
    "  <link rel=\"icon\" type=\"image/png\" sizes=\"96x96\" href=\"/assets/favicon-96.png\">"
  )?;
  writeln!(
    html,
    "  <link rel=\"icon\" type=\"image/png\" sizes=\"32x32\" href=\"/assets/favicon-32.png\">"
  )?;
  writeln!(
    html,
    "  <link rel=\"apple-touch-icon\" sizes=\"256x256\" href=\"/assets/icon-256.png\">"
  )?;
  writeln!(html, "  <script type=\"application/ld+json\">{structured_data}</script>")?;
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

  // Topbar
  writeln!(html, "  <header class=\"topbar\">")?;
  writeln!(html, "    <div class=\"shell topbar-inner\">")?;
  writeln!(html, "      <a class=\"brand\" href=\"{}\">", page.locale.canonical_path())?;
  writeln!(
    html,
    "        <img class=\"brand-icon\" src=\"/assets/icon-256.png\" alt=\"DataOmni icon\" width=\"28\" height=\"28\">"
  )?;
  writeln!(html, "        <span>DataOmni</span>")?;
  writeln!(html, "        <span class=\"version-pill\">v{}</span>", escape_html(version))?;
  writeln!(html, "      </a>")?;
  writeln!(html, "      <nav class=\"topnav\" aria-label=\"Primary\">")?;
  for slug in ALL_DOC_SLUGS {
    let current = if *slug == page.slug { " aria-current=\"page\"" } else { "" };
    writeln!(
      html,
      "        <a href=\"{}\"{current}>{}</a>",
      slug.path(page.locale),
      escape_html(slug.nav_label(page.locale))
    )?;
  }
  writeln!(html, "        <a href=\"{install_href}\">{}</a>", escape_html(copy.nav_install))?;
  writeln!(html, "      </nav>")?;
  writeln!(html, "      <div class=\"topbar-actions\">")?;
  writeln!(
    html,
    "        <a class=\"control-btn\" href=\"{alternate_path}\">{}</a>",
    escape_html(page.locale.alternate_label())
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

  // Main Documentation Shell
  let sidebar_aria = match page.locale {
    Locale::En => "Documentation pages",
    Locale::Zh => "文档页面导航",
  };
  let home_crumb = match page.locale {
    Locale::En => "Home",
    Locale::Zh => "首页",
  };

  writeln!(html, "  <main id=\"main\" class=\"docs-main\">")?;
  writeln!(html, "    <div class=\"shell docs-layout\">")?;
  writeln!(html, "      <aside class=\"docs-sidebar\">")?;
  writeln!(
    html,
    "        <nav class=\"docs-page-nav\" aria-label=\"{}\">",
    escape_html(sidebar_aria)
  )?;
  for slug in ALL_DOC_SLUGS {
    let current = if *slug == page.slug { " aria-current=\"page\"" } else { "" };
    writeln!(
      html,
      "          <a class=\"docs-nav-link\" href=\"{}\"{current}>{}</a>",
      slug.path(page.locale),
      escape_html(slug.nav_label(page.locale))
    )?;
  }
  writeln!(html, "        </nav>")?;

  writeln!(
    html,
    "        <nav class=\"docs-toc\" aria-label=\"{}\">",
    escape_html(page.toc_heading)
  )?;
  writeln!(html, "          <p class=\"docs-toc-title\">{}</p>", escape_html(page.toc_heading))?;
  writeln!(html, "          <ul class=\"docs-toc-list\">")?;
  for section in page.sections {
    writeln!(
      html,
      "            <li><a href=\"#{}\">{}</a></li>",
      escape_html(section.id),
      escape_html(section.toc_label)
    )?;
  }
  writeln!(html, "          </ul>")?;
  writeln!(html, "        </nav>")?;
  writeln!(html, "      </aside>")?;

  writeln!(html, "      <article class=\"docs-article\">")?;
  writeln!(html, "        <nav class=\"docs-breadcrumb\" aria-label=\"Breadcrumb\">")?;
  writeln!(
    html,
    "          <a href=\"{}\">{}</a><span aria-hidden=\"true\">/</span>",
    page.locale.canonical_path(),
    escape_html(home_crumb)
  )?;
  if !page.slug.is_root_route() {
    writeln!(
      html,
      "          <a href=\"{}\">{}</a><span aria-hidden=\"true\">/</span>",
      DocSlug::Quickstart.path(page.locale),
      escape_html(page.breadcrumb_group)
    )?;
  }
  writeln!(html, "          <span aria-current=\"page\">{}</span>", escape_html(page.title))?;
  writeln!(html, "        </nav>")?;

  writeln!(html, "        <header class=\"docs-hero\">")?;
  writeln!(html, "          <h1 class=\"docs-title\">{}</h1>", escape_html(page.title))?;
  writeln!(html, "          <p class=\"docs-lead\">{}</p>", escape_html(page.lead))?;
  writeln!(html, "        </header>")?;

  for (sec_idx, section) in page.sections.iter().enumerate() {
    writeln!(
      html,
      "        <section id=\"{}\" class=\"docs-section\" tabindex=\"-1\">",
      escape_html(section.id)
    )?;
    writeln!(
      html,
      "          <h2 class=\"docs-h2\">{} <a class=\"docs-anchor\" href=\"#{}\" aria-label=\"{}\">#</a></h2>",
      escape_html(section.heading),
      escape_html(section.id),
      escape_html(section.heading)
    )?;

    for (blk_idx, block) in section.blocks.iter().enumerate() {
      match block {
        DocBlock::Paragraph(text) => {
          writeln!(html, "          <p class=\"docs-p\">{}</p>", escape_html(text))?;
        }
        DocBlock::Bullets(items) => {
          writeln!(html, "          <ul class=\"docs-list\">")?;
          for item in *items {
            writeln!(html, "            <li>{}</li>", escape_html(item))?;
          }
          writeln!(html, "          </ul>")?;
        }
        DocBlock::Steps(steps) => {
          writeln!(html, "          <ol class=\"docs-steps\">")?;
          for (step_title, step_body) in *steps {
            writeln!(html, "            <li>")?;
            writeln!(
              html,
              "              <strong class=\"docs-step-title\">{}</strong>",
              escape_html(step_title)
            )?;
            writeln!(html, "              <p>{}</p>", escape_html(step_body))?;
            writeln!(html, "            </li>")?;
          }
          writeln!(html, "          </ol>")?;
        }
        DocBlock::Callout { tone, title, body } => {
          let tone_class = match tone {
            CalloutTone::Note => "docs-callout docs-callout-note",
            CalloutTone::Guardrail => "docs-callout docs-callout-guardrail",
          };
          writeln!(html, "          <div class=\"{tone_class}\">")?;
          writeln!(html, "            <p class=\"docs-callout-title\">{}</p>", escape_html(title))?;
          writeln!(html, "            <p class=\"docs-callout-body\">{}</p>", escape_html(body))?;
          writeln!(html, "          </div>")?;
        }
        DocBlock::Code { label, copy_text, raw_code } => {
          let expanded_code = expand_version(raw_code, version);
          writeln!(html, "          <div class=\"code-panel docs-code-panel\">")?;
          writeln!(html, "            <div class=\"code-panel-bar\">")?;
          writeln!(
            html,
            "              <span class=\"panel-bar-title\"><span class=\"win-dots\" aria-hidden=\"true\"><i></i><i></i><i></i></span><span>{}</span></span>",
            escape_html(label)
          )?;
          if let Some(copy_cmd) = copy_text {
            let expanded_copy = expand_version(copy_cmd, version);
            writeln!(
              html,
              "              <button class=\"copy-btn\" type=\"button\" data-copy-text=\"{}\" data-default-label=\"{}\" data-copied-label=\"{}\">{}</button>",
              escape_html(&expanded_copy),
              escape_html(copy.copy_label),
              escape_html(copy.copied_label),
              escape_html(copy.copy_label)
            )?;
          }
          writeln!(html, "            </div>")?;
          writeln!(
            html,
            "            <pre><code>{}</code></pre>",
            highlight_code(&expanded_code, CodeLang::Shell)
          )?;
          writeln!(html, "          </div>")?;
        }
        DocBlock::TabbedCode { label, tabs } => {
          let has_any_copy = tabs.iter().any(|tab| tab.copy_text.is_some());
          let default_copy =
            tabs.first().and_then(|tab| tab.copy_text).map(|text| expand_version(text, version));
          let tablist_aria = match page.locale {
            Locale::En => format!("{label} — operating system"),
            Locale::Zh => format!("{label} — 操作系统切换"),
          };
          writeln!(html, "          <div class=\"code-panel docs-code-panel\" data-code-tabs>")?;
          writeln!(html, "            <div class=\"code-panel-bar\">")?;
          writeln!(
            html,
            "              <span class=\"panel-bar-title\"><span class=\"win-dots\" aria-hidden=\"true\"><i></i><i></i><i></i></span><span>{}</span></span>",
            escape_html(label)
          )?;
          if has_any_copy {
            let hidden_btn = if default_copy.is_some() { "" } else { " hidden" };
            writeln!(
              html,
              "              <button class=\"copy-btn\" type=\"button\" data-code-copy-btn data-copy-text=\"{}\" data-default-label=\"{}\" data-copied-label=\"{}\"{hidden_btn}>{}</button>",
              escape_html(default_copy.as_deref().unwrap_or("")),
              escape_html(copy.copy_label),
              escape_html(copy.copied_label),
              escape_html(copy.copy_label)
            )?;
          }
          writeln!(html, "            </div>")?;
          writeln!(
            html,
            "            <div class=\"code-tab-bar\" role=\"tablist\" aria-label=\"{}\">",
            escape_html(&tablist_aria)
          )?;
          for (tab_idx, tab) in tabs.iter().enumerate() {
            let tab_dom_id = format!("code-tab-{}-{sec_idx}-{blk_idx}-{}", section.id, tab.id);
            let selected = if tab_idx == 0 { "true" } else { "false" };
            let tabindex = if tab_idx == 0 { "0" } else { "-1" };
            let primary_os = tab.os_targets.split_whitespace().next().unwrap_or("macos");
            writeln!(
              html,
              "              <button id=\"{tab_dom_id}\" class=\"code-tab\" type=\"button\" role=\"tab\" aria-selected=\"{selected}\" tabindex=\"{tabindex}\" data-code-tab=\"{tab_idx}\" data-os-targets=\"{targets}\" data-os-sync=\"{primary_os}\">{tab_label}</button>",
              targets = escape_html(tab.os_targets),
              tab_label = escape_html(tab.label),
            )?;
          }
          writeln!(html, "            </div>")?;
          for (tab_idx, tab) in tabs.iter().enumerate() {
            let tab_dom_id = format!("code-tab-{}-{sec_idx}-{blk_idx}-{}", section.id, tab.id);
            let hidden_attr = if tab_idx == 0 { "" } else { " hidden=\"until-found\"" };
            let expanded_code = expand_version(tab.raw_code, version);
            let expanded_copy = tab.copy_text.map(|text| expand_version(text, version));
            writeln!(
              html,
              "            <div class=\"code-tab-panel\" role=\"tabpanel\" aria-labelledby=\"{tab_dom_id}\" data-code-panel=\"{tab_idx}\" data-panel-copy=\"{}\"{hidden_attr}>",
              escape_html(expanded_copy.as_deref().unwrap_or(""))
            )?;
            writeln!(
              html,
              "              <pre><code>{}</code></pre>",
              highlight_code(&expanded_code, tab.lang)
            )?;
            writeln!(html, "            </div>")?;
          }
          writeln!(html, "          </div>")?;
        }
        DocBlock::Table { caption, headers, rows } => {
          let cap_id = format!("tbl-{}-{sec_idx}-{blk_idx}", section.id);
          writeln!(
            html,
            "          <div class=\"docs-table-wrap\" tabindex=\"0\" role=\"region\" aria-labelledby=\"{cap_id}\">"
          )?;
          writeln!(html, "            <table class=\"docs-table\">")?;
          writeln!(
            html,
            "              <caption id=\"{cap_id}\" class=\"sr-only\">{}</caption>",
            escape_html(caption)
          )?;
          writeln!(html, "              <thead>")?;
          writeln!(html, "                <tr>")?;
          for head in *headers {
            writeln!(html, "                  <th scope=\"col\">{}</th>", escape_html(head))?;
          }
          writeln!(html, "                </tr>")?;
          writeln!(html, "              </thead>")?;
          writeln!(html, "              <tbody>")?;
          for row in *rows {
            writeln!(html, "                <tr>")?;
            for (col_idx, cell) in row.iter().enumerate() {
              if col_idx == 0 {
                writeln!(
                  html,
                  "                  <th scope=\"row\" class=\"docs-row-head\">{}</th>",
                  escape_html(cell)
                )?;
              } else {
                writeln!(html, "                  <td>{}</td>", escape_html(cell))?;
              }
            }
            writeln!(html, "                </tr>")?;
          }
          writeln!(html, "              </tbody>")?;
          writeln!(html, "            </table>")?;
          writeln!(html, "          </div>")?;
        }
        DocBlock::Figure { webp_path, image_path, width, height, alt, caption } => {
          writeln!(html, "          <figure class=\"docs-figure\">")?;
          writeln!(html, "            <picture>")?;
          writeln!(
            html,
            "              <source type=\"image/webp\" srcset=\"{}\">",
            escape_html(webp_path)
          )?;
          writeln!(
            html,
            "              <img class=\"docs-figure-img\" src=\"{}\" alt=\"{}\" width=\"{width}\" height=\"{height}\" loading=\"lazy\" decoding=\"async\">",
            escape_html(image_path),
            escape_html(alt)
          )?;
          writeln!(html, "            </picture>")?;
          writeln!(
            html,
            "            <figcaption class=\"docs-figcaption\">{}</figcaption>",
            escape_html(caption)
          )?;
          writeln!(html, "          </figure>")?;
        }
        DocBlock::McpPresets => {
          writeln!(html, "          <div class=\"code-panel docs-code-panel\">")?;
          writeln!(html, "            <div class=\"code-panel-bar\">")?;
          writeln!(
            html,
            "              <span class=\"panel-bar-title\"><span class=\"win-dots\" aria-hidden=\"true\"><i></i><i></i><i></i></span><span>{}</span></span>",
            escape_html(copy.agents_terminal_bar)
          )?;
          writeln!(html, "              <span class=\"env-pill-prod\">PROD HIDDEN</span>")?;
          writeln!(html, "            </div>")?;
          render_mcp_switcher(&mut html, copy, "mcp-switcher docs-mcp-switcher")?;
          writeln!(html, "          </div>")?;
        }
        DocBlock::CompatibilityMatrix => {
          writeln!(html, "          <div class=\"docs-ledger-box\">")?;
          writeln!(html, "            <div class=\"ledger-controls\">")?;
          writeln!(
            html,
            "              <div class=\"filter-group\" role=\"group\" aria-label=\"Database category\">"
          )?;
          writeln!(
            html,
            "                <button class=\"filter-btn\" type=\"button\" aria-pressed=\"true\" data-ledger-filter=\"all\">{}</button>",
            escape_html(copy.filter_all)
          )?;
          writeln!(
            html,
            "                <button class=\"filter-btn\" type=\"button\" aria-pressed=\"false\" data-ledger-filter=\"relational\">{}</button>",
            escape_html(copy.filter_relational)
          )?;
          writeln!(
            html,
            "                <button class=\"filter-btn\" type=\"button\" aria-pressed=\"false\" data-ledger-filter=\"workspace\">{}</button>",
            escape_html(copy.filter_workspaces)
          )?;
          writeln!(html, "              </div>")?;
          writeln!(html, "            </div>")?;
          writeln!(
            html,
            "            <div class=\"ledger-wrap\" tabindex=\"0\" role=\"region\" aria-label=\"{}\">",
            escape_html(copy.compatibility_heading)
          )?;
          writeln!(html, "              <table class=\"ledger-table\">")?;
          writeln!(html, "                <thead>")?;
          writeln!(html, "                  <tr>")?;
          writeln!(
            html,
            "                    <th scope=\"col\">{}</th>",
            escape_html(copy.col_server)
          )?;
          writeln!(
            html,
            "                    <th scope=\"col\">{}</th>",
            escape_html(copy.col_version)
          )?;
          writeln!(
            html,
            "                    <th scope=\"col\">{}</th>",
            escape_html(copy.col_driver)
          )?;
          writeln!(
            html,
            "                    <th scope=\"col\">{}</th>",
            escape_html(copy.col_status)
          )?;
          writeln!(
            html,
            "                    <th scope=\"col\">{}</th>",
            escape_html(copy.col_smoke)
          )?;
          writeln!(
            html,
            "                    <th scope=\"col\">{}</th>",
            escape_html(copy.col_notes)
          )?;
          writeln!(html, "                  </tr>")?;
          writeln!(html, "                </thead>")?;
          writeln!(html, "                <tbody>")?;
          for row in copy.servers {
            let (status_class, status_label) = match row.level {
              SupportLevel::Supported => ("status-ok", copy.status_supported),
              SupportLevel::UsableWithGaps => ("status-gap", copy.status_gaps),
            };
            writeln!(html, "                  <tr data-ledger-kind=\"{}\">", row.kind.as_attr())?;
            writeln!(
              html,
              "                    <td class=\"ledger-server\">{}</td>",
              escape_html(row.server)
            )?;
            writeln!(
              html,
              "                    <td class=\"ledger-mono\">{}</td>",
              escape_html(row.tested_version)
            )?;
            writeln!(
              html,
              "                    <td><span class=\"driver-chip\">{}</span></td>",
              escape_html(row.connection_type)
            )?;
            writeln!(
              html,
              "                    <td><span class=\"{status_class}\">{}</span></td>",
              escape_html(status_label)
            )?;
            write!(html, "                    <td class=\"ledger-mono\">")?;
            render_smoke_counts(&mut html, row.smoke_counts)?;
            writeln!(html, "</td>")?;
            writeln!(
              html,
              "                    <td class=\"ledger-note\">{}</td>",
              escape_html(row.notes)
            )?;
            writeln!(html, "                  </tr>")?;
          }
          writeln!(html, "                </tbody>")?;
          writeln!(html, "              </table>")?;
          writeln!(html, "            </div>")?;
          writeln!(html, "          </div>")?;
        }
      }
    }

    writeln!(html, "        </section>")?;
  }

  // Prev / Next pager
  let prev_word = match page.locale {
    Locale::En => "Previous",
    Locale::Zh => "上一篇",
  };
  let next_word = match page.locale {
    Locale::En => "Next",
    Locale::Zh => "下一篇",
  };
  writeln!(html, "        <nav class=\"docs-pager\" aria-label=\"Pagination\">")?;
  if let Some(prev_slug) = page.slug.prev() {
    writeln!(
      html,
      "          <a class=\"docs-pager-card\" href=\"{}\"><span class=\"docs-pager-dir\">{}</span><strong class=\"docs-pager-label\">{}</strong></a>",
      prev_slug.path(page.locale),
      escape_html(prev_word),
      escape_html(prev_slug.nav_label(page.locale))
    )?;
  } else {
    writeln!(html, "          <span></span>")?;
  }
  if let Some(next_slug) = page.slug.next() {
    writeln!(
      html,
      "          <a class=\"docs-pager-card docs-pager-next\" href=\"{}\"><span class=\"docs-pager-dir\">{}</span><strong class=\"docs-pager-label\">{}</strong></a>",
      next_slug.path(page.locale),
      escape_html(next_word),
      escape_html(next_slug.nav_label(page.locale))
    )?;
  }
  writeln!(html, "        </nav>")?;

  writeln!(html, "      </article>")?;
  writeln!(html, "    </div>")?;
  writeln!(html, "  </main>")?;

  render_footer(&mut html, copy, alternate_path)?;

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
  <meta name=\"robots\" content=\"noindex\">\n\
  <title>404 — DataOmni</title>\n\
  <link rel=\"icon\" href=\"/favicon.ico\" sizes=\"any\">\n\
  <link rel=\"icon\" type=\"image/png\" sizes=\"96x96\" href=\"/assets/favicon-96.png\">\n\
  <link rel=\"icon\" type=\"image/png\" sizes=\"32x32\" href=\"/assets/favicon-32.png\">\n\
  <link rel=\"stylesheet\" href=\"/assets/style.css\">\n\
</head>\n\
<body>\n\
  <main class=\"shell hero\">\n\
    <h1 class=\"hero-title\">Page not found (404)</h1>\n\
    <p class=\"hero-lead\">The requested path does not exist on dataomni.dev.</p>\n\
    <p><a class=\"btn-primary\" href=\"/\">English home</a> <a class=\"btn-secondary\" href=\"/docs/quickstart/\">Documentation</a> <a class=\"btn-secondary\" href=\"/zh/\">中文首页</a></p>\n\
  </main>\n\
</body>\n\
</html>\n"
}

pub fn render_docs_hub_redirect(locale: Locale) -> String {
  let target_path = DocSlug::Quickstart.path(locale);
  let canonical_url = format!("{SITE_ORIGIN}{target_path}");
  let (title, heading, cta) = match locale {
    Locale::En => ("Documentation — DataOmni", "DataOmni Documentation", "Continue to Quickstart"),
    Locale::Zh => ("使用文档 — DataOmni", "DataOmni 使用文档", "前往快速上手"),
  };
  format!(
    "<!DOCTYPE html>\n\
<html lang=\"{lang}\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
  <meta name=\"robots\" content=\"noindex, follow\">\n\
  <link rel=\"canonical\" href=\"{canonical_url}\">\n\
  <meta http-equiv=\"refresh\" content=\"0; url={target_path}\">\n\
  <title>{title}</title>\n\
  <link rel=\"icon\" href=\"/favicon.ico\" sizes=\"any\">\n\
  <link rel=\"icon\" type=\"image/png\" sizes=\"96x96\" href=\"/assets/favicon-96.png\">\n\
  <link rel=\"icon\" type=\"image/png\" sizes=\"32x32\" href=\"/assets/favicon-32.png\">\n\
  <link rel=\"stylesheet\" href=\"/assets/style.css\">\n\
  <script>window.location.replace(\"{target_path}\");</script>\n\
</head>\n\
<body>\n\
  <main class=\"shell hero\">\n\
    <h1 class=\"hero-title\">{heading}</h1>\n\
    <p><a class=\"btn-primary\" href=\"{target_path}\">{cta}</a></p>\n\
  </main>\n\
</body>\n\
</html>\n",
    lang = locale.html_lang(),
    canonical_url = escape_html(&canonical_url),
    target_path = target_path,
    title = escape_html(title),
    heading = escape_html(heading),
    cta = escape_html(cta),
  )
}

pub fn render_robots_txt() -> String {
  format!("User-agent: *\nAllow: /\n\nSitemap: {SITE_ORIGIN}/sitemap.xml\n")
}

pub fn render_sitemap_xml(released_at: &str) -> String {
  let mut xml = String::with_capacity(6144);
  let _ = writeln!(xml, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
  let _ = writeln!(
    xml,
    "<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\" xmlns:image=\"http://www.google.com/schemas/sitemap-image/1.1\">"
  );
  for copy in [&EN_COPY, &ZH_COPY] {
    let loc = format!("{SITE_ORIGIN}{}", copy.locale.canonical_path());
    let _ = writeln!(xml, "  <url>");
    let _ = writeln!(xml, "    <loc>{}</loc>", escape_html(&loc));
    let _ = writeln!(xml, "    <lastmod>{}</lastmod>", escape_html(released_at));
    let _ =
      writeln!(xml, "    <xhtml:link rel=\"alternate\" hreflang=\"en\" href=\"{SITE_ORIGIN}/\"/>");
    let _ = writeln!(
      xml,
      "    <xhtml:link rel=\"alternate\" hreflang=\"zh-CN\" href=\"{SITE_ORIGIN}/zh/\"/>"
    );
    let _ = writeln!(
      xml,
      "    <xhtml:link rel=\"alternate\" hreflang=\"x-default\" href=\"{SITE_ORIGIN}/\"/>"
    );
    for tab in copy.tabs {
      let _ = writeln!(
        xml,
        "    <image:image><image:loc>{SITE_ORIGIN}{}</image:loc></image:image>",
        escape_html(tab.webp_path)
      );
    }
    let _ = writeln!(xml, "  </url>");
  }

  for slug in ALL_DOC_SLUGS {
    let en_url = format!("{SITE_ORIGIN}{}", slug.path(Locale::En));
    let zh_url = format!("{SITE_ORIGIN}{}", slug.path(Locale::Zh));
    for loc in [&en_url, &zh_url] {
      let _ = writeln!(xml, "  <url>");
      let _ = writeln!(xml, "    <loc>{}</loc>", escape_html(loc));
      let _ = writeln!(xml, "    <lastmod>{}</lastmod>", escape_html(released_at));
      let _ = writeln!(
        xml,
        "    <xhtml:link rel=\"alternate\" hreflang=\"en\" href=\"{}\"/>",
        escape_html(&en_url)
      );
      let _ = writeln!(
        xml,
        "    <xhtml:link rel=\"alternate\" hreflang=\"zh-CN\" href=\"{}\"/>",
        escape_html(&zh_url)
      );
      let _ = writeln!(
        xml,
        "    <xhtml:link rel=\"alternate\" hreflang=\"x-default\" href=\"{}\"/>",
        escape_html(&en_url)
      );
      let _ = writeln!(xml, "  </url>");
    }
  }

  let _ = writeln!(xml, "</urlset>");
  xml
}

pub fn render_llms_txt(version: &str, released_at: &str) -> Result<String, std::fmt::Error> {
  let assets = ReleaseAssets::for_version(version);
  let mut md = String::with_capacity(4096);
  writeln!(md, "# DataOmni")?;
  writeln!(md)?;
  writeln!(
    md,
    "> Native Rust + Tauri 2 desktop database client for PostgreSQL, MySQL, SQLite, SQL Server, Oracle, DuckDB, ClickHouse, MongoDB, Redis, Neo4j, and Elasticsearch — with previewed SQL transactions, OS keychain credentials, schema diff, deterministic codegen, and a built-in read-only stdio MCP server (`dataomni mcp`)."
  )?;
  writeln!(md)?;
  writeln!(md, "- Latest stable version: `v{version}` ({released_at})")?;
  writeln!(md, "- License: MIT (`{REPO_URL}`)")?;
  writeln!(md, "- Platforms: macOS 12+ (Apple Silicon, Developer ID signed & notarized), Windows 10/11 (x64 MSI & NSIS), Linux (x86_64 `.deb` & `.rpm`)")?;
  writeln!(md, "- macOS Homebrew Tap: `brew install --cask dataomni-rs/tap/dataomni`")?;
  writeln!(
    md,
    "- Zero telemetry, no cloud account, credentials stored exclusively in OS keychains"
  )?;
  writeln!(md)?;
  writeln!(md, "## Documentation")?;
  writeln!(md)?;
  for slug in ALL_DOC_SLUGS {
    let page = doc_page(*slug, Locale::En);
    writeln!(
      md,
      "- [{}]({SITE_ORIGIN}{}): {}",
      page.title,
      slug.path(Locale::En),
      page.meta_description
    )?;
  }
  writeln!(
    md,
    "- [Full LLM Reference (Single-File Markdown)]({SITE_ORIGIN}/llms-full.txt): Complete documentation, compatibility matrix, MCP tools, CLI exit codes, and security invariants."
  )?;
  writeln!(
    md,
    "- [CLI Agent Skill (SKILL.md)]({SITE_ORIGIN}/SKILL.md): Drop-in SKILL.md for Claude Code, Codex, Cursor, and terminal agents covering all read-only dataomni CLI commands, display flags, and exit codes."
  )?;
  writeln!(md)?;
  writeln!(md, "## Direct Release Packages (`v{version}`)")?;
  writeln!(md)?;
  writeln!(md, "- [Release Notes (`v{version}`)]({})", assets.release_tag_url)?;
  writeln!(md, "- [macOS Apple Silicon (`.dmg`)]({})", assets.dmg_url)?;
  writeln!(md, "- [Windows Installer (`-setup.exe`)]({})", assets.exe_url)?;
  writeln!(md, "- [Windows Enterprise (`.msi`)]({})", assets.msi_url)?;
  writeln!(md, "- [Linux Debian/Ubuntu (`.deb`)]({})", assets.deb_url)?;
  writeln!(md, "- [Linux Fedora/RHEL (`.rpm`)]({})", assets.rpm_url)?;
  writeln!(md)?;
  writeln!(md, "## Chinese Documentation (中文文档)")?;
  writeln!(md)?;
  writeln!(md, "- [中文首页]({SITE_ORIGIN}/zh/): {}", ZH_COPY.meta_description)?;
  for slug in ALL_DOC_SLUGS {
    let page = doc_page(*slug, Locale::Zh);
    writeln!(
      md,
      "- [{}]({SITE_ORIGIN}{}): {}",
      page.title,
      slug.path(Locale::Zh),
      page.meta_description
    )?;
  }
  Ok(md)
}

pub fn render_llms_full_txt(version: &str, released_at: &str) -> Result<String, std::fmt::Error> {
  let mut md = render_llms_txt(version, released_at)?;
  writeln!(md)?;
  writeln!(md, "---")?;
  writeln!(md)?;
  writeln!(md, "## Frequently Asked Questions")?;
  writeln!(md)?;
  for item in EN_COPY.faq_items {
    writeln!(md, "### {}", item.question)?;
    writeln!(md)?;
    writeln!(md, "{}", item.answer)?;
    writeln!(md)?;
  }

  for slug in ALL_DOC_SLUGS {
    let page = doc_page(*slug, Locale::En);
    writeln!(md, "---")?;
    writeln!(md)?;
    writeln!(md, "# {} (`{SITE_ORIGIN}{}`)", page.title, slug.path(Locale::En))?;
    writeln!(md)?;
    writeln!(md, "{}", page.lead)?;
    writeln!(md)?;

    for section in page.sections {
      writeln!(md, "## {}", section.heading)?;
      writeln!(md)?;
      for block in section.blocks {
        match block {
          DocBlock::Paragraph(text) => {
            writeln!(md, "{text}")?;
            writeln!(md)?;
          }
          DocBlock::Bullets(items) => {
            for item in *items {
              writeln!(md, "- {item}")?;
            }
            writeln!(md)?;
          }
          DocBlock::Steps(steps) => {
            for (idx, (title, body)) in steps.iter().enumerate() {
              writeln!(md, "{}. **{}** — {}", idx + 1, title, body)?;
            }
            writeln!(md)?;
          }
          DocBlock::Callout { title, body, .. } => {
            writeln!(md, "> **{title}**: {body}")?;
            writeln!(md)?;
          }
          DocBlock::Code { label, raw_code, .. } => {
            let expanded = expand_version(raw_code, version);
            writeln!(md, "*{label}*:")?;
            writeln!(md, "```sh")?;
            writeln!(md, "{expanded}")?;
            writeln!(md, "```")?;
            writeln!(md)?;
          }
          DocBlock::TabbedCode { label, tabs } => {
            writeln!(md, "*{label}*:")?;
            writeln!(md)?;
            for tab in *tabs {
              let fence = match tab.lang {
                CodeLang::Shell => "sh",
                CodeLang::PowerShell => "powershell",
                CodeLang::Json => "json",
              };
              let expanded = expand_version(tab.raw_code, version);
              writeln!(md, "**{}**:", tab.label)?;
              writeln!(md, "```{fence}")?;
              writeln!(md, "{expanded}")?;
              writeln!(md, "```")?;
              writeln!(md)?;
            }
          }
          DocBlock::Table { headers, rows, .. } => {
            writeln!(md, "| {} |", headers.join(" | "))?;
            let sep: Vec<&str> = headers.iter().map(|_| "---").collect();
            writeln!(md, "| {} |", sep.join(" | "))?;
            for row in *rows {
              writeln!(md, "| {} |", row.join(" | "))?;
            }
            writeln!(md)?;
          }
          DocBlock::Figure { webp_path, alt, caption, .. } => {
            writeln!(md, "![{}]({SITE_ORIGIN}{})", alt, webp_path)?;
            writeln!(md, "*{caption}*")?;
            writeln!(md)?;
          }
          DocBlock::McpPresets => {
            for preset in EN_COPY.mcp_presets {
              writeln!(md, "- **{}**:", preset.label)?;
              writeln!(md, "  - macOS: `{}`", preset.macos_command)?;
              writeln!(md, "  - Windows: `{}`", preset.windows_command)?;
              writeln!(md, "  - Linux: `{}`", preset.linux_command)?;
            }
            writeln!(md)?;
          }
          DocBlock::CompatibilityMatrix => {
            writeln!(
              md,
              "| Server | Tested Version | Driver | Status | Smoke (pass/skip/gap) | Notes |"
            )?;
            writeln!(md, "| --- | --- | --- | --- | --- | --- |")?;
            for row in EN_COPY.servers {
              let status = match row.level {
                SupportLevel::Supported => EN_COPY.status_supported,
                SupportLevel::UsableWithGaps => EN_COPY.status_gaps,
              };
              writeln!(
                md,
                "| {} | {} | {} | {} | {} | {} |",
                row.server,
                row.tested_version,
                row.connection_type,
                status,
                row.smoke_counts,
                row.notes
              )?;
            }
            writeln!(md)?;
          }
        }
      }
    }
  }
  Ok(md)
}
