import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import { Activity, AlertTriangle, Loader2, RefreshCw, Square, X } from 'lucide-react';
import { useLanguageStore } from '../stores/languageStore';
import { describeError } from '../utils/describeError';
import { formatProfileRatio } from '../utils/analyticsProfile';
import { isImeKeyEvent } from '../utils/shortcuts';
import { ttlView, type RedisKeyRow } from '../utils/redisKeys';
import {
  aggregateRedisKeyspace,
  DEFAULT_REDIS_SAMPLE_SIZE,
  formatRedisBytes,
  rankRedisBigKeys,
  REDIS_PREFIX_SEPARATORS,
  REDIS_SAMPLE_SIZE_OPTIONS,
  type RedisKeyspaceAnalysis,
  type RedisPrefixSeparator,
  type RedisTtlBucketId
} from '../utils/redisAnalytics';
import { PLAIN_TEXT_INPUT } from './FormControls';

interface RedisKeyspaceAnalyticsDialogProps {
  connectionString: string;
  database: number;
  initialPattern: string;
  onSelectKey: (row: RedisKeyRow) => void;
  onClose: () => void;
}

type AnalyticsTab = 'prefixes' | 'ttl' | 'bigkeys';

export function RedisKeyspaceAnalyticsDialog({
  connectionString,
  database,
  initialPattern,
  onSelectKey,
  onClose
}: RedisKeyspaceAnalyticsDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const [pattern, setPattern] = useState(initialPattern || '*');
  const [sampleLimit, setSampleLimit] = useState<number>(DEFAULT_REDIS_SAMPLE_SIZE);
  const [separator, setSeparator] = useState<RedisPrefixSeparator>(':');
  const [tab, setTab] = useState<AnalyticsTab>('prefixes');
  const [bigKeySort, setBigKeySort] = useState<'memory' | 'length'>('memory');
  const [activeTtlFilter, setActiveTtlFilter] = useState<RedisTtlBucketId | null>(null);
  const [activePrefixFilter, setActivePrefixFilter] = useState<string | null>(null);

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [analysis, setAnalysis] = useState<RedisKeyspaceAnalysis | null>(null);

  const requestIdRef = useRef<string | null>(null);
  const mountedRef = useRef(true);

  const runAnalysis = useCallback(
    async (targetPattern: string, limit: number) => {
      const prevId = requestIdRef.current;
      if (prevId) {
        void invoke('redis_cancel_analysis', { requestId: prevId }).catch(() => {});
      }
      const reqId = crypto.randomUUID();
      requestIdRef.current = reqId;
      setLoading(true);
      setError(null);
      try {
        const result = await invoke<RedisKeyspaceAnalysis>('redis_analyze_keyspace', {
          connectionString,
          database,
          pattern: targetPattern.trim() || '*',
          sampleLimit: limit,
          timeoutMs: 15_000,
          requestId: reqId
        });
        if (!mountedRef.current || requestIdRef.current !== reqId) {
          return;
        }
        setAnalysis(result);
        setActiveTtlFilter(null);
        setActivePrefixFilter(null);
      } catch (caught) {
        if (!mountedRef.current || requestIdRef.current !== reqId) {
          return;
        }
        setError(describeError(caught));
      } finally {
        if (mountedRef.current && requestIdRef.current === reqId) {
          requestIdRef.current = null;
          setLoading(false);
        }
      }
    },
    [connectionString, database]
  );

  useEffect(() => {
    mountedRef.current = true;
    void runAnalysis(initialPattern || '*', DEFAULT_REDIS_SAMPLE_SIZE);
    return () => {
      mountedRef.current = false;
      const activeId = requestIdRef.current;
      requestIdRef.current = null;
      if (activeId) {
        void invoke('redis_cancel_analysis', { requestId: activeId }).catch(() => {});
      }
    };
  }, [initialPattern, runAnalysis]);

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

  const handleCancel = () => {
    const activeId = requestIdRef.current;
    if (activeId) {
      void invoke('redis_cancel_analysis', { requestId: activeId }).catch(() => {});
    }
  };

  const summary = useMemo(
    () => (analysis ? aggregateRedisKeyspace(analysis, separator, 1) : null),
    [analysis, separator]
  );

  const bigKeys = useMemo(() => {
    if (!analysis) {
      return [];
    }
    return rankRedisBigKeys(analysis.keys, {
      sortBy: bigKeySort,
      limit: 50,
      ttlBucket: activeTtlFilter,
      prefix: activePrefixFilter,
      separator
    });
  }, [analysis, bigKeySort, activeTtlFilter, activePrefixFilter, separator]);

  const formatTtl = (ms: number): string => {
    const view = ttlView(ms);
    if (view.kind === 'persistent') return t('redis.ttl.persistent');
    if (view.kind === 'gone') return t('redis.ttl.gone');
    return view.parts.map((part) => t(`redis.ttl.${part.unit}`, { value: part.value })).join(' ');
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="redis-analytics-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[88vh] w-[960px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        {/* Header */}
        <div className="flex shrink-0 items-center justify-between border-b border-line px-5 py-3">
          <div className="flex flex-wrap items-center gap-2">
            <Activity size={16} className="text-accent" />
            <h3 id="redis-analytics-title" className="text-sm font-semibold text-fg">
              {t('redis.analytics.title')} · {`db${database}`}
            </h3>
            {analysis && (
              <span className="text-xs text-fg-muted">
                {t('redis.analytics.summary', {
                  count: analysis.sampledCount,
                  ms: analysis.elapsedMs
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

        {/* Controls */}
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-line bg-surface px-5 py-2.5 text-xs">
          <div className="flex flex-wrap items-center gap-2">
            <input
              {...PLAIN_TEXT_INPUT}
              value={pattern}
              onChange={(event) => setPattern(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && !isImeKeyEvent(event.nativeEvent)) {
                  event.preventDefault();
                  void runAnalysis(pattern, sampleLimit);
                }
              }}
              placeholder={t('redis.analytics.patternPlaceholder')}
              className="w-44 rounded-control border border-line-strong bg-surface-raised px-2.5 py-1 font-mono text-xs text-fg focus:border-accent focus:outline-none"
            />
            <select
              value={sampleLimit}
              onChange={(event) => setSampleLimit(Number(event.target.value))}
              className="rounded-control border border-line-strong bg-surface-raised px-2 py-1 text-xs text-fg"
            >
              {REDIS_SAMPLE_SIZE_OPTIONS.map((size) => (
                <option key={size} value={size}>
                  {t('redis.analytics.sampleOption', { count: size })}
                </option>
              ))}
            </select>
            <label className="flex items-center gap-1 text-fg-muted">
              <span>{t('redis.analytics.separatorLabel')}</span>
              <select
                value={separator}
                onChange={(event) => setSeparator(event.target.value as RedisPrefixSeparator)}
                className="rounded-control border border-line-strong bg-surface-raised px-1.5 py-1 font-mono text-xs text-fg"
              >
                {REDIS_PREFIX_SEPARATORS.map((sep) => (
                  <option key={sep} value={sep}>
                    {sep}
                  </option>
                ))}
              </select>
            </label>
            {loading ? (
              <button
                type="button"
                onClick={handleCancel}
                className="flex items-center gap-1 rounded-control border border-danger/40 bg-danger-soft px-2.5 py-1 text-danger"
              >
                <Square size={11} />
                <span>{t('redis.analytics.cancel')}</span>
              </button>
            ) : (
              <button
                type="button"
                onClick={() => void runAnalysis(pattern, sampleLimit)}
                className="flex items-center gap-1 rounded-control bg-accent px-2.5 py-1 font-medium text-white hover:opacity-90"
              >
                <RefreshCw size={11} />
                <span>{t('redis.analytics.start')}</span>
              </button>
            )}
          </div>

          <div className="flex overflow-hidden rounded-control border border-line" role="tablist">
            {(['prefixes', 'ttl', 'bigkeys'] as const).map((item) => (
              <button
                key={item}
                type="button"
                role="tab"
                aria-selected={tab === item}
                onClick={() => setTab(item)}
                className={clsx(
                  'px-2.5 py-1 transition-colors',
                  item !== 'prefixes' && 'border-l border-line',
                  tab === item
                    ? 'bg-accent-soft font-medium text-accent'
                    : 'text-fg-muted hover:bg-surface-hover'
                )}
              >
                {t(`redis.analytics.tab.${item}` as const)}
              </button>
            ))}
          </div>
        </div>

        {/* Notices */}
        {analysis && (analysis.partial || !analysis.memoryUsageAvailable) && (
          <div className="flex shrink-0 flex-wrap items-center gap-3 border-b border-warning-line bg-warning-soft px-5 py-2 text-xs text-warning">
            <AlertTriangle size={14} className="shrink-0" />
            {analysis.partial && (
              <span>{t('redis.analytics.partialNotice', { count: analysis.sampledCount })}</span>
            )}
            {!analysis.memoryUsageAvailable && (
              <span>{t('redis.analytics.memoryDegraded')}</span>
            )}
          </div>
        )}

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          {loading ? (
            <div className="flex flex-col items-center justify-center gap-3 py-14">
              <Loader2 size={24} className="animate-spin text-accent" />
              <p className="text-sm text-fg-muted">
                {t('redis.analytics.scanning', { count: sampleLimit })}
              </p>
            </div>
          ) : error ? (
            <p className="py-10 text-center text-sm text-danger">{error}</p>
          ) : !analysis || !summary || summary.totalKeys === 0 ? (
            <p className="py-10 text-center text-sm text-fg-muted">{t('redis.analytics.empty')}</p>
          ) : tab === 'prefixes' ? (
            <table className="w-full border-collapse text-left text-xs">
              <thead>
                <tr className="border-b border-line text-[11px] text-fg-subtle">
                  <th className="py-1.5 pr-3 font-medium">{t('redis.analytics.col.prefix')}</th>
                  <th className="py-1.5 pr-3 text-right font-medium">{t('redis.analytics.col.keys')}</th>
                  <th className="py-1.5 pr-3 text-right font-medium">
                    {analysis.memoryUsageAvailable
                      ? t('redis.analytics.col.memory')
                      : t('redis.analytics.col.weight')}
                  </th>
                  <th className="py-1.5 pr-3 text-right font-medium">{t('redis.analytics.col.noExpire')}</th>
                  <th className="py-1.5 font-medium">{t('redis.analytics.col.topKind')}</th>
                </tr>
              </thead>
              <tbody>
                {summary.prefixes.slice(0, 100).map((item) => (
                  <tr
                    key={item.prefix}
                    onClick={() => {
                      setActivePrefixFilter(item.prefix);
                      setActiveTtlFilter(null);
                      setTab('bigkeys');
                    }}
                    className="cursor-pointer border-b border-line/60 transition-colors hover:bg-surface-hover"
                  >
                    <td className="py-1.5 pr-3 font-mono text-fg">
                      <div className="flex items-center gap-2">
                        <span>{item.prefix}</span>
                        <div className="h-1.5 w-20 overflow-hidden rounded-full bg-surface-sunken">
                          <div
                            className="h-full rounded-full bg-accent"
                            style={{ width: `${Math.max(2, Math.round(item.byteRatio * 100))}%` }}
                          />
                        </div>
                      </div>
                    </td>
                    <td className="py-1.5 pr-3 text-right font-mono text-fg-muted">
                      {item.keyCount.toLocaleString()} ({formatProfileRatio(item.keyRatio)})
                    </td>
                    <td className="py-1.5 pr-3 text-right font-mono text-fg">
                      {analysis.memoryUsageAvailable
                        ? `${formatRedisBytes(item.totalBytes)} (${formatProfileRatio(item.byteRatio)})`
                        : `${item.totalBytes.toLocaleString()} (${formatProfileRatio(item.byteRatio)})`}
                    </td>
                    <td
                      className={clsx(
                        'py-1.5 pr-3 text-right font-mono',
                        item.noExpireRatio >= 0.5 ? 'text-warning' : 'text-fg-muted'
                      )}
                    >
                      {formatProfileRatio(item.noExpireRatio)}
                    </td>
                    <td className="py-1.5 font-mono text-fg-muted">{item.topKind}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : tab === 'ttl' ? (
            <div className="space-y-3">
              {summary.ttlBuckets.map((bucket) => (
                <button
                  key={bucket.id}
                  type="button"
                  onClick={() => {
                    setActiveTtlFilter(bucket.id);
                    setActivePrefixFilter(null);
                    setTab('bigkeys');
                  }}
                  className="flex w-full items-center gap-3 rounded-control border border-line bg-surface p-3 text-left transition-colors hover:border-accent/50 hover:bg-surface-hover"
                >
                  <span className="w-32 shrink-0 text-xs font-medium text-fg">
                    {t(`redis.analytics.ttl.${bucket.id}` as const)}
                  </span>
                  <div className="h-2.5 flex-1 overflow-hidden rounded-full bg-surface-sunken">
                    <div
                      className={clsx(
                        'h-full rounded-full',
                        bucket.id === 'no-expire' ? 'bg-warning' : 'bg-accent'
                      )}
                      style={{ width: `${Math.max(bucket.keyCount > 0 ? 2 : 0, Math.round(bucket.keyRatio * 100))}%` }}
                    />
                  </div>
                  <span className="w-40 shrink-0 text-right font-mono text-xs text-fg-muted">
                    {bucket.keyCount.toLocaleString()} ({formatProfileRatio(bucket.keyRatio)})
                  </span>
                  <span className="w-24 shrink-0 text-right font-mono text-xs text-fg">
                    {analysis.memoryUsageAvailable
                      ? formatRedisBytes(bucket.totalBytes)
                      : bucket.totalBytes.toLocaleString()}
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <div className="space-y-2.5">
              <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
                <div className="flex items-center gap-2">
                  <span className="text-fg-subtle">{t('redis.analytics.openKeyHint')}</span>
                  {(activePrefixFilter || activeTtlFilter) && (
                    <button
                      type="button"
                      onClick={() => {
                        setActivePrefixFilter(null);
                        setActiveTtlFilter(null);
                      }}
                      className="rounded bg-accent-soft px-2 py-0.5 text-accent hover:opacity-90"
                    >
                      {activePrefixFilter ?? (activeTtlFilter ? t(`redis.analytics.ttl.${activeTtlFilter}` as const) : '')}
                      {' · '}
                      {t('redis.analytics.clearFilter')}
                    </button>
                  )}
                </div>
                <div className="flex overflow-hidden rounded-control border border-line">
                  <button
                    type="button"
                    onClick={() => setBigKeySort('memory')}
                    className={clsx(
                      'px-2 py-0.5',
                      bigKeySort === 'memory'
                        ? 'bg-accent-soft font-medium text-accent'
                        : 'text-fg-muted hover:bg-surface-hover'
                    )}
                  >
                    {t('redis.analytics.sortMemory')}
                  </button>
                  <button
                    type="button"
                    onClick={() => setBigKeySort('length')}
                    className={clsx(
                      'border-l border-line px-2 py-0.5',
                      bigKeySort === 'length'
                        ? 'bg-accent-soft font-medium text-accent'
                        : 'text-fg-muted hover:bg-surface-hover'
                    )}
                  >
                    {t('redis.analytics.sortLength')}
                  </button>
                </div>
              </div>

              <table className="w-full border-collapse text-left text-xs">
                <thead>
                  <tr className="border-b border-line text-[11px] text-fg-subtle">
                    <th className="py-1.5 pr-3 font-medium">{t('redis.analytics.col.key')}</th>
                    <th className="py-1.5 pr-3 font-medium">{t('redis.analytics.col.kind')}</th>
                    <th className="py-1.5 pr-3 text-right font-medium">
                      {t('redis.analytics.col.memory')}
                    </th>
                    <th className="py-1.5 pr-3 text-right font-medium">
                      {t('redis.analytics.col.length')}
                    </th>
                    <th className="py-1.5 text-right font-medium">{t('redis.analytics.col.ttl')}</th>
                  </tr>
                </thead>
                <tbody>
                  {bigKeys.map((row) => (
                    <tr
                      key={row.key.raw}
                      onClick={() => {
                        onSelectKey({ key: row.key, kind: row.kind, ttlMs: row.ttlMs });
                        onClose();
                      }}
                      className="cursor-pointer border-b border-line/60 transition-colors hover:bg-surface-hover"
                    >
                      <td className="max-w-[360px] truncate py-1.5 pr-3 font-mono text-fg" title={row.key.text}>
                        {row.key.text}
                      </td>
                      <td className="py-1.5 pr-3 font-mono text-fg-muted">{row.kind}</td>
                      <td className="py-1.5 pr-3 text-right font-mono text-fg">
                        {row.memoryBytes !== undefined ? formatRedisBytes(row.memoryBytes) : '—'}
                      </td>
                      <td className="py-1.5 pr-3 text-right font-mono text-fg-muted">
                        {row.length !== undefined ? row.length.toLocaleString() : '—'}
                      </td>
                      <td className="py-1.5 text-right font-mono text-fg-muted">
                        {formatTtl(row.ttlMs)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
