import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import { BarChart3, Loader2, Network, Play, X } from 'lucide-react';
import { useLanguageStore } from '../stores/languageStore';
import { describeError } from '../utils/describeError';
import { formatProfileRatio } from '../utils/analyticsProfile';
import { isImeKeyEvent } from '../utils/shortcuts';
import type { CypherValue } from '../utils/cypherValue';
import {
  buildInspectLabelCypher,
  buildInspectSupernodeCypher,
  buildLabelCountCypher,
  buildNeo4jDegreeSampleCypher,
  buildRelationshipCountCypher,
  cypherValueAsNumber,
  cypherValueAsString,
  DEFAULT_NEO4J_SAMPLE_LIMIT,
  MAX_NEO4J_OVERVIEW_ITEMS,
  NEO4J_SAMPLE_LIMIT_OPTIONS,
  neo4jCountsToChartTable,
  summarizeNeo4jDegreeSample,
  type Neo4jCountItem,
  type Neo4jDegreeSampleSummary
} from '../utils/neo4jAnalytics';
import { ResultChartDialog } from './ResultChartDialog';

interface CypherRawResult {
  columns: string[];
  rows: CypherValue[][];
}

interface Neo4jTopologyAnalyticsDialogProps {
  connectionString: string;
  timeoutMs: number;
  onRunCypher: (statement: string) => void;
  onClose: () => void;
}

type Tab = 'overview' | 'topology';

