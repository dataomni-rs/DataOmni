export interface AgentCliInfo {
  executablePath: string;
  onPath: boolean;
  os: string;
  openConnectionCount: number;
  auditLogPath: string | null;
  guideMarkdown: string;
}

export interface AgentAuditLogEntry {
  timestamp: string;
  command: string;
  connection: string;
  firstKeyword: string;
  length: number;
  sha256: string;
  result: string;
  ms: number;
}

export type AgentAuditResultFilter = 'all' | 'ok' | 'refused' | 'not_found' | 'database';

export interface AgentAuditFilter {
  result: AgentAuditResultFilter;
  search: string;
}

function escapePosixDoubleQuoted(value: string): string {
  return value.replace(/(["\\$`])/g, '\\$1');
}

function escapeWindowsDoubleQuoted(value: string): string {
  return value.replace(/"/g, '\\"');
}

/**
 * 生成 Claude Code 一行注册命令。
 * 路径外始终加双引号，避免 macOS/Windows 带空格的应用路径被拆成多个参数。
 */
export function buildClaudeCodeCommand(executablePath: string, os: string): string {
  const quoted =
    os === 'windows'
      ? `"${escapeWindowsDoubleQuoted(executablePath)}"`
      : `"${escapePosixDoubleQuoted(executablePath)}"`;
  return `claude mcp add dataomni -- ${quoted} mcp`;
}

/**
 * 生成 Claude Desktop / Cursor 通用的 `mcpServers` JSON 配置片段。
 * 交给 `JSON.stringify` 自动处理 Windows 反斜杠与特殊字符转义，确保合法 JSON。
 */
export function buildMcpServersJson(executablePath: string): string {
  return JSON.stringify(
    {
      mcpServers: {
        dataomni: {
          command: executablePath,
          args: ['mcp']
        }
      }
    },
    null,
    2
  );
}

/**
 * 按执行结果与关键字筛选只读审计记录。
 */
export function filterAuditEntries(
  entries: readonly AgentAuditLogEntry[],
  filter: AgentAuditFilter
): AgentAuditLogEntry[] {
  const needle = filter.search.trim().toLowerCase();
  return entries.filter((entry) => {
    if (filter.result !== 'all' && entry.result !== filter.result) {
      return false;
    }
    if (needle.length === 0) {
      return true;
    }
    return (
      entry.connection.toLowerCase().includes(needle)
      || entry.command.toLowerCase().includes(needle)
      || entry.firstKeyword.toLowerCase().includes(needle)
      || entry.result.toLowerCase().includes(needle)
    );
  });
}

/**
 * 把 ISO8601 时间戳缩成更紧凑的展示形式（保留日期与时分秒）。
 */
export function formatAuditTimestamp(timestamp: string): string {
  const match = /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})/.exec(timestamp);
  if (!match) {
    return timestamp;
  }
  return `${match[1]} ${match[2]}`;
}
