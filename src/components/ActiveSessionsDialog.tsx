import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import {
  Activity,
  AlertTriangle,
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  Loader2,
  RefreshCw,
  Search,
  StopCircle,
  X,
  XCircle
} from 'lucide-react';
import type { ConnectionProfile } from '../contracts';
import type { DatabaseHandle } from '../utils/databaseHandle';
import { useLanguageStore } from '../stores/languageStore';
import { describeError } from '../utils/describeError';
import { isImeKeyEvent } from '../utils/shortcuts';
import { serverLabel } from '../utils/serverPresets';
import {
  buildSessionActionSql,
  filterActiveSessions,
  formatSessionDuration,
  normalizeActiveSessionRows,
  shouldFallbackSessionsQuery,
  summarizeActiveSessions,
  truncateSqlPreview,
  type ActiveSessionRow,
  type ActiveSessionsQueryDescriptor,
  type SessionAction,
  type TerminateSessionResult
} from '../utils/activeSessions';
import { EnvironmentBadgeTag } from './EnvironmentBadge';
import { DestructiveStatementPrompt } from './DestructiveStatementPrompt';
import { HighlightedCode } from './HighlightedCode';
import { PLAIN_TEXT_INPUT } from './FormControls';

interface ActiveSessionsDialogProps {
  connection: ConnectionProfile;
  database: DatabaseHandle;
  onClose: () => void;
}

interface PendingSessionAction {
  readonly row: ActiveSessionRow;
  readonly action: SessionAction;
  readonly sql: string;
}

/**
 * 关系库活跃会话与锁等待诊断弹窗（DC7）。
 *
 * 设计约束：
 * - 严格按需手动刷新，绝不做 `setInterval` 后台轮询，避免对生产库持续产生系统视图查询压力；
 * - 所有取消/终止操作必须经过二次确认（`DestructiveStatementPrompt`），并在 `PROD` 环境下
 *   呈现红色高危警示与待执行 SQL 原文；
 * - 自身诊断连接行标出「当前」徽标并禁用终止操作，防止误杀活动连接池。
 */
