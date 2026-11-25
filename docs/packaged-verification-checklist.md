# 跨平台打包版实机闭环清单（V1：Windows 11 & Linux）

本清单对应 [`TODOs.md`](../TODOs.md)「`v1.3` 路线 → `V1`」与 [`rfcs/roadmap-1.3.md`](../rfcs/roadmap-1.3.md)，用于在 **Windows 11（WebView2，`.msi` / `-setup.exe`）** 与 **Linux 桌面环境（WebKitGTK，`.deb` / `.rpm`）** 的正式发布包上完成三项跨平台实机闭环：

1. **SQLite 后端 `sqlx_pool` 闭环**：打开文件、对象树与表结构读取、内联编辑预览与事务提交、多连接独立关闭、不存在路径不误建 0 字节空文件；
2. **严格 CSP 与多工作区闭环**：SQL 编辑器、ER 图导出 PNG、MongoDB / Redis / Neo4j / Elasticsearch 四大工作区渲染及应用更新检查正常，`dataomni.log` 零 CSP 违规与 `panic:` 记录；
3. **打包二进制 `dataomni mcp`（stdio）闭环**：直接调打包后的主程序跑通只读查询、写语句 `refused` 与 `PROD` 生产连接 `not_found`。

> **与现有文档的分工**：Windows 与 Linux 各自完整的平台专项清单见 [`windows-checklist.md`](windows-checklist.md) 与 [`linux-checklist.md`](linux-checklist.md)；发版通用全功能回归见 [`release-regression.md`](release-regression.md)。

---

## 0. 安装包与环境准备

- **下载正式包**（以 `v1.3.1` 为例，从 `https://dataomni.dev/zh/` 或 [GitHub Releases](https://github.com/dataomni-rs/DataOmni/releases/latest) 获取）：
  - **Windows 11 (`x64`)**：
    - `DataOmni_1.3.1_x64-setup.exe`（NSIS，默认安装至 `%LOCALAPPDATA%\DataOmni\dataomni.exe`）
    - 或 `DataOmni_1.3.1_x64_en-US.msi`（MSI，默认安装至 `C:\Program Files\DataOmni\dataomni.exe`）
    - *注：接入商业代码签名前，首次双击弹出 Windows SmartScreen（「Windows 已保护你的电脑」→ 更多信息 → 仍要运行）属预期现象。*
  - **Linux 虚拟机 (`x86_64` / `amd64`)**：
    - 发布产物 `DataOmni_1.3.1_amd64.deb` / `DataOmni-1.3.1-1.x86_64.rpm` 为 `x86_64` 架构，推荐直接在 Windows 11 Pro（自带 Hyper-V 或 VMware）上安装 **Ubuntu 24.04 Desktop (`amd64`)** 走原生硬件虚拟化（避免在 Apple Silicon Mac 上软件模拟 x86_64）。
    - Ubuntu / Debian 安装命令（一并确认系统钥匙串组件已装）：
      ```bash
      sudo apt update
      sudo apt install -y gnome-keyring libsecret-1-0 ./DataOmni_1.3.1_amd64.deb
      ```
    - Fedora / RHEL 安装命令：
      ```bash
      sudo dnf install -y gnome-keyring libsecret ./DataOmni-1.3.1-1.x86_64.rpm
      ```

- **可选：一键启动四大 NoSQL 测试容器**（若局域网内没有现成测试实例，可在任意装有 Docker 的主机运行）：
  ```bash
  docker run -d --name v1-mongo -p 27017:27017 mongo:7
  docker run -d --name v1-redis -p 6379:6379 redis:7-alpine
  docker run -d --name v1-neo4j -p 7687:7687 -p 7474:7474 -e NEO4J_AUTH=neo4j/testpass123 neo4j:5
  docker run -d --name v1-es -p 9200:9200 -e "discovery.type=single-node" -e "xpack.security.enabled=false" elasticsearch:8.15.0
  ```
  验收结束后清理：`docker rm -f v1-mongo v1-redis v1-neo4j v1-es`。

---

## 1. Windows 11 Pro 真机验收（V1-Win）

### 1.1 SQLite `sqlx_pool` 全链路与空文件防护

- [ ] **不存在路径不误建空文件**：新建 **SQLite** 连接，保持打开已有文件模式（**不要**点新建文件），路径手填不存在的文件 `C:\Users\Public\not_exist_v1.db` → 点击「测试连接」。
  - **预期**：提示数据库文件打开失败，且检查 `C:\Users\Public\` 目录**未生成** 0 字节的 `not_exist_v1.db`。
- [ ] **新建两个测试库（供 1.3 节 MCP 校验直接复用）**：
  - **连接 `dev_v1`**：新建本地 SQLite 文件 `C:\Users\Public\dev_v1.db`，环境选 **`DEV`（开发）**，将表单下方 **「命令行与 Agent 访问」设为「只读」**，保存并打开。
  - **连接 `prod_v1`**：新建本地 SQLite 文件 `C:\Users\Public\prod_v1.db`，环境选 **`PROD`（生产）**，保存并同时打开。
- [ ] **对象树、表结构与内联编辑提交**：在 `dev_v1` 的 SQL 编辑器执行：
  ```sql
  CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL, price INTEGER NOT NULL);
  INSERT INTO items (name, price) VALUES ('keyboard', 299), ('mouse', 129);
  ```
  - 刷新左侧对象树看到 `items`；
  - 打开 `items` **表结构**，核对列定义与主键显示正常；
  - 打开 `items` **表数据**，双击将 `keyboard` 的 `299` 改为 `319` → 点击**预览 SQL** 并**提交**，刷新确认已落盘。
- [ ] **多连接独立关闭**：保持两库同时打开，右键断开/关闭 `prod_v1`，在 `dev_v1` 中继续执行查询确认互不干扰。

### 1.2 严格 CSP 下各工作区渲染与更新检查

- [ ] **SQL 编辑器与 ER 图导出 PNG**：确认 SQL 编辑器语法高亮与自动补全正常；打开 `dev_v1` 的 **ER 图**，点击 **导出 PNG** 保存到本地并打开图片核对清晰完整（验证严格 CSP 下 `data:` / `blob:` 与 Canvas 导出不被拦截）。
- [ ] **四大 NoSQL 工作区渲染**：分别连接并打开 **MongoDB**、**Redis**、**Neo4j**、**Elasticsearch** 工作区，浏览集合/键空间/标签/索引并各执行一次基础读取，确认无白屏、无样式异常。
- [ ] **更新检查与诊断信息**：打开 **设置**，记录「诊断信息」中的系统与 WebView2 版本号；点击一次**检查更新**，确认正常请求 `latest.json` 并提示当前已是最新版本。

### 1.3 打包二进制 `dataomni.exe mcp`（stdio）与日志一键自检

- [ ] 打开 **PowerShell** 粘贴运行以下脚本，将完整输出贴回验收记录：

```powershell
$exe = @(
  "$env:LOCALAPPDATA\DataOmni\dataomni.exe",
  "$env:ProgramFiles\DataOmni\dataomni.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $exe) { throw "未找到 dataomni.exe，请确认已安装 DataOmni" }
Write-Host "Binary: $exe"

$requests = @(
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"v1-check","version":"1.0"}}}'
  '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"connections","arguments":{}}}'
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"query","arguments":{"connection":"dev_v1","sql":"SELECT id, name, price FROM items ORDER BY id"}}}'
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"query","arguments":{"connection":"dev_v1","sql":"DELETE FROM items WHERE id = 1"}}}'
  '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"query","arguments":{"connection":"prod_v1","sql":"SELECT 1"}}}'
) -join "`n"

