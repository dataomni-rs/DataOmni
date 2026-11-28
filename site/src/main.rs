mod content;
mod docs;
mod render;

use content::{Locale, EN_COPY, ZH_COPY};
use docs::{doc_page, DocBlock, ALL_DOC_SLUGS};
use render::{
  render_doc_page, render_docs_hub_redirect, render_llms_full_txt, render_llms_txt,
  render_not_found, render_page, render_robots_txt, render_sitemap_xml, INDEXNOW_KEY,
  ROBOTS_INDEX_DIRECTIVE,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const CUSTOM_DOMAIN: &str = "dataomni.dev\n";
const RELEASE_VERSION_ENV: &str = "DATAOMNI_SITE_RELEASE_VERSION";
const RELEASED_AT_ENV: &str = "DATAOMNI_SITE_RELEASED_AT";

fn extract_json_string_field<'a>(config_text: &'a str, field_name: &str) -> Option<&'a str> {
  let quoted_key = format!("\"{field_name}\"");
  for line in config_text.lines() {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix(&quoted_key) else {
      continue;
    };
    let Some(after_colon) = rest.trim_start().strip_prefix(':') else {
      continue;
    };
    let candidate = after_colon.trim().trim_end_matches(',').trim();
    if let Some(unquoted) = candidate.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
      return Some(unquoted);
    }
  }
  None
}

fn extract_release_version(config_text: &str) -> Result<String, String> {
  let Some(unquoted) = extract_json_string_field(config_text, "version") else {
    return Err("missing \"version\" field in site/release.json".to_string());
  };
  parse_stable_version(unquoted)
}

fn extract_released_at(config_text: &str) -> Result<String, String> {
  let Some(unquoted) = extract_json_string_field(config_text, "released_at") else {
    return Err("missing \"released_at\" field in site/release.json".to_string());
  };
  parse_iso_date(unquoted)
}

fn parse_stable_version(raw: &str) -> Result<String, String> {
  let version = raw.trim().strip_prefix('v').unwrap_or(raw.trim());
  if is_stable_semver(version) {
    Ok(version.to_string())
  } else {
    Err(format!("invalid stable release version (prereleases not allowed on site): {raw}"))
  }
}

fn parse_iso_date(raw: &str) -> Result<String, String> {
  let date = raw.trim();
  if is_iso_date(date) {
    Ok(date.to_string())
  } else {
    Err(format!("invalid ISO release date (expected YYYY-MM-DD): {raw}"))
  }
}

fn is_iso_date(date: &str) -> bool {
  let mut parts = date.split('-');
  let (Some(year), Some(month), Some(day), None) =
    (parts.next(), parts.next(), parts.next(), parts.next())
  else {
    return false;
  };
  if year.len() != 4 || month.len() != 2 || day.len() != 2 {
    return false;
  }
  let Ok(y) = year.parse::<u16>() else {
    return false;
  };
  let Ok(m) = month.parse::<u8>() else {
    return false;
  };
  let Ok(d) = day.parse::<u8>() else {
    return false;
  };
  (2025..=2099).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d)
}

fn is_stable_semver(version: &str) -> bool {
  let mut parts = version.split('.');
  let (Some(major), Some(minor), Some(patch), None) =
    (parts.next(), parts.next(), parts.next(), parts.next())
  else {
    return false;
  };
  let digits = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
  digits(major) && digits(minor) && digits(patch)
}

fn resolve_release_version(repo_root: &Path, env_override: Option<&str>) -> Result<String, String> {
  if let Some(raw) = env_override.map(str::trim).filter(|value| !value.is_empty()) {
    return parse_stable_version(raw);
  }
  let release_path = repo_root.join("site/release.json");
  let release_json = fs::read_to_string(&release_path)
    .map_err(|err| format!("failed to read {}: {err}", release_path.display()))?;
  extract_release_version(&release_json)
}

fn resolve_released_at(repo_root: &Path, env_override: Option<&str>) -> Result<String, String> {
  if let Some(raw) = env_override.map(str::trim).filter(|value| !value.is_empty()) {
    return parse_iso_date(raw);
  }
  let release_path = repo_root.join("site/release.json");
  let release_json = fs::read_to_string(&release_path)
    .map_err(|err| format!("failed to read {}: {err}", release_path.display()))?;
  extract_released_at(&release_json)
}

fn copy_checked(src: &Path, dst: &Path) -> Result<(), String> {
  if let Some(parent) = dst.parent() {
    fs::create_dir_all(parent)
      .map_err(|err| format!("failed to create directory {}: {err}", parent.display()))?;
  }
  fs::copy(src, dst)
    .map(|_| ())
    .map_err(|err| format!("failed to copy {} -> {}: {err}", src.display(), dst.display()))
}

fn write_checked(path: &Path, contents: &str) -> Result<(), String> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)
      .map_err(|err| format!("failed to create directory {}: {err}", parent.display()))?;
  }
  fs::write(path, contents).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

