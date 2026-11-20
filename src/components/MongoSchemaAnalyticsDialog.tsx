import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import { AlertTriangle, Layers, Loader2, RefreshCw, Search, X } from 'lucide-react';
import { useLanguageStore } from '../stores/languageStore';
import { describeError } from '../utils/describeError';
import { formatProfileRatio } from '../utils/analyticsProfile';
import { formatRedisBytes } from '../utils/redisAnalytics';
import { isImeKeyEvent } from '../utils/shortcuts';
import {
  buildMongoTypeFilter,
  DEFAULT_MONGO_SAMPLE_SIZE,
  filterMongoSchemaPaths,
  MONGO_SAMPLE_SIZE_OPTIONS,
  type MongoCollectionAnalysis
} from '../utils/mongoAnalytics';
import { PLAIN_TEXT_INPUT } from './FormControls';

interface MongoSchemaAnalyticsDialogProps {
  connectionString: string;
  database: string;
  collection: string;
  filter: string;
  timeoutMs: number;
  onApplyFilter: (filterText: string) => void;
  onClose: () => void;
}

type Tab = 'schema' | 'indexes';

export function MongoSchemaAnalyticsDialog({
  connectionString,
  database,
  collection,
  filter,
  timeoutMs,
  onApplyFilter,
  onClose
}: MongoSchemaAnalyticsDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const [sampleSize, setSampleSize] = useState<number>(DEFAULT_MONGO_SAMPLE_SIZE);
  const [tab, setTab] = useState<Tab>('schema');
  const [search, setSearch] = useState('');
  const [onlyDrift, setOnlyDrift] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [analysis, setAnalysis] = useState<MongoCollectionAnalysis | null>(null);

  const runAnalysis = useCallback(
    async (limit: number) => {
      setLoading(true);
      setError(null);
      try {
        const res = await invoke<MongoCollectionAnalysis>('mongodb_analyze_collection', {
          connectionString,
          database,
          collection,
          filter,
          sampleSize: limit,
          timeoutMs
        });
        setAnalysis(res);
      } catch (caught) {
        setError(describeError(caught));
      } finally {
        setLoading(false);
      }
    },
    [collection, connectionString, database, filter, timeoutMs]
  );

  useEffect(() => {
    void runAnalysis(DEFAULT_MONGO_SAMPLE_SIZE);
  }, [runAnalysis]);

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

  const visiblePaths = useMemo(
    () => filterMongoSchemaPaths(analysis?.paths ?? [], search, onlyDrift),
    [analysis?.paths, search, onlyDrift]
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="mongo-analytics-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[88vh] w-[960px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        {/* Header */}
        <div className="flex shrink-0 items-center justify-between border-b border-line px-5 py-3">
          <div className="flex flex-wrap items-center gap-2">
            <Layers size={16} className="text-accent" />
            <h3 id="mongo-analytics-title" className="text-sm font-semibold text-fg">
              {t('mongo.analytics.title')} · {`${database}.${collection}`}
            </h3>
            {filter.trim() !== '' && (
              <span className="rounded bg-accent-soft px-1.5 py-0.5 text-[10px] text-accent">
                {t('mongo.analytics.filteredBadge')}
              </span>
            )}
            {analysis && (
              <span className="text-xs text-fg-muted">
                {t('mongo.analytics.summary', {
                  count: analysis.sampledDocs,
                  paths: analysis.paths.length,
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
          <div className="flex items-center gap-2">
            <select
              value={sampleSize}
              onChange={(event) => setSampleSize(Number(event.target.value))}
              className="rounded-control border border-line-strong bg-surface-raised px-2 py-1 text-xs text-fg"
            >
              {MONGO_SAMPLE_SIZE_OPTIONS.map((size) => (
                <option key={size} value={size}>
                  {t('mongo.analytics.sampleOption', { count: size })}
                </option>
              ))}
            </select>
            <button
              type="button"
              disabled={loading}
              onClick={() => void runAnalysis(sampleSize)}
              className="flex items-center gap-1 rounded-control bg-accent px-2.5 py-1 font-medium text-white hover:opacity-90 disabled:opacity-50"
            >
              <RefreshCw size={11} className={loading ? 'animate-spin' : ''} />
              <span>{t('mongo.analytics.rescan')}</span>
            </button>
          </div>
          <div className="flex overflow-hidden rounded-control border border-line" role="tablist">
            {(['schema', 'indexes'] as const).map((item) => (
              <button
                key={item}
                type="button"
                role="tab"
                aria-selected={tab === item}
                onClick={() => setTab(item)}
                className={clsx(
                  'px-2.5 py-1 transition-colors',
                  item === 'indexes' && 'border-l border-line',
                  tab === item
                    ? 'bg-accent-soft font-medium text-accent'
                    : 'text-fg-muted hover:bg-surface-hover'
                )}
              >
                {t(`mongo.analytics.tab.${item}` as const)}
              </button>
            ))}
          </div>
        </div>

        {/* Truncation warning */}
        {analysis && analysis.truncatedPathsCount > 0 && (
          <div className="flex shrink-0 items-center gap-2 border-b border-warning-line bg-warning-soft px-5 py-2 text-xs text-warning">
            <AlertTriangle size={14} className="shrink-0" />
            <span>
              {t('mongo.analytics.truncatedPaths', { count: analysis.truncatedPathsCount })}
            </span>
          </div>
        )}

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          {loading ? (
            <div className="flex flex-col items-center justify-center gap-3 py-14">
              <Loader2 size={24} className="animate-spin text-accent" />
              <p className="text-sm text-fg-muted">
                {t('mongo.analytics.loading', { count: sampleSize })}
              </p>
            </div>
          ) : error ? (
            <p className="py-10 text-center text-sm text-danger">{error}</p>
          ) : !analysis ? (
            <p className="py-10 text-center text-sm text-fg-muted">{t('mongo.analytics.empty')}</p>
          ) : tab === 'schema' ? (
            analysis.sampledDocs === 0 ? (
              <p className="py-10 text-center text-sm text-fg-muted">{t('mongo.analytics.empty')}</p>
            ) : (
              <div className="space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
                  <div className="flex items-center gap-2">
                    <div className="relative w-64">
                      <Search
                        size={13}
                        className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-fg-subtle"
                      />
                      <input
                        {...PLAIN_TEXT_INPUT}
                        value={search}
                        onChange={(event) => setSearch(event.target.value)}
                        placeholder={t('mongo.analytics.searchPlaceholder')}
                        className="w-full rounded-control border border-line-strong bg-surface py-1 pl-8 pr-2.5 text-xs text-fg focus:border-accent focus:outline-none"
                      />
                    </div>
                    <label className="flex cursor-pointer items-center gap-1.5 text-fg-muted">
                      <input
                        type="checkbox"
                        checked={onlyDrift}
                        onChange={(event) => setOnlyDrift(event.target.checked)}
                      />
                      <span>{t('mongo.analytics.onlyDrift')}</span>
                    </label>
                  </div>
                  <span className="text-[11px] text-fg-subtle">
                    {t('mongo.analytics.clickTypeFilter')}
                  </span>
                </div>

                {visiblePaths.length === 0 ? (
                  <p className="py-8 text-center text-sm text-fg-muted">
                    {t('mongo.analytics.noMatchingPaths')}
                  </p>
                ) : (
                  <div className="space-y-2">
                    {visiblePaths.map((item) => (
                      <div
                        key={item.path}
                        className="rounded-control border border-line bg-surface p-3 text-xs"
                      >
                        <div className="flex flex-wrap items-center justify-between gap-2">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="font-mono font-semibold text-fg">{item.path}</span>
                            {item.hasTypeDrift && (
                              <span className="rounded bg-warning-soft px-1.5 py-0.5 text-[10px] font-medium text-warning">
                                {t('mongo.analytics.typeDriftBadge')}
                              </span>
                            )}
                            {item.arrayLen && (
                              <span className="rounded bg-surface-sunken px-1.5 py-0.5 font-mono text-[10px] text-fg-muted">
                                {t('mongo.analytics.arrayLen', {
                                  min: item.arrayLen.min,
                                  p95: item.arrayLen.p95,
                                  max: item.arrayLen.max
                                })}
                              </span>
                            )}
                          </div>
                          <div className="flex items-center gap-2 font-mono text-[11px] text-fg-muted">
                            <span>
                              {t('profile.presence')}: {formatProfileRatio(item.presentRatio)}
                            </span>
                            {item.missingCount > 0 && (
                              <button
                                type="button"
                                onClick={() => {
                                  onApplyFilter(buildMongoTypeFilter(item.path, 'missing'));
                                  onClose();
                                }}
                                className="rounded border border-line px-1.5 py-0.5 text-[10px] text-fg-muted hover:border-accent hover:text-accent"
                              >
                                {t('mongo.analytics.filterMissing', { count: item.missingCount })}
                              </button>
                            )}
                          </div>
                        </div>

                        <div className="mt-2 flex flex-wrap items-center gap-1.5">
                          {item.types.map((share) => (
                            <button
                              key={share.bsonType}
                              type="button"
                              onClick={() => {
                                onApplyFilter(buildMongoTypeFilter(item.path, share.bsonType));
                                onClose();
                              }}
                              className={clsx(
                                'rounded border px-2 py-0.5 font-mono text-[11px] transition-colors hover:border-accent hover:text-accent',
                                item.hasTypeDrift
                                  ? 'border-warning-line bg-warning-soft text-warning'
                                  : 'border-line bg-surface-sunken text-fg'
                              )}
                            >
                              {share.bsonType} {formatProfileRatio(share.ratio)} ({share.count})
                            </button>
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )
          ) : (
            <div className="space-y-4">
              {analysis.statsUnavailableReason && (
                <div className="flex items-start gap-2 rounded-control border border-warning-line bg-warning-soft p-3 text-xs text-warning">
                  <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                  <span>
                    {t('mongo.analytics.statsUnavailable', {
                      reason: analysis.statsUnavailableReason
                    })}
                  </span>
                </div>
              )}

              {analysis.storage && (
                <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                  <StatCard
                    label={t('mongo.analytics.storageDocs')}
                    value={analysis.storage.count.toLocaleString()}
                  />
                  <StatCard
                    label={t('mongo.analytics.storageAvgSize')}
                    value={formatRedisBytes(analysis.storage.avgObjSize)}
                  />
                  <StatCard
                    label={t('mongo.analytics.storageSize')}
                    value={formatRedisBytes(analysis.storage.storageSize)}
                  />
                  <StatCard
                    label={t('mongo.analytics.storageIndexSize')}
                    value={formatRedisBytes(analysis.storage.totalIndexSize)}
                  />
                </div>
              )}

              {analysis.indexUsage.length > 0 && (
                <table className="w-full border-collapse text-left text-xs">
                  <thead>
                    <tr className="border-b border-line text-[11px] text-fg-subtle">
                      <th className="py-1.5 pr-3 font-medium">
                        {t('mongo.analytics.col.indexName')}
                      </th>
                      <th className="py-1.5 pr-3 text-right font-medium">
                        {t('mongo.analytics.col.ops')}
                      </th>
                      <th className="py-1.5 pr-3 font-medium">{t('mongo.analytics.col.since')}</th>
                      <th className="py-1.5 font-medium">{t('mongo.analytics.col.status')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {analysis.indexUsage.map((idx) => (
                      <tr key={idx.name} className="border-b border-line/60">
                        <td className="py-1.5 pr-3 font-mono text-fg">{idx.name}</td>
                        <td className="py-1.5 pr-3 text-right font-mono text-fg">
                          {idx.ops.toLocaleString()}
                        </td>
                        <td className="py-1.5 pr-3 font-mono text-fg-muted">{idx.since ?? '—'}</td>
                        <td className="py-1.5">
                          {idx.unused ? (
                            <span className="rounded bg-warning-soft px-1.5 py-0.5 text-[10px] font-medium text-warning">
                              {t('mongo.analytics.unusedIndex')}
                            </span>
                          ) : (
                            <span className="text-[11px] text-fg-muted">
                              {t('mongo.analytics.activeIndex')}
                            </span>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-control border border-line bg-surface p-3">
      <p className="text-[11px] text-fg-subtle">{label}</p>
      <p className="mt-1 font-mono text-sm font-semibold text-fg">{value}</p>
    </div>
  );
}