$psi = [System.Diagnostics.ProcessStartInfo]::new($exe, "mcp")
$psi.UseShellExecute = $false
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
$psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8

$proc = [System.Diagnostics.Process]::Start($psi)
$bytes = [System.Text.Encoding]::UTF8.GetBytes($requests + "`n")
$proc.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
$proc.StandardInput.Close()
$stdout = $proc.StandardOutput.ReadToEnd()
$stderr = $proc.StandardError.ReadToEnd()
$proc.WaitForExit()

Write-Host "`n=== MCP stdout (exit=$($proc.ExitCode)) ==="
Write-Host $stdout
if ($stderr) { Write-Host "`n=== MCP stderr ===`n$stderr" }

$logPath = "$env:LOCALAPPDATA\com.dataomni.app\logs\dataomni.log"
Write-Host "`n=== Log check ($logPath) ==="
if (Test-Path $logPath) {
  $cspHits = Select-String -Path $logPath -Pattern "Content Security Policy|CSP|Refused to|panic:" -CaseSensitive:$false
  Write-Host ("Log lines: " + (Get-Content $logPath).Count + ", CSP/panic hits: " + @($cspHits).Count)
  Get-Content $logPath -Tail 15
} else {
  Write-Host "Log file not found!"
}
```

**通过判据**：
- `exit=0`；`id=1` 返回当前版本号；
- `id=2` (`connections`) 列出 `dev_v1` 且**不含** `prod_v1`；
- `id=3` (`SELECT`) `isError: false`，返回修改后的 `319` 与 `129`；
- `id=4` (`DELETE`) `isError: true`，且错误 JSON 中 `"kind":"refused"`；
- `id=5` (`prod_v1`) `isError: true`，且错误 JSON 中 `"kind":"not_found"`；
- 日志包含 `DataOmni <版本> 启动`，且 `CSP/panic hits: 0`。

---

## 2. Linux 桌面虚拟机验收（V1-Linux）

### 2.1 SQLite `sqlx_pool` 全链路与空文件防护

- [ ] **不存在路径不误建空文件**：在图形桌面会话中启动 DataOmni，新建 **SQLite** 连接（打开已有文件模式），路径手填 `/tmp/not_exist_v1.db` → 点击「测试连接」。
  - **预期**：提示打开失败，且终端执行 `ls -l /tmp/not_exist_v1.db` 显示文件不存在。
- [ ] **新建两个测试库**：
  - **连接 `dev_v1`**：新建本地 SQLite 文件 `/tmp/dev_v1.db`，环境选 **`DEV`**，将 **「命令行与 Agent 访问」设为「只读」**，保存打开，执行 1.1 节同一段 `CREATE TABLE items ... INSERT ...` 语句。
  - **连接 `prod_v1`**：新建本地 SQLite 文件 `/tmp/prod_v1.db`，环境选 **`PROD`**，保存并同时打开。
- [ ] **对象树、表结构、内联编辑与独立关闭**：在 `dev_v1` 中核对对象树、表结构页、双击将 `299` 改为 `319` 预览并提交；断开 `prod_v1` 后确认 `dev_v1` 仍能正常查询。

### 2.2 严格 CSP 下 WebKitGTK 各工作区与系统包渠道识别

- [ ] **SQL 编辑器与 ER 图导出 PNG**：验证 WebKitGTK 下 SQL 编辑器输入与补全正常；打开 `dev_v1` 的 **ER 图** 导出 PNG 并查看图片正常。
- [ ] **四大 NoSQL 工作区渲染**：分别连接并打开 **MongoDB**、**Redis**、**Neo4j**、**Elasticsearch** 工作区执行一次浏览与基础读取，确认 WebKitGTK 下无白屏或错位。
- [ ] **更新检查与系统包渠道行为**：打开 **设置**，记录「诊断信息」中的发行版与 WebKitGTK 版本号；点击**检查更新**，确认正常完成检查（`/usr/bin/dataomni` 识别为 `systemPackage`，有新版时只引导包管理器升级、不尝试覆盖系统目录）。

### 2.3 打包二进制 `/usr/bin/dataomni mcp`（stdio）与日志一键自检

- [ ] 在 Linux 桌面终端（Bash）运行以下脚本，将输出贴回验收记录：

```bash
BIN="$(command -v dataomni)"
echo "Binary: ${BIN}"

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"v1-check","version":"1.0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"connections","arguments":{}}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"query","arguments":{"connection":"dev_v1","sql":"SELECT id, name, price FROM items ORDER BY id"}}}' \
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"query","arguments":{"connection":"dev_v1","sql":"DELETE FROM items WHERE id = 1"}}}' \
  '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"query","arguments":{"connection":"prod_v1","sql":"SELECT 1"}}}' \
  | "${BIN}" mcp
