#!/usr/bin/env bash
# 为一个已发布且已通过 Apple 公证的版本生成 Homebrew Cask（TODOs S4 / U2）。
#
#   scripts/homebrew-cask.sh 1.2.1 [--dmg /path/to/DataOmni_1.2.1_aarch64.dmg] [--out /path/to/Casks/dataomni.rb]
#
# 默认写到 packaging/homebrew/Casks/dataomni.rb，目录结构与 dataomni-rs/homebrew-tap 及
# Homebrew/homebrew-cask 完全一致：直接拷进 tap 仓库的 Casks/dataomni.rb 即可供
# `brew install --cask dataomni-rs/tap/dataomni` 安装，应用装进 /Applications/DataOmni.app，
# 同时把内置命令行软链到 $(brew --prefix)/bin/dataomni。
#
# 安全与质量门：
# 1. 拒绝草稿与预发布版本；
# 2. 取 GitHub Releases 服务端计算的 SHA-256 digest，下载（或使用 --dmg 指定的本地文件）逐字节复核；
# 3. 在 macOS 上强制跑 `xcrun stapler validate` 与 `spctl` 公证校验——未签名或未钉合公证票据的
#    .dmg（如 v1.2.0 及更早版本）直接报错退出，防止把未公证包推进 Homebrew Tap。
set -euo pipefail

version="${1:-}"
shift || true
local_dmg=""
out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dmg)
      local_dmg="${2:-}"
      shift 2
      ;;
    --out)
      out="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 <version> [--dmg <file>] [--out <file>]   (published notarized release such as 1.2.1)" >&2
  exit 2
fi

for cmd in gh shasum; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "missing required tool: $cmd" >&2
    exit 1
  fi
done

repo="dataomni-rs/DataOmni"
tag="v$version"
root="$(cd "$(dirname "$0")/.." && pwd)"
out="${out:-$root/packaging/homebrew/Casks/dataomni.rb}"
dmg="DataOmni_${version}_aarch64.dmg"

if [[ "$(gh release view "$tag" --repo "$repo" --json isDraft,isPrerelease -q '.isDraft or .isPrerelease')" != "false" ]]; then
  echo "$tag is a draft or a prerelease; Homebrew cask only takes published stable releases" >&2
  exit 1
fi

digest="$(gh release view "$tag" --repo "$repo" --json assets \
  -q ".assets[] | select(.name == \"$dmg\") | .digest")"
if [[ "$digest" != sha256:* ]]; then
  echo "no sha256 digest for $dmg in release $tag" >&2
  exit 1
fi
expected_sha="$(echo "${digest#sha256:}" | tr '[:upper:]' '[:lower:]')"

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

if [[ -n "$local_dmg" ]]; then
  dmg_file="$local_dmg"
else
  gh release download "$tag" --repo "$repo" --pattern "$dmg" --dir "$tmpdir"
  dmg_file="$tmpdir/$dmg"
fi

actual_sha="$(shasum -a 256 "$dmg_file" | awk '{print $1}' | tr '[:upper:]' '[:lower:]')"
if [[ "$actual_sha" != "$expected_sha" ]]; then
  echo "sha256 mismatch for $dmg_file: expected $expected_sha, got $actual_sha" >&2
  exit 1
fi

if [[ "$(uname -s)" == "Darwin" ]]; then
  xcrun stapler validate "$dmg_file" >/dev/null
  spctl_out="$(spctl -a -vvv -t open --context context:primary-signature "$dmg_file" 2>&1 || true)"
  if ! grep -q "source=Notarized Developer ID" <<<"$spctl_out"; then
    echo "$dmg_file is not notarized with an Apple Developer ID:" >&2
    echo "$spctl_out" >&2
    exit 1
  fi
fi

mkdir -p "$(dirname "$out")"
cat > "$out" <<EOF
cask "dataomni" do
  version "$version"
  sha256 "$expected_sha"

  url "https://github.com/dataomni-rs/DataOmni/releases/download/v#{version}/DataOmni_#{version}_aarch64.dmg"
  name "DataOmni"
  desc "Desktop client for SQL databases, MongoDB, Redis, Neo4j, and Elasticsearch"
  homepage "https://dataomni.dev/"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on arch: :arm64
  depends_on macos: :monterey

  app "DataOmni.app"
  binary "#{appdir}/DataOmni.app/Contents/MacOS/dataomni"

  zap trash: [
    "~/Library/Application Support/com.dataomni.app",
    "~/Library/Caches/com.dataomni.app",
    "~/Library/Logs/com.dataomni.app",
    "~/Library/Preferences/com.dataomni.app.plist",
    "~/Library/Saved Application State/com.dataomni.app.savedState",
    "~/Library/WebKit/com.dataomni.app",
  ]
end
EOF

echo "wrote $out (sha256=$expected_sha)"