pub fn verify_html_invariants(page_name: &str, html: &str, out_dir: &Path) -> Result<(), String> {
  let high_priority_count = html.matches("fetchpriority=\"high\"").count();
  let expected_high_priority = usize::from(matches!(page_name, "index.html" | "zh/index.html"));
  if high_priority_count != expected_high_priority {
    return Err(format!(
      "{page_name}: expected {expected_high_priority} fetchpriority=\"high\" LCP image(s), found {high_priority_count}"
    ));
  }

  let mut cursor = html;
  while let Some(start) = cursor.find("<img ") {
    let after = &cursor[start..];
    let Some(end) = after.find('>') else {
      return Err(format!("{page_name}: unclosed <img> tag"));
    };
    let tag = &after[..=end];
    if !tag.contains("width=\"") || !tag.contains("height=\"") {
      return Err(format!("{page_name}: <img> missing explicit width/height: {tag}"));
    }
    if !tag.contains("alt=\"") || tag.contains("alt=\"\"") {
      return Err(format!("{page_name}: <img> missing non-empty alt text: {tag}"));
    }
    if tag.contains("fetchpriority=\"high\"") && tag.contains("loading=\"lazy\"") {
      return Err(format!(
        "{page_name}: LCP image must not combine fetchpriority=\"high\" with loading=\"lazy\""
      ));
    }
    cursor = &after[end + 1..];
  }

  // Screen readers (JAWS/TalkBack) drop headings nested inside <summary>
  let mut summary_scan = html;
  while let Some(start) = summary_scan.find("<summary") {
    let after = &summary_scan[start..];
    let Some(end) = after.find("</summary>") else {
      return Err(format!("{page_name}: unclosed <summary> element"));
    };
    let summary_block = &after[..end];
    for heading in ["<h1", "<h2", "<h3", "<h4", "<h5", "<h6"] {
      if summary_block.contains(heading) {
        return Err(format!("{page_name}: <summary> must not contain heading element {heading}"));
      }
    }
    summary_scan = &after[end + "</summary>".len()..];
  }

  // Every in-page anchor href="#..." must target a real id="..."
  let mut anchor_scan = html;
  while let Some(pos) = anchor_scan.find("href=\"#") {
    let after = &anchor_scan[pos + "href=\"#".len()..];
    let Some(end) = after.find('"') else {
      return Err(format!("{page_name}: unclosed anchor href"));
    };
    let target_id = &after[..end];
    if target_id.is_empty() || !html.contains(&format!("id=\"{target_id}\"")) {
      return Err(format!(
        "{page_name}: broken in-page anchor href=\"#{target_id}\" (no matching id)"
      ));
    }
    anchor_scan = &after[end + 1..];
  }

  let is_utility_page = matches!(page_name, "404.html" | "docs/index.html" | "zh/docs/index.html");
  if is_utility_page {
    if !html.contains("<meta name=\"robots\" content=\"noindex") {
      return Err(format!("{page_name}: utility/redirect page must declare robots noindex"));
    }
    if html.contains("max-image-preview:large") {
      return Err(format!("{page_name}: noindex utility page must not declare max-image-preview"));
    }
  } else {
    let expected_robots = format!("<meta name=\"robots\" content=\"{ROBOTS_INDEX_DIRECTIVE}\">");
    if !html.contains(&expected_robots) {
      return Err(format!("{page_name}: missing canonical robots directive `{expected_robots}`"));
    }
    if html.contains("noindex") {
      return Err(format!("{page_name}: indexed page must not contain `noindex`"));
    }

    let Some(title_start) = html.find("<title>") else {
      return Err(format!("{page_name}: missing <title>"));
    };
    let after_title = &html[title_start + "<title>".len()..];
    let Some(title_end) = after_title.find("</title>") else {
      return Err(format!("{page_name}: unclosed <title>"));
    };
    let title_chars = after_title[..title_end].chars().count();
    if !(20..=65).contains(&title_chars) {
      return Err(format!(
        "{page_name}: <title> length {title_chars} outside SERP-safe 20..=65 chars"
      ));
    }

    let desc_prefix = "<meta name=\"description\" content=\"";
    let Some(desc_start) = html.find(desc_prefix) else {
      return Err(format!("{page_name}: missing meta description"));
    };
    let after_desc = &html[desc_start + desc_prefix.len()..];
    let Some(desc_end) = after_desc.find("\">") else {
      return Err(format!("{page_name}: unclosed meta description"));
    };
    let desc_chars = after_desc[..desc_end].chars().count();
    if !(70..=165).contains(&desc_chars) {
      return Err(format!(
        "{page_name}: meta description length {desc_chars} outside SERP-safe 70..=165 chars"
      ));
    }
  }

  for attr in [
    "src=\"/",
    "srcset=\"/",
    "data-deferred-src=\"/",
    "data-deferred-srcset=\"/",
    "href=\"/favicon.ico",
    "href=\"/llms.txt",
    "href=\"/llms-full.txt",
    "href=\"/SKILL.md",
    "href=\"/assets/",
    "href=\"/images/",
  ] {
    let mut scan = html;
    while let Some(pos) = scan.find(attr) {
      let quote_start = pos + attr.find('"').map_or(0, |idx| idx + 1);
      let rest = &scan[quote_start..];
      let Some(quote_end) = rest.find('"') else {
        return Err(format!("{page_name}: unclosed attribute near {attr}"));
      };
      let url_path = &rest[..quote_end];
      let rel = url_path.trim_start_matches('/');
      let target = out_dir.join(rel);
      if !target.is_file() {
        return Err(format!(
          "{page_name}: referenced local asset `{url_path}` does not exist at {}",
          target.display()
        ));
      }
      scan = &rest[quote_end + 1..];
    }
  }

  let og_prefix = "content=\"https://dataomni.dev/";
  let mut og_scan = html;
  while let Some(pos) = og_scan.find(og_prefix) {
    let after = &og_scan[pos + og_prefix.len()..];
    let Some(end) = after.find('"') else {
      return Err(format!("{page_name}: unclosed https://dataomni.dev/ meta content"));
    };
    let rel = &after[..end];
    if !rel.is_empty() && !rel.ends_with('/') {
      let target = out_dir.join(rel);
      if !target.is_file() {
        return Err(format!(
          "{page_name}: meta URL https://dataomni.dev/{rel} does not exist at {}",
          target.display()
        ));
      }
    }
    og_scan = &after[end + 1..];
  }

  // Every internal page route href="/..." must resolve to a generated HTML page (and fragment if any)
  let mut route_scan = html;
  while let Some(pos) = route_scan.find("href=\"/") {
    let after = &route_scan[pos + "href=\"".len()..];
    let Some(end) = after.find('"') else {
      return Err(format!("{page_name}: unclosed root-relative href"));
    };
    let raw_href = &after[..end];
    if !raw_href.starts_with("/favicon.ico")
      && !raw_href.starts_with("/llms.txt")
      && !raw_href.starts_with("/llms-full.txt")
      && !raw_href.starts_with("/SKILL.md")
      && !raw_href.starts_with("/assets/")
      && !raw_href.starts_with("/images/")
    {
      let (route_path, fragment) = match raw_href.split_once('#') {
        Some((path, frag)) => (path, Some(frag)),
        None => (raw_href, None),
      };
      if !route_path.ends_with('/') {
        return Err(format!(
          "{page_name}: internal route href `{raw_href}` must end with trailing slash `/`"
        ));
      }
      let trimmed = route_path.trim_matches('/');
      let target_html = if trimmed.is_empty() {
        out_dir.join("index.html")
      } else {
        out_dir.join(trimmed).join("index.html")
      };
      if !target_html.is_file() {
        return Err(format!(
          "{page_name}: internal route `{raw_href}` does not exist at {}",
          target_html.display()
        ));
      }
      if let Some(frag) = fragment {
        let target_contents = fs::read_to_string(&target_html)
          .map_err(|err| format!("failed to read {}: {err}", target_html.display()))?;
        if frag.is_empty() || !target_contents.contains(&format!("id=\"{frag}\"")) {
          return Err(format!(
            "{page_name}: cross-page anchor `{raw_href}` missing id=\"{frag}\" in {}",
            target_html.display()
          ));
        }
      }
    }
    route_scan = &after[end + 1..];
  }

  Ok(())
}

