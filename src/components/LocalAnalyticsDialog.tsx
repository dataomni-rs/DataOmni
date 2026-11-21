import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import CodeMirror from '@uiw/react-codemirror';
import type { Completion } from '@codemirror/autocomplete';
import { EditorState } from '@codemirror/state';
import { PostgreSQL, sql as sqlLanguage, type SQLNamespace } from '@codemirror/lang-sql';
import { oneDark } from '@codemirror/theme-one-dark';
import type { EditorView } from '@codemirror/view';
import { clsx } from 'clsx';
import {
  AlertTriangle,
  BarChart2,
  BarChart3,
  ChevronLeft,
  ChevronRight,
  Database,
  Download,
  FilePlus2,
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
  AttachedTableSummary,
  ParquetExportSummary,
  ProfileResult
} from '../contracts/analytics';
import type { SerializedResultValue } from '../contracts/resultSet';
import { useLanguageStore } from '../stores/languageStore';
import { useQueryStore } from '../stores/queryStore';
import { useThemeStore } from '../stores/themeStore';
import {
  ANALYTICS_QUERY_LIMIT_OPTIONS,
  buildDuplicateCheckTemplate,
  buildExceptDiffTemplate,
  buildGroupByTemplate,
  buildLeftJoinTemplate,
  buildPivotTemplate,
  defaultParquetFileName,
  LOCAL_DATA_FILE_EXTENSIONS,
  localFileBasename,
  MAX_ATTACHED_TABLES,
  nextAttachTableName,
  quoteDuckIdentifier
} from '../utils/analyticsQuery';
import { filterAndSortProfiles, type ProfileSortMode } from '../utils/analyticsProfile';
import { describeError } from '../utils/describeError';
import { editorPhrases } from '../utils/editorPhrases';
import { appEditorTheme } from '../utils/editorTheme';
import { GRID_PAGE_SIZE_OPTIONS } from '../utils/gridPagination';
import { formatResultValueOneLine } from '../utils/resultValues';
import { runShortcutKeymap, type RunShortcutHandlers } from '../utils/runShortcutKeymap';
import { isImeKeyEvent } from '../utils/shortcuts';
import type { SqlIdentifierDialect } from '../utils/sqlIdentifiers';
import {
  DEFAULT_TABLE_SAMPLE_SIZE,
  TABLE_SAMPLE_SIZE_OPTIONS
} from '../utils/tableSampleQuery';
import { ColumnProfileCard, type TableSampleConfig } from './DataProfileDialog';
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
  tableSample?: TableSampleConfig;
  sourceDialect?: SqlIdentifierDialect;
  onApplyTableFilter?: (columnName: string, operator: 'eq' | 'is-null', value: string) => void;
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

