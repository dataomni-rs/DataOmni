# Linux 验证清单

与 [`windows-checklist.md`](windows-checklist.md) 配套，列出打包出 `.deb` 与 `.rpm` 之后**跟 Linux 桌面环境有关**的验证项（WebKitGTK、Wayland / X11、Secret Service 钥匙串、XDG 目录、`DT_RPATH` 动态库搜索与系统包渠道防冲突）。`v1.3` 跨平台闭环必测子集及一键脚本汇总见 [`packaged-verification-checklist.md`](packaged-verification-checklist.md)。

每项后面写了**期望**；不符合时记下：哪一项、发行版与桌面环境（`cat /etc/os-release`、`$XDG_SESSION_TYPE`、`$XDG_CURRENT_DESKTOP`）、WebKitGTK 版本（「设置 → 诊断」）、安装包种类（`.deb` / `.rpm`）以及界面原话或截图。

## 0. 准备

- **虚拟机架构**：当前发布的 Linux 安装包（`DataOmni_<版本>_amd64.deb`、`DataOmni-<版本>-1.x86_64.rpm`）为 **`x86_64` (`amd64`)** 架构。
  - 推荐在 `x86_64` 主机（例如 Windows 11 Pro 自带的 Hyper-V 或 VMware）上创建 **Ubuntu 24.04 LTS Desktop (`amd64`)** 虚拟机（测 `.deb`），有条件再加一台 **Fedora Workstation 40/41 (`x86_64`)**（测 `.rpm`）。
- **基础桌面依赖确认**（最小化安装的桌面镜像建议先确认已装钥匙串与中文字体）：
  ```bash
  # Ubuntu / Debian
  sudo apt update
  sudo apt install -y gnome-keyring libsecret-1-0 libsecret-tools fonts-noto-cjk

  # Fedora / RHEL
  sudo dnf install -y gnome-keyring libsecret google-noto-sans-cjk-ttc-fonts
  ```
