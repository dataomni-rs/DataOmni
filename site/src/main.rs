mod content;
mod render;

use content::{EN_COPY, ZH_COPY};
use render::{render_not_found, render_page, render_robots_txt, render_sitemap_xml};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const CUSTOM_DOMAIN: &str = "dataomni.dev\n";

fn extract_tauri_version(config_text: &str) -> Result<String, String> {
  for line in config_text.lines() {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix("\"version\"") else {
      continue;
    };
    let Some(after_colon) = rest.trim_start().strip_prefix(':') else {
      continue;
    };
    let candidate = after_colon.trim().trim_end_matches(',').trim();
    let Some(unquoted) = candidate.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
      continue;
    };
    if is_semver_like(unquoted) {
      return Ok(unquoted.to_string());
    }
    return Err(format!("invalid version string in tauri.conf.json: {unquoted}"));
  }
  Err("missing \"version\" field in src-tauri/tauri.conf.json".to_string())
}

fn is_semver_like(version: &str) -> bool {
  let base = version.split_once('-').map_or(version, |(head, _)| head);
  let mut parts = base.split('.');
  let (Some(major), Some(minor), Some(patch), None) =
    (parts.next(), parts.next(), parts.next(), parts.next())
  else {
    return false;
  };
  let digits = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
  digits(major)
    && digits(minor)
    && digits(patch)
    && version.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
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
  if page_name != "404.html" && high_priority_count != 1 {
    return Err(format!(
      "{page_name}: expected exactly 1 fetchpriority=\"high\" LCP image, found {high_priority_count}"
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

  for attr in [
    "src=\"/",
    "srcset=\"/",
    "data-deferred-src=\"/",
    "data-deferred-srcset=\"/",
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

  Ok(())
}

pub fn build_site(repo_root: &Path, out_dir: &Path) -> Result<String, String> {
  let tauri_conf_path = repo_root.join("src-tauri/tauri.conf.json");
  let tauri_conf = fs::read_to_string(&tauri_conf_path)
    .map_err(|err| format!("failed to read {}: {err}", tauri_conf_path.display()))?;
  let version = extract_tauri_version(&tauri_conf)?;

  if out_dir.exists() {
    fs::remove_dir_all(out_dir)
      .map_err(|err| format!("failed to clean {}: {err}", out_dir.display()))?;
  }
  fs::create_dir_all(out_dir)
    .map_err(|err| format!("failed to create {}: {err}", out_dir.display()))?;

  // Static styles, progressive-enhancement script, and app icons
  let site_dir = repo_root.join("site");
  copy_checked(&site_dir.join("assets/style.css"), &out_dir.join("assets/style.css"))?;
  copy_checked(&site_dir.join("assets/site.js"), &out_dir.join("assets/site.js"))?;
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

  let en_html =
    render_page(&EN_COPY, &version).map_err(|err| format!("render EN failed: {err}"))?;
  let zh_html =
    render_page(&ZH_COPY, &version).map_err(|err| format!("render ZH failed: {err}"))?;
  let not_found_html = render_not_found();

  write_checked(&out_dir.join("index.html"), &en_html)?;
  write_checked(&out_dir.join("zh/index.html"), &zh_html)?;
  write_checked(&out_dir.join("404.html"), not_found_html)?;
  write_checked(&out_dir.join("CNAME"), CUSTOM_DOMAIN)?;
  write_checked(&out_dir.join(".nojekyll"), "")?;
  write_checked(&out_dir.join("robots.txt"), &render_robots_txt())?;
  write_checked(&out_dir.join("sitemap.xml"), &render_sitemap_xml())?;

  verify_html_invariants("index.html", &en_html, out_dir)?;
  verify_html_invariants("zh/index.html", &zh_html, out_dir)?;
  verify_html_invariants("404.html", not_found_html, out_dir)?;

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
  use crate::render::escape_html;

  #[test]
  fn parses_tauri_version_and_rejects_malformed_values() {
    let valid = "{\n  \"productName\": \"DataOmni\",\n  \"version\": \"1.1.0\",\n}\n";
    assert_eq!(extract_tauri_version(valid).unwrap(), "1.1.0");

    let rc = "{\n  \"version\": \"1.2.0-rc.1\"\n}\n";
    assert_eq!(extract_tauri_version(rc).unwrap(), "1.2.0-rc.1");

    let invalid = "{\n  \"version\": \"1.1; rm -rf /\"\n}\n";
    assert!(extract_tauri_version(invalid).is_err());
  }

  #[test]
  fn escapes_all_html_special_characters() {
    assert_eq!(
      escape_html("a < b && c > \"d\" 'e'"),
      "a &lt; b &amp;&amp; c &gt; &quot;d&quot; &#39;e&#39;"
    );
  }

  #[test]
  fn builds_complete_site_and_verifies_every_asset_and_cname() {
    let repo_root = repo_root_from_manifest().unwrap();
    let temp_dir = std::env::temp_dir().join(format!("dataomni-site-test-{}", std::process::id()));
    let version = build_site(&repo_root, &temp_dir).unwrap();

    assert_eq!(fs::read_to_string(temp_dir.join("CNAME")).unwrap(), "dataomni.dev\n");
    assert!(temp_dir.join(".nojekyll").is_file());
    assert!(temp_dir.join("robots.txt").is_file());
    assert!(temp_dir.join("sitemap.xml").is_file());

    let en = fs::read_to_string(temp_dir.join("index.html")).unwrap();
    let zh = fs::read_to_string(temp_dir.join("zh/index.html")).unwrap();
    assert!(en.contains(&format!("DataOmni_{version}_aarch64.dmg")));
    assert!(en.contains(&format!("DataOmni-{version}-1.x86_64.rpm")));
    assert!(en.contains("https://github.com/dataomni-rs/DataOmni"));
    assert!(!en.contains("yuxuetr"));
    assert!(!zh.contains("yuxuetr"));

    // Multi-tool MCP presets, static syntax tokens, and upgraded cards/tables
    for html in [&en, &zh] {
      assert!(html.contains("class=\"trust-icon\""));
      assert!(html.contains("class=\"tok-kw\""));
      assert!(html.contains("class=\"tok-err\""));
      assert!(html.contains("class=\"smoke-triplet\""));
      assert!(html.contains("codex mcp add dataomni"));
      assert!(html.contains("gemini mcp add dataomni"));
      assert!(html.contains("mcpServers"));
    }

    // Reverse check: referencing a missing image or WebP source must fail verify_html_invariants
    let broken = en.replace("/images/en/overview.png", "/images/en/missing.png");
    assert!(verify_html_invariants("index.html", &broken, &temp_dir).is_err());
    let broken_webp = en.replace("/assets/shots/en/overview.webp", "/assets/shots/en/missing.webp");
    assert!(verify_html_invariants("index.html", &broken_webp, &temp_dir).is_err());

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
}