export function LocalAnalyticsDialog({
  source,
  tableSample,
  sourceDialect,
  onApplyTableFilter,
  onClose
}: LocalAnalyticsDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const [tab, setTab] = useState<WorkbenchTab>('sql');
  const [scope, setScope] = useState<'current' | 'sample'>('current');
  const [sampleSize, setSampleSize] = useState<number>(DEFAULT_TABLE_SAMPLE_SIZE);
  const [sampling, setSampling] = useState(false);
  const [sampledTruncated, setSampledTruncated] = useState(false);
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
  const [attachedTables, setAttachedTables] = useState<
    Array<AttachedTableSummary & { label: string }>
  >([]);
  const [attaching, setAttaching] = useState(false);
  const [showChart, setShowChart] = useState(false);
  const [showExport, setShowExport] = useState(false);

  const queryDocuments = useQueryStore((state) => state.documents);
  const attachableQueryResults = useMemo(() => {
    const items: Array<{
      id: string;
      label: string;
      columns: AnalyticsColumnInput[];
      rows: SerializedResultValue[][];
    }> = [];
    for (const [docId, doc] of Object.entries(queryDocuments)) {
      for (const stmt of doc.statements) {
        if (stmt.result && stmt.result.columns.length > 0 && stmt.result.rows.length > 0) {
          const preview = stmt.sql.replace(/\s+/g, ' ').trim().slice(0, 40);
          items.push({
            id: `${docId}:${stmt.id}`,
            label: `${preview || stmt.id} (${stmt.result.rows.length})`,
            columns: stmt.result.columns.map((name, idx) => ({
              name,
              logicalType: stmt.result?.column_metadata?.[idx]?.logical_type ?? null,
              databaseType: stmt.result?.column_metadata?.[idx]?.database_type ?? null
            })),
            rows: stmt.result.rows
          });
        }
      }
    }
    return items;
  }, [queryDocuments]);

  const resolvedTheme = useThemeStore((state) => state.resolved);
  const sessionIdRef = useRef<string | null>(null);
  const sampleExecutionIdRef = useRef<string | null>(null);
  const tabRef = useRef<WorkbenchTab>('sql');
  const mountedRef = useRef(true);
  const editorViewRef = useRef<EditorView | null>(null);
  const runShortcutsRef = useRef<RunShortcutHandlers>({ runCurrent: () => {}, runAll: () => {} });

  tabRef.current = tab;

  const currentRowCount =
    source.kind === 'resultSet'
      ? source.rows.length
      : source.kind === 'jsonDocuments'
        ? source.documents.length
        : (summary?.rowCount ?? 0);

  const titleLabel =
    source.kind === 'localFile' ? localFileBasename(source.filePath) : source.title;

  const editorExtensions = useMemo(() => {
    const toCompletions = (cols: readonly { name: string; duckdbType: string }[]): Completion[] =>
      cols.map((col, index) => {
        const quoted = quoteDuckIdentifier(col.name);
        const needsQuote = !/^[a-z_][a-z0-9_]*$/.test(col.name);
        return {
          label: col.name,
          ...(needsQuote ? { apply: quoted } : {}),
          type: 'property',
          detail: col.duckdbType,
          boost: Math.max(99 - index, -99)
        };
      });
    const schema: Record<string, { self: Completion; children: Completion[] }> = {
      result_set: {
        self: { label: 'result_set', type: 'type', detail: 'DuckDB' },
        children: toCompletions(summary?.columns ?? [])
      }
    };
    for (const attached of attachedTables) {
      schema[attached.tableName] = {
        self: { label: attached.tableName, type: 'type', detail: attached.label },
        children: toCompletions(attached.columns)
      };
    }
    return [
      runShortcutKeymap(runShortcutsRef),
      sqlLanguage({
        dialect: PostgreSQL,
        schema: schema as SQLNamespace,
        defaultTable: 'result_set',
        upperCaseKeywords: true
      }),
      EditorState.phrases.of(editorPhrases(t)),
      appEditorTheme
    ];
  }, [attachedTables, summary?.columns, t]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      const sampleExecId = sampleExecutionIdRef.current;
      sampleExecutionIdRef.current = null;
      if (sampleExecId && tableSample) {
        tableSample.cancelSample(sampleExecId);
      }
      const id = sessionIdRef.current;
      sessionIdRef.current = null;
      if (id) {
        void invoke('analytics_close', { analyticsId: id }).catch(() => {});
      }
    };
  }, [tableSample]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) {
        return;
      }
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
    setSampling(scope === 'sample' && Boolean(tableSample));
    setSampledTruncated(false);
    setSessionError(null);
    setSummary(null);
    setAttachedTables([]);
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
        let loaded: AnalyticsSessionSummary;
        if (scope === 'sample' && tableSample) {
          const execId = crypto.randomUUID();
          sampleExecutionIdRef.current = execId;
          const sampled = await tableSample.fetchSample(sampleSize, execId);
          sampleExecutionIdRef.current = null;
          if (stale || !mountedRef.current) {
            return;
          }
          setSampling(false);
          setSampledTruncated(sampled.truncated);
          loaded = await invoke<AnalyticsSessionSummary>('analytics_load_result', {
            columns: sampled.columns,
            rows: sampled.rows
          });
        } else {
          loaded =
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
        }

        if (stale || !mountedRef.current) {
          void invoke('analytics_close', { analyticsId: loaded.analyticsId }).catch(() => {});
          return;
        }

        sessionIdRef.current = loaded.analyticsId;
        setSummary(loaded);
        setLoadingSession(false);
        if (tabRef.current === 'profile') {
          setProfileLoading(true);
          try {
            const profiled = await invoke<ProfileResult>('analytics_profile', {
              analyticsId: loaded.analyticsId
            });
            if (!stale && mountedRef.current && sessionIdRef.current === loaded.analyticsId) {
              setProfile(profiled);
            }
          } catch (err) {
            if (!stale && mountedRef.current && sessionIdRef.current === loaded.analyticsId) {
              setProfileError(describeError(err));
            }
          } finally {
            if (!stale && mountedRef.current && sessionIdRef.current === loaded.analyticsId) {
              setProfileLoading(false);
            }
          }
        }
        await runSql(loaded.analyticsId, DEFAULT_PREVIEW_SQL, 1000);
      } catch (err) {
        if (stale || !mountedRef.current) {
          return;
        }
        setSampling(false);
        setSessionError(describeError(err));
        setLoadingSession(false);
      }
    };

    void init();
    return () => {
      stale = true;
      const sampleExecId = sampleExecutionIdRef.current;
      sampleExecutionIdRef.current = null;
      if (sampleExecId && tableSample) {
        tableSample.cancelSample(sampleExecId);
      }
    };
  }, [source, scope, sampleSize, tableSample, runSql]);

  const handleCancelSample = () => {
    const sampleExecId = sampleExecutionIdRef.current;
    if (sampleExecId && tableSample) {
      sampleExecutionIdRef.current = null;
      tableSample.cancelSample(sampleExecId);
    }
    setScope('current');
  };

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

  const handleAttachFile = async () => {
    const id = sessionIdRef.current;
    if (!id || attaching || attachedTables.length >= MAX_ATTACHED_TABLES) {
      return;
    }
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: t('analytics.fileFilterName'),
            extensions: [...LOCAL_DATA_FILE_EXTENSIONS]
          }
        ]
      });
      if (typeof selected !== 'string' || selected.trim().length === 0) {
        return;
      }
      setAttaching(true);
      setQueryError(null);
      const tableName = nextAttachTableName(attachedTables.map((item) => item.tableName));
      const attached = await invoke<AttachedTableSummary>('analytics_attach_file', {
        analyticsId: id,
        tableName,
        path: selected
      });
      if (!mountedRef.current || sessionIdRef.current !== id) {
        return;
      }
      setAttachedTables((current) => [
        ...current,
        { ...attached, label: localFileBasename(selected) }
      ]);
    } catch (err) {
      if (mountedRef.current && sessionIdRef.current === id) {
        setQueryError(describeError(err));
      }
    } finally {
      if (mountedRef.current && sessionIdRef.current === id) {
        setAttaching(false);
      }
    }
  };

  const handleAttachQueryResult = async (candidateId: string) => {
    const id = sessionIdRef.current;
    const candidate = attachableQueryResults.find((item) => item.id === candidateId);
    if (!id || !candidate || attaching || attachedTables.length >= MAX_ATTACHED_TABLES) {
      return;
    }
    setAttaching(true);
    setQueryError(null);
    try {
      const tableName = nextAttachTableName(attachedTables.map((item) => item.tableName));
      const attached = await invoke<AttachedTableSummary>('analytics_attach_result', {
        analyticsId: id,
        tableName,
        columns: candidate.columns,
        rows: candidate.rows
      });
      if (!mountedRef.current || sessionIdRef.current !== id) {
        return;
      }
      setAttachedTables((current) => [
        ...current,
        { ...attached, label: candidate.label }
      ]);
    } catch (err) {
      if (mountedRef.current && sessionIdRef.current === id) {
        setQueryError(describeError(err));
      }
    } finally {
      if (mountedRef.current && sessionIdRef.current === id) {
        setAttaching(false);
      }
    }
  };

  const handleRunCurrentSql = () => {
    const id = sessionIdRef.current;
    if (!id || running) {
      return;
    }
    void runSql(id, sql, rowLimit);
  };

  runShortcutsRef.current = {
    runCurrent: handleRunCurrentSql,
    runAll: handleRunCurrentSql
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

  const handleInsertSqlFragment = (fragment: string) => {
    const view = editorViewRef.current;
    if (!view) {
      setSql((prev) => `${prev} ${fragment}`);
      return;
    }
    const { from, to } = view.state.selection.main;
    view.dispatch({
      changes: { from, to, insert: fragment },
      selection: { anchor: from + fragment.length }
    });
    view.focus();
  };

  const handleInsertColumn = (columnName: string) => {
    handleInsertSqlFragment(quoteDuckIdentifier(columnName));
  };

  const handleInsertQualifiedColumn = (tableName: string, columnName: string) => {
    handleInsertSqlFragment(`${quoteDuckIdentifier(tableName)}.${quoteDuckIdentifier(columnName)}`);
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
    setQueryError(null);
    setParquetNotice(null);
    try {
      const chosen = await save({
        defaultPath: defaultParquetFileName(titleLabel),
        filters: [{ name: t('analytics.parquetFilterName'), extensions: ['parquet'] }]
      });
      if (!chosen || !mountedRef.current) {
        return;
      }
      setExportingParquet(true);
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
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-line px-5 py-3">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
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
            {scope === 'sample' && summary && tableSample && (
              <span className="shrink-0 rounded bg-accent-soft px-1.5 py-0.5 text-[10px] text-accent">
                {tableSample.hasActiveFilters
                  ? t('profile.sampleBadgeFiltered', { count: summary.rowCount.toLocaleString() })
                  : t('profile.sampleBadge', { count: summary.rowCount.toLocaleString() })}
              </span>
            )}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            {tableSample && (
              <div className="flex items-center gap-1 rounded-control border border-line-strong bg-surface p-0.5 text-xs">
                <button
                  type="button"
                  onClick={() => setScope('current')}
                  className={clsx(
                    'rounded px-2 py-0.5 transition-colors',
                    scope === 'current'
                      ? 'bg-accent-soft font-medium text-accent'
                      : 'text-fg-muted hover:text-fg'
                  )}
                >
                  {t('profile.scopeCurrentPage', { count: currentRowCount.toLocaleString() })}
                </button>
                <button
                  type="button"
                  onClick={() => setScope('sample')}
                  className={clsx(
                    'flex items-center gap-1 rounded px-2 py-0.5 transition-colors',
                    scope === 'sample'
                      ? 'bg-accent-soft font-medium text-accent'
                      : 'text-fg-muted hover:text-fg'
                  )}
                >
                  <Database size={11} />
                  <span>{t('profile.scopeTableSample')}</span>
                </button>
                {scope === 'sample' && (
                  <select
                    aria-label={t('profile.scopeTableSample')}
                    value={sampleSize}
                    onChange={(event) => setSampleSize(Number(event.target.value))}
                    className="rounded border-none bg-transparent px-1 py-0.5 font-mono text-[11px] text-fg focus:outline-none"
                  >
                    {TABLE_SAMPLE_SIZE_OPTIONS.map((size) => (
                      <option key={size} value={size}>
                        {t('profile.scopeSampleOption', { count: size.toLocaleString() })}
                      </option>
                    ))}
                  </select>
                )}
              </div>
            )}
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
        {(summary?.truncated || sampledTruncated) && summary && (
          <div className="flex shrink-0 items-center gap-2 border-b border-warning-line bg-warning-soft px-5 py-2 text-xs text-warning">
            <AlertTriangle size={14} className="shrink-0" />
            <span>{t('profile.truncatedNotice', { count: summary.rowCount })}</span>
          </div>
        )}

        {/* Loading / Session error */}
        {loadingSession ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-3">
            <Loader2 size={24} className="animate-spin text-accent" />
            <p className="text-sm text-fg-muted">
              {sampling
                ? t('profile.samplingRows', { count: sampleSize.toLocaleString() })
                : t('analytics.loadingSession')}
            </p>
            {sampling && (
              <button
                type="button"
                onClick={handleCancelSample}
                className="rounded-control border border-line-strong px-3 py-1 text-xs text-fg hover:bg-surface-hover"
              >
                {t('analytics.cancel')}
              </button>
            )}
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
                  <button
                    type="button"
                    onClick={handleAttachFile}
                    disabled={attaching || attachedTables.length >= MAX_ATTACHED_TABLES}
                    title={
                      attachedTables.length >= MAX_ATTACHED_TABLES
                        ? t('analytics.attachLimitReached', { count: MAX_ATTACHED_TABLES })
                        : t('analytics.attachFileTitle')
                    }
                    className="flex items-center gap-1 rounded-control border border-line px-2 py-0.5 text-xs text-fg-muted hover:bg-surface-hover hover:text-fg disabled:opacity-50"
                  >
                    <FilePlus2 size={12} />
                    <span>{attaching ? t('analytics.attaching') : t('analytics.attachFile')}</span>
                  </button>
                  {attachableQueryResults.length > 0 && attachedTables.length < MAX_ATTACHED_TABLES && (
                    <select
                      aria-label={t('analytics.attachResult')}
                      value=""
                      disabled={attaching}
                      onChange={(event) => {
                        const value = event.target.value;
                        if (value) {
                          void handleAttachQueryResult(value);
                        }
                      }}
                      className="max-w-[180px] rounded-control border border-line bg-surface px-2 py-0.5 text-xs text-fg-muted hover:text-fg"
                    >
                      <option value="">{t('analytics.attachResultPlaceholder')}</option>
                      {attachableQueryResults.map((item) => (
                        <option key={item.id} value={item.id}>
                          {item.label}
                        </option>
                      ))}
                    </select>
                  )}
                  {attachedTables.length > 0 && (
                    <>
                      <button
                        type="button"
                        onClick={() => {
                          const latest = attachedTables[attachedTables.length - 1];
                          if (latest) {
                            handleApplyTemplate(
                              buildLeftJoinTemplate(summary.columns, latest.tableName, latest.columns)
                            );
                          }
                        }}
                        className="rounded-control border border-accent/40 bg-accent-soft px-2 py-0.5 text-xs text-accent hover:opacity-90"
                      >
                        {t('analytics.template.leftJoin')}
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          const latest = attachedTables[attachedTables.length - 1];
                          if (latest) {
                            handleApplyTemplate(
                              buildExceptDiffTemplate(summary.columns, latest.tableName, latest.columns)
                            );
                          }
                        }}
                        className="rounded-control border border-accent/40 bg-accent-soft px-2 py-0.5 text-xs text-accent hover:opacity-90"
                      >
                        {t('analytics.template.exceptDiff')}
                      </button>
                    </>
                  )}
                </div>
              </div>

              {/* Attached reconciliation tables (with clickable column chips) */}
              {attachedTables.length > 0 && (
                <div className="space-y-1">
                  {attachedTables.map((table) => (
                    <div key={table.tableName} className="flex flex-wrap items-center gap-1">
                      <button
                        type="button"
                        onClick={() => handleInsertColumn(table.tableName)}
                        title={table.columns.map((c) => `${c.name}: ${c.duckdbType}`).join(', ')}
                        className="rounded border border-accent/30 bg-accent-soft px-2 py-0.5 font-mono text-[11px] text-accent hover:opacity-90"
                      >
                        {t('analytics.attachedMeta', {
                          table: table.tableName,
                          count: table.rowCount,
                          label: table.label
                        })}
                      </button>
                      {table.columns.map((col) => (
                        <button
                          key={`${table.tableName}.${col.name}`}
                          type="button"
                          onClick={() => handleInsertQualifiedColumn(table.tableName, col.name)}
                          title={`${table.tableName}.${col.name} (${col.duckdbType})`}
                          className="rounded bg-surface-sunken px-1.5 py-0.5 font-mono text-[11px] text-fg-muted hover:bg-surface-hover hover:text-fg"
                        >
                          {col.name}
                        </button>
                      ))}
                    </div>
                  ))}
                </div>
              )}

              {/* Primary table column chips */}
              {summary.columns.length > 0 && (
                <div className="flex max-h-14 flex-wrap items-center gap-1 overflow-y-auto">
                  {attachedTables.length > 0 && (
                    <button
                      type="button"
                      onClick={() => handleInsertColumn('result_set')}
                      className="rounded border border-line bg-surface-sunken px-1.5 py-0.5 font-mono text-[11px] text-fg-subtle hover:text-fg"
                    >
                      result_set
                    </button>
                  )}
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

              <div className="overflow-hidden rounded-control border border-line-strong focus-within:border-accent">
                <CodeMirror
                  value={sql}
                  onChange={(value) => setSql(value)}
                  onCreateEditor={(view) => {
                    editorViewRef.current = view;
                  }}
                  theme={resolvedTheme === 'dark' ? oneDark : undefined}
                  extensions={editorExtensions}
                  minHeight="96px"
                  maxHeight="220px"
                  basicSetup={{
                    lineNumbers: true,
                    foldGutter: false,
                    dropCursor: false,
                    allowMultipleSelections: false,
                    indentOnInput: true,
                    bracketMatching: true,
                    closeBrackets: true,
                    autocompletion: true,
                    highlightSelectionMatches: true,
                    searchKeymap: true
                  }}
                  className="text-xs"
                />
              </div>

              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  {running ? (
                    <button
                      type="button"
                      onClick={handleCancel}
                      className="flex items-center gap-1.5 rounded-control bg-danger-solid px-3 py-1.5 text-xs font-medium text-fg-on-solid hover:opacity-90"
                    >
                      <Square size={12} />
                      <span>{t('analytics.cancel')}</span>
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={handleRunCurrentSql}
                      disabled={sql.trim() === ''}
                      className="flex items-center gap-1.5 rounded-control bg-accent px-3 py-1.5 text-xs font-medium text-fg-on-accent hover:opacity-90 disabled:opacity-50"
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
                      sourceDialect={sourceDialect}
                      onCopySql={handleCopySql}
                      onRunFilterSql={handleRunFromProfile}
                      onApplyTableFilter={
                        onApplyTableFilter
                          ? (columnName, operator, value) => {
                              onApplyTableFilter(columnName, operator, value);
                              onClose();
                            }
                          : undefined
                      }
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
