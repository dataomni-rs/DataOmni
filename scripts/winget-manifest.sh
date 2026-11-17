#!/usr/bin/env bash
# 为一个已发布的版本生成 winget 清单（TODOs「1.x」U2）。
#
#   scripts/winget-manifest.sh 1.1.0 [--msi /path/to/DataOmni_1.1.0_x64_en-US.msi]
#
# 写到 packaging/winget/manifests/d/DataOmni/DataOmni/<版本>/，目录结构与 microsoft/winget-pkgs 相同，
# 整个目录拷进那个仓库的同一路径就能提 PR。
#
# 安装包的 SHA-256 取 GitHub 发布页给的 digest（服务端算的）；同时校验并拆开 .msi 的 OLE Property 表，
# 取出当次构建生成的 ProductCode、UpgradeCode 与 Manufacturer（winget-pkgs 对 wix 安装包强制校验 ProductCode，
# 且 Windows ARP 注册表里的 Publisher 来自 Tauri identifier 中间段 dataomni）。
# 两种安装包都列上：NSIS 按用户装（%LOCALAPPDATA%\DataOmni），MSI 按机器装（C:\Program Files\DataOmni），见 R5。
set -euo pipefail

version="${1:-}"
shift || true
local_msi=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --msi)
      local_msi="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 <version> [--msi <file>]   (a published release such as 1.1.0; prereleases do not go to winget)" >&2
  exit 2
fi

for cmd in gh 7z python3 shasum; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "missing required tool: $cmd" >&2
    exit 1
  fi
done

repo="dataomni-rs/DataOmni"
tag="v$version"
id="DataOmni.DataOmni"
manifest_version="1.10.0"
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/packaging/winget/manifests/d/DataOmni/DataOmni/$version"

exe="DataOmni_${version}_x64-setup.exe"
msi="DataOmni_${version}_x64_en-US.msi"

digest_of() {
  local name="$1" digest
  digest="$(gh release view "$tag" --repo "$repo" --json assets \
    -q ".assets[] | select(.name == \"$name\") | .digest")"
  if [[ "$digest" != sha256:* ]]; then
    echo "no sha256 digest for $name in release $tag" >&2
    exit 1
  fi
  echo "${digest#sha256:}" | tr '[:lower:]' '[:upper:]'
}

if [[ "$(gh release view "$tag" --repo "$repo" --json isDraft,isPrerelease -q '.isDraft or .isPrerelease')" != "false" ]]; then
  echo "$tag is a draft or a prerelease; winget only takes published releases" >&2
  exit 1
fi
release_date="$(gh release view "$tag" --repo "$repo" --json publishedAt -q '.publishedAt[:10]')"
exe_sha="$(digest_of "$exe")"
msi_sha="$(digest_of "$msi")"

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

if [[ -n "$local_msi" ]]; then
  msi_file="$local_msi"
else
  gh release download "$tag" --repo "$repo" --pattern "$msi" --dir "$tmpdir"
  msi_file="$tmpdir/$msi"
fi

actual_msi_sha="$(shasum -a 256 "$msi_file" | awk '{print $1}' | tr '[:lower:]' '[:upper:]')"
if [[ "$actual_msi_sha" != "$msi_sha" ]]; then
  echo "sha256 mismatch for $msi_file: expected $msi_sha, got $actual_msi_sha" >&2
  exit 1
fi

7z e -tCompound -y -o"$tmpdir/msi_meta" "$msi_file" '!_StringData' '!_StringPool' '!Property' >/dev/null

msi_props="$(python3 - "$tmpdir/msi_meta" "$version" <<'PY'
import re
import struct
import sys

meta_dir, expected_version = sys.argv[1], sys.argv[2]
pool = open(f"{meta_dir}/!_StringPool", "rb").read()
data = open(f"{meta_dir}/!_StringData", "rb").read()
prop = open(f"{meta_dir}/!Property", "rb").read()

_, flags = struct.unpack_from("<HH", pool, 0)
long_ref = bool(flags & 0x8000)

strings = [""]
pos = 0
idx = 4
while idx < len(pool):
  length, refcount = struct.unpack_from("<HH", pool, idx)
  idx += 4
  if length == 0 and refcount > 0:
    length, = struct.unpack_from("<I", pool, idx)
    idx += 4
  strings.append(data[pos:pos + length].decode("utf-8"))
  pos += length

stride = 3 if long_ref else 2
nrows = len(prop) // (2 * stride)

