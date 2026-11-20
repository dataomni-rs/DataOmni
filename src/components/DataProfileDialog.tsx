import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import {
  AlertTriangle,
  BarChart2,
  Check,
  Copy,
  Loader2,
  RotateCcw,
  Search,
  X
} from 'lucide-react';
import type {
  AnalyticsColumnInput,
  AnalyticsSessionSummary,
  ColumnProfile,
  ProfileResult
} from '../contracts/analytics';
import type { SerializedResultValue } from '../contracts/resultSet';
import { useLanguageStore } from '../stores/languageStore';
import {
  buildProfileFilterSql,
  buildProfileNullFilterSql,
  filterAndSortProfiles,
  formatProfileNumber,
  formatProfileRatio,
  hasSchemaDrift,
  histogramHeights,
  nullSeverity,
  type ProfileSortMode
} from '../utils/analyticsProfile';
import { describeError } from '../utils/describeError';
import { isImeKeyEvent } from '../utils/shortcuts';
import { PLAIN_TEXT_INPUT } from './FormControls';

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
  onClose: () => void;
}

const SORT_OPTIONS: readonly ProfileSortMode[] = ['order', 'nulls', 'distinct', 'drift'];

export function DataProfileDialog({ source, onClose }: DataProfileDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const [loading, setLoading] = useState(true);
  const [cancelled, setCancelled] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [summary, setSummary] = useState<AnalyticsSessionSummary | null>(null);
  const [profile, setProfile] = useState<ProfileResult | null>(null);
  const [search, setSearch] = useState('');
  const [sortMode, setSortMode] = useState<ProfileSortMode>('order');
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  const sessionIdRef = useRef<string | null>(null);
  const mountedRef = useRef(true);

  const inputRowCount = source.kind === 'resultSet' ? source.rows.length : source.documents.length;

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
      if (event.key === 'Escape' && !isImeKeyEvent(event)) {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  useEffect(() => {
    if (inputRowCount === 0) {
      setLoading(false);
      setError(null);
      setProfile(null);
      return;
    }

    let stale = false;
    setLoading(true);
    setCancelled(false);
    setError(null);

    const run = async () => {
      const previousId = sessionIdRef.current;
      sessionIdRef.current = null;
      if (previousId) {
        void invoke('analytics_close', { analyticsId: previousId }).catch(() => {});
      }

      try {
        const loaded =
          source.kind === 'resultSet'
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
        if (code === 'DATAOMNI_ANALYTICS_CANCELLED' || message.includes('DATAOMNI_ANALYTICS_CANCELLED')) {
          setCancelled(true);
        } else {
          setError(message);
        }
      } finally {
        if (!stale && mountedRef.current) {
          setLoading(false);
        }
      }
    };

    void run();
    return () => {
      stale = true;
    };
  }, [source, inputRowCount, attempt]);

  const handleCancel = useCallback(() => {
    const id = sessionIdRef.current;
    if (id) {
      void invoke('analytics_cancel', { analyticsId: id }).catch(() => {});
    }
    setCancelled(true);
  }, []);

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
        <div className="flex shrink-0 items-center justify-between border-b border-line px-5 py-3">
          <div className="flex items-center gap-2">
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
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t('common.close')}
            className="rounded-control p-1 text-fg-muted transition-colors hover:bg-surface-hover"
          >
            <X size={16} />
          </button>
        </div>

        {/* Truncated banner */}
        {summary?.truncated && (
          <div className="flex shrink-0 items-center gap-2 border-b border-warning-line bg-warning-soft px-5 py-2 text-xs text-warning">
            <AlertTriangle size={14} className="shrink-0" />
            <span>{t('profile.truncatedNotice', { count: summary.rowCount })}</span>
          </div>
        )}

        {/* Toolbar */}
        {profile && !loading && (
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
        )}

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          {inputRowCount === 0 ? (
            <p className="py-8 text-center text-sm text-fg-muted">{t('profile.empty')}</p>
          ) : loading ? (
            <div className="flex flex-col items-center justify-center gap-3 py-12">
              <Loader2 size={24} className="animate-spin text-accent" />
              <p className="text-sm text-fg-muted">
                {t('profile.loading', { count: inputRowCount })}
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
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export interface ColumnProfileCardProps {
  column: ColumnProfile;
  copiedKey: string | null;
  onCopySql: (key: string, sql: string) => void;
  /** 当宿主工作台支持直接执行二次 SQL 时，点击高频值/空值直接跳转执行而非仅复制 */
  onRunFilterSql?: (sql: string) => void;
}

export function ColumnProfileCard({
  column,
  copiedKey,
  onCopySql,
  onRunFilterSql
}: ColumnProfileCardProps) {
  const t = useLanguageStore((state) => state.t);
  const severity = nullSeverity(column.nullRatio);
  const drift = hasSchemaDrift(column);
  const heights = useMemo(() => histogramHeights(column.bins), [column.bins]);
  const nullCopyId = `null:${column.name}`;

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
            className="flex shrink-0 items-center gap-1 rounded px-1.5 py-0.5 text-[10px] text-fg-subtle transition-colors hover:bg-surface-hover hover:text-fg"
          >
            {copiedKey === nullCopyId ? <Check size={11} className="text-success" /> : <Copy size={11} />}
            <span>{copiedKey === nullCopyId ? t('profile.copiedSql') : 'IS NULL'}</span>
          </button>
        )}
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
          </div>
        )}

      {/* Temporal min/max */}
      {(column.inferredType === 'date'
        || column.inferredType === 'time'
        || column.inferredType === 'datetime')
        && column.min !== undefined && (
          <div className="text-[11px]">
            <span className="text-fg-subtle">{t('profile.range')}: </span>
            <span className="font-mono text-fg">
              {column.min} → {column.max ?? '—'}
            </span>
          </div>
        )}

      {/* Length / Byte length range */}
      {column.minLength !== undefined && column.maxLength !== undefined && (
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
      )}

      {/* Top 5 values */}
      {column.topValues.length > 0 && (
        <div className="space-y-1">
          <p className="text-[10px] text-fg-subtle">{t('profile.topValues')}</p>
          <ul className="space-y-1">
            {column.topValues.map((entry, idx) => {
              const copyId = `top:${column.name}:${idx}`;
              const display = entry.value === '' ? '""' : entry.value;
              return (
                <li key={copyId} className="group flex items-center gap-2 text-[11px]">
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
                    className="relative flex min-w-0 flex-1 items-center justify-between overflow-hidden rounded bg-surface-sunken px-2 py-0.5 text-left transition-colors hover:bg-surface-hover"
                  >
                    <span
                      aria-hidden
                      className="pointer-events-none absolute inset-y-0 left-0 bg-accent-soft opacity-60"
                      style={{ width: `${Math.max(2, Math.round(entry.ratio * 100))}%` }}
                    />
                    <span className="relative truncate font-mono text-fg" title={display}>
                      {display}
                    </span>
                    <span className="relative ml-2 shrink-0 font-mono text-[10px] text-fg-muted">
                      {copiedKey === copyId
                        ? t('profile.copiedSql')
                        : `${entry.count.toLocaleString()} (${formatProfileRatio(entry.ratio)})`}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
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
