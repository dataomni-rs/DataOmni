import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { Check, Copy, Download, RefreshCw, Terminal } from 'lucide-react';
import { useLanguageStore } from '../stores/languageStore';
import { describeError } from '../utils/describeError';
import {
  buildClaudeCodeCommand,
  buildMcpServersJson,
  filterAuditEntries,
  formatAuditTimestamp,
  type AgentAuditLogEntry,
  type AgentAuditResultFilter,
  type AgentCliInfo
} from '../utils/agentConfigSnippets';
import { PLAIN_TEXT_INPUT } from './FormControls';

const BUTTON_CLASS =
  'inline-flex items-center gap-1.5 rounded-control border border-line-strong bg-surface px-2.5 py-1 text-xs text-fg hover:bg-surface-hover disabled:opacity-50';

type CopiedTarget = 'claudeCode' | 'mcpJson' | 'skill' | null;

function resultBadgeClass(result: string): string {
  if (result === 'ok') {
    return 'bg-success-soft text-success';
  }
  if (result === 'refused') {
    return 'bg-warning-soft text-warning';
  }
  return 'bg-danger-soft text-danger';
}

interface AgentCliSectionProps {
  sectionRef?: React.RefObject<HTMLDivElement>;
}

/**
 * 设置页「命令行与 Agent」小节（CM1）：
 * - 展示当前应用二进制的真实路径、PATH 状态与已开放的非生产连接数；
 * - 提供 Claude Code、Claude Desktop / Cursor 一键复制接入配置与 `SKILL.md` 导出；
 * - 提供只读审计日志查看器（直接读 `dataomni-cli.log`，绝不提供删除或篡改入口）。
 */