export function ActiveSessionsDialog({
  connection,
  database,
  onClose
}: ActiveSessionsDialogProps) {
  const t = useLanguageStore((state) => state.t);

  const [descriptor, setDescriptor] = useState<ActiveSessionsQueryDescriptor | null>(null);
  const [sessions, setSessions] = useState<ActiveSessionRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [usedFallback, setUsedFallback] = useState(false);
  const [lastUpdatedAt, setLastUpdatedAt] = useState<string | null>(null);

  const [search, setSearch] = useState('');
  const [onlyActive, setOnlyActive] = useState(false);
  const [onlyBlocked, setOnlyBlocked] = useState(false);
  const [hideCurrent, setHideCurrent] = useState(false);

  const [expandedSessionId, setExpandedSessionId] = useState<string | null>(null);
  const [copiedSessionId, setCopiedSessionId] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<PendingSessionAction | null>(null);
  const [actingSessionId, setActingSessionId] = useState<string | null>(null);
  const [actionNotice, setActionNotice] = useState<{ kind: 'success' | 'warning'; text: string } | null>(null);

  const mountedRef = useRef(true);
  const requestSeqRef = useRef(0);

  const loadSessions = useCallback(async () => {
    const seq = ++requestSeqRef.current;
    setLoading(true);
    setError(null);
    try {
      const queryDescriptor = await invoke<ActiveSessionsQueryDescriptor>(
        'get_active_sessions_query',
        { dbType: connection.db_type }
      );
      if (!mountedRef.current || requestSeqRef.current !== seq) {
        return;
      }
      setDescriptor(queryDescriptor);

      let rawRows: Record<string, unknown>[];
      let fellBack = false;
      try {
        rawRows = await database.select<Record<string, unknown>[]>(queryDescriptor.sql);
      } catch (primaryError) {
        const message = describeError(primaryError);
        if (queryDescriptor.fallback_sql && shouldFallbackSessionsQuery(message)) {
          rawRows = await database.select<Record<string, unknown>[]>(queryDescriptor.fallback_sql);
          fellBack = true;
        } else {
          throw primaryError;
        }
      }

      if (!mountedRef.current || requestSeqRef.current !== seq) {
        return;
      }
      setSessions(normalizeActiveSessionRows(rawRows, connection.db_type));
      setUsedFallback(fellBack);
      setLastUpdatedAt(new Date().toLocaleTimeString());
    } catch (caught) {
      if (!mountedRef.current || requestSeqRef.current !== seq) {
        return;
      }
      setError(describeError(caught));
    } finally {
      if (mountedRef.current && requestSeqRef.current === seq) {
        setLoading(false);
      }
    }
  }, [connection.db_type, database]);

  useEffect(() => {
    mountedRef.current = true;
    void loadSessions();
    return () => {
      mountedRef.current = false;
    };
  }, [loadSessions]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImeKeyEvent(event) && !pendingAction) {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, pendingAction]);

  const summary = useMemo(() => summarizeActiveSessions(sessions), [sessions]);
  const visibleSessions = useMemo(
    () => filterActiveSessions(sessions, { search, onlyActive, onlyBlocked, hideCurrent }),
    [sessions, search, onlyActive, onlyBlocked, hideCurrent]
  );

  const handleRequestAction = (row: ActiveSessionRow, action: SessionAction) => {
    if (row.isCurrent) {
      return;
    }
    const sql = buildSessionActionSql(connection.db_type, action, row.sessionId);
    if (!sql) {
      return;
    }
    setPendingAction({ row, action, sql });
  };

  const handleConfirmAction = async (target: PendingSessionAction) => {
    setPendingAction(null);
    setActingSessionId(target.row.sessionId);
    setActionNotice(null);
    setError(null);
    try {
      const result = await invoke<TerminateSessionResult>('terminate_database_session', {
        request: {
          connectionId: connection.id,
          action: target.action,
          sessionId: target.row.sessionId
        }
      });
      if (!mountedRef.current) {
        return;
      }
      if (result.applied) {
        setActionNotice({
          kind: 'success',
          text: t('sessions.result.applied', { statement: result.statement })
        });
      } else {
        setActionNotice({
          kind: 'warning',
          text: t('sessions.result.notApplied', { id: target.row.sessionId })
        });
      }
      await loadSessions();
    } catch (caught) {
      if (!mountedRef.current) {
        return;
      }
      setError(describeError(caught));
    } finally {
      if (mountedRef.current) {
        setActingSessionId(null);
      }
    }
  };

  const handleCopySql = async (row: ActiveSessionRow) => {
    if (!row.sqlText) return;
    try {
      await navigator.clipboard.writeText(row.sqlText);
      setCopiedSessionId(row.sessionId);
      setTimeout(() => {
        if (mountedRef.current) {
          setCopiedSessionId((prev) => (prev === row.sessionId ? null : prev));
        }
      }, 1500);
    } catch {
      // 剪贴板不可用时静默降级，用户仍可直接选中文本复制
    }
  };

  const expandedRow = useMemo(
    () => visibleSessions.find((row) => row.sessionId === expandedSessionId) ?? null,
    [visibleSessions, expandedSessionId]
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="active-sessions-title"
      onClick={onClose}
    >
      <div
        className="flex h-[82vh] w-[1080px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel border border-line bg-surface shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        {/* 头部 */}
        <div className="flex items-center justify-between gap-3 border-b border-line bg-surface-sunken px-5 py-3">
          <div className="flex min-w-0 items-center gap-2.5">
            <Activity size={16} className="shrink-0 text-accent" />
            <h2 id="active-sessions-title" className="truncate text-base font-medium text-fg">
              {t('sessions.title')}
            </h2>
            <span className="truncate text-sm text-fg-muted">{connection.name}</span>
            <EnvironmentBadgeTag environment={connection.environment} compact />
          </div>

          <div className="flex shrink-0 items-center gap-2">
            {lastUpdatedAt && (
              <span className="text-xs text-fg-subtle">
                {t('sessions.lastUpdated', { time: lastUpdatedAt })}
              </span>
            )}
            <button
              type="button"
              onClick={() => void loadSessions()}
              disabled={loading}
              className="flex items-center gap-1.5 rounded-control border border-line-strong bg-surface px-3 py-1.5 text-xs font-medium text-fg hover:bg-surface-hover disabled:opacity-50"
            >
              <RefreshCw size={13} className={loading ? 'animate-spin' : ''} />
              <span>{loading ? t('sessions.refreshing') : t('sessions.refresh')}</span>
            </button>
            <button
              type="button"
              onClick={onClose}
              aria-label={t('common.close')}
              className="rounded-control p-1 text-fg-subtle hover:bg-surface-hover hover:text-fg"
            >
              <X size={16} />
            </button>
          </div>
        </div>

        {/* 概览 + 筛选工具条 */}
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-2.5">
          <div className="flex flex-wrap items-center gap-2 text-xs">
            <span className="rounded-control bg-surface-sunken px-2.5 py-1 font-medium text-fg">
              {t('sessions.summary.total', { count: summary.total })}
            </span>
            <span className="rounded-control bg-accent-soft px-2.5 py-1 font-medium text-accent">
              {t('sessions.summary.active', { count: summary.active })}
            </span>
            <span
              className={clsx(
                'rounded-control px-2.5 py-1 font-medium',
                summary.blocked > 0
                  ? 'bg-danger-soft text-danger'
                  : 'bg-surface-sunken text-fg-muted'
              )}
            >
              {t('sessions.summary.blocked', { count: summary.blocked })}
            </span>
            <span className="text-fg-subtle">{t('sessions.manualRefreshNote')}</span>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <div className="flex items-center gap-1.5 rounded-control border border-line bg-surface px-2.5 py-1">
              <Search size={13} className="shrink-0 text-fg-subtle" />
              <input
                type="text"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder={t('sessions.searchPlaceholder')}
                className="w-56 bg-transparent text-xs text-fg outline-none placeholder:text-fg-subtle"
                {...PLAIN_TEXT_INPUT}
              />
              {search && (
                <button
                  type="button"
                  onClick={() => setSearch('')}
                  aria-label={t('explorer.filterClear')}
                  className="text-fg-subtle hover:text-fg"
                >
                  <X size={12} />
                </button>
              )}
            </div>

            <label className="flex cursor-pointer items-center gap-1.5 text-xs text-fg-muted select-none">
              <input
                type="checkbox"
                checked={onlyActive}
                onChange={(event) => setOnlyActive(event.target.checked)}
              />
              <span>{t('sessions.filter.onlyActive')}</span>
            </label>
            <label className="flex cursor-pointer items-center gap-1.5 text-xs text-fg-muted select-none">
              <input
                type="checkbox"
                checked={onlyBlocked}
                onChange={(event) => setOnlyBlocked(event.target.checked)}
              />
              <span>{t('sessions.filter.onlyBlocked')}</span>
            </label>
            <label className="flex cursor-pointer items-center gap-1.5 text-xs text-fg-muted select-none">
              <input
                type="checkbox"
                checked={hideCurrent}
                onChange={(event) => setHideCurrent(event.target.checked)}
              />
              <span>{t('sessions.filter.hideCurrent')}</span>
            </label>
          </div>
        </div>

        {/* 回退提示 / 操作回执 / 错误横幅 */}
        {usedFallback && (
          <div className="border-b border-line bg-surface-sunken px-5 py-1.5 text-xs text-fg-muted">
            {t('sessions.fallbackNotice')}
          </div>
        )}
        {actionNotice && (
          <div
            className={clsx(
              'flex items-center justify-between gap-2 border-b px-5 py-2 text-xs',
              actionNotice.kind === 'success'
                ? 'border-line bg-success-soft text-success'
                : 'border-warning-line bg-warning-soft text-warning'
            )}
          >
            <span className="min-w-0 flex-1 break-all font-mono select-text">{actionNotice.text}</span>
            <button
              type="button"
              onClick={() => setActionNotice(null)}
              aria-label={t('common.close')}
              className="shrink-0 opacity-80 hover:opacity-100"
            >
              <X size={13} />
            </button>
          </div>
        )}
        {error && (
          <div className="flex items-start gap-2 border-b border-danger-line bg-danger-soft px-5 py-2.5 text-xs text-danger">
            <AlertTriangle size={14} className="mt-0.5 shrink-0" />
            <span className="min-w-0 flex-1 break-words select-text">{error}</span>
          </div>
        )}

        {/* 主表格区 */}
        <div className="min-h-0 flex-1 overflow-auto">
          {loading && sessions.length === 0 ? (
            <div className="flex h-full items-center justify-center gap-2 text-sm text-fg-muted">
              <Loader2 size={16} className="animate-spin text-accent" />
              <span>{t('sessions.refreshing')}</span>
            </div>
          ) : visibleSessions.length === 0 ? (
            <div className="flex h-full items-center justify-center p-8 text-sm text-fg-muted">
              {sessions.length === 0 ? t('sessions.empty') : t('sessions.emptyFiltered')}
            </div>
          ) : (
            <table className="w-full border-collapse text-left text-xs">
              <thead className="sticky top-0 z-10 border-b border-line bg-surface-sunken text-fg-muted">
                <tr>
                  <th className="w-28 px-4 py-2 font-medium">{t('sessions.col.id')}</th>
                  <th className="w-40 px-3 py-2 font-medium">{t('sessions.col.userDb')}</th>
                  <th className="w-44 px-3 py-2 font-medium">{t('sessions.col.state')}</th>
                  <th className="w-20 px-3 py-2 text-right font-medium">{t('sessions.col.duration')}</th>
                  <th className="px-3 py-2 font-medium">{t('sessions.col.sql')}</th>
                  <th className="w-36 px-4 py-2 text-right font-medium">{t('sessions.col.actions')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {visibleSessions.map((row) => {
                  const expanded = expandedSessionId === row.sessionId;
                  const busy = actingSessionId === row.sessionId;
                  const canCancel = Boolean(descriptor?.supports_cancel)
                    && !row.isCurrent
                    && Boolean(buildSessionActionSql(connection.db_type, 'cancel', row.sessionId));
                  const canTerminate = Boolean(descriptor?.supports_terminate)
                    && !row.isCurrent
                    && Boolean(buildSessionActionSql(connection.db_type, 'terminate', row.sessionId));

                  return (
                    <tr
                      key={row.sessionId}
                      className={clsx(
                        'align-top hover:bg-surface-hover',
                        row.isBlocked && 'bg-danger-soft/40'
                      )}
                    >
                      <td className="px-4 py-2.5 font-mono text-fg">
                        <div className="flex flex-wrap items-center gap-1.5">
                          <span className="select-text">{row.sessionId}</span>
                          {row.isCurrent && (
                            <span className="rounded-control bg-accent-soft px-1.5 py-0.5 text-[10px] font-medium text-accent">
                              {t('sessions.currentBadge')}
                            </span>
                          )}
                        </div>
                      </td>

                      <td className="px-3 py-2.5 text-fg">
                        <div className="truncate font-medium" title={row.username}>
                          {row.username || '—'}
                        </div>
                        <div className="truncate text-fg-subtle" title={`${row.databaseName} · ${row.clientAddr}`}>
                          {[row.databaseName, row.clientAddr].filter(Boolean).join(' · ') || '—'}
                        </div>
                      </td>

                      <td className="px-3 py-2.5">
                        <div className="flex flex-wrap items-center gap-1">
                          <span
                            className={clsx(
                              'rounded-control px-1.5 py-0.5 font-medium',
                              row.isBlocked
                                ? 'bg-danger-soft text-danger'
                                : row.isActive
                                  ? 'bg-accent-soft text-accent'
                                  : 'bg-surface-sunken text-fg-muted'
                            )}
                          >
                            {row.state || '—'}
                          </span>
                          {row.blockingPid && (
                            <span className="rounded-control bg-danger-soft px-1.5 py-0.5 font-mono font-medium text-danger">
                              {t('sessions.blockedBy', { pid: row.blockingPid })}
                            </span>
                          )}
                        </div>
                        {row.waitEvent && (
                          <div className="mt-1 truncate font-mono text-[11px] text-fg-muted" title={row.waitEvent}>
                            {row.waitEvent}
                          </div>
                        )}
                      </td>

                      <td className="px-3 py-2.5 text-right font-mono text-fg">
                        {formatSessionDuration(row.durationSeconds)}
                      </td>

                      <td className="max-w-0 px-3 py-2.5">
                        {row.sqlText ? (
                          <button
                            type="button"
                            onClick={() => setExpandedSessionId(expanded ? null : row.sessionId)}
                            className="flex w-full items-start gap-1 text-left font-mono text-fg hover:text-accent"
                            title={expanded ? t('sessions.collapseSql') : t('sessions.expandSql')}
                          >
                            {expanded
                              ? <ChevronDown size={13} className="mt-0.5 shrink-0 text-fg-subtle" />
                              : <ChevronRight size={13} className="mt-0.5 shrink-0 text-fg-subtle" />}
                            <span className="truncate">{truncateSqlPreview(row.sqlText)}</span>
                          </button>
                        ) : (
                          <span className="text-fg-subtle">{t('sessions.noSql')}</span>
                        )}
                      </td>

                      <td className="px-4 py-2.5 text-right">
                        {row.isCurrent ? (
                          <span
                            className="text-[11px] text-fg-subtle"
                            title={t('sessions.action.currentDisabled')}
                          >
                            —
                          </span>
                        ) : (
                          <div className="flex items-center justify-end gap-1">
                            {canCancel && (
                              <button
                                type="button"
                                disabled={busy}
                                onClick={() => handleRequestAction(row, 'cancel')}
                                title={t('sessions.action.cancel')}
                                className="inline-flex items-center gap-1 rounded-control border border-line-strong bg-surface px-2 py-1 text-[11px] text-fg-muted hover:bg-surface-hover hover:text-fg disabled:opacity-40"
                              >
                                <StopCircle size={12} />
                                <span>{t('sessions.action.cancel')}</span>
                              </button>
                            )}
                            {canTerminate && (
                              <button
                                type="button"
                                disabled={busy}
                                onClick={() => handleRequestAction(row, 'terminate')}
                                title={t('sessions.action.terminate')}
                                className="inline-flex items-center gap-1 rounded-control border border-danger-line bg-danger-soft px-2 py-1 text-[11px] text-danger hover:opacity-85 disabled:opacity-40"
                              >
                                <XCircle size={12} />
                                <span>{t('sessions.action.terminate')}</span>
                              </button>
                            )}
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>

        {/* 展开的完整 SQL 详情栏 */}
        {expandedRow && expandedRow.sqlText && (
          <div className="border-t border-line bg-surface-sunken px-5 py-3">
            <div className="mb-1.5 flex items-center justify-between gap-2 text-xs">
              <span className="font-mono font-medium text-fg">
                #{expandedRow.sessionId} · {expandedRow.username || '—'}
              </span>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => void handleCopySql(expandedRow)}
                  className="inline-flex items-center gap-1 rounded-control border border-line bg-surface px-2 py-1 text-xs text-fg-muted hover:bg-surface-hover hover:text-fg"
                >
                  {copiedSessionId === expandedRow.sessionId ? <Check size={12} /> : <Copy size={12} />}
                  <span>
                    {copiedSessionId === expandedRow.sessionId
                      ? t('sessions.copiedSql')
                      : t('sessions.copySql')}
                  </span>
                </button>
                <button
                  type="button"
                  onClick={() => setExpandedSessionId(null)}
                  aria-label={t('sessions.collapseSql')}
                  className="rounded-control p-1 text-fg-subtle hover:bg-surface-hover hover:text-fg"
                >
                  <X size={13} />
                </button>
              </div>
            </div>
            <pre className="max-h-36 overflow-auto rounded-control border border-line bg-surface px-3 py-2 font-mono text-xs text-fg select-text whitespace-pre-wrap break-words">
              <HighlightedCode code={expandedRow.sqlText} language="sql" />
            </pre>
          </div>
        )}
      </div>

      {/* 强制二次确认（自带连接名、PROD 红色高危徽标与精确 SQL 预览） */}
      {pendingAction && (
        <DestructiveStatementPrompt
          sql={pendingAction.sql}
          language="sql"
          risk="destructive"
          riskDescription={
            pendingAction.action === 'cancel'
              ? t('sessions.confirm.cancelRisk')
              : t('sessions.confirm.terminateRisk')
          }
          statementCount={1}
          connectionName={connection.name}
          environment={connection.environment}
          databaseLabel={serverLabel(connection)}
          alwaysAsks
          reversibility={{ kind: 'no-transaction' }}
          impacts={[
            pendingAction.action === 'cancel'
              ? t('sessions.confirm.cancelImpact', { id: pendingAction.row.sessionId })
              : t('sessions.confirm.terminateImpact', { id: pendingAction.row.sessionId })
          ]}
          onCancel={() => setPendingAction(null)}
          onConfirm={() => void handleConfirmAction(pendingAction)}
        />
      )}
    </div>
  );
}