def read_idx(offset: int) -> int:
  if stride == 2:
    return struct.unpack_from("<H", prop, offset)[0]
  b0, b1, b2 = prop[offset:offset + 3]
  return b0 | (b1 << 8) | (b2 << 16)

props = {}
for row in range(nrows):
  k = strings[read_idx(row * stride)]
  v = strings[read_idx((nrows + row) * stride)]
  props[k] = v

guid_re = re.compile(r"^\{[0-9A-F]{8}-(?:[0-9A-F]{4}-){3}[0-9A-F]{12}\}$")
product_code = props.get("ProductCode", "")
upgrade_code = props.get("UpgradeCode", "")
product_version = props.get("ProductVersion", "")
manufacturer = props.get("Manufacturer", "")

if not guid_re.match(product_code):
  raise SystemExit(f"invalid MSI ProductCode: {product_code!r}")
if not guid_re.match(upgrade_code):
  raise SystemExit(f"invalid MSI UpgradeCode: {upgrade_code!r}")
if product_version != expected_version:
  raise SystemExit(f"MSI ProductVersion {product_version!r} != {expected_version!r}")
if not manufacturer:
  raise SystemExit("missing MSI Manufacturer")

print(f"{product_code}\t{upgrade_code}\t{manufacturer}")
PY
)"

IFS=$'\t' read -r msi_product_code msi_upgrade_code msi_manufacturer <<<"$msi_props"

mkdir -p "$out"
download="https://github.com/$repo/releases/download/$tag"
header="# Created with scripts/winget-manifest.sh"

cat > "$out/$id.yaml" <<EOF
$header
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.$manifest_version.schema.json

PackageIdentifier: $id
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $manifest_version
EOF

cat > "$out/$id.installer.yaml" <<EOF
$header
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.$manifest_version.schema.json

PackageIdentifier: $id
PackageVersion: $version
ReleaseDate: $release_date
UpgradeBehavior: install
Installers:
- Architecture: x64
  InstallerType: nullsoft
  Scope: user
  InstallerUrl: $download/$exe
  InstallerSha256: $exe_sha
  ProductCode: DataOmni
  AppsAndFeaturesEntries:
  - Publisher: $msi_manufacturer
  InstallationMetadata:
    DefaultInstallLocation: '%LOCALAPPDATA%\DataOmni'
- Architecture: x64
  InstallerType: wix
  Scope: machine
  InstallerLocale: en-US
  InstallerUrl: $download/$msi
  InstallerSha256: $msi_sha
  ProductCode: '$msi_product_code'
  AppsAndFeaturesEntries:
  - Publisher: $msi_manufacturer
    UpgradeCode: '$msi_upgrade_code'
  InstallationMetadata:
    DefaultInstallLocation: '%ProgramFiles%\DataOmni'
ManifestType: installer
ManifestVersion: $manifest_version
EOF

cat > "$out/$id.locale.en-US.yaml" <<EOF
$header
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.$manifest_version.schema.json

PackageIdentifier: $id
PackageVersion: $version
PackageLocale: en-US
Publisher: DataOmni
PublisherUrl: https://dataomni.dev
PublisherSupportUrl: https://github.com/$repo/issues
PackageName: DataOmni
PackageUrl: https://dataomni.dev
License: MIT
LicenseUrl: https://github.com/$repo/blob/main/LICENSE
ShortDescription: Native desktop client for SQL, MongoDB, Redis, Neo4j and Elasticsearch
Description: |-
  Browse schemas, edit table data with previewed SQL guardrails, run queries, diff schemas, generate ORM
  models, and export results for MySQL, PostgreSQL, SQLite, SQL Server, Oracle, DuckDB and ClickHouse,
  with dedicated workspaces for MongoDB, Redis, Neo4j and Elasticsearch plus a read-only CLI and MCP server.
  Passwords are kept in the system keychain, never in plain text on disk.
Moniker: dataomni
Tags:
- clickhouse
- database
- database-client
- duckdb
- elasticsearch
- mcp
- mongodb
- mysql
- neo4j
- oracle
- postgresql
- redis
- sql
- sql-server
- sqlite
ReleaseNotesUrl: https://github.com/$repo/releases/tag/$tag
ManifestType: defaultLocale
ManifestVersion: $manifest_version
EOF

echo "wrote $out (ProductCode=$msi_product_code UpgradeCode=$msi_upgrade_code)"
