#!/usr/bin/env python3
"""Assemble Tauri v2 static update manifest (`latest.json`) from signed release bundles."""

from __future__ import annotations

import argparse
import base64
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import sys
import urllib.parse

SEMVER_STABLE_RE = re.compile(r"^\d+\.\d+\.\d+$")


def read_verified_signature(sig_path: Path) -> str:
  if not sig_path.is_file():
    raise ValueError(f"Missing signature file: {sig_path}")
  encoded = sig_path.read_text(encoding="utf-8").strip()
  if not encoded:
    raise ValueError(f"Empty signature file: {sig_path}")
  try:
    decoded = base64.b64decode(encoded, validate=True).decode("utf-8")
  except Exception as exc:
    raise ValueError(f"Invalid base64 Minisign signature in {sig_path}: {exc}") from exc
  if not decoded.startswith("untrusted comment: signature from "):
    raise ValueError(f"Unexpected Minisign header in {sig_path}")
  return encoded


def find_single(files: list[Path], label: str) -> Path | None:
  if not files:
    return None
  if len(files) > 1:
    joined = ", ".join(str(path) for path in files)
    raise ValueError(f"Expected at most 1 {label}, found {len(files)}: {joined}")
  return files[0]


def asset_url(repo: str, tag: str, filename: str) -> str:
  quoted = urllib.parse.quote(filename)
  return f"https://github.com/{repo}/releases/download/{tag}/{quoted}"


def build_manifest(
  *,
  tag: str,
  repo: str,
  packages_dir: Path,
  notes: str | None,
  pub_date: str,
  allow_partial: bool = False,
) -> dict[str, object]:
  if not tag.startswith("v"):
    raise ValueError(f"Release tag must start with 'v': {tag}")
  version = tag[1:]
  if not SEMVER_STABLE_RE.match(version):
    raise ValueError(
      f"Refusing to build latest.json for non-stable version '{version}' (tag '{tag}')"
    )

  all_files = sorted(
    path for path in packages_dir.rglob("*") if path.is_file() and not path.name.endswith(".sig")
  )
  macos_tar = find_single(
    [path for path in all_files if path.name.endswith(".app.tar.gz")],
    "macOS updater archive (*.app.tar.gz)",
  )
  windows_nsis = find_single(
    [path for path in all_files if path.name.endswith("-setup.exe")],
    "Windows NSIS installer (*-setup.exe)",
  )
  windows_msi = find_single(
    [path for path in all_files if path.name.endswith(".msi")],
    "Windows MSI installer (*.msi)",
  )

  platforms: dict[str, dict[str, str]] = {}

  if macos_tar is not None:
    sig = read_verified_signature(Path(f"{macos_tar}.sig"))
    entry = {"signature": sig, "url": asset_url(repo, tag, macos_tar.name)}
    platforms["darwin-aarch64"] = entry
    platforms["darwin-aarch64-app"] = dict(entry)

  if windows_nsis is not None:
    sig = read_verified_signature(Path(f"{windows_nsis}.sig"))
    entry = {"signature": sig, "url": asset_url(repo, tag, windows_nsis.name)}
    platforms["windows-x86_64-nsis"] = entry
    platforms["windows-x86_64"] = dict(entry)

  if windows_msi is not None:
    sig = read_verified_signature(Path(f"{windows_msi}.sig"))
    entry = {"signature": sig, "url": asset_url(repo, tag, windows_msi.name)}
    platforms["windows-x86_64-msi"] = entry
    if "windows-x86_64" not in platforms:
      platforms["windows-x86_64"] = dict(entry)

  if not allow_partial:
    missing = [
      target for target in ("darwin-aarch64", "windows-x86_64") if target not in platforms
    ]
    if missing:
      raise ValueError(f"Missing required signed updater targets: {', '.join(missing)}")
  elif not platforms:
    raise ValueError("No signed updater artifacts found")

  manifest: dict[str, object] = {
    "version": version,
    "pub_date": pub_date,
    "platforms": platforms,
  }
  if notes is not None and notes.strip():
    manifest["notes"] = notes.strip()
  return manifest


def parse_args(argv: list[str]) -> argparse.Namespace:
  parser = argparse.ArgumentParser(description=__doc__)
  parser.add_argument("--tag", required=True, help="Git release tag (e.g. v1.2.2)")
  parser.add_argument("--repo", required=True, help="GitHub owner/repo (e.g. dataomni-rs/DataOmni)")
  parser.add_argument("--packages-dir", required=True, type=Path, help="Directory with bundles")
  parser.add_argument("--notes-file", type=Path, help="Optional release notes markdown file")
  parser.add_argument("--pub-date", help="Optional RFC3339 UTC timestamp")
  parser.add_argument("--allow-partial", action="store_true", help="Allow single-OS manifest")
  parser.add_argument("--output", required=True, type=Path, help="Output latest.json path")
  return parser.parse_args(argv)


def main(argv: list[str]) -> int:
  args = parse_args(argv)
  notes = args.notes_file.read_text(encoding="utf-8") if args.notes_file else None
  pub_date = args.pub_date or datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
  try:
    manifest = build_manifest(
      tag=args.tag,
      repo=args.repo,
      packages_dir=args.packages_dir,
      notes=notes,
      pub_date=pub_date,
      allow_partial=args.allow_partial,
    )
  except ValueError as exc:
    print(f"build-latest-json error: {exc}", file=sys.stderr)
    return 1

  args.output.parent.mkdir(parents=True, exist_ok=True)
  args.output.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
  return 0


if __name__ == "__main__":
  raise SystemExit(main(sys.argv[1:]))
