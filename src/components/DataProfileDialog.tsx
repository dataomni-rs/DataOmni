import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import {
  AlertTriangle,
  BarChart2,
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  Database,
  Filter,
  Grid,
  Loader2,
  RotateCcw,
  Search,
  X
} from 'lucide-react';
import type {
  AnalyticsColumnInput,
  AnalyticsQueryResult,
  AnalyticsSessionSummary,
  ColumnProfile,
  ProfileResult
} from '../contracts/analytics';
import type { SerializedResultValue } from '../contracts/resultSet';
import { useLanguageStore } from '../stores/languageStore';
import {
  buildProfileEmptyStringFilterSql,
  buildProfileFilterSql,
  buildProfileGroupBySql,
  buildProfileNullFilterSql,
  buildProfilePaddedWhitespaceFilterSql,
  buildSourceWhereClause,
  columnCardinalityHint,
  filterAndSortProfiles,
  formatProfileMarkdownReport,
  formatProfileNumber,
  formatProfileRatio,
  hasSchemaDrift,
  histogramHeights,
  nullSeverity,
  numericOutlierFilter,
  shouldCollapseTopValues,
  summarizeProfileFindings,
  timeSparklinePoints,
  type ProfileHealthFinding,
  type ProfileSortMode,
  type ProfileTableFilterOperator
} from '../utils/analyticsProfile';
import { describeError } from '../utils/describeError';
import { isImeKeyEvent } from '../utils/shortcuts';
import type { SqlIdentifierDialect } from '../utils/sqlIdentifiers';
import { buildCorrelationMatrixSql } from '../utils/analyticsQuery';
import {
  DEFAULT_TABLE_SAMPLE_SIZE,
  TABLE_SAMPLE_SIZE_OPTIONS
} from '../utils/tableSampleQuery';
import { PLAIN_TEXT_INPUT } from './FormControls';
import { ResultChartDialog } from './ResultChartDialog';

export interface TableSampleResult {
  readonly columns: readonly AnalyticsColumnInput[];
  readonly rows: ReadonlyArray<readonly SerializedResultValue[]>;
  readonly truncated: boolean;
}

export interface TableSampleConfig {
  readonly hasActiveFilters: boolean;
  readonly fetchSample: (limit: number, executionId: string) => Promise<TableSampleResult>;
  readonly cancelSample: (executionId: string) => void;
}

export type DataProfileSource =
  | {
      readonly kind: 'resultSet';
      readonly columns: readonly AnalyticsColumnInput[];
      readonly rows: ReadonlyArray<readonly SerializedResultValue[]>;
    }
  | {
      readonly kind: 'jsonDocuments';
      readonly documents: ReadonlyArray<Record<string, unknown>>;
    };

interface DataProfileDialogProps {
  source: DataProfileSource;
  tableSample?: TableSampleConfig;
  sourceDialect?: SqlIdentifierDialect;
  onApplyTableFilter?: (columnName: string, operator: ProfileTableFilterOperator, value: string) => void;
  onClose: () => void;
}

const SORT_OPTIONS: readonly ProfileSortMode[] = ['order', 'nulls', 'distinct', 'drift'];

