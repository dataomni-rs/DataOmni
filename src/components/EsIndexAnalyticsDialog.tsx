import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import { AlertTriangle, BarChart3, Loader2, Play, X } from 'lucide-react';
import { useLanguageStore } from '../stores/languageStore';
import { describeError } from '../utils/describeError';
import { formatProfileNumber, formatProfileRatio } from '../utils/analyticsProfile';
import { isImeKeyEvent } from '../utils/shortcuts';
import type { MappingField } from '../utils/esMapping';
import {
  buildEsDateHistogramBody,
  buildEsNumericStatsBody,
  buildEsTermsAggBody,
  ES_DATE_INTERVAL_OPTIONS,
  esBucketsToChartTable,
  parseEsDateHistogramResponse,
  parseEsIndexHealth,
  parseEsNumericStatsResponse,
  parseEsTermsResponse,
  summarizeEsMapping,
  type EsDateInterval,
  type EsIndexHealthSummary,
  type EsNumericStatsResult,
  type EsTermsBucket,
  type EsTermsResult
} from '../utils/esAnalytics';
import { ResultChartDialog } from './ResultChartDialog';

interface EsResponse {
  status: number;
  body: string;
  elapsedMs?: number;
}

interface EsIndexAnalyticsDialogProps {
  connectionString: string;
  index: string;
  mappingFields: readonly MappingField[];
  timeoutMs: number;
  onClose: () => void;
}

type AggTab = 'timeline' | 'terms' | 'numeric';

