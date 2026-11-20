import { describe, expect, it } from 'vitest';
import {
  buildClaudeCodeCommand,
  buildMcpServersJson,
  filterAuditEntries,
  formatAuditTimestamp,
  type AgentAuditLogEntry
} from './agentConfigSnippets';

describe('agentConfigSnippets', () => {
  it('generates valid Claude Code commands across macOS (with spaces), Windows, and Linux', () => {
    expect(
      buildClaudeCodeCommand('/Applications/My DataOmni.app/Contents/MacOS/dataomni', 'macos')
    ).toBe('claude mcp add dataomni -- "/Applications/My DataOmni.app/Contents/MacOS/dataomni" mcp');

    expect(
      buildClaudeCodeCommand('C:\\Program Files\\DataOmni\\dataomni.exe', 'windows')
    ).toBe('claude mcp add dataomni -- "C:\\Program Files\\DataOmni\\dataomni.exe" mcp');

    expect(buildClaudeCodeCommand('/usr/bin/dataomni', 'linux')).toBe(
      'claude mcp add dataomni -- "/usr/bin/dataomni" mcp'
    );
  });

  it('produces strictly valid JSON with proper backslash escaping for Windows and spaces for macOS', () => {
    const cases = [
      '/Applications/DataOmni Preview.app/Contents/MacOS/dataomni',
      'C:\\Program Files\\DataOmni\\dataomni.exe',
      '/usr/bin/dataomni'
    ];

    for (const executablePath of cases) {
      const jsonText = buildMcpServersJson(executablePath);
      const parsed = JSON.parse(jsonText) as {
        mcpServers: { dataomni: { command: string; args: string[] } };
      };
      expect(parsed.mcpServers.dataomni.command).toBe(executablePath);
      expect(parsed.mcpServers.dataomni.args).toEqual(['mcp']);
    }

    const winJson = buildMcpServersJson('C:\\Program Files\\DataOmni\\dataomni.exe');
    expect(winJson).toContain('C:\\\\Program Files\\\\DataOmni\\\\dataomni.exe');
  });

  it('filters audit entries by result status and case-insensitive search', () => {
    const entries: AgentAuditLogEntry[] = [
      {
        timestamp: '2026-11-20T10:15:30+08:00',
        command: 'query',
        connection: 'dev-orders',
        firstKeyword: 'SELECT',
        length: 42,
        sha256: '1234567890abcdef',
        result: 'ok',
        ms: 15
      },
      {
        timestamp: '2026-11-20T10:14:10+08:00',
        command: 'query',
        connection: 'dev-orders',
        firstKeyword: 'DELETE',
        length: 28,
        sha256: 'abcdef0123456789',
        result: 'refused',
        ms: 1
      },
      {
        timestamp: '2026-11-20T10:10:00+08:00',
        command: 'schema',
        connection: 'staging-pg',
        firstKeyword: 'users',
        length: 5,
        sha256: 'fedcba9876543210',
        result: 'not_found',
        ms: 2
      }
    ];

    expect(filterAuditEntries(entries, { result: 'all', search: '' })).toHaveLength(3);
    expect(filterAuditEntries(entries, { result: 'refused', search: '' })).toHaveLength(1);
    expect(filterAuditEntries(entries, { result: 'ok', search: 'ORDERS' })).toHaveLength(1);
    expect(filterAuditEntries(entries, { result: 'all', search: 'staging' })).toHaveLength(1);
    expect(filterAuditEntries(entries, { result: 'database', search: '' })).toHaveLength(0);
  });

  it('formats ISO timestamps concisely while keeping unknown strings intact', () => {
    expect(formatAuditTimestamp('2026-11-20T10:15:30+08:00')).toBe('2026-11-20 10:15:30');
    expect(formatAuditTimestamp('custom-time')).toBe('custom-time');
  });
});
