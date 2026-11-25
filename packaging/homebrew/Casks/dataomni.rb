cask "dataomni" do
  version "1.3.1"
  sha256 "e1235fa60ebe539120dbd7090aef6a25cbac8681b57af7b879568830cfef4f78"

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