echo "MCP exit=$?"

LOG_FILE="${HOME}/.local/share/com.dataomni.app/logs/dataomni.log"
echo ""
echo "=== Log check (${LOG_FILE}) ==="
if [[ -f "${LOG_FILE}" ]]; then
  CSP_HITS="$(grep -iE 'Content Security Policy|CSP|Refused to|panic:' "${LOG_FILE}" | wc -l | tr -d ' ')"
  echo "Log lines: $(wc -l < "${LOG_FILE}" | tr -d ' '), CSP/panic hits: ${CSP_HITS}"
  tail -n 15 "${LOG_FILE}"
else
  echo "Log file not found (check Settings -> Diagnostics for exact path)"
fi
```

**通过判据**：同 1.3 节（`MCP exit=0`、`dev_v1` 查询返回 `319` 与 `129`、`DELETE` 返回 `"kind":"refused"`、`prod_v1` 返回 `"kind":"not_found"`、`CSP/panic hits: 0`）。

---

## 3. 实机验收记录

| 平台 | 系统与 WebView 版本（设置 → 诊断） | 安装包 | 1. SQLite 闭环 | 2. 严格 CSP 与工作区 | 3. MCP stdio 护栏 | 结论与日期 |
| --- | --- | --- | --- | --- | --- | --- |
| Windows 11 Pro | 待填 | `.exe` / `.msi` | 待测 | 待测 | 待测 | 待测 |
| Linux Desktop | 待填 | `.deb` / `.rpm` | 待测 | 待测 | 待测 | 待测 |