pub fn build_site(repo_root: &Path, out_dir: &Path) -> Result<String, String> {
  let env_override = std::env::var(RELEASE_VERSION_ENV).ok();
  let env_date_override = std::env::var(RELEASED_AT_ENV).ok();
  let version = resolve_release_version(repo_root, env_override.as_deref())?;
  let released_at = resolve_released_at(repo_root, env_date_override.as_deref())?;

  if out_dir.exists() {
    fs::remove_dir_all(out_dir)
      .map_err(|err| format!("failed to clean {}: {err}", out_dir.display()))?;
  }
  fs::create_dir_all(out_dir)
    .map_err(|err| format!("failed to create {}: {err}", out_dir.display()))?;

  // Static styles, progressive-enhancement script, OpenGraph cards, and Google-compliant favicons
  let site_dir = repo_root.join("site");
  copy_checked(&site_dir.join("assets/style.css"), &out_dir.join("assets/style.css"))?;
  copy_checked(&site_dir.join("assets/site.js"), &out_dir.join("assets/site.js"))?;
  copy_checked(&site_dir.join("assets/favicon-96.png"), &out_dir.join("assets/favicon-96.png"))?;
  copy_checked(&site_dir.join("assets/og-en.png"), &out_dir.join("assets/og-en.png"))?;
  copy_checked(&site_dir.join("assets/og-zh.png"), &out_dir.join("assets/og-zh.png"))?;
  copy_checked(&repo_root.join("src-tauri/icons/icon.ico"), &out_dir.join("favicon.ico"))?;
  copy_checked(
    &repo_root.join("src-tauri/icons/128x128@2x.png"),
    &out_dir.join("assets/icon-256.png"),
  )?;
  copy_checked(
    &repo_root.join("src-tauri/icons/32x32.png"),
    &out_dir.join("assets/favicon-32.png"),
  )?;

  // Copy real Retina screenshots (WebP primary + PNG fallback/OpenGraph) for both locales
  for tab in EN_COPY.tabs.iter().chain(ZH_COPY.tabs.iter()) {
    let png_rel = tab.image_path.trim_start_matches('/');
    copy_checked(&repo_root.join(png_rel), &out_dir.join(png_rel))?;
    let webp_rel = tab.webp_path.trim_start_matches('/');
    copy_checked(&site_dir.join(webp_rel), &out_dir.join(webp_rel))?;
  }

  // Copy documentation figure screenshots (WebP + PNG)
  for slug in ALL_DOC_SLUGS {
    for locale in [Locale::En, Locale::Zh] {
      for section in doc_page(*slug, locale).sections {
        for block in section.blocks {
          if let DocBlock::Figure { webp_path, image_path, .. } = block {
            let png_rel = image_path.trim_start_matches('/');
            copy_checked(&repo_root.join(png_rel), &out_dir.join(png_rel))?;
            let webp_rel = webp_path.trim_start_matches('/');
            copy_checked(&site_dir.join(webp_rel), &out_dir.join(webp_rel))?;
          }
        }
      }
    }
  }

  let en_html = render_page(&EN_COPY, &version, &released_at)
    .map_err(|err| format!("render EN failed: {err}"))?;
  let zh_html = render_page(&ZH_COPY, &version, &released_at)
    .map_err(|err| format!("render ZH failed: {err}"))?;
  let not_found_html = render_not_found();
  let en_docs_hub_html = render_docs_hub_redirect(Locale::En);
  let zh_docs_hub_html = render_docs_hub_redirect(Locale::Zh);
  let llms_txt = render_llms_txt(&version, &released_at)
    .map_err(|err| format!("render llms.txt failed: {err}"))?;
  let llms_full_txt = render_llms_full_txt(&version, &released_at)
    .map_err(|err| format!("render llms-full.txt failed: {err}"))?;

  let mut generated_docs: Vec<(&'static str, String)> = Vec::with_capacity(ALL_DOC_SLUGS.len() * 2);
  for slug in ALL_DOC_SLUGS {
    for locale in [Locale::En, Locale::Zh] {
      let rel = slug.output_rel_html(locale);
      let html = render_doc_page(doc_page(*slug, locale), &version, &released_at)
        .map_err(|err| format!("render {rel} failed: {err}"))?;
      write_checked(&out_dir.join(rel), &html)?;
      generated_docs.push((rel, html));
    }
  }

  write_checked(&out_dir.join("index.html"), &en_html)?;
  write_checked(&out_dir.join("zh/index.html"), &zh_html)?;
  write_checked(&out_dir.join("docs/index.html"), &en_docs_hub_html)?;
  write_checked(&out_dir.join("zh/docs/index.html"), &zh_docs_hub_html)?;
  write_checked(&out_dir.join("404.html"), not_found_html)?;
  write_checked(&out_dir.join("CNAME"), CUSTOM_DOMAIN)?;
  write_checked(&out_dir.join(".nojekyll"), "")?;
  write_checked(&out_dir.join("robots.txt"), &render_robots_txt())?;
  write_checked(&out_dir.join("sitemap.xml"), &render_sitemap_xml(&released_at))?;
  write_checked(&out_dir.join("llms.txt"), &llms_txt)?;
  write_checked(&out_dir.join("llms-full.txt"), &llms_full_txt)?;
  copy_checked(&repo_root.join("src-tauri/src/cli/guide.md"), &out_dir.join("SKILL.md"))?;
  write_checked(&out_dir.join(format!("{INDEXNOW_KEY}.txt")), INDEXNOW_KEY)?;

  verify_html_invariants("index.html", &en_html, out_dir)?;
  verify_html_invariants("zh/index.html", &zh_html, out_dir)?;
  verify_html_invariants("docs/index.html", &en_docs_hub_html, out_dir)?;
  verify_html_invariants("zh/docs/index.html", &zh_docs_hub_html, out_dir)?;
  verify_html_invariants("404.html", not_found_html, out_dir)?;
  for (rel, html) in &generated_docs {
    verify_html_invariants(rel, html, out_dir)?;
  }

  Ok(version)
}

fn repo_root_from_manifest() -> Result<PathBuf, String> {
  let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  manifest_dir
    .parent()
    .map(Path::to_path_buf)
    .ok_or_else(|| format!("cannot find parent of {}", manifest_dir.display()))
}

fn run() -> Result<(), String> {
  let repo_root = repo_root_from_manifest()?;
  let out_dir =
    std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| repo_root.join("site/dist"));
  let version = build_site(&repo_root, &out_dir)?;
  println!("built dataomni.dev (v{version}) -> {}", out_dir.display());
  Ok(())
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      eprintln!("error: {err}");
      ExitCode::FAILURE
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::docs::CodeLang;
  use crate::render::{escape_html, highlight_code};

  #[test]
  fn parses_stable_release_version_and_rejects_prereleases_or_malformed_values() {
    let valid = "{\n  \"version\": \"1.1.0\"\n}\n";
    assert_eq!(extract_release_version(valid).unwrap(), "1.1.0");
    assert_eq!(parse_stable_version("v1.2.0").unwrap(), "1.2.0");

    let rc = "{\n  \"version\": \"1.2.0-rc.1\"\n}\n";
    assert!(extract_release_version(rc).is_err());
    assert!(parse_stable_version("v1.2.0-rc.1").is_err());

    let invalid = "{\n  \"version\": \"1.1; rm -rf /\"\n}\n";
    assert!(extract_release_version(invalid).is_err());
  }

  #[test]
  fn site_release_version_stays_decoupled_from_unreleased_tauri_conf_bumps() {
    let temp_root =
      std::env::temp_dir().join(format!("dataomni-site-version-{}", std::process::id()));
    write_checked(&temp_root.join("site/release.json"), "{\n  \"version\": \"1.1.0\"\n}\n")
      .unwrap();
    write_checked(&temp_root.join("src-tauri/tauri.conf.json"), "{\n  \"version\": \"9.9.9\"\n}\n")
      .unwrap();

    assert_eq!(resolve_release_version(&temp_root, None).unwrap(), "1.1.0");
    assert_eq!(resolve_release_version(&temp_root, Some("")).unwrap(), "1.1.0");
    assert_eq!(resolve_release_version(&temp_root, Some("v1.2.0")).unwrap(), "1.2.0");
    assert!(resolve_release_version(&temp_root, Some("v1.2.0-rc.1")).is_err());

    let _ = fs::remove_dir_all(&temp_root);
  }

  #[test]
  fn escapes_all_html_special_characters() {
    assert_eq!(
      escape_html("a < b && c > \"d\" 'e'"),
      "a &lt; b &amp;&amp; c &gt; &quot;d&quot; &#39;e&#39;"
    );
  }

  #[test]
  fn parses_iso_release_date_and_rejects_malformed_or_missing_values() {
    let valid = "{\n  \"version\": \"1.2.2\",\n  \"released_at\": \"2026-11-19\"\n}\n";
    assert_eq!(extract_released_at(valid).unwrap(), "2026-11-19");
    assert!(extract_released_at("{\n  \"version\": \"1.2.2\"\n}\n").is_err());
    assert!(parse_iso_date("2026/11/19").is_err());
    assert!(parse_iso_date("2026-13-01").is_err());
    assert!(parse_iso_date("2026-11-00").is_err());
    assert!(parse_iso_date("2024-12-31").is_err());
  }

  #[test]
  fn builds_complete_site_and_verifies_every_asset_and_cname() {
    let repo_root = repo_root_from_manifest().unwrap();
    let temp_dir = std::env::temp_dir().join(format!("dataomni-site-test-{}", std::process::id()));
    let version = build_site(&repo_root, &temp_dir).unwrap();
    let released_at = resolve_released_at(&repo_root, None).unwrap();

    assert_eq!(fs::read_to_string(temp_dir.join("CNAME")).unwrap(), "dataomni.dev\n");
    assert!(temp_dir.join(".nojekyll").is_file());
    assert!(temp_dir.join("robots.txt").is_file());
    assert!(temp_dir.join("sitemap.xml").is_file());
    assert!(temp_dir.join("llms.txt").is_file());
    assert!(temp_dir.join("llms-full.txt").is_file());
    assert!(temp_dir.join("SKILL.md").is_file());
    let skill_md = fs::read_to_string(temp_dir.join("SKILL.md")).unwrap();
    assert!(skill_md.starts_with("---\nname: dataomni\ndescription: \""));
    assert!(temp_dir.join(format!("{INDEXNOW_KEY}.txt")).is_file());
    assert!(temp_dir.join("docs/index.html").is_file());
    assert!(temp_dir.join("zh/docs/index.html").is_file());
    assert!(temp_dir.join("favicon.ico").is_file());
    assert!(temp_dir.join("assets/favicon-96.png").is_file());
    assert!(temp_dir.join("assets/og-en.png").is_file());
    assert!(temp_dir.join("assets/og-zh.png").is_file());

    let en = fs::read_to_string(temp_dir.join("index.html")).unwrap();
    let zh = fs::read_to_string(temp_dir.join("zh/index.html")).unwrap();
    let not_found = fs::read_to_string(temp_dir.join("404.html")).unwrap();
    let docs_hub_en = fs::read_to_string(temp_dir.join("docs/index.html")).unwrap();
    let docs_hub_zh = fs::read_to_string(temp_dir.join("zh/docs/index.html")).unwrap();
    let sitemap = fs::read_to_string(temp_dir.join("sitemap.xml")).unwrap();
    let llms = fs::read_to_string(temp_dir.join("llms.txt")).unwrap();
    let llms_full = fs::read_to_string(temp_dir.join("llms-full.txt")).unwrap();
    let indexnow = fs::read_to_string(temp_dir.join(format!("{INDEXNOW_KEY}.txt"))).unwrap();

    assert_eq!(indexnow, INDEXNOW_KEY);
    assert!(not_found.contains("<meta name=\"robots\" content=\"noindex\">"));
    assert!(docs_hub_en.contains("href=\"https://dataomni.dev/docs/quickstart/\""));
    assert!(docs_hub_zh.contains("href=\"https://dataomni.dev/zh/docs/quickstart/\""));
    assert!(sitemap.contains("xmlns:xhtml=\"http://www.w3.org/1999/xhtml\""));
    assert!(sitemap.contains("xmlns:image=\"http://www.google.com/schemas/sitemap-image/1.1\""));
    assert!(sitemap.contains(&format!("<lastmod>{released_at}</lastmod>")));
    assert!(!sitemap.contains("<lastmod>2026-11-17</lastmod>"));
    assert!(llms.contains(&format!("`v{version}` ({released_at})")));
    assert!(llms.contains("https://dataomni.dev/llms-full.txt"));
    assert!(llms.contains("https://dataomni.dev/SKILL.md"));
    assert!(llms_full.contains("# Read-only MCP server (`https://dataomni.dev/docs/mcp/`)"));
    assert!(llms_full.contains("brew install --cask dataomni-rs/tap/dataomni"));
    assert!(
      sitemap.contains("<image:loc>https://dataomni.dev/assets/shots/en/mongodb.webp</image:loc>")
    );
    assert!(
      sitemap.contains("<image:loc>https://dataomni.dev/assets/shots/zh/redis.webp</image:loc>")
    );

    assert!(en.contains(&format!("DataOmni_{version}_aarch64.dmg")));
    assert!(en.contains(&format!("DataOmni-{version}-1.x86_64.rpm")));
    assert!(en.contains("https://github.com/dataomni-rs/DataOmni"));
    assert!(en.contains("https://dataomni.dev/assets/og-en.png"));
    assert!(zh.contains("https://dataomni.dev/assets/og-zh.png"));
    assert!(!en.contains("yuxuetr"));
    assert!(!zh.contains("yuxuetr"));

    // Multi-tool MCP presets, static syntax tokens, structured data, and upgraded cards/tables
    for html in [&en, &zh] {
      assert!(html.contains("sizes=\"96x96\" href=\"/assets/favicon-96.png\""));
      assert!(html.contains("\"@type\":\"Organization\""));
      assert!(html.contains("\"@type\":\"WebSite\""));
      assert!(html.contains("\"@type\":\"SoftwareApplication\""));
      assert!(html.contains("\"@type\":\"FAQPage\""));
      assert!(html.contains("\"@type\":\"ImageObject\""));
      assert!(html.contains("max-image-preview:large"));
      assert!(html.contains("href=\"/llms.txt\""));
      assert!(html.contains("og:image:width\" content=\"1200\""));
      assert!(html.contains("og:image:height\" content=\"630\""));
      assert!(html.contains("<noscript><img class=\"workbench-shot\""));
      assert!(html.contains("class=\"trust-icon\""));
      assert!(html.contains("class=\"tok-kw\""));
      assert!(html.contains("class=\"tok-err\""));
      assert!(html.contains("class=\"smoke-triplet\""));
      assert!(html.contains("codex mcp add dataomni"));
      assert!(html.contains("gemini mcp add dataomni"));
      assert!(html.contains("mcpServers"));
    }

    // Reverse check: stripping max-image-preview:large or adding noindex to indexed page must fail
    let missing_robots =
      en.replace(&format!("<meta name=\"robots\" content=\"{ROBOTS_INDEX_DIRECTIVE}\">"), "");
    assert!(verify_html_invariants("index.html", &missing_robots, &temp_dir).is_err());
    let accidental_noindex = en.replace(ROBOTS_INDEX_DIRECTIVE, "noindex, follow");
    assert!(verify_html_invariants("index.html", &accidental_noindex, &temp_dir).is_err());
    let broken_llms_ref = en.replace("href=\"/llms.txt\"", "href=\"/llms-missing.txt\"");
    assert!(verify_html_invariants("index.html", &broken_llms_ref, &temp_dir).is_err());

    // Reverse check: referencing a missing image, WebP source, or OG card must fail verify_html_invariants
    let broken = en.replace("/images/en/overview.png", "/images/en/missing.png");
    assert!(verify_html_invariants("index.html", &broken, &temp_dir).is_err());
    let broken_webp = en.replace("/assets/shots/en/overview.webp", "/assets/shots/en/missing.webp");
    assert!(verify_html_invariants("index.html", &broken_webp, &temp_dir).is_err());
    let broken_og = en.replace("/assets/og-en.png", "/assets/og-missing.png");
    assert!(verify_html_invariants("index.html", &broken_og, &temp_dir).is_err());

    // Reverse check: SERP-truncated <title> (> 65 chars) or meta description (> 165 chars) must fail
    let bloated_title = en.replace(
      "<title>",
      "<title>Bloated Title Padding Exceeding Sixty Five Characters In Google Search Results ",
    );
    assert!(verify_html_invariants("index.html", &bloated_title, &temp_dir).is_err());
    let bloated_desc = en.replace(
      "<meta name=\"description\" content=\"",
      "<meta name=\"description\" content=\"Bloated description padding that deliberately pushes total character count well past the one hundred and sixty five character SERP snippet budget ",
    );
    assert!(verify_html_invariants("index.html", &bloated_desc, &temp_dir).is_err());

    // Reverse check: combining fetchpriority="high" with loading="lazy" must fail
    let lazy_lcp = en.replace("fetchpriority=\"high\"", "fetchpriority=\"high\" loading=\"lazy\"");
    assert!(verify_html_invariants("index.html", &lazy_lcp, &temp_dir).is_err());

    // Reverse check: broken in-page anchor href="#..." must fail
    let broken_anchor = en.replace("href=\"#faq\"", "href=\"#missing-section\"");
    assert!(verify_html_invariants("index.html", &broken_anchor, &temp_dir).is_err());

    // Reverse check: nesting a heading inside <summary> must fail
    let heading_in_summary =
      en.replace("<summary class=\"faq-summary\">", "<summary class=\"faq-summary\"><h3>");
    assert!(verify_html_invariants("index.html", &heading_in_summary, &temp_dir).is_err());

    let _ = fs::remove_dir_all(&temp_dir);
  }

  #[test]
  fn builds_bilingual_docs_and_changelog_pages_and_verifies_cross_links() {
    let repo_root = repo_root_from_manifest().unwrap();
    let temp_dir =
      std::env::temp_dir().join(format!("dataomni-site-docs-test-{}", std::process::id()));
    let _version = build_site(&repo_root, &temp_dir).unwrap();

    let expected_pages = [
      ("docs/quickstart/index.html", "/zh/docs/quickstart/"),
      ("zh/docs/quickstart/index.html", "/docs/quickstart/"),
      ("docs/workbench/index.html", "/zh/docs/workbench/"),
      ("zh/docs/workbench/index.html", "/docs/workbench/"),
      ("docs/mcp/index.html", "/zh/docs/mcp/"),
      ("zh/docs/mcp/index.html", "/docs/mcp/"),
      ("docs/cli/index.html", "/zh/docs/cli/"),
      ("zh/docs/cli/index.html", "/docs/cli/"),
      ("docs/databases/index.html", "/zh/docs/databases/"),
      ("zh/docs/databases/index.html", "/docs/databases/"),
      ("changelog/index.html", "/zh/changelog/"),
      ("zh/changelog/index.html", "/changelog/"),
      ("privacy/index.html", "/zh/privacy/"),
      ("zh/privacy/index.html", "/privacy/"),
    ];

    let sitemap = fs::read_to_string(temp_dir.join("sitemap.xml")).unwrap();
    for (rel_html, alternate_href) in expected_pages {
      let path = temp_dir.join(rel_html);
      assert!(path.is_file(), "missing generated page {rel_html}");
      let html = fs::read_to_string(&path).unwrap();
      assert!(html.contains("\"@type\":\"Organization\""), "{rel_html}: missing Organization");
      assert!(
        html.contains("\"@type\":\"SoftwareApplication\""),
        "{rel_html}: missing SoftwareApplication anchor"
      );
      assert!(html.contains("\"@type\":\"BreadcrumbList\""), "{rel_html}: missing BreadcrumbList");
      if rel_html.contains("privacy/") {
        assert!(html.contains("\"@type\":\"WebPage\""), "{rel_html}: privacy should use WebPage");
      } else {
        assert!(html.contains("\"@type\":\"TechArticle\""), "{rel_html}: missing TechArticle");
      }
      assert!(html.contains("aria-current=\"page\""), "{rel_html}: missing aria-current");
      assert!(
        html.contains(&format!("href=\"{alternate_href}\"")),
        "{rel_html}: missing exact locale switch {alternate_href}"
      );
      assert!(!html.contains("yuxuetr"), "{rel_html}: stale owner reference");
    }

    for url in [
      "https://dataomni.dev/docs/quickstart/",
      "https://dataomni.dev/zh/docs/quickstart/",
      "https://dataomni.dev/docs/workbench/",
      "https://dataomni.dev/zh/docs/workbench/",
      "https://dataomni.dev/docs/mcp/",
      "https://dataomni.dev/zh/docs/mcp/",
      "https://dataomni.dev/docs/cli/",
      "https://dataomni.dev/zh/docs/cli/",
      "https://dataomni.dev/docs/databases/",
      "https://dataomni.dev/zh/docs/databases/",
      "https://dataomni.dev/changelog/",
      "https://dataomni.dev/zh/changelog/",
      "https://dataomni.dev/privacy/",
      "https://dataomni.dev/zh/privacy/",
    ] {
      assert!(sitemap.contains(&format!("<loc>{url}</loc>")), "sitemap missing {url}");
    }

    let en_home = fs::read_to_string(temp_dir.join("index.html")).unwrap();
    let zh_home = fs::read_to_string(temp_dir.join("zh/index.html")).unwrap();
    assert!(en_home.contains("href=\"/privacy/\""), "EN footer missing /privacy/");
    assert!(zh_home.contains("href=\"/zh/privacy/\""), "ZH footer missing /zh/privacy/");

    let privacy_en = fs::read_to_string(temp_dir.join("privacy/index.html")).unwrap();
    let privacy_zh = fs::read_to_string(temp_dir.join("zh/privacy/index.html")).unwrap();
    assert!(privacy_en.contains("--no-default-features"));
    assert!(privacy_zh.contains("--no-default-features"));

    // Reverse check: broken internal route link or cross-page anchor must fail verify_html_invariants
    let mcp_en = fs::read_to_string(temp_dir.join("docs/mcp/index.html")).unwrap();
    let broken_route = mcp_en.replace("href=\"/docs/cli/\"", "href=\"/docs/nonexistent/\"");
    assert!(verify_html_invariants("docs/mcp/index.html", &broken_route, &temp_dir).is_err());
    let broken_cross_anchor = mcp_en.replace("href=\"/#install\"", "href=\"/#nonexistent-anchor\"");
    assert!(verify_html_invariants("docs/mcp/index.html", &broken_cross_anchor, &temp_dir).is_err());

    // Multi-platform command tabs, syntax highlighting, and Windows/Linux MCP presets
    let version = resolve_release_version(&repo_root, None).unwrap();
    for rel in [
      "docs/quickstart/index.html",
      "zh/docs/quickstart/index.html",
      "docs/mcp/index.html",
      "zh/docs/mcp/index.html",
      "docs/cli/index.html",
      "zh/docs/cli/index.html",
    ] {
      let html = fs::read_to_string(temp_dir.join(rel)).unwrap();
      assert!(!html.contains("{VERSION}"), "{rel}: unexpanded {{VERSION}} placeholder");
      assert!(html.contains("class=\"tok-cmd\""), "{rel}: missing shell command highlighting");
    }

    let qs_en = fs::read_to_string(temp_dir.join("docs/quickstart/index.html")).unwrap();
    assert!(qs_en.contains("brew install --cask dataomni-rs/tap/dataomni"));
    assert!(qs_en.contains(&format!("DataOmni_{version}_x64-setup.exe")));
    assert!(qs_en.contains(&format!("DataOmni_{version}_amd64.deb")));
    assert!(qs_en.contains(&format!("DataOmni-{version}-1.x86_64.rpm")));

    let mcp_zh = fs::read_to_string(temp_dir.join("zh/docs/mcp/index.html")).unwrap();
    for html in [&mcp_en, &mcp_zh] {
      assert!(html.contains("data-mcp-os-btn=\"macos\""));
      assert!(html.contains("data-mcp-os-btn=\"windows\""));
      assert!(html.contains("data-mcp-os-btn=\"linux\""));
      assert!(html.contains("$env:LOCALAPPDATA\\DataOmni\\dataomni.exe"));
      assert!(html.contains("/usr/bin/dataomni"));
      assert!(html.contains("class=\"tok-prop\""));
    }

    let cli_en = fs::read_to_string(temp_dir.join("docs/cli/index.html")).unwrap();
    let cli_zh = fs::read_to_string(temp_dir.join("zh/docs/cli/index.html")).unwrap();
    for html in [&cli_en, &cli_zh] {
      assert!(html.contains("Set-Content"));
      assert!(html.contains("ConvertFrom-Json"));
      assert!(html.contains("which</span> dataomni"));
      assert!(html.contains("class=\"tok-attr\""));
      assert!(html.contains("class=\"tok-str\""));
    }

    let llms_full = fs::read_to_string(temp_dir.join("llms-full.txt")).unwrap();
    assert!(!llms_full.contains("{VERSION}"));
    assert!(llms_full.contains("```powershell"));
    assert!(llms_full.contains("/usr/bin/dataomni mcp"));

    let _ = fs::remove_dir_all(&temp_dir);
  }

  #[test]
  fn highlights_shell_powershell_and_json_safely() {
    let sh = highlight_code(
      "# comment <script>\ndataomni query shop_dev \"SELECT * FROM t WHERE a < 1\" \\\n  --param cid=1042 --limit 50 | jq '.len'",
      CodeLang::Shell,
    );
    assert!(sh.contains("<span class=\"code-comment\"># comment &lt;script&gt;</span>"));
    assert!(
      sh.contains("<span class=\"tok-cmd\">dataomni</span> <span class=\"tok-type\">query</span>")
    );
    assert!(
      sh.contains("<span class=\"tok-str\">&quot;SELECT * FROM t WHERE a &lt; 1&quot;</span>")
    );
    assert!(sh.contains("<span class=\"tok-kw\">\\</span>"));
    assert!(sh.contains("<span class=\"tok-attr\">--param</span> <span class=\"tok-prop\">cid</span>=<span class=\"tok-num\">1042</span>"));
    assert!(sh.contains("<span class=\"tok-kw\">|</span> <span class=\"tok-cmd\">jq</span>"));

    let ps = highlight_code(
      "Get-Content -Raw report.sql | dataomni export shop_dev - --out \"$env:TEMP\\a.parquet\" `\n  --format parquet",
      CodeLang::PowerShell,
    );
    assert!(ps
      .contains("<span class=\"tok-cmd\">Get-Content</span> <span class=\"tok-attr\">-Raw</span>"));
    assert!(ps.contains("<span class=\"tok-kw\">`</span>"));
    assert!(ps.contains("<span class=\"tok-attr\">--format</span> parquet"));

    let json = highlight_code(
      "{\"mcpServers\":{\"dataomni\":{\"command\":\"/usr/bin/dataomni\",\"args\":[\"mcp\"]}}}",
      CodeLang::Json,
    );
    assert!(json.contains("<span class=\"tok-prop\">&quot;mcpServers&quot;</span>"));
    assert!(json.contains("<span class=\"tok-str\">&quot;/usr/bin/dataomni&quot;</span>"));
  }
}
