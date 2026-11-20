import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { clsx } from 'clsx';
import {
  AlertTriangle,
  BarChart2,
  BarChart3,
  ChevronLeft,
  ChevronRight,
  Database,
  Download,
  FileSpreadsheet,
  Loader2,
  Play,
  Search,
  Square,
  X
} from 'lucide-react';
import type {
  AnalyticsColumnInput,
  AnalyticsQueryResult,
  AnalyticsSessionSummary,
  ParquetExportSummary,
  ProfileResult
} from '../contracts/analytics';
import type { SerializedResultValue } from '../contracts/resultSet';
import { useLanguageStore } from '../stores/languageStore';
import {
  ANALYTICS_QUERY_LIMIT_OPTIONS,
  buildDuplicateCheckTemplate,
  buildGroupByTemplate,
  buildPivotTemplate,
  defaultParquetFileName,
  localFileBasename,
  quoteDuckIdentifier
} from '../utils/analyticsQuery';
import { filterAndSortProfiles, type ProfileSortMode } from '../utils/analyticsProfile';
import { describeError } from '../utils/describeError';
import { GRID_PAGE_SIZE_OPTIONS } from '../utils/gridPagination';
import { formatResultValueOneLine } from '../utils/resultValues';
import { isImeKeyEvent, matchesShortcut, SHORTCUTS } from '../utils/shortcuts';
import { ColumnProfileCard } from './DataProfileDialog';
import { ExportResultDialog } from './ExportResultDialog';
import { PLAIN_TEXT_INPUT } from './FormControls';
import { ResultChartDialog } from './ResultChartDialog';

export type LocalAnalyticsSource =
  | {
      readonly kind: 'resultSet';
      readonly title: string;
      readonly columns: readonly AnalyticsColumnInput[];
      readonly rows: ReadonlyArray<readonly SerializedResultValue[]>;
    }
  | {
      readonly kind: 'jsonDocuments';
      readonly title: string;
      readonly documents: ReadonlyArray<Record<string, unknown>>;
    }
  | {
      readonly kind: 'localFile';
      readonly filePath: string;
    };

type WorkbenchTab = 'sql' | 'profile';

interface LocalAnalyticsDialogProps {
  source: LocalAnalyticsSource;
  onClose: () => void;
}