export function EsIndexAnalyticsDialog({
  connectionString,
  index,
  mappingFields,
  timeoutMs,
  onClose
}: EsIndexAnalyticsDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const diagnostics = useMemo(() => summarizeEsMapping(mappingFields), [mappingFields]);
  const [tab, setTab] = useState<AggTab>(
    diagnostics.dateFields.length > 0
      ? 'timeline'
      : diagnostics.keywordFields.length > 0
        ? 'terms'
        : 'numeric'
  );
  const [dateField, setDateField] = useState(diagnostics.dateFields[0] ?? '');
  const [interval, setInterval] = useState<EsDateInterval>('1h');
  const [keywordField, setKeywordField] = useState(diagnostics.keywordFields[0] ?? '');
  const [numericField, setNumericField] = useState(diagnostics.numericFields[0] ?? '');

  const [health, setHealth] = useState<EsIndexHealthSummary | null>(null);
  const [loadingAgg, setLoadingAgg] = useState(false);
  const [aggError, setAggError] = useState<string | null>(null);
  const [timelineBuckets, setTimelineBuckets] = useState<EsTermsBucket[] | null>(null);
  const [termsResult, setTermsResult] = useState<EsTermsResult | null>(null);
  const [numericStats, setNumericStats] = useState<EsNumericStatsResult | null>(null);
  const [chartState, setChartState] = useState<{ labelCol: string; buckets: readonly EsTermsBucket[] } | null>(null);

  const esRequest = useCallback(
    (method: 'GET' | 'POST', path: string, body: string | null) =>
      invoke<EsResponse>('elasticsearch_run', {
        connectionString,
        method,
        path,
        body,
        ndjson: false,
        timeoutMs
      }),
    [connectionString, timeoutMs]
  );

  // 可选监控路径：_cat/indices + _cat/shards，403 时局部降级不阻塞只读聚合
  useEffect(() => {
    let stale = false;
    const encoded = encodeURIComponent(index);
    const loadHealth = async () => {
      const [indicesResp, shardsResp] = await Promise.all([
        esRequest('GET', `/_cat/indices/${encoded}?format=json`, null).catch(() => null),
        esRequest('GET', `/_cat/shards/${encoded}?format=json`, null).catch(() => null)
      ]);
      if (!stale) {
        setHealth(parseEsIndexHealth(indicesResp, shardsResp));
      }
    };
    void loadHealth();
    return () => {
      stale = true;
    };
  }, [esRequest, index]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImeKeyEvent(event) && !chartState) {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [chartState, onClose]);

  const runCurrentAgg = useCallback(async () => {
    const encoded = encodeURIComponent(index);
    setLoadingAgg(true);
    setAggError(null);
    try {
      if (tab === 'timeline' && dateField) {
        const resp = await esRequest('POST', `/${encoded}/_search`, buildEsDateHistogramBody(dateField, interval));
        if (resp.status >= 300) {
          throw new Error(`HTTP ${resp.status}: ${resp.body.slice(0, 300)}`);
        }
        setTimelineBuckets(parseEsDateHistogramResponse(resp.body));
      } else if (tab === 'terms' && keywordField) {
        const resp = await esRequest('POST', `/${encoded}/_search`, buildEsTermsAggBody(keywordField, 20));
        if (resp.status >= 300) {
          throw new Error(`HTTP ${resp.status}: ${resp.body.slice(0, 300)}`);
        }
        setTermsResult(parseEsTermsResponse(resp.body));
      } else if (tab === 'numeric' && numericField) {
        const resp = await esRequest('POST', `/${encoded}/_search`, buildEsNumericStatsBody(numericField));
        if (resp.status >= 300) {
          throw new Error(`HTTP ${resp.status}: ${resp.body.slice(0, 300)}`);
        }
        setNumericStats(parseEsNumericStatsResponse(resp.body));
      }
    } catch (caught) {
      setAggError(describeError(caught));
    } finally {
      setLoadingAgg(false);
    }
  }, [dateField, esRequest, index, interval, keywordField, numericField, tab]);

  useEffect(() => {
    if (
      (tab === 'timeline' && dateField)
      || (tab === 'terms' && keywordField)
      || (tab === 'numeric' && numericField)
    ) {
      void runCurrentAgg();
    }
  }, [dateField, interval, keywordField, numericField, runCurrentAgg, tab]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="es-analytics-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[88vh] w-[960px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        {/* Header */}
        <div className="flex shrink-0 items-center justify-between border-b border-line px-5 py-3">
          <div className="flex items-center gap-2">
            <BarChart3 size={16} className="text-accent" />
            <h3 id="es-analytics-title" className="text-sm font-semibold text-fg">
              {t('es.analytics.title')} · {index}
            </h3>
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

        {/* Mapping & Health summary strip */}
        <div className="shrink-0 space-y-2 border-b border-line bg-surface px-5 py-3 text-xs">
          <div className="flex flex-wrap items-center gap-4">
            <span>
              <span className="text-fg-subtle">{t('es.analytics.mappingFields')}: </span>
              <span
                className={clsx(
                  'font-mono font-semibold',
                  diagnostics.explosionSeverity === 'critical'
                    ? 'text-danger'
                    : diagnostics.explosionSeverity === 'warn'
                      ? 'text-warning'
                      : 'text-fg'
                )}
              >
                {diagnostics.totalFields.toLocaleString()}
              </span>
            </span>
            <span>
              <span className="text-fg-subtle">{t('es.analytics.dualStorage')}: </span>
              <span className="font-mono text-fg">{diagnostics.dualTextKeywordCount}</span>
            </span>
            {health && !health.permissionDenied && health.health && (
              <>
                <span>
                  <span className="text-fg-subtle">{t('es.analytics.healthStatus')}: </span>
                  <span className="font-mono font-semibold text-fg">{health.health}</span>
                </span>
                {health.primaryShards !== null && (
                  <span>
                    <span className="text-fg-subtle">{t('es.analytics.shardsSummary')}: </span>
                    <span className="font-mono text-fg">
                      {health.primaryShards}/{health.replicaShards ?? 0}
                    </span>
                  </span>
                )}
                {health.storeSize && (
                  <span>
                    <span className="text-fg-subtle">{t('es.analytics.storeSize')}: </span>
                    <span className="font-mono text-fg">{health.storeSize}</span>
                  </span>
                )}
                {health.deletedRatio !== null && (
                  <span>
                    <span className="text-fg-subtle">{t('es.analytics.deletedRatio')}: </span>
                    <span
                      className={clsx(
                        'font-mono',
                        health.deletedRatio >= 0.2 ? 'text-warning' : 'text-fg'
                      )}
                    >
                      {formatProfileRatio(health.deletedRatio)}
                    </span>
                  </span>
                )}
              </>
            )}
          </div>

          {diagnostics.explosionSeverity !== 'normal' && (
            <div className="flex items-center gap-1.5 text-warning">
              <AlertTriangle size={13} className="shrink-0" />
              <span>{t('es.analytics.mappingWarn', { count: diagnostics.totalFields })}</span>
            </div>
          )}

          {health?.permissionDenied && (
            <p className="text-[11px] text-fg-muted">{t('es.analytics.healthDenied')}</p>
          )}

          {health && !health.permissionDenied && (health.unassignedShards > 0 || health.oversizedShards > 0 || health.undersizedShards > 0) && (
            <p className="text-[11px] text-warning">
              {t('es.analytics.shardAlerts', {
                unassigned: health.unassignedShards,
                oversized: health.oversizedShards,
                undersized: health.undersizedShards
              })}
            </p>
          )}
        </div>

        {/* Tab & field controls */}
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-line px-5 py-2.5 text-xs">
          <div className="flex overflow-hidden rounded-control border border-line" role="tablist">
            {(['timeline', 'terms', 'numeric'] as const).map((item) => (
              <button
                key={item}
                type="button"
                role="tab"
                aria-selected={tab === item}
                onClick={() => setTab(item)}
                className={clsx(
                  'px-2.5 py-1 transition-colors',
                  item !== 'timeline' && 'border-l border-line',
                  tab === item
                    ? 'bg-accent-soft font-medium text-accent'
                    : 'text-fg-muted hover:bg-surface-hover'
                )}
              >
                {t(`es.analytics.tab.${item}` as const)}
              </button>
            ))}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            {tab === 'timeline' && diagnostics.dateFields.length > 0 && (
              <>
                <select
                  value={dateField}
                  onChange={(event) => setDateField(event.target.value)}
                  className="rounded-control border border-line-strong bg-surface px-2 py-1 font-mono text-xs text-fg"
                >
                  {diagnostics.dateFields.map((f) => (
                    <option key={f} value={f}>{f}</option>
                  ))}
                </select>
                <select
                  value={interval}
                  onChange={(event) => setInterval(event.target.value as EsDateInterval)}
                  className="rounded-control border border-line-strong bg-surface px-2 py-1 font-mono text-xs text-fg"
                >
                  {ES_DATE_INTERVAL_OPTIONS.map((opt) => (
                    <option key={opt} value={opt}>{opt}</option>
                  ))}
                </select>
              </>
            )}
            {tab === 'terms' && diagnostics.keywordFields.length > 0 && (
              <select
                value={keywordField}
                onChange={(event) => setKeywordField(event.target.value)}
                className="rounded-control border border-line-strong bg-surface px-2 py-1 font-mono text-xs text-fg"
              >
                {diagnostics.keywordFields.map((f) => (
                  <option key={f} value={f}>{f}</option>
                ))}
              </select>
            )}
            {tab === 'numeric' && diagnostics.numericFields.length > 0 && (
              <select
                value={numericField}
                onChange={(event) => setNumericField(event.target.value)}
                className="rounded-control border border-line-strong bg-surface px-2 py-1 font-mono text-xs text-fg"
              >
                {diagnostics.numericFields.map((f) => (
                  <option key={f} value={f}>{f}</option>
                ))}
              </select>
            )}
            <button
              type="button"
              disabled={loadingAgg}
              onClick={() => void runCurrentAgg()}
              className="flex items-center gap-1 rounded-control bg-accent px-2.5 py-1 font-medium text-white hover:opacity-90 disabled:opacity-50"
            >
              <Play size={11} />
              <span>{t('es.analytics.runAgg')}</span>
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          {loadingAgg ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 size={22} className="animate-spin text-accent" />
            </div>
          ) : aggError ? (
            <p className="py-8 text-center text-sm text-danger">{aggError}</p>
          ) : tab === 'timeline' ? (
            diagnostics.dateFields.length === 0 ? (
              <p className="py-8 text-center text-sm text-fg-muted">{t('es.analytics.noDateFields')}</p>
            ) : (
              <BucketListView
                buckets={timelineBuckets ?? []}
                onOpenChart={() => setChartState({ labelCol: dateField, buckets: timelineBuckets ?? [] })}
                openChartLabel={t('es.analytics.openChart')}
              />
            )
          ) : tab === 'terms' ? (
            diagnostics.keywordFields.length === 0 ? (
              <p className="py-8 text-center text-sm text-fg-muted">{t('es.analytics.noKeywordFields')}</p>
            ) : termsResult ? (
              <div className="space-y-3">
                <p className="text-xs text-fg-muted">
                  {t('es.analytics.missingDocs', { count: termsResult.missingCount })}
                </p>
                <BucketListView
                  buckets={termsResult.buckets}
                  onOpenChart={() => setChartState({ labelCol: keywordField, buckets: termsResult.buckets })}
                  openChartLabel={t('es.analytics.openChart')}
                />
              </div>
            ) : null
          ) : diagnostics.numericFields.length === 0 ? (
            <p className="py-8 text-center text-sm text-fg-muted">{t('es.analytics.noNumericFields')}</p>
          ) : numericStats ? (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
              {[
                ['Count', numericStats.count.toLocaleString()],
                ['Min', formatProfileNumber(numericStats.min ?? undefined)],
                ['Avg', formatProfileNumber(numericStats.avg ?? undefined)],
                ['StdDev', formatProfileNumber(numericStats.stdDeviation ?? undefined)],
                ['Max', formatProfileNumber(numericStats.max ?? undefined)],
                ['P25', formatProfileNumber(numericStats.p25 ?? undefined)],
                ['P50', formatProfileNumber(numericStats.p50 ?? undefined)],
                ['P75', formatProfileNumber(numericStats.p75 ?? undefined)],
                ['P95', formatProfileNumber(numericStats.p95 ?? undefined)],
                ['P99', formatProfileNumber(numericStats.p99 ?? undefined)]
              ].map(([label, val]) => (
                <div key={label} className="rounded-control border border-line bg-surface p-3">
                  <p className="text-[11px] text-fg-subtle">{label}</p>
                  <p className="mt-1 font-mono text-sm font-semibold text-fg">{val}</p>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      </div>

      {chartState && (() => {
        const table = esBucketsToChartTable(chartState.labelCol, chartState.buckets);
        return (
          <ResultChartDialog
            columns={table.columns}
            rows={table.rows}
            onClose={() => setChartState(null)}
          />
        );
      })()}
    </div>
  );
}

function BucketListView({
  buckets,
  onOpenChart,
  openChartLabel
}: {
  buckets: readonly EsTermsBucket[];
  onOpenChart: () => void;
  openChartLabel: string;
}) {
  const maxCount = buckets.reduce((best, item) => Math.max(best, item.count), 0);
  if (buckets.length === 0) {
    return null;
  }
  return (
    <div className="space-y-2">
      <div className="flex justify-end">
        <button
          type="button"
          onClick={onOpenChart}
          className="flex items-center gap-1 rounded-control border border-line-strong px-2.5 py-1 text-xs text-fg hover:bg-surface-hover"
        >
          <BarChart3 size={12} />
          <span>{openChartLabel}</span>
        </button>
      </div>
      <div className="space-y-1.5">
        {buckets.slice(0, 100).map((b) => (
          <div key={b.key} className="flex items-center gap-3 text-xs">
            <span className="w-48 shrink-0 truncate font-mono text-fg" title={b.key}>
              {b.key}
            </span>
            <div className="h-2 flex-1 overflow-hidden rounded-full bg-surface-sunken">
              <div
                className="h-full rounded-full bg-accent"
                style={{ width: `${maxCount > 0 ? Math.max(b.count > 0 ? 2 : 0, Math.round((b.count / maxCount) * 100)) : 0}%` }}
              />
            </div>
            <span className="w-20 shrink-0 text-right font-mono text-fg-muted">
              {b.count.toLocaleString()}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