- **获取正式安装包**：从 `https://dataomni.dev/zh/` 或 [GitHub Releases](https://github.com/dataomni-rs/DataOmni/releases/latest) 下载对应版本的 `.deb` 或 `.rpm`。

## 1. 安装、升级、卸载与系统包渠道（`.deb` 与 `.rpm`）

| # | 操作 | 期望 |
|---|---|---|
| 1.1 | 用包管理器安装：`sudo apt install -y ./DataOmni_<版本>_amd64.deb`（或 `sudo dnf install -y ./DataOmni-<版本>-1.x86_64.rpm`） | 自动解析并安装 `ca-certificates`、WebKitGTK 等系统依赖，退出码为 `0` |
| 1.2 | 检查桌面入口与二进制位置 | 应用启动器（Activities / 应用菜单）有 DataOmni 且图标清晰；`which dataomni` 为 `/usr/bin/dataomni`；`/usr/share/applications/` 下有对应 `.desktop` 文件 |
| 1.3 | 从应用菜单图标启动 | 正常弹出单个主窗口；`~/.local/share/com.dataomni.app/logs/dataomni.log` 写入 `DataOmni <版本> 启动` |
| 1.4 | 系统包渠道识别（`install_channel`） | `/usr/bin/dataomni` 自动识别为 `systemPackage`：「设置」中点击检查更新正常完成，遇到新版本时只提示通过包管理器/下载页升级，**绝不尝试自覆盖 `/usr/bin/dataomni`** |
| 1.5 | 建一个连接、存密码、调窗口大小后退出，再用高版本 `.deb` / `.rpm` 直接覆盖安装 | 无需先卸载；版本号更新；原有连接、钥匙串密码与窗口尺寸位置均保留 |
| 1.6 | 卸载软件包：`sudo apt remove -y dataomni`（或 `sudo dnf remove -y dataomni`） | `/usr/bin/dataomni`、`/usr/lib/DataOmni`（含 `instantclient`）与 `.desktop` 快捷方式均被清理干净；`~/.config/com.dataomni.app`（连接配置）与系统钥匙串条目默认保留 |
| 1.7 | 卸载后重新安装同版本 | 1.6 保留的 `~/.config/com.dataomni.app` 与钥匙串口令直接恢复可用，免重新输入密码 |

## 2. 窗口、WebKitGTK、严格 CSP 与输入法

| # | 操作 | 期望 |
|---|---|---|
| 2.1 | 检查当前显示协议（`echo $XDG_SESSION_TYPE`）：默认 Wayland 会话下运行，必要时切 X11（Ubuntu 登录齿轮选 `Ubuntu on Xorg`）对照 | Wayland 与 X11 下均正常渲染，无黑边、闪烁或鼠标点击偏移 |
| 2.2 | 系统显示缩放调到 125% / 150% / 200% | 字体清晰不发虚，工具栏、对象树与数据网格不重字、不截断 |
| 2.3 | 系统外观切换深色 / 浅色（应用内设为「跟随系统」） | 界面配色实时跟随切换 |
| 2.4 | 中文与等宽字体检查 | 对象树、表格、弹窗中文无方块字（豆腐块）；SQL 编辑器为等宽字体 |
| 2.5 | 窗口最大化、还原、分屏贴靠（Super+←/→）、关闭重开 | 窗口标题栏与控制按钮正常；重开后恢复上次的尺寸与位置 |
| 2.6 | 快捷键：`Ctrl+Enter` 执行、`Ctrl+S` 保存脚本、`Ctrl+W` 关标签、`Ctrl+K` 命令面板、`Ctrl+B` 收起侧栏 | 全部走 `Ctrl` 修饰键，不被桌面环境快捷键拦截 |
| 2.7 | 中文输入法（IBus / Fcitx5）：在 SQL 编辑器、单元格内联编辑、对象树筛选框打拼音，**候选词未上屏时按 `Enter` 与 `Esc`** | `Enter` 只上屏字母，不触发执行查询或提交单元格；`Esc` 只关闭候选框，不误关弹窗或退出编辑（`!isImeKeyEvent` 护栏） |
| 2.8 | 严格 CSP 下多工作区与画布导出：打开 SQL 编辑器、ER 图导出 PNG、以及 MongoDB / Redis / Neo4j / Elasticsearch 四个工作区 | 界面均不白屏，导出的 PNG 图片完整可看；检查 `~/.local/share/com.dataomni.app/logs/dataomni.log` 无 `CSP` / `Refused to` / `panic:` 报错 |

## 3. 系统钥匙串（Secret Service）与凭据

| # | 操作 | 期望 |
|---|---|---|
| 3.1 | 建连接、勾选保存密码，完全退出应用后重开并连接 | 免输密码直接连上 |
| 3.2 | 核验钥匙串存储与磁盘零明文 | 终端运行 `secret-tool search service DataOmni` 能查到对应凭据条目；检查 `cat ~/.config/com.dataomni.app/connections.json` **不包含任何明文密码** |
| 3.3 | 在应用内删除该连接 | 再次运行 `secret-tool search service DataOmni`，对应连接的口令与 `#agent` 授权条目一并删除 |
| 3.4 | 钥匙串守护进程不可用或未解锁场景（如最小化无头会话未起 `gnome-keyring-daemon`） | 给出可理解的凭据存储错误提示，不崩溃、不退回明文落盘 |

## 4. 文件、XDG 路径与编码

| # | 操作 | 期望 |
|---|---|---|
| 4.1 | SQLite 打开不存在的路径（选择已有文件模式，手填 `/tmp/not_exist_check.db` 点测试连接） | 报错提示无法打开文件，且 `ls -l /tmp/not_exist_check.db` 确认**未在磁盘上误建 0 字节空文件** |
| 4.2 | 新建 SQLite 库到**带中文与空格的路径**（如 `~/测试 数据/a.db`），建表、内联改格预览提交、关掉重开 | 能建、能改、事务提交落盘，重开后数据完整 |
| 4.3 | 多连接独立关闭：同时打开两个 SQLite 连接，断开其中一个 | 另一个连接继续正常查询与翻页，互不锁死或误关 |
| 4.4 | DuckDB 新建到 `~/测试 数据/b.duckdb` 并读写 | 同 4.2，正常建表读写 |
| 4.5 | 打开一份 Windows 记事本格式（UTF-8 BOM + CRLF）的 `.sql` 文件，执行选中语句后保存 | 「执行选中」偏移准确；保存后 `file` / `xxd` 检查 BOM 与 CRLF 原样保留 |
| 4.6 | 导入一份 GBK 编码的中文 CSV；再将查询结果导出为 CSV | 导入自动识别 `gb18030` 中文不乱码；导出文件带 UTF-8 BOM（`EF BB BF`），LibreOffice Calc 打开中文不乱码 |
| 4.7 | 从文件管理器（Nautilus / Dolphin）拖入一个 `.sql` 文件到主窗口 | `dragDropEnabled: false` 生效，WebView 不跳转页面、不白屏 |

## 5. 网络、SSH 隧道与 CLI / MCP（stdio）

| # | 操作 | 期望 |
|---|---|---|
| 5.1 | 连 MySQL、PostgreSQL，各跑一条查询并内联改一格提交 | 正常 |
| 5.2 | SSH 隧道连接：私钥选 `~/.ssh/id_ed25519`（或手填 `~/.ssh/id_ed25519`） | 主机不在 `~/.ssh/known_hosts` 时报 `DATAOMNI_SSH_HOST_KEY_UNKNOWN` 并展示 `SHA256:...` 指纹；写入 `known_hosts` 后正常连通 |
| 5.3 | 终端执行 `dataomni connections` 与 `dataomni query <dev连接> "SELECT ..."` | 不弹 GUI 窗口，交互终端自动格式化高亮输出（管道重定向或加 `-c` 时为紧凑纯 JSON，`schema: 1`），退出码 `0` |
| 5.4 | 打包二进制 `/usr/bin/dataomni mcp`（stdio）护栏验证 | 见下方一键核验脚本：`DEV` 只读连接查询成功，写语句（`DELETE`）返回 `isError: true` 且 `kind = "refused"`，`PROD` 连接返回 `isError: true` 且 `kind = "not_found"` |

### MCP 与日志一键核验脚本（Bash）

先在应用内建好两个本地 SQLite 连接：`dev_v1`（环境 `DEV`，开启「命令行与 Agent 访问：只读」，建表 `items(id, name, price)`）与 `prod_v1`（环境 `PROD`），然后在终端运行：

```bash
BIN="$(command -v dataomni)"
echo "Binary: ${BIN}"

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"linux-check","version":"1.0"}}}' \
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

## 6. 备份（`pg_dump` / `mysqldump`）

| # | 操作 | 期望 |
|---|---|---|
| 6.1 | 安装 `postgresql-client` 后在应用内备份 PostgreSQL 库并还原 | 自动定位 `/usr/bin/pg_dump` 与 `pg_restore`，备份还原成功 |
| 6.2 | 安装 `mysql-client`（或 `mariadb-client`）后备份 MySQL 库 | 自动定位 `mysqldump`，备份成功 |
| 6.3 | 未安装客户端工具时点击备份 | 给出明确提示说明未找到 `pg_dump` / `mysqldump` |

## 7. Oracle（内置 Instant Client 与 `libaio`）

Linux 安装包将 Oracle Instant Client Basic Light 放在 `/usr/lib/DataOmni/instantclient`，并在编译期向 `/usr/bin/dataomni` 写入 `DT_RPATH`（`$ORIGIN/../lib/DataOmni/instantclient`，无需手动设 `LD_LIBRARY_PATH`）。

| # | 操作 | 期望 |
|---|---|---|
| 7.1 | 核验二进制 `DT_RPATH` 与随包动态库 | `ls /usr/lib/DataOmni/instantclient/libclntsh.so.23.1` 存在；`readelf -d /usr/bin/dataomni \| grep -E 'RPATH\|RUNPATH'` 显示 `RPATH` 指向 `$ORIGIN/../lib/DataOmni/instantclient` |
| 7.2 | 未装 `libaio` 时尝试连接 Oracle | 报 `DATAOMNI_ORACLE_CLIENT_LOAD_FAILED` 并带出缺少 `libaio.so.1` 的原始原因，不崩溃 |
| 7.3 | 安装 `libaio` 后连接 Oracle（**Ubuntu 24.04 注意事项见下**） | 能正常连接、展开 schema/表结构、执行查询与查看执行计划 |

> **Ubuntu 24.04 LTS (`noble`) `libaio` 说明**：  
> Ubuntu 24.04 起包名改为 `libaio1t64`，默认只安装 `/usr/lib/x86_64-linux-gnu/libaio.so.1t64`，而 Oracle Instant Client 的 `libclntsh.so` 按 SONAME 查找 `libaio.so.1`。在 Ubuntu 24.04 上连接 Oracle 前需执行一次：
> ```bash
> sudo apt install -y libaio1t64
> if [[ ! -e /usr/lib/x86_64-linux-gnu/libaio.so.1 && -e /usr/lib/x86_64-linux-gnu/libaio.so.1t64 ]]; then
>   sudo ln -s /usr/lib/x86_64-linux-gnu/libaio.so.1t64 /usr/lib/x86_64-linux-gnu/libaio.so.1
> fi
> ```

## 8. 结果回填记录

逐项写「过 / 不过 / 没测」，不过的附报错原文或截图。

| 日期 | 发行版与内核 | 桌面环境与会话（Wayland / X11） | WebKitGTK 版本 | 安装包 | 结果摘要与备注 |
| --- | --- | --- | --- | --- | --- |
| 待填 | 待填 | 待填 | 待填 | `.deb` / `.rpm` | 待测 |