export function AgentCliSection({ sectionRef }: AgentCliSectionProps) {
  const t = useLanguageStore((state) => state.t);
  const [info, setInfo] = useState<AgentCliInfo | null>(null);
  const [auditEntries, setAuditEntries] = useState<AgentAuditLogEntry[]>([]);
  const [loadingAudit, setLoadingAudit] = useState(false);
  const [resultFilter, setResultFilter] = useState<AgentAuditResultFilter>('all');
  const [searchText, setSearchText] = useState('');
  const [copiedTarget, setCopiedTarget] = useState<CopiedTarget>(null);
  const [exportedPath, setExportedPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadAudit = useCallback(async () => {
    setLoadingAudit(true);
    try {
      const entries = await invoke<AgentAuditLogEntry[]>('read_agent_audit_log', { limit: 200 });
      setAuditEntries(entries);
    } catch (caught) {
      setError(describeError(caught));
    } finally {
      setLoadingAudit(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    invoke<AgentCliInfo>('agent_cli_info')
      .then((value) => {
        if (!cancelled) {
          setInfo(value);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(describeError(caught));
        }
      });

    void (async () => {
      setLoadingAudit(true);
      try {
        const entries = await invoke<AgentAuditLogEntry[]>('read_agent_audit_log', { limit: 200 });
        if (!cancelled) {
          setAuditEntries(entries);
        }
      } catch (caught) {
        if (!cancelled) {
          setError(describeError(caught));
        }
      } finally {
        if (!cancelled) {
          setLoadingAudit(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  const claudeCodeCommand = useMemo(
    () => (info ? buildClaudeCodeCommand(info.executablePath, info.os) : ''),
    [info]
  );

  const mcpServersJson = useMemo(
    () => (info ? buildMcpServersJson(info.executablePath) : ''),
    [info]
  );

  const filteredEntries = useMemo(
    () => filterAuditEntries(auditEntries, { result: resultFilter, search: searchText }),
    [auditEntries, resultFilter, searchText]
  );

  const copyText = async (target: Exclude<CopiedTarget, null>, text: string) => {
    if (!text) return;
    setError(null);
    try {
      await navigator.clipboard.writeText(text);
      setCopiedTarget(target);
      window.setTimeout(() => {
        setCopiedTarget((current) => (current === target ? null : current));
      }, 1500);
    } catch (caught) {
      setError(describeError(caught, t('common.copyFailed')));
    }
  };

  const exportSkillFile = async () => {
    if (!info) return;
    setError(null);
    setExportedPath(null);
    try {
      const target = await save({
        defaultPath: 'SKILL.md',
        filters: [{ name: 'Markdown', extensions: ['md'] }]
      });
      if (!target) {
        return;
      }
      await invoke('write_text_file', { path: target, contents: info.guideMarkdown });
      setExportedPath(target);
    } catch (caught) {
      setError(describeError(caught));
    }
  };

  return (
    <div
      ref={sectionRef}
      id="settings-agent-section"
      className="border-t border-line px-5 py-4"
    >
      <div className="flex items-center gap-2">
        <Terminal size={14} className="text-fg-muted" />
        <h3 className="text-sm font-medium text-fg">{t('settings.agent.title')}</h3>
      </div>
      <p className="mt-1 text-xs leading-relaxed text-fg-muted">
        {t('settings.agent.description')}
      </p>

      {info && (
        <div className="mt-3 space-y-3">
          <div className="rounded-control border border-line bg-surface-sunken p-2.5 text-xs">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="font-medium text-fg">{t('settings.agent.binaryPath')}</span>
              <div className="flex items-center gap-1.5">
                <span
                  className={
                    info.onPath
                      ? 'rounded px-1.5 py-0.5 text-[11px] bg-success-soft text-success'
                      : 'rounded px-1.5 py-0.5 text-[11px] bg-surface text-fg-subtle border border-line'
                  }
                >
                  {info.onPath ? t('settings.agent.onPath') : t('settings.agent.notOnPath')}
                </span>
                <span className="rounded border border-line bg-surface px-1.5 py-0.5 text-[11px] text-fg-muted">
                  {t('settings.agent.openCount', { count: info.openConnectionCount })}
                </span>
              </div>
            </div>
            <p className="mt-1.5 select-text break-all font-mono text-[11px] text-fg-muted">
              {info.executablePath}
            </p>
          </div>

          <div className="space-y-1.5">
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs font-medium text-fg">{t('settings.agent.claudeCode')}</span>
              <button
                type="button"
                onClick={() => void copyText('claudeCode', claudeCodeCommand)}
                className={BUTTON_CLASS}
              >
                {copiedTarget === 'claudeCode' ? <Check size={12} /> : <Copy size={12} />}
                {copiedTarget === 'claudeCode' ? t('settings.about.copied') : t('settings.agent.copyCommand')}
              </button>
            </div>
            <pre className="overflow-x-auto rounded-control border border-line bg-surface-sunken px-2.5 py-2 font-mono text-[11px] text-fg select-text">
              {claudeCodeCommand}
            </pre>
          </div>

          <div className="space-y-1.5">
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs font-medium text-fg">{t('settings.agent.mcpJson')}</span>
              <button
                type="button"
                onClick={() => void copyText('mcpJson', mcpServersJson)}
                className={BUTTON_CLASS}
              >
                {copiedTarget === 'mcpJson' ? <Check size={12} /> : <Copy size={12} />}
                {copiedTarget === 'mcpJson' ? t('settings.about.copied') : t('settings.agent.copyJson')}
              </button>
            </div>
            <pre className="overflow-x-auto rounded-control border border-line bg-surface-sunken px-2.5 py-2 font-mono text-[11px] text-fg select-text">
              {mcpServersJson}
            </pre>
          </div>

          <div className="flex flex-wrap items-center justify-between gap-2 rounded-control border border-line bg-surface-sunken px-2.5 py-2">
            <div className="min-w-0">
              <p className="text-xs font-medium text-fg">{t('settings.agent.skillTitle')}</p>
              <p className="text-[11px] text-fg-subtle">{t('settings.agent.skillHint')}</p>
            </div>
            <div className="flex shrink-0 items-center gap-1.5">
              <button
                type="button"
                onClick={() => void copyText('skill', info.guideMarkdown)}
                className={BUTTON_CLASS}
              >
                {copiedTarget === 'skill' ? <Check size={12} /> : <Copy size={12} />}
                {copiedTarget === 'skill' ? t('settings.about.copied') : t('settings.agent.copySkill')}
              </button>
              <button
                type="button"
                onClick={() => void exportSkillFile()}
                className={BUTTON_CLASS}
              >
                <Download size={12} />
                {t('settings.agent.exportSkill')}
              </button>
            </div>
          </div>
          {exportedPath && (
            <p className="break-all text-xs text-success">
              {t('settings.agent.skillExported', { path: exportedPath })}
            </p>
          )}
        </div>
      )}

      <div className="mt-4 border-t border-line pt-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h4 className="text-xs font-medium text-fg">{t('settings.agent.auditTitle')}</h4>
            <p className="text-[11px] text-fg-subtle">{t('settings.agent.auditNote')}</p>
          </div>
          <button
            type="button"
            onClick={() => void loadAudit()}
            disabled={loadingAudit}
            className={BUTTON_CLASS}
          >
            <RefreshCw size={12} className={loadingAudit ? 'animate-spin' : ''} />
            {t('settings.agent.auditRefresh')}
          </button>
        </div>

        <div className="mt-2 flex flex-wrap items-center gap-2">
          <select
            value={resultFilter}
            onChange={(event) => setResultFilter(event.target.value as AgentAuditResultFilter)}
            aria-label={t('settings.agent.auditResultFilter')}
            className="rounded-control border border-line-strong bg-surface px-2 py-1 text-xs text-fg"
          >
            <option value="all">{t('settings.agent.auditFilter.all')}</option>
            <option value="ok">ok</option>
            <option value="refused">refused</option>
            <option value="not_found">not_found</option>
            <option value="database">database</option>
          </select>
          <input
            type="text"
            value={searchText}
            onChange={(event) => setSearchText(event.target.value)}
            placeholder={t('settings.agent.auditSearchPlaceholder')}
            aria-label={t('settings.agent.auditSearchPlaceholder')}
            {...PLAIN_TEXT_INPUT}
            className="min-w-[180px] flex-1 rounded-control border border-line-strong bg-surface px-2 py-1 text-xs text-fg placeholder:text-fg-subtle"
          />
          <span className="text-[11px] text-fg-subtle">
            {t('settings.agent.auditCount', { count: filteredEntries.length })}
          </span>
        </div>

        <div className="mt-2 max-h-52 overflow-auto rounded-control border border-line">
          {filteredEntries.length === 0 ? (
            <p className="px-3 py-4 text-center text-xs text-fg-subtle">
              {t('settings.agent.auditEmpty')}
            </p>
          ) : (
            <table className="w-full border-collapse text-left text-[11px]">
              <thead className="sticky top-0 bg-surface-sunken text-fg-muted">
                <tr className="border-b border-line">
                  <th className="px-2 py-1.5 font-medium">{t('settings.agent.auditColTime')}</th>
                  <th className="px-2 py-1.5 font-medium">{t('settings.agent.auditColCommand')}</th>
                  <th className="px-2 py-1.5 font-medium">{t('settings.agent.auditColConnection')}</th>
                  <th className="px-2 py-1.5 font-medium">{t('settings.agent.auditColKeyword')}</th>
                  <th className="px-2 py-1.5 font-medium">{t('settings.agent.auditColResult')}</th>
                  <th className="px-2 py-1.5 text-right font-medium">{t('settings.agent.auditColMs')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {filteredEntries.map((entry, index) => (
                  <tr key={`${entry.timestamp}-${entry.sha256}-${index}`} className="hover:bg-surface-hover">
                    <td className="whitespace-nowrap px-2 py-1 font-mono text-fg-subtle">
                      {formatAuditTimestamp(entry.timestamp)}
                    </td>
                    <td className="whitespace-nowrap px-2 py-1 font-mono text-fg">
                      {entry.command}
                    </td>
                    <td className="max-w-[120px] truncate px-2 py-1 text-fg" title={entry.connection}>
                      {entry.connection}
                    </td>
                    <td
                      className="whitespace-nowrap px-2 py-1 font-mono text-fg-muted"
                      title={`sha256: ${entry.sha256} (${entry.length} B)`}
                    >
                      {entry.firstKeyword || '—'}
                    </td>
                    <td className="whitespace-nowrap px-2 py-1">
                      <span className={`rounded px-1.5 py-0.5 font-mono text-[10px] ${resultBadgeClass(entry.result)}`}>
                        {entry.result}
                      </span>
                    </td>
                    <td className="whitespace-nowrap px-2 py-1 text-right font-mono text-fg-subtle">
                      {entry.ms}ms
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>

      {error && <p className="mt-2 text-xs text-danger">{error}</p>}
    </div>
  );
}