export function Neo4jTopologyAnalyticsDialog({
  connectionString,
  timeoutMs,
  onRunCypher,
  onClose
}: Neo4jTopologyAnalyticsDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const [tab, setTab] = useState<Tab>('overview');
  const [loadingOverview, setLoadingOverview] = useState(true);
  const [overviewError, setOverviewError] = useState<string | null>(null);
  const [labelCounts, setLabelCounts] = useState<Neo4jCountItem[]>([]);
  const [relCounts, setRelCounts] = useState<Neo4jCountItem[]>([]);

  const [selectedLabel, setSelectedLabel] = useState<string>('');
  const [sampleLimit, setSampleLimit] = useState<number>(DEFAULT_NEO4J_SAMPLE_LIMIT);
  const [loadingSample, setLoadingSample] = useState(false);
  const [sampleError, setSampleError] = useState<string | null>(null);
  const [sampleSummary, setSampleSummary] = useState<Neo4jDegreeSampleSummary | null>(null);
  const [usedLegacyId, setUsedLegacyId] = useState(false);

  const [chartState, setChartState] = useState<{ labelCol: string; items: readonly Neo4jCountItem[] } | null>(null);

  const runReadOnly = useCallback(
    (query: string, limit = 1000) =>
      invoke<CypherRawResult>('neo4j_run', {
        connectionString,
        database: null,
        query,
        limit,
        timeoutMs: Math.min(timeoutMs, 30_000),
        readAll: false,
        readOnly: true
      }),
    [connectionString, timeoutMs]
  );

  useEffect(() => {
    let stale = false;
    const loadOverview = async () => {
      setLoadingOverview(true);
      setOverviewError(null);
      try {
        const [labelsRes, relsRes] = await Promise.all([
          runReadOnly('CALL db.labels() YIELD label RETURN label ORDER BY label', 200),
          runReadOnly(
            'CALL db.relationshipTypes() YIELD relationshipType RETURN relationshipType ORDER BY relationshipType',
            200
          )
        ]);
        const labels = labelsRes.rows
          .map((row) => cypherValueAsString(row[0]))
          .filter((name) => name.length > 0)
          .slice(0, MAX_NEO4J_OVERVIEW_ITEMS);
        const rels = relsRes.rows
          .map((row) => cypherValueAsString(row[0]))
          .filter((name) => name.length > 0)
          .slice(0, MAX_NEO4J_OVERVIEW_ITEMS);

        const [countedLabels, countedRels] = await Promise.all([
          Promise.all(
            labels.map(async (name): Promise<Neo4jCountItem> => {
              const res = await runReadOnly(buildLabelCountCypher(name), 1);
              return { name, count: cypherValueAsNumber(res.rows[0]?.[0]) };
            })
          ),
          Promise.all(
            rels.map(async (name): Promise<Neo4jCountItem> => {
              const res = await runReadOnly(buildRelationshipCountCypher(name), 1);
              return { name, count: cypherValueAsNumber(res.rows[0]?.[0]) };
            })
          )
        ]);

        if (stale) return;
        countedLabels.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
        countedRels.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
        setLabelCounts(countedLabels);
        setRelCounts(countedRels);
        if (countedLabels.length > 0) {
          setSelectedLabel((cur) => cur || countedLabels[0].name);
        }
      } catch (caught) {
        if (!stale) {
          setOverviewError(describeError(caught));
        }
      } finally {
        if (!stale) {
          setLoadingOverview(false);
        }
      }
    };
    void loadOverview();
    return () => {
      stale = true;
    };
  }, [runReadOnly]);

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

  const runDegreeSample = async () => {
    setLoadingSample(true);
    setSampleError(null);
    const label = selectedLabel || null;
    try {
      let legacy = false;
      let res: CypherRawResult;
      try {
        res = await runReadOnly(buildNeo4jDegreeSampleCypher(label, sampleLimit, false), sampleLimit);
      } catch {
        // 兼容 Neo4j 4.x（不支持 COUNT { ... } 与 elementId）
        legacy = true;
        res = await runReadOnly(buildNeo4jDegreeSampleCypher(label, sampleLimit, true), sampleLimit);
      }
      setUsedLegacyId(legacy);
      setSampleSummary(summarizeNeo4jDegreeSample(res.rows));
    } catch (caught) {
      setSampleError(describeError(caught));
    } finally {
      setLoadingSample(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="neo4j-analytics-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[88vh] w-[940px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        {/* Header */}
        <div className="flex shrink-0 items-center justify-between border-b border-line px-5 py-3">
          <div className="flex items-center gap-2">
            <Network size={16} className="text-accent" />
            <h3 id="neo4j-analytics-title" className="text-sm font-semibold text-fg">
              {t('cypher.analytics.title')}
            </h3>
          </div>
          <div className="flex items-center gap-3">
            <div className="flex overflow-hidden rounded-control border border-line text-xs" role="tablist">
              {(['overview', 'topology'] as const).map((item) => (
                <button
                  key={item}
                  type="button"
                  role="tab"
                  aria-selected={tab === item}
                  onClick={() => setTab(item)}
                  className={clsx(
                    'px-2.5 py-1 transition-colors',
                    item === 'topology' && 'border-l border-line',
                    tab === item
                      ? 'bg-accent-soft font-medium text-accent'
                      : 'text-fg-muted hover:bg-surface-hover'
                  )}
                >
                  {t(`cypher.analytics.tab.${item}` as const)}
                </button>
              ))}
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
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          {tab === 'overview' ? (
            loadingOverview ? (
              <div className="flex items-center justify-center py-12">
                <Loader2 size={22} className="animate-spin text-accent" />
              </div>
            ) : overviewError ? (
              <p className="py-8 text-center text-sm text-danger">{overviewError}</p>
            ) : labelCounts.length === 0 && relCounts.length === 0 ? (
              <p className="py-8 text-center text-sm text-fg-muted">{t('cypher.analytics.empty')}</p>
            ) : (
              <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                <CountSection
                  title={t('cypher.analytics.labelsHeading')}
                  items={labelCounts}
                  openChartLabel={t('cypher.analytics.openChart')}
                  onOpenChart={() => setChartState({ labelCol: 'label', items: labelCounts })}
                  onClickItem={(name) => {
                    onRunCypher(buildInspectLabelCypher(name));
                    onClose();
                  }}
                />
                <CountSection
                  title={t('cypher.analytics.relsHeading')}
                  items={relCounts}
                  openChartLabel={t('cypher.analytics.openChart')}
                  onOpenChart={() => setChartState({ labelCol: 'relationship', items: relCounts })}
                />
              </div>
            )
          ) : (
            <div className="space-y-4">
              <div className="flex flex-wrap items-center gap-2 text-xs">
                <select
                  value={selectedLabel}
                  onChange={(event) => setSelectedLabel(event.target.value)}
                  className="rounded-control border border-line-strong bg-surface px-2.5 py-1 font-mono text-xs text-fg"
                >
                  <option value="">{t('cypher.analytics.allLabels')}</option>
                  {labelCounts.map((item) => (
                    <option key={item.name} value={item.name}>
                      {item.name} ({item.count.toLocaleString()})
                    </option>
                  ))}
                </select>
                <select
                  value={sampleLimit}
                  onChange={(event) => setSampleLimit(Number(event.target.value))}
                  className="rounded-control border border-line-strong bg-surface px-2 py-1 text-xs text-fg"
                >
                  {NEO4J_SAMPLE_LIMIT_OPTIONS.map((size) => (
                    <option key={size} value={size}>
                      {t('cypher.analytics.sampleOption', { count: size })}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  disabled={loadingSample}
                  onClick={() => void runDegreeSample()}
                  className="flex items-center gap-1 rounded-control bg-accent px-3 py-1 font-medium text-white hover:opacity-90 disabled:opacity-50"
                >
                  {loadingSample ? <Loader2 size={12} className="animate-spin" /> : <Play size={12} />}
                  <span>{t('cypher.analytics.runSample')}</span>
                </button>
                {sampleSummary && (
                  <span className="text-fg-muted">
                    {t('cypher.analytics.sampledSummary', { count: sampleSummary.sampledNodes })}
                  </span>
                )}
              </div>

              {sampleError && <p className="text-sm text-danger">{sampleError}</p>}

              {sampleSummary && (
                <>
                  <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                    <MetricBox
                      label={t('cypher.analytics.isolatedRatio')}
                      value={`${formatProfileRatio(sampleSummary.isolatedRatio)} (${sampleSummary.isolatedCount})`}
                    />
                    <MetricBox
                      label={t('cypher.analytics.degreeP50')}
                      value={sampleSummary.p50Degree.toLocaleString()}
                    />
                    <MetricBox
                      label={t('cypher.analytics.degreeP95')}
                      value={sampleSummary.p95Degree.toLocaleString()}
                    />
                    <MetricBox
                      label={t('cypher.analytics.degreeMax')}
                      value={sampleSummary.maxDegree.toLocaleString()}
                    />
                  </div>

                  <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                    <div className="space-y-2">
                      <h4 className="text-xs font-semibold text-fg">
                        {t('cypher.analytics.supernodes')}
                      </h4>
                      <table className="w-full border-collapse text-left text-xs">
                        <thead>
                          <tr className="border-b border-line text-[11px] text-fg-subtle">
                            <th className="py-1 pr-2 font-medium">{t('cypher.analytics.col.nodeId')}</th>
                            <th className="py-1 pr-2 font-medium">{t('cypher.analytics.col.labels')}</th>
                            <th className="py-1 text-right font-medium">{t('cypher.analytics.col.degree')}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {sampleSummary.supernodes.map((node) => (
                            <tr
                              key={node.id}
                              onClick={() => {
                                onRunCypher(buildInspectSupernodeCypher(node.id, usedLegacyId));
                                onClose();
                              }}
                              className="cursor-pointer border-b border-line/60 hover:bg-surface-hover"
                            >
                              <td className="max-w-[160px] truncate py-1 pr-2 font-mono text-fg" title={node.id}>
                                {node.id}
                              </td>
                              <td className="py-1 pr-2 font-mono text-fg-muted">
                                {node.labels.join(', ') || '—'}
                              </td>
                              <td className="py-1 text-right font-mono font-semibold text-accent">
                                {node.degree.toLocaleString()}
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>

                    <div className="space-y-2">
                      <h4 className="text-xs font-semibold text-fg">
                        {t('cypher.analytics.propertyCoverage')}
                      </h4>
                      <table className="w-full border-collapse text-left text-xs">
                        <thead>
                          <tr className="border-b border-line text-[11px] text-fg-subtle">
                            <th className="py-1 pr-2 font-medium">{t('cypher.analytics.col.property')}</th>
                            <th className="py-1 text-right font-medium">{t('cypher.analytics.col.coverage')}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {sampleSummary.propertyCoverage.map((prop) => (
                            <tr key={prop.key} className="border-b border-line/60">
                              <td className="py-1 pr-2 font-mono text-fg">{prop.key}</td>
                              <td className="py-1 text-right font-mono text-fg-muted">
                                {prop.count.toLocaleString()} ({formatProfileRatio(prop.ratio)})
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                </>
              )}
            </div>
          )}
        </div>
      </div>

      {chartState && (() => {
        const table = neo4jCountsToChartTable(chartState.labelCol, chartState.items);
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

function CountSection({
  title,
  items,
  openChartLabel,
  onOpenChart,
  onClickItem
}: {
  title: string;
  items: readonly Neo4jCountItem[];
  openChartLabel: string;
  onOpenChart: () => void;
  onClickItem?: (name: string) => void;
}) {
  const maxCount = items.reduce((best, item) => Math.max(best, item.count), 0);
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <h4 className="text-xs font-semibold text-fg">{title}</h4>
        {items.length > 0 && (
          <button
            type="button"
            onClick={onOpenChart}
            className="flex items-center gap-1 rounded border border-line px-2 py-0.5 text-[11px] text-fg-muted hover:bg-surface-hover hover:text-fg"
          >
            <BarChart3 size={11} />
            <span>{openChartLabel}</span>
          </button>
        )}
      </div>
      <div className="space-y-1.5">
        {items.map((item) => (
          <div
            key={item.name}
            onClick={onClickItem ? () => onClickItem(item.name) : undefined}
            className={clsx(
              'flex items-center gap-2 rounded px-1.5 py-1 text-xs',
              onClickItem && 'cursor-pointer hover:bg-surface-hover'
            )}
          >
            <span className="w-36 shrink-0 truncate font-mono text-fg" title={item.name}>
              {item.name}
            </span>
            <div className="h-2 flex-1 overflow-hidden rounded-full bg-surface-sunken">
              <div
                className="h-full rounded-full bg-accent"
                style={{ width: `${maxCount > 0 ? Math.max(item.count > 0 ? 2 : 0, Math.round((item.count / maxCount) * 100)) : 0}%` }}
              />
            </div>
            <span className="w-16 shrink-0 text-right font-mono text-fg-muted">
              {item.count.toLocaleString()}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function MetricBox({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-control border border-line bg-surface p-3">
      <p className="text-[11px] text-fg-subtle">{label}</p>
      <p className="mt-1 font-mono text-sm font-semibold text-fg">{value}</p>
    </div>
  );
}