export function DataProfileDialog({
  source,
  tableSample,
  sourceDialect,
  onApplyTableFilter,
  onClose
}: DataProfileDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const language = useLanguageStore((state) => state.resolved);
  const [scope, setScope] = useState<'current' | 'sample'>('current');
  const [sampleSize, setSampleSize] = useState<number>(DEFAULT_TABLE_SAMPLE_SIZE);
  const [sampling, setSampling] = useState(false);
  const [sampledTruncated, setSampledTruncated] = useState(false);
  const [loading, setLoading] = useState(true);
  const [cancelled, setCancelled] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [summary, setSummary] = useState<AnalyticsSessionSummary | null>(null);
  const [profile, setProfile] = useState<ProfileResult | null>(null);
  const [search, setSearch] = useState('');
  const [sortMode, setSortMode] = useState<ProfileSortMode>('order');
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [corrLoading, setCorrLoading] = useState(false);
  const [corrResult, setCorrResult] = useState<AnalyticsQueryResult | null>(null);

  const sessionIdRef = useRef<string | null>(null);
  const sampleExecutionIdRef = useRef<string | null>(null);
  const mountedRef = useRef(true);

  const currentRowCount = source.kind === 'resultSet' ? source.rows.length : source.documents.length;
  const inputRowCount = scope === 'sample' ? sampleSize : currentRowCount;

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
      if (event.key === 'Escape' && !isImeKeyEvent(event) && !corrResult) {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, corrResult]);

  useEffect(() => {
    if (scope === 'current' && currentRowCount === 0) {
      setLoading(false);
      setSampling(false);
      setError(null);
      setProfile(null);
      return;
    }

    let stale = false;
    setLoading(true);
    setSampling(scope === 'sample');
    setSampledTruncated(false);
    setCancelled(false);
    setError(null);

    const run = async () => {
      const previousId = sessionIdRef.current;
      sessionIdRef.current = null;
      if (previousId) {
        void invoke('analytics_close', { analyticsId: previousId }).catch(() => {});
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
          if (sampled.rows.length === 0) {
            setSummary(null);
            setProfile(null);
            return;
          }
          loaded = await invoke<AnalyticsSessionSummary>('analytics_load_result', {
            columns: sampled.columns,
            rows: sampled.rows
          });
        } else {
          loaded =
            source.kind === 'resultSet'
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

        const profiled = await invoke<ProfileResult>('analytics_profile', {
          analyticsId: loaded.analyticsId
        });

        if (stale || !mountedRef.current) {
          return;
        }
        setProfile(profiled);
      } catch (err) {
        if (stale || !mountedRef.current) {
          return;
        }
        const message = describeError(err);
        const code =
          err !== null && typeof err === 'object' && 'code' in err
            ? String((err as { code?: unknown }).code ?? '')
            : '';
        if (
          code === 'DATAOMNI_ANALYTICS_CANCELLED'
          || message.includes('DATAOMNI_ANALYTICS_CANCELLED')
          || code === 'DATAOMNI_QUERY_CANCELLED'
          || message.includes('DATAOMNI_QUERY_CANCELLED')
        ) {
          setCancelled(true);
        } else {
          setError(message);
        }
      } finally {
        sampleExecutionIdRef.current = null;
        if (!stale && mountedRef.current) {
          setSampling(false);
          setLoading(false);
        }
      }
    };

    void run();
    return () => {
      stale = true;
      const sampleExecId = sampleExecutionIdRef.current;
      sampleExecutionIdRef.current = null;
      if (sampleExecId && tableSample) {
        tableSample.cancelSample(sampleExecId);
      }
    };
  }, [source, scope, sampleSize, tableSample, currentRowCount, attempt]);

  const handleCancel = useCallback(() => {
    const sampleExecId = sampleExecutionIdRef.current;
    if (sampleExecId && tableSample) {
      sampleExecutionIdRef.current = null;
      tableSample.cancelSample(sampleExecId);
    }
    const id = sessionIdRef.current;
    if (id) {
      void invoke('analytics_cancel', { analyticsId: id }).catch(() => {});
    }
    setCancelled(true);
  }, [tableSample]);

  const handleCopySql = useCallback((key: string, sql: string) => {
    navigator.clipboard
      .writeText(sql)
      .then(() => {
        setCopiedKey(key);
        window.setTimeout(() => {
          setCopiedKey((current) => (current === key ? null : current));
        }, 1400);
      })
      .catch(() => {});
  }, []);

  const hasPresenceInfo = useMemo(
    () => (profile?.columns ?? []).some((col) => col.presentRatio !== undefined),
    [profile]
  );

  const visibleColumns = useMemo(
    () => filterAndSortProfiles(profile?.columns ?? [], search, sortMode),
    [profile, search, sortMode]
  );

  const correlationSql = useMemo(
    () => (profile ? buildCorrelationMatrixSql(profile.columns) : null),
    [profile]
  );

  const handleOpenCorrelation = useCallback(async () => {
    const id = sessionIdRef.current;
    if (!id || !correlationSql || corrLoading) {
      return;
    }
    setCorrLoading(true);
    setError(null);
    try {
      const res = await invoke<AnalyticsQueryResult>('analytics_query', {
        analyticsId: id,
        sql: correlationSql,
        limit: 200,
        timeoutSeconds: 15
      });
      if (mountedRef.current && sessionIdRef.current === id) {
        setCorrResult(res);
      }
    } catch (err) {
      if (mountedRef.current && sessionIdRef.current === id) {
        setError(describeError(err));
      }
    } finally {
      if (mountedRef.current && sessionIdRef.current === id) {
        setCorrLoading(false);
      }
    }
  }, [correlationSql, corrLoading]);

  const sortOptions = useMemo(
    () => SORT_OPTIONS.filter((mode) => mode !== 'drift' || hasPresenceInfo),
    [hasPresenceInfo]
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="data-profile-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[88vh] w-[960px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        {/* Header */}
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-line px-5 py-3">
          <div className="flex flex-wrap items-center gap-2">
            <BarChart2 size={16} className="text-accent" />
            <h3 id="data-profile-title" className="text-sm font-semibold text-fg">
              {t('profile.title')}
            </h3>
            {profile && (
              <span className="text-xs text-fg-muted">
                {t('profile.summary', {
                  rows: profile.rowCount.toLocaleString(),
                  columns: profile.columnCount,
                  ms: profile.elapsedMs
                })}
              </span>
            )}
            {scope === 'sample' && profile && tableSample && (
              <span className="rounded bg-accent-soft px-1.5 py-0.5 text-[10px] text-accent">
                {tableSample.hasActiveFilters
                  ? t('profile.sampleBadgeFiltered', { count: profile.rowCount.toLocaleString() })
                  : t('profile.sampleBadge', { count: profile.rowCount.toLocaleString() })}
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
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
        {(summary?.truncated || sampledTruncated) && (
          <div className="flex shrink-0 items-center gap-2 border-b border-warning-line bg-warning-soft px-5 py-2 text-xs text-warning">
            <AlertTriangle size={14} className="shrink-0" />
            <span>{t('profile.truncatedNotice', { count: summary?.rowCount ?? profile?.rowCount ?? 0 })}</span>
          </div>
        )}

        {/* Toolbar */}
        {profile && !loading && (
          <>
            <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-line px-5 py-2.5">
              <div className="relative min-w-[200px] flex-1 max-w-xs">
                <Search
                  size={13}
                  className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-fg-subtle"
                />
                <input
                  {...PLAIN_TEXT_INPUT}
                  type="text"
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder={t('profile.searchPlaceholder')}
                  className="w-full rounded-control border border-line-strong bg-surface py-1 pl-8 pr-2.5 text-xs text-fg placeholder:text-fg-subtle focus:border-accent focus:outline-none"
                />
              </div>
              <div className="flex items-center gap-2">
                {correlationSql && (
                  <button
                    type="button"
                    disabled={corrLoading}
                    onClick={() => void handleOpenCorrelation()}
                    className="flex items-center gap-1 rounded-control border border-line-strong bg-surface px-2.5 py-1 text-xs text-fg hover:bg-surface-hover disabled:opacity-50"
                  >
                    {corrLoading ? (
                      <Loader2 size={12} className="animate-spin" />
                    ) : (
                      <Grid size={12} />
                    )}
                    <span>
                      {corrLoading
                        ? t('profile.correlationLoading')
                        : t('profile.correlationMatrix')}
                    </span>
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => {
                    const scopeLabel =
                      scope === 'sample' && tableSample
                        ? tableSample.hasActiveFilters
                          ? t('profile.sampleBadgeFiltered', {
                              count: profile.rowCount.toLocaleString()
                            })
                          : t('profile.sampleBadge', {
                              count: profile.rowCount.toLocaleString()
                            })
                        : t('profile.scopeCurrentPage', {
                            count: profile.rowCount.toLocaleString()
                          });
                    const md = formatProfileMarkdownReport({
                      profile,
                      scopeLabel,
                      language
                    });
                    handleCopySql('__report__', md);
                  }}
                  className="flex items-center gap-1 rounded-control border border-line-strong bg-surface px-2.5 py-1 text-xs text-fg hover:bg-surface-hover"
                >
                  {copiedKey === '__report__' ? (
                    <Check size={12} className="text-success" />
                  ) : (
                    <Copy size={12} />
                  )}
                  <span>
                    {copiedKey === '__report__'
                      ? t('profile.copiedReport')
                      : t('profile.copyReport')}
                  </span>
                </button>
                <select
                  value={sortMode}
                  onChange={(event) => setSortMode(event.target.value as ProfileSortMode)}
                  className="rounded-control border border-line-strong bg-surface px-2.5 py-1 text-xs text-fg"
                >
                  {sortOptions.map((mode) => (
                    <option key={mode} value={mode}>
                      {t(`profile.sort.${mode}` as const)}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <ProfileHealthSummaryBar
              columns={profile.columns}
              activeSearch={search}
              onSelectColumn={(columnName) =>
                setSearch((current) => (current === columnName ? '' : columnName))
              }
            />
          </>
        )}

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          {scope === 'current' && currentRowCount === 0 ? (
            <p className="py-8 text-center text-sm text-fg-muted">{t('profile.empty')}</p>
          ) : loading ? (
            <div className="flex flex-col items-center justify-center gap-3 py-12">
              <Loader2 size={24} className="animate-spin text-accent" />
              <p className="text-sm text-fg-muted">
                {sampling
                  ? t('profile.samplingRows', { count: sampleSize.toLocaleString() })
                  : t('profile.loading', { count: inputRowCount })}
              </p>
              <button
                type="button"
                onClick={handleCancel}
                className="rounded-control border border-line-strong px-3 py-1.5 text-xs text-fg hover:bg-surface-hover"
              >
                {t('profile.cancel')}
              </button>
            </div>
          ) : cancelled ? (
            <div className="flex flex-col items-center justify-center gap-3 py-12">
              <p className="text-sm text-fg-muted">{t('profile.cancelled')}</p>
              <button
                type="button"
                onClick={() => setAttempt((n) => n + 1)}
                className="flex items-center gap-1.5 rounded-control border border-line-strong px-3 py-1.5 text-xs text-fg hover:bg-surface-hover"
              >
                <RotateCcw size={13} />
                <span>{t('profile.retry')}</span>
              </button>
            </div>
          ) : error ? (
            <div className="flex flex-col items-center justify-center gap-3 py-10">
              <p className="max-w-lg text-center text-sm text-danger">{error}</p>
              <button
                type="button"
                onClick={() => setAttempt((n) => n + 1)}
                className="flex items-center gap-1.5 rounded-control border border-line-strong px-3 py-1.5 text-xs text-fg hover:bg-surface-hover"
              >
                <RotateCcw size={13} />
                <span>{t('profile.retry')}</span>
              </button>
            </div>
          ) : !profile || profile.rowCount === 0 ? (
            <p className="py-8 text-center text-sm text-fg-muted">{t('profile.empty')}</p>
          ) : visibleColumns.length === 0 ? (
            <p className="py-8 text-center text-sm text-fg-muted">{t('profile.noMatchingColumns')}</p>
          ) : (
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              {visibleColumns.map((column) => (
                <ColumnProfileCard
                  key={column.name}
                  column={column}
                  copiedKey={copiedKey}
                  onCopySql={handleCopySql}
                  sourceDialect={sourceDialect}
                  onApplyTableFilter={
                    onApplyTableFilter
                      ? (col, op, val) => {
                          onApplyTableFilter(col, op, val);
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

      {corrResult && (
        <ResultChartDialog
          columns={corrResult.columns}
          rows={corrResult.rows}
          initialChartType="heatmap"
          onClose={() => setCorrResult(null)}
        />
      )}
    </div>
  );
}

export interface ProfileHealthSummaryBarProps {
  columns: readonly ColumnProfile[];
  activeSearch: string;
  onSelectColumn: (columnName: string) => void;
}

export function ProfileHealthSummaryBar({
  columns,
  activeSearch,
  onSelectColumn
}: ProfileHealthSummaryBarProps) {
  const t = useLanguageStore((state) => state.t);
  const findings = useMemo(() => summarizeProfileFindings(columns), [columns]);
  if (findings.length === 0) {
    return null;
  }

  const labelFor = (finding: ProfileHealthFinding): string => {
    switch (finding.kind) {
      case 'negative':
        return t('profile.finding.negative', { column: finding.column });
      case 'high-outlier':
        return t('profile.finding.highOutlier', {
          column: finding.column,
          threshold: finding.threshold ?? ''
        });
      case 'high-nulls':
        return t('profile.finding.highNulls', {
          column: finding.column,
          ratio: formatProfileRatio(finding.ratio ?? 0)
        });
      case 'empty-strings':
        return t('profile.finding.emptyStrings', {
          column: finding.column,
          count: finding.count ?? 0
        });
      case 'padded-whitespace':
        return t('profile.finding.paddedWhitespace', {
          column: finding.column,
          count: finding.count ?? 0
        });
      case 'schema-drift':
        return t('profile.finding.schemaDrift', { column: finding.column });
    }
  };

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-line bg-surface px-5 py-2 text-[11px]">
      <span className="flex items-center gap-1 font-medium text-fg-muted">
        <AlertTriangle size={12} className="text-warning" />
        <span>{t('profile.findingsSummary', { count: findings.length })}</span>
      </span>
      {findings.slice(0, 10).map((finding) => {
        const active = activeSearch === finding.column;
        return (
          <button
            key={finding.id}
            type="button"
            onClick={() => onSelectColumn(finding.column)}
            className={clsx(
              'rounded border px-1.5 py-0.5 font-mono text-[10px] transition-colors',
              active
                ? 'border-accent bg-accent-soft font-medium text-accent'
                : finding.severity === 'high'
                  ? 'border-danger-line bg-danger-soft text-danger hover:opacity-90'
                  : 'border-warning-line bg-warning-soft text-warning hover:opacity-90'
            )}
          >
            {labelFor(finding)}
          </button>
        );
      })}
    </div>
  );
}

export interface ColumnProfileCardProps {
  column: ColumnProfile;
  copiedKey: string | null;
  onCopySql: (key: string, sql: string) => void;
  /** 当宿主工作台支持直接执行二次 SQL 时，点击高频值/空值直接跳转执行而非仅复制 */
  onRunFilterSql?: (sql: string) => void;
  /** 原库方言（提供时允许一键复制原库 WHERE 条件） */
  sourceDialect?: SqlIdentifierDialect;
  /** 表视图回填筛选（提供时允许一键带条件回到原表网格） */
  onApplyTableFilter?: (columnName: string, operator: ProfileTableFilterOperator, value: string) => void;
}

export function ColumnProfileCard({
  column,
  copiedKey,
  onCopySql,
  onRunFilterSql,
  sourceDialect,
  onApplyTableFilter
}: ColumnProfileCardProps) {
  const t = useLanguageStore((state) => state.t);
  const severity = nullSeverity(column.nullRatio);
  const drift = hasSchemaDrift(column);
  const cardinalityHint = columnCardinalityHint(column);
  const collapseByDefault = shouldCollapseTopValues(column);
  const [topValuesExpanded, setTopValuesExpanded] = useState(!collapseByDefault);
  const outlier = useMemo(() => numericOutlierFilter(column), [column]);
  const heights = useMemo(() => histogramHeights(column.bins), [column.bins]);
  const sparkline = useMemo(
    () => timeSparklinePoints(column.timeBins ?? [], 200, 32, 4, 4),
    [column.timeBins]
  );
  const nullCopyId = `null:${column.name}`;
  const groupCopyId = `group:${column.name}`;
  const outlierCopyId = `outlier:${column.name}`;

  useEffect(() => {
    setTopValuesExpanded(!collapseByDefault);
  }, [collapseByDefault, column.name]);

  const nullBadgeClass =
    severity === 'high'
      ? 'text-danger'
      : severity === 'warn'
        ? 'text-warning'
        : 'text-fg-muted';

  const absentCount =
    column.presentCount !== undefined
      ? Math.max(0, column.totalCount - column.presentCount)
      : 0;

  return (
    <section className="flex flex-col gap-2.5 rounded-control border border-line bg-surface p-3.5">
      {/* Column header */}
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <h4 className="truncate font-mono text-xs font-semibold text-fg" title={column.name}>
            {column.name}
          </h4>
          <div className="mt-0.5 flex flex-wrap items-center gap-1.5">
            <span className="rounded bg-surface-sunken px-1.5 py-0.5 font-mono text-[10px] text-fg-muted">
              {column.duckdbType}
            </span>
            {cardinalityHint === 'all-null' && (
              <span className="rounded bg-danger-soft px-1.5 py-0.5 text-[10px] text-danger">
                {t('profile.hint.allNull')}
              </span>
            )}
            {cardinalityHint === 'constant' && (
              <span className="rounded bg-surface-sunken px-1.5 py-0.5 text-[10px] text-fg-subtle">
                {t('profile.hint.constant')}
              </span>
            )}
            {cardinalityHint === 'unique-key' && (
              <span className="rounded bg-accent-soft px-1.5 py-0.5 text-[10px] text-accent">
                {t('profile.hint.uniqueKey')}
              </span>
            )}
            {drift && (
              <span className="rounded bg-warning-soft px-1.5 py-0.5 text-[10px] text-warning">
                {t('profile.schemaDrift')}
              </span>
            )}
            {column.precisionDegraded && (
              <span
                className="rounded bg-warning-soft px-1.5 py-0.5 text-[10px] text-warning"
                title={t('profile.precisionDegraded')}
              >
                {t('profile.precisionDegraded')}
              </span>
            )}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <button
            type="button"
            onClick={() => {
              const sql = buildProfileGroupBySql(column.name);
              if (onRunFilterSql) {
                onRunFilterSql(sql);
              } else {
                onCopySql(groupCopyId, sql);
              }
            }}
            title={onRunFilterSql ? t('analytics.applyGroupBySql') : t('profile.copyGroupBySql')}
            className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] text-fg-subtle transition-colors hover:bg-surface-hover hover:text-fg"
          >
            {copiedKey === groupCopyId ? (
              <Check size={11} className="text-success" />
            ) : (
              <BarChart2 size={11} />
            )}
            <span>{copiedKey === groupCopyId ? t('profile.copiedSql') : t('profile.groupByAction')}</span>
          </button>
          {column.nullCount > 0 && (
            <button
              type="button"
              onClick={() => {
                const sql = buildProfileNullFilterSql(column.name);
                if (onRunFilterSql) {
                  onRunFilterSql(sql);
                } else {
                  onCopySql(nullCopyId, sql);
                }
              }}
              title={onRunFilterSql ? t('analytics.applyNullSql') : t('profile.copyNullSql')}
              className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] text-fg-subtle transition-colors hover:bg-surface-hover hover:text-fg"
            >
              {copiedKey === nullCopyId ? (
                <Check size={11} className="text-success" />
              ) : onRunFilterSql ? (
                <Filter size={11} />
              ) : (
                <Copy size={11} />
              )}
              <span>{copiedKey === nullCopyId ? t('profile.copiedSql') : 'IS NULL'}</span>
            </button>
          )}
        </div>
      </div>

      {/* Key metrics row: Null rate & Distinct */}
      <div className="grid grid-cols-2 gap-2 text-xs">
        <div className="rounded bg-surface-sunken px-2.5 py-1.5">
          <div className="flex items-center justify-between text-[10px] text-fg-subtle">
            <span>{t('profile.nullRate')}</span>
            <span className={clsx('font-mono font-medium', nullBadgeClass)}>
              {formatProfileRatio(column.nullRatio)}
            </span>
          </div>
          <div className="mt-1 h-1.5 w-full overflow-hidden rounded-full bg-line">
            <div
              className={clsx(
                'h-full rounded-full',
                severity === 'high'
                  ? 'bg-danger'
                  : severity === 'warn'
                    ? 'bg-warning'
                    : 'bg-accent'
              )}
              style={{ width: `${Math.min(100, Math.round(column.nullRatio * 100))}%` }}
            />
          </div>
          <p className="mt-1 font-mono text-[10px] text-fg-subtle">
            {column.nullCount.toLocaleString()} / {column.totalCount.toLocaleString()}
          </p>
        </div>

        <div className="rounded bg-surface-sunken px-2.5 py-1.5">
          <div className="flex items-center justify-between text-[10px] text-fg-subtle">
            <span>{t('profile.distinct')}</span>
            <span className="font-mono font-medium text-fg">
              {formatProfileRatio(column.distinctRatio)}
            </span>
          </div>
          <p className="mt-2 font-mono text-xs font-medium text-fg">
            {column.distinctCount.toLocaleString()}
          </p>
        </div>
      </div>

      {/* NoSQL presence breakdown */}
      {column.presentRatio !== undefined && column.presentCount !== undefined && (
        <div className="rounded bg-surface-sunken px-2.5 py-1.5 text-[11px]">
          <div className="flex items-center justify-between">
            <span className="text-fg-subtle">{t('profile.presence')}</span>
            <span className={clsx('font-mono font-medium', drift ? 'text-warning' : 'text-fg')}>
              {formatProfileRatio(column.presentRatio)}
            </span>
          </div>
          <p className="mt-0.5 text-[10px] text-fg-subtle">
            {t('profile.presenceBreakdown', {
              present: column.presentCount.toLocaleString(),
              absent: absentCount.toLocaleString(),
              explicitNull: (column.explicitNullCount ?? 0).toLocaleString()
            })}
          </p>
        </div>
      )}

      {/* Numeric summary & mini histogram */}
      {(column.inferredType === 'integer' || column.inferredType === 'numeric') &&
        column.min !== undefined && (
          <div className="space-y-2">
            <div className="grid grid-cols-3 gap-x-2 gap-y-1 font-mono text-[11px]">
              <MetricCell label="Min" value={column.min} />
              <MetricCell label="P50" value={formatProfileNumber(column.p50)} />
              <MetricCell label="Max" value={column.max ?? '—'} />
              <MetricCell label={t('profile.mean')} value={formatProfileNumber(column.mean)} />
              <MetricCell label="P95" value={formatProfileNumber(column.p95)} />
              <MetricCell label={t('profile.stddev')} value={formatProfileNumber(column.stddev)} />
            </div>
            {column.bins.length > 0 && (
              <div>
                <p className="mb-1 text-[10px] text-fg-subtle">{t('profile.histogram')}</p>
                <svg
                  viewBox="0 0 200 44"
                  className="h-11 w-full"
                  role="img"
                  aria-label={`${column.name} ${t('profile.histogram')}`}
                >
                  {column.bins.map((bin, idx) => {
                    const slotWidth = 200 / column.bins.length;
                    const barWidth = Math.max(2, slotWidth - 2);
                    const ratio = heights[idx] ?? 0;
                    const barHeight = bin.count === 0 ? 0 : Math.max(2, Math.round(ratio * 40));
                    const x = idx * slotWidth + 1;
                    const y = 42 - barHeight;
                    return (
                      <rect
                        key={idx}
                        x={x}
                        y={y}
                        width={barWidth}
                        height={barHeight}
                        rx={1.5}
                        fill="var(--dm-accent)"
                      >
                        <title>{`${formatProfileNumber(bin.lower)} – ${formatProfileNumber(bin.upper)}: ${bin.count.toLocaleString()}`}</title>
                      </rect>
                    );
                  })}
                </svg>
              </div>
            )}
            {outlier && (
              <div className="flex flex-wrap items-center justify-end gap-1.5">
                <button
                  type="button"
                  onClick={() => {
                    if (onRunFilterSql) {
                      onRunFilterSql(outlier.sql);
                    } else {
                      onCopySql(outlierCopyId, outlier.sql);
                    }
                  }}
                  title={t('profile.outlierTooltip')}
                  className="flex items-center gap-1 rounded border border-warning-line bg-warning-soft px-2 py-0.5 text-[10px] font-medium text-warning transition-colors hover:opacity-90"
                >
                  {copiedKey === outlierCopyId ? (
                    <Check size={10} className="text-success" />
                  ) : (
                    <AlertTriangle size={10} />
                  )}
                  <span>
                    {copiedKey === outlierCopyId
                      ? t('profile.copiedSql')
                      : outlier.kind === 'negative'
                        ? t('profile.outlierNegative')
                        : t('profile.outlierHigh', { threshold: outlier.threshold })}
                  </span>
                </button>
                {onApplyTableFilter && (
                  <button
                    type="button"
                    onClick={() =>
                      onApplyTableFilter(column.name, outlier.operator, outlier.filterValue)
                    }
                    title={t('profile.applyTableFilterTitle')}
                    className="flex items-center gap-1 rounded border border-line-strong px-1.5 py-0.5 text-[10px] text-fg-muted transition-colors hover:bg-surface-hover hover:text-fg"
                  >
                    <Filter size={10} />
                    <span>{t('profile.applyTableFilter')}</span>
                  </button>
                )}
              </div>
            )}
          </div>
        )}

      {/* Temporal min/max + mini sparkline */}
      {(column.inferredType === 'date'
        || column.inferredType === 'time'
        || column.inferredType === 'datetime')
        && column.min !== undefined && (
          <div className="space-y-1.5 text-[11px]">
            <div>
              <span className="text-fg-subtle">{t('profile.range')}: </span>
              <span className="font-mono text-fg">
                {column.min} → {column.max ?? '—'}
              </span>
            </div>
            {sparkline.length >= 2 && (
              <div>
                <p className="mb-0.5 text-[10px] text-fg-subtle">{t('profile.timeTrend')}</p>
                <svg
                  viewBox="0 0 200 32"
                  className="h-8 w-full"
                  role="img"
                  aria-label={`${column.name} ${t('profile.timeTrend')}`}
                >
                  <polygon
                    points={`4,28 ${sparkline.map((pt) => `${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(' ')} 196,28`}
                    fill="var(--dm-accent)"
                    fillOpacity="0.14"
                  />
                  <polyline
                    points={sparkline.map((pt) => `${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(' ')}
                    fill="none"
                    stroke="var(--dm-accent)"
                    strokeWidth="1.5"
                    strokeLinejoin="round"
                  />
                  {sparkline.map((pt, idx) => (
                    <circle
                      key={idx}
                      cx={pt.x}
                      cy={pt.y}
                      r={2.2}
                      fill="var(--dm-accent)"
                    >
                      <title>{`${pt.label}: ${pt.count.toLocaleString()}`}</title>
                    </circle>
                  ))}
                </svg>
              </div>
            )}
          </div>
        )}

      {/* Length / Byte length range & whitespace dirt chips */}
      {column.minLength !== undefined && column.maxLength !== undefined && (
        <div className="flex flex-wrap items-center justify-between gap-1.5">
          <p className="font-mono text-[11px] text-fg-muted">
            {column.inferredType === 'binary'
              ? t('profile.bytesRange', {
                  min: column.minLength.toLocaleString(),
                  max: column.maxLength.toLocaleString()
                })
              : t('profile.lengthRange', {
                  min: column.minLength.toLocaleString(),
                  max: column.maxLength.toLocaleString()
                })}
          </p>
          {((column.emptyStringCount ?? 0) > 0 || (column.paddedWhitespaceCount ?? 0) > 0) && (
            <div className="flex flex-wrap items-center gap-1">
              {(column.emptyStringCount ?? 0) > 0 && (
                <>
                  <button
                    type="button"
                    onClick={() => {
                      const sql = buildProfileEmptyStringFilterSql(column.name);
                      if (onRunFilterSql) {
                        onRunFilterSql(sql);
                      } else {
                        onCopySql(`empty:${column.name}`, sql);
                      }
                    }}
                    className="rounded border border-warning-line bg-warning-soft px-1.5 py-0.5 font-mono text-[10px] text-warning hover:opacity-90"
                  >
                    {copiedKey === `empty:${column.name}`
                      ? t('profile.copiedSql')
                      : t('profile.emptyStringChip', { count: column.emptyStringCount ?? 0 })}
                  </button>
                  {onApplyTableFilter && (
                    <button
                      type="button"
                      onClick={() => onApplyTableFilter(column.name, 'eq', '')}
                      title={t('profile.applyTableFilterTitle')}
                      className="rounded border border-line-strong px-1 py-0.5 text-[10px] text-fg-muted hover:bg-surface-hover hover:text-fg"
                    >
                      {t('profile.applyTableFilter')}
                    </button>
                  )}
                </>
              )}
              {(column.paddedWhitespaceCount ?? 0) > 0 && (
                <button
                  type="button"
                  onClick={() => {
                    const sql = buildProfilePaddedWhitespaceFilterSql(column.name);
                    if (onRunFilterSql) {
                      onRunFilterSql(sql);
                    } else {
                      onCopySql(`padded:${column.name}`, sql);
                    }
                  }}
                  className="rounded border border-warning-line bg-warning-soft px-1.5 py-0.5 font-mono text-[10px] text-warning hover:opacity-90"
                >
                  {copiedKey === `padded:${column.name}`
                    ? t('profile.copiedSql')
                    : t('profile.paddedWhitespaceChip', {
                        count: column.paddedWhitespaceCount ?? 0
                      })}
                </button>
              )}
            </div>
          )}
        </div>
      )}

      {/* Top 5 values (default-collapsed for unique/high-cardinality noise columns) */}
      {column.topValues.length > 0 && (
        <div className="space-y-1">
          {collapseByDefault && !topValuesExpanded ? (
            <button
              type="button"
              onClick={() => setTopValuesExpanded(true)}
              className="flex w-full items-center justify-between rounded bg-surface-sunken px-2 py-1 text-[10px] text-fg-subtle transition-colors hover:bg-surface-hover hover:text-fg"
            >
              <span className="flex items-center gap-1">
                <ChevronRight size={11} />
                <span>{t('profile.uniqueValuesCollapsed')}</span>
              </span>
              <span className="text-accent">{t('profile.showSampleValues')}</span>
            </button>
          ) : (
            <>
              <div className="flex items-center justify-between text-[10px] text-fg-subtle">
                <span className="flex items-center gap-1">
                  {collapseByDefault && (
                    <button
                      type="button"
                      onClick={() => setTopValuesExpanded(false)}
                      className="inline-flex items-center gap-0.5 text-fg-subtle hover:text-fg"
                      title={t('profile.hideSampleValues')}
                    >
                      <ChevronDown size={11} />
                    </button>
                  )}
                  <span>{t('profile.topValues')}</span>
                </span>
                <span>{onRunFilterSql ? t('analytics.clickValueToFilter') : t('profile.clickValueToCopy')}</span>
              </div>
              <ul className="space-y-1">
                {column.topValues.map((entry, idx) => {
                  const copyId = `top:${column.name}:${idx}`;
                  const whereCopyId = `where:${column.name}:${idx}`;
                  const display = entry.value === '' ? '""' : entry.value;
                  return (
                    <li key={copyId} className="group flex items-center gap-1.5 text-[11px]">
                      <button
                        type="button"
                        onClick={() => {
                          const sql = buildProfileFilterSql(column.name, column.inferredType, entry.value);
                          if (onRunFilterSql) {
                            onRunFilterSql(sql);
                          } else {
                            onCopySql(copyId, sql);
                          }
                        }}
                        title={onRunFilterSql ? t('analytics.applyFilterSql') : t('profile.copyFilterSql')}
                        className="relative flex min-w-0 flex-1 items-center justify-between overflow-hidden rounded border border-transparent bg-surface-sunken px-2 py-0.5 text-left transition-colors hover:border-accent/40 hover:bg-surface-hover"
                      >
                        <span
                          aria-hidden
                          className="pointer-events-none absolute inset-y-0 left-0 bg-accent-soft opacity-60"
                          style={{ width: `${Math.max(2, Math.round(entry.ratio * 100))}%` }}
                        />
                        <span className="relative truncate font-mono text-fg" title={display}>
                          {display}
                        </span>
                        <span className="relative ml-2 flex shrink-0 items-center gap-1 font-mono text-[10px] text-fg-muted">
                          {copiedKey === copyId ? (
                            <>
                              <Check size={10} className="text-success" />
                              <span>{t('profile.copiedSql')}</span>
                            </>
                          ) : (
                            <>
                              <span>{`${entry.count.toLocaleString()} (${formatProfileRatio(entry.ratio)})`}</span>
                              {onRunFilterSql ? (
                                <Filter size={10} className="opacity-0 transition-opacity group-hover:opacity-80" />
                              ) : (
                                <Copy size={10} className="opacity-0 transition-opacity group-hover:opacity-80" />
                              )}
                            </>
                          )}
                        </span>
                      </button>
                      {onApplyTableFilter && (
                        <button
                          type="button"
                          onClick={() => onApplyTableFilter(column.name, 'eq', entry.value)}
                          title={t('profile.applyTableFilterTitle')}
                          className="shrink-0 rounded px-1.5 py-0.5 text-[10px] text-fg-subtle opacity-0 transition-opacity hover:bg-surface-hover hover:text-accent group-hover:opacity-100 focus:opacity-100"
                        >
                          {t('profile.applyTableFilter')}
                        </button>
                      )}
                      {!onApplyTableFilter && sourceDialect && (
                        <button
                          type="button"
                          onClick={() =>
                            onCopySql(
                              whereCopyId,
                              buildSourceWhereClause(
                                column.name,
                                column.inferredType,
                                entry.value,
                                sourceDialect
                              )
                            )
                          }
                          title={t('profile.copySourceWhereTitle')}
                          className="shrink-0 rounded px-1.5 py-0.5 text-[10px] text-fg-subtle opacity-0 transition-opacity hover:bg-surface-hover hover:text-accent group-hover:opacity-100 focus:opacity-100"
                        >
                          {copiedKey === whereCopyId ? t('profile.copiedSql') : t('profile.copySourceWhere')}
                        </button>
                      )}
                    </li>
                  );
                })}
              </ul>
            </>
          )}
        </div>
      )}
    </section>
  );
}

function MetricCell({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <span className="block text-[10px] text-fg-subtle">{label}</span>
      <span className="block truncate text-fg" title={value}>
        {value}
      </span>
    </div>
  );
}
