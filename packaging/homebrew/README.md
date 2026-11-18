# Homebrew Cask 清单

`Casks/dataomni.rb` 的写法与官方 [Homebrew/homebrew-cask](https://github.com/Homebrew/homebrew-cask) 完全一致，当前直接发布在官方 Tap 仓库 [`dataomni-rs/homebrew-tap`](https://github.com/dataomni-rs/homebrew-tap)。

用户安装命令：

```bash
brew install --cask dataomni-rs/tap/dataomni
```

装好后：
- 桌面端位于 `/Applications/DataOmni.app`（Apple Developer ID 签名 + Apple 公证钉合，直接打开无需手动清隔离标记）；
- 内置只读命令行与 MCP 服务自动软链到 `$(brew --prefix)/bin/dataomni`（Apple Silicon 上即 `/opt/homebrew/bin/dataomni`），终端可直接执行 `dataomni cli version` 或 `dataomni mcp`。

## 发一个新版本后更新 Tap

1. 新版本在 GitHub Releases 正式公开（非草稿、非预发布，且 macOS `.dmg` 已由 `release.yml` 完成签名、公证与 `stapler` 票据钉合）。
2. 在本仓库运行：
   ```bash
   scripts/homebrew-cask.sh <版本>
   ```
   脚本会拉取该版本的 `DataOmni_<版本>_aarch64.dmg`，核对 GitHub 服务端 SHA-256 摘要，在 macOS 上跑 `xcrun stapler validate` 与 `spctl` 公证核验，通过后写出 `packaging/homebrew/Casks/dataomni.rb`。
3. 风格与本地安装校验：
   ```bash
   brew style packaging/homebrew/Casks/dataomni.rb
   ```
4. 将生成的 `Casks/dataomni.rb` 同步提交到 `dataomni-rs/homebrew-tap` 的 `main` 分支。

## 何时提交到上游 `Homebrew/homebrew-cask`

Homebrew 官方主库 `Homebrew/homebrew-cask` 的自动门禁（`SharedAudits`）要求满足两条硬指标后才收新包：
1. `dataomni.dev` 域名注册满 30 天（`NEW_DOMAIN_THRESHOLD_DAYS = 30`）；
2. `dataomni-rs/DataOmni` 仓库达到 GitHub 知名度门槛（他人提交需 `stars >= 75` 或 `forks >= 30`；仓库维护者自提乘 3 倍即 `stars >= 225`）。

上述两条满足后，直接把 `Casks/dataomni.rb` 原样拷进 `Homebrew/homebrew-cask` 的 `Casks/d/dataomni.rb` 提 PR 即可，无需改动语法。