const DEFAULT_PREVIEW_SQL = 'SELECT * FROM result_set LIMIT 200';

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${bytes} B`;
}

export function LocalAnalyticsDialog({ source, onClose }: LocalAnalyticsDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const [tab, setTab] = useState<WorkbenchTab>('sql');
  const [loadingSession, setLoadingSession] = useState(true);
  const [sessionError, setSessionError] = useState<string | null>(null);
  const [summary, setSummary] = useState<AnalyticsSessionSummary | null>(null);

  const [sql, setSql] = useState(DEFAULT_PREVIEW_SQL);
  const [lastExecutedSql, setLastExecutedSql] = useState<string>(DEFAULT_PREVIEW_SQL);
  const [rowLimit, setRowLimit] = useState<number>(1000);
  const [running, setRunning] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [queryResult, setQueryResult] = useState<AnalyticsQueryResult | null>(null);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState<number>(GRID_PAGE_SIZE_OPTIONS[1]);

  const [profile, setProfile] = useState<ProfileResult | null>(null);
  const [profileLoading, setProfileLoading] = useState(false);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [profileSearch, setProfileSearch] = useState('');
  const [profileSort, setProfileSort] = useState<ProfileSortMode>('order');
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  const [exportingParquet, setExportingParquet] = useState(false);
  const [parquetNotice, setParquetNotice] = useState<string | null>(null);
  const [showChart, setShowChart] = useState(false);
  const [showExport, setShowExport] = useState(false);

  const sessionIdRef = useRef<string | null>(null);
  const mountedRef = useRef(true);
  const sqlInputRef = useRef<HTMLTextAreaElement | null>(null);

  const titleLabel =
    source.kind === 'localFile' ? localFileBasename(source.filePath) : source.title;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      const id = sessionIdRef.current;
      sessionIdRef.current = null;
      if (id) {
        void invoke('analytics_close', { analyticsId: id }).catch(() => {});
      }
    };
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImeKeyEvent(event) && !showChart && !showExport) {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, showChart, showExport]);

  const runSql = useCallback(
    async (targetId: string, statement: string, limit: number) => {
      setRunning(true);
      setQueryError(null);
      setParquetNotice(null);
      try {
        const res = await invoke<AnalyticsQueryResult>('analytics_query', {
          analyticsId: targetId,
          sql: statement,
          limit,
          timeoutSeconds: 30
        });
        if (!mountedRef.current || sessionIdRef.current !== targetId) {
          return;
        }
        setQueryResult(res);
        setLastExecutedSql(statement);
        setPage(1);
      } catch (err) {
        if (!mountedRef.current || sessionIdRef.current !== targetId) {
          return;
        }
        setQueryError(describeError(err));
      } finally {
        if (mountedRef.current && sessionIdRef.current === targetId) {
          setRunning(false);
        }
      }
    },
    []
  );

  useEffect(() => {
    let stale = false;
    setLoadingSession(true);
    setSessionError(null);
    setSummary(null);
    setQueryResult(null);
    setProfile(null);
    setQueryError(null);
    setProfileError(null);

    const init = async () => {
      const prev = sessionIdRef.current;
      sessionIdRef.current = null;
      if (prev) {
        void invoke('analytics_close', { analyticsId: prev }).catch(() => {});
      }
      try {
        const loaded =
          source.kind === 'localFile'
            ? await invoke<AnalyticsSessionSummary>('analytics_open_file', {
                path: source.filePath
              })
            : source.kind === 'resultSet'
              ? await invoke<AnalyticsSessionSummary>('analytics_load_result', {
                  columns: source.columns,
                  rows: source.rows
                })
              : await invoke<AnalyticsSessionSummary>('analytics_load_json_docs', {
                  documents: source.documents
                });

        if (stale || !mountedRef.current) {
          void invoke('analytics_close', { analyticsId: loaded.analyticsId }).catch(() => {});
          return;
        }

        sessionIdRef.current = loaded.analyticsId;
        setSummary(loaded);
        setLoadingSession(false);
        await runSql(loaded.analyticsId, DEFAULT_PREVIEW_SQL, 1000);
      } catch (err) {
        if (stale || !mountedRef.current) {
          return;
        }
        setSessionError(describeError(err));
        setLoadingSession(false);
      }
    };

    void init();
    return () => {
      stale = true;
    };
  }, [source, runSql]);

  const loadProfile = useCallback(async () => {
    const id = sessionIdRef.current;
    if (!id || profileLoading) {
      return;
    }
    setProfileLoading(true);
    setProfileError(null);
    try {
      const result = await invoke<ProfileResult>('analytics_profile', { analyticsId: id });
      if (mountedRef.current && sessionIdRef.current === id) {
        setProfile(result);
      }
    } catch (err) {
      if (mountedRef.current && sessionIdRef.current === id) {
        setProfileError(describeError(err));
      }
    } finally {
      if (mountedRef.current && sessionIdRef.current === id) {
        setProfileLoading(false);
      }
    }
  }, [profileLoading]);

  const handleSelectTab = (nextTab: WorkbenchTab) => {
    setTab(nextTab);
    if (nextTab === 'profile' && !profile && !profileLoading && sessionIdRef.current) {
      void loadProfile();
    }
  };

  const handleRunCurrentSql = () => {
    const id = sessionIdRef.current;
    if (!id || running) {
      return;
    }
    void runSql(id, sql, rowLimit);
  };

  const handleCancel = () => {
    const id = sessionIdRef.current;
    if (id) {
      void invoke('analytics_cancel', { analyticsId: id }).catch(() => {});
    }
  };

  const handleApplyTemplate = (statement: string) => {
    setSql(statement);
    const id = sessionIdRef.current;
    if (id && !running) {
      void runSql(id, statement, rowLimit);
    }
  };

  const handleInsertColumn = (columnName: string) => {
    const quoted = quoteDuckIdentifier(columnName);
    const el = sqlInputRef.current;
    if (!el) {
      setSql((prev) => `${prev} ${quoted}`);
      return;
    }
    const start = el.selectionStart ?? sql.length;
    const end = el.selectionEnd ?? sql.length;
    const next = `${sql.slice(0, start)}${quoted}${sql.slice(end)}`;
    setSql(next);
    window.requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(start + quoted.length, start + quoted.length);
    });
  };

  const handleRunFromProfile = (filterSql: string) => {
    setTab('sql');
    setSql(filterSql);
    const id = sessionIdRef.current;
    if (id) {
      void runSql(id, filterSql, rowLimit);
    }
  };

  const handleExportParquet = async () => {
    const id = sessionIdRef.current;
    if (!id || exportingParquet) {
      return;
    }
    const chosen = await save({
      defaultPath: defaultParquetFileName(titleLabel),
      filters: [{ name: t('analytics.parquetFilterName'), extensions: ['parquet'] }]
    });
    if (!chosen) {
      return;
    }
    setExportingParquet(true);
    setQueryError(null);
    setParquetNotice(null);
    try {
      const res = await invoke<ParquetExportSummary>('analytics_export_parquet', {
        analyticsId: id,
        sql: lastExecutedSql,
        outputPath: chosen
      });
      if (mountedRef.current) {
        setParquetNotice(
          t('analytics.exportParquetSuccess', {
            count: res.rowCount,
            bytes: formatBytes(res.bytes)
          })
        );
      }
    } catch (err) {
      if (mountedRef.current) {
        setQueryError(describeError(err));
      }
    } finally {
      if (mountedRef.current) {
        setExportingParquet(false);
      }
    }
  };

  const handleCopySql = useCallback((key: string, text: string) => {
    navigator.clipboard
      .writeText(text)
      .then(() => {
        setCopiedKey(key);
        window.setTimeout(() => {
          setCopiedKey((current) => (current === key ? null : current));
        }, 1400);
      })
      .catch(() => {});
  }, []);

  const totalRows = queryResult?.rows.length ?? 0;
  const totalPages = Math.max(1, Math.ceil(totalRows / pageSize));
  const safePage = Math.min(page, totalPages);
  const pageStart = (safePage - 1) * pageSize;
  const pageRows = useMemo(
    () => (queryResult ? queryResult.rows.slice(pageStart, pageStart + pageSize) : []),
    [queryResult, pageStart, pageSize]
  );

  const visibleProfiles = useMemo(
    () => filterAndSortProfiles(profile?.columns ?? [], profileSearch, profileSort),
    [profile, profileSearch, profileSort]
  );
  const hasPresenceInfo = useMemo(
    () => (profile?.columns ?? []).some((col) => col.presentRatio !== undefined),
    [profile]
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="local-analytics-title"
      onClick={onClose}
    >
      <div
        className="flex h-[88vh] w-[1120px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        {/* Header */}
        <div className="flex shrink-0 items-center justify-between border-b border-line px-5 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <Database size={16} className="shrink-0 text-accent" />
            <h3 id="local-analytics-title" className="truncate text-sm font-semibold text-fg">
              {t('analytics.title')} · {titleLabel}
            </h3>
            {summary && (
              <span className="shrink-0 rounded bg-surface-sunken px-2 py-0.5 font-mono text-[11px] text-fg-muted">
                {summary.rowCount.toLocaleString()} × {summary.columnCount}
                {summary.fileKind ? ` · ${summary.fileKind.toUpperCase()}` : ''}
              </span>
            )}
          </div>
          <div className="flex items-center gap-3">
            {summary && (
              <div className="flex overflow-hidden rounded-control border border-line text-xs" role="tablist">
                <button
                  type="button"
                  role="tab"
                  aria-selected={tab === 'sql'}
                  onClick={() => handleSelectTab('sql')}
                  className={clsx(
                    'px-3 py-1 transition-colors',
                    tab === 'sql'
                      ? 'bg-accent-soft font-medium text-accent'
                      : 'text-fg-muted hover:bg-surface-hover'
                  )}
                >
                  {t('analytics.tab.sql')}
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={tab === 'profile'}
                  onClick={() => handleSelectTab('profile')}
                  className={clsx(
                    'flex items-center gap-1 border-l border-line px-3 py-1 transition-colors',
                    tab === 'profile'
                      ? 'bg-accent-soft font-medium text-accent'
                      : 'text-fg-muted hover:bg-surface-hover'
                  )}
                >
                  <BarChart2 size={12} />
                  <span>{t('analytics.tab.profile')}</span>
                </button>
              </div>
            )}
            <button
              type="button"
              onClick={onClose}
              aria-label={t('common.close')}
              className="rounded-control p-1 text-fg-muted transition-colors hover:bg-surface-hover"
            >
              <X size={16} />
            </button>
          </div>
        </div>

        {/* Truncated banner */}
        {summary?.truncated && (
          <div className="flex shrink-0 items-center gap-2 border-b border-warning-line bg-warning-soft px-5 py-2 text-xs text-warning">
            <AlertTriangle size={14} className="shrink-0" />
            <span>{t('profile.truncatedNotice', { count: summary.rowCount })}</span>
          </div>
        )}

        {/* Loading / Session error */}
        {loadingSession ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-3">
            <Loader2 size={24} className="animate-spin text-accent" />
            <p className="text-sm text-fg-muted">{t('analytics.loadingSession')}</p>
          </div>
        ) : sessionError || !summary ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6">
            <p className="max-w-xl text-center text-sm text-danger">
              {sessionError ?? t('profile.empty')}
            </p>
          </div>
        ) : tab === 'sql' ? (
          <div className="flex min-h-0 flex-1 flex-col">
            {/* SQL Editor & templates */}
            <div className="shrink-0 space-y-2 border-b border-line bg-surface p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="text-xs text-fg-subtle">{t('analytics.tableHint')}</span>
                <div className="flex flex-wrap items-center gap-1.5">
                  <button
                    type="button"
                    onClick={() => handleApplyTemplate(buildGroupByTemplate(summary.columns))}
                    className="rounded-control border border-line px-2 py-0.5 text-xs text-fg-muted hover:bg-surface-hover hover:text-fg"
                  >
                    {t('analytics.template.groupBy')}
                  </button>
                  <button
                    type="button"
                    onClick={() => handleApplyTemplate(buildPivotTemplate(summary.columns))}
                    className="rounded-control border border-line px-2 py-0.5 text-xs text-fg-muted hover:bg-surface-hover hover:text-fg"
                  >
                    {t('analytics.template.pivot')}
                  </button>
                  <button
                    type="button"
                    onClick={() => handleApplyTemplate(buildDuplicateCheckTemplate(summary.columns))}
                    className="rounded-control border border-line px-2 py-0.5 text-xs text-fg-muted hover:bg-surface-hover hover:text-fg"
                  >
                    {t('analytics.template.duplicates')}
                  </button>
                </div>
              </div>

              {/* Column chips */}
              {summary.columns.length > 0 && (
                <div className="flex max-h-14 flex-wrap gap-1 overflow-y-auto">
                  {summary.columns.map((col) => (
                    <button
                      key={col.name}
                      type="button"
                      onClick={() => handleInsertColumn(col.name)}
                      title={col.duckdbType}
                      className="rounded bg-surface-sunken px-1.5 py-0.5 font-mono text-[11px] text-fg-muted hover:bg-surface-hover hover:text-fg"
                    >
                      {col.name}
                    </button>
                  ))}
                </div>
              )}

              <textarea
                {...PLAIN_TEXT_INPUT}
                ref={sqlInputRef}
                value={sql}
                onChange={(event) => setSql(event.target.value)}
                onKeyDown={(event) => {
                  if (matchesShortcut(event.nativeEvent, SHORTCUTS.runCurrent) && !isImeKeyEvent(event.nativeEvent)) {
                    event.preventDefault();
                    handleRunCurrentSql();
                  }
                }}
                rows={4}
                className="w-full resize-y rounded-control border border-line-strong bg-surface-sunken p-2.5 font-mono text-xs text-fg focus:border-accent focus:outline-none"
              />

              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  {running ? (
                    <button
                      type="button"
                      onClick={handleCancel}
                      className="flex items-center gap-1.5 rounded-control bg-danger px-3 py-1.5 text-xs font-medium text-white hover:opacity-90"
                    >
                      <Square size={12} />
                      <span>{t('analytics.cancel')}</span>
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={handleRunCurrentSql}
                      disabled={sql.trim() === ''}
                      className="flex items-center gap-1.5 rounded-control bg-accent px-3 py-1.5 text-xs font-medium text-white hover:opacity-90 disabled:opacity-50"
                    >
                      <Play size={12} />
                      <span>{t('analytics.run')}</span>
                    </button>
                  )}
                  <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                    <span>{t('analytics.limitLabel')}</span>
                    <select
                      value={rowLimit}
                      onChange={(event) => setRowLimit(Number(event.target.value))}
                      className="rounded-control border border-line-strong bg-surface px-2 py-1 text-xs text-fg"
                    >
                      {ANALYTICS_QUERY_LIMIT_OPTIONS.map((opt) => (
                        <option key={opt} value={opt}>
                          {opt.toLocaleString()}
                        </option>
                      ))}
                    </select>
                  </label>
                  {queryResult && (
                    <span className="text-xs text-fg-muted">
                      {t('analytics.resultMeta', {
                        rows: queryResult.rowCount.toLocaleString(),
                        columns: queryResult.columns.length,
                        ms: queryResult.elapsedMs
                      })}
                      {queryResult.truncated && (
                        <span className="ml-2 text-warning">{t('analytics.resultTruncated')}</span>
                      )}
                    </span>
                  )}
                </div>

                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setShowChart(true)}
                    disabled={!queryResult || queryResult.rows.length === 0}
                    className="flex items-center gap-1 rounded-control border border-line-strong px-2.5 py-1 text-xs text-fg hover:bg-surface-hover disabled:opacity-50"
                  >
                    <BarChart3 size={13} />
                    <span>{t('result.chart')}</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => setShowExport(true)}
                    disabled={!queryResult || queryResult.rows.length === 0}
                    className="flex items-center gap-1 rounded-control border border-line-strong px-2.5 py-1 text-xs text-fg hover:bg-surface-hover disabled:opacity-50"
                  >
                    <Download size={13} />
                    <span>{t('result.export')}</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleExportParquet()}
                    disabled={exportingParquet || running}
                    className="flex items-center gap-1 rounded-control border border-line-strong px-2.5 py-1 text-xs text-fg hover:bg-surface-hover disabled:opacity-50"
                  >
                    <FileSpreadsheet size={13} />
                    <span>{t('analytics.exportParquet')}</span>
                  </button>
                </div>
              </div>

              {queryError && (
                <div className="rounded-control border border-danger-line bg-danger-soft px-3 py-2 text-xs text-danger">
                  {queryError}
                </div>
              )}
              {parquetNotice && (
                <div className="rounded-control border border-success-line bg-success-soft px-3 py-2 text-xs text-success">
                  {parquetNotice}
                </div>
              )}
            </div>

            {/* Result grid */}
            <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
              {!queryResult ? (
                <div className="flex flex-1 items-center justify-center text-xs text-fg-muted">
                  {running ? <Loader2 size={18} className="animate-spin text-accent" /> : null}
                </div>
              ) : queryResult.rows.length === 0 ? (
                <div className="flex flex-1 items-center justify-center text-sm text-fg-muted">
                  {t('analytics.emptyResult')}
                </div>
              ) : (
                <>
                  <div className="flex-1 overflow-auto">
                    <table className="w-full border-collapse text-left font-mono text-xs">
                      <thead className="sticky top-0 z-10 bg-surface-sunken">
                        <tr className="border-b border-line">
                          <th className="w-12 border-r border-line px-2 py-1.5 text-right text-[10px] font-medium text-fg-subtle">
                            #
                          </th>
                          {queryResult.columns.map((col, idx) => (
                            <th
                              key={`${col}-${idx}`}
                              className="max-w-[20rem] border-r border-line px-2.5 py-1.5 font-medium text-fg"
                            >
                              <div className="truncate" title={col}>
                                {col}
                              </div>
                              {queryResult.columnMetadata[idx] && (
                                <div className="truncate text-[10px] font-normal text-fg-subtle">
                                  {queryResult.columnMetadata[idx].database_type}
                                </div>
                              )}
                            </th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {pageRows.map((row, rowIdx) => (
                          <tr
                            key={pageStart + rowIdx}
                            className="border-b border-line hover:bg-surface-hover"
                          >
                            <td className="border-r border-line px-2 py-1 text-right text-[10px] text-fg-subtle">
                              {pageStart + rowIdx + 1}
                            </td>
                            {row.map((cell, cellIdx) => {
                              const isNull = cell === null || cell === undefined;
                              const text = isNull ? 'NULL' : formatResultValueOneLine(cell);
                              return (
                                <td
                                  key={cellIdx}
                                  title={text}
                                  className={clsx(
                                    'max-w-[20rem] truncate border-r border-line px-2.5 py-1',
                                    isNull ? 'italic text-fg-subtle' : 'text-fg'
                                  )}
                                >
                                  {text}
                                </td>
                              );
                            })}
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  {/* Pagination bar */}
                  <div className="flex shrink-0 items-center justify-between border-t border-line bg-surface px-4 py-2 text-xs text-fg-muted">
                    <span>
                      {t('result.range', {
                        from: pageStart + 1,
                        to: Math.min(pageStart + pageSize, totalRows),
                        total: totalRows
                      })}
                    </span>
                    <div className="flex items-center gap-2">
                      <select
                        value={pageSize}
                        onChange={(event) => {
                          setPageSize(Number(event.target.value));
                          setPage(1);
                        }}
                        className="rounded-control border border-line-strong bg-surface px-2 py-0.5 text-xs text-fg"
                      >
                        {GRID_PAGE_SIZE_OPTIONS.map((size) => (
                          <option key={size} value={size}>
                            {t('result.pageSizeOption', { size })}
                          </option>
                        ))}
                      </select>
                      <button
                        type="button"
                        onClick={() => setPage((p) => Math.max(1, p - 1))}
                        disabled={safePage <= 1}
                        className="rounded-control border border-line p-1 hover:bg-surface-hover disabled:opacity-40"
                      >
                        <ChevronLeft size={13} />
                      </button>
                      <span>{t('result.pageOf', { page: safePage, total: totalPages })}</span>
                      <button
                        type="button"
                        onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                        disabled={safePage >= totalPages}
                        className="rounded-control border border-line p-1 hover:bg-surface-hover disabled:opacity-40"
                      >
                        <ChevronRight size={13} />
                      </button>
                    </div>
                  </div>
                </>
              )}
            </div>
          </div>
        ) : (
          /* Profile tab */
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            {profile && !profileLoading && (
              <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-line px-5 py-2.5">
                <div className="relative max-w-xs flex-1 min-w-[200px]">
                  <Search
                    size={13}
                    className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-fg-subtle"
                  />
                  <input
                    {...PLAIN_TEXT_INPUT}
                    type="text"
                    value={profileSearch}
                    onChange={(event) => setProfileSearch(event.target.value)}
                    placeholder={t('profile.searchPlaceholder')}
                    className="w-full rounded-control border border-line-strong bg-surface py-1 pl-8 pr-2.5 text-xs text-fg placeholder:text-fg-subtle focus:border-accent focus:outline-none"
                  />
                </div>
                <select
                  value={profileSort}
                  onChange={(event) => setProfileSort(event.target.value as ProfileSortMode)}
                  className="rounded-control border border-line-strong bg-surface px-2.5 py-1 text-xs text-fg"
                >
                  {(['order', 'nulls', 'distinct', 'drift'] as const)
                    .filter((m) => m !== 'drift' || hasPresenceInfo)
                    .map((mode) => (
                      <option key={mode} value={mode}>
                        {t(`profile.sort.${mode}` as const)}
                      </option>
                    ))}
                </select>
              </div>
            )}
            <div className="flex-1 overflow-y-auto px-5 py-4">
              {profileLoading ? (
                <div className="flex flex-col items-center justify-center gap-3 py-12">
                  <Loader2 size={24} className="animate-spin text-accent" />
                  <p className="text-sm text-fg-muted">
                    {t('profile.loading', { count: summary.rowCount })}
                  </p>
                </div>
              ) : profileError ? (
                <p className="py-10 text-center text-sm text-danger">{profileError}</p>
              ) : visibleProfiles.length === 0 ? (
                <p className="py-8 text-center text-sm text-fg-muted">{t('profile.noMatchingColumns')}</p>
              ) : (
                <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                  {visibleProfiles.map((col) => (
                    <ColumnProfileCard
                      key={col.name}
                      column={col}
                      copiedKey={copiedKey}
                      onCopySql={handleCopySql}
                      onRunFilterSql={handleRunFromProfile}
                    />
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {showChart && queryResult && (
        <ResultChartDialog
          columns={queryResult.columns}
          rows={queryResult.rows}
          onClose={() => setShowChart(false)}
        />
      )}

      {showExport && queryResult && (
        <ExportResultDialog
          columns={[...queryResult.columns]}
          rows={queryResult.rows.map((r) => [...r])}
          sourceName={titleLabel}
          truncated={queryResult.truncated}
          onClose={() => setShowExport(false)}
        />
      )}
    </div>
  );
}
