import { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { Check, Copy, Download, Image as ImageIcon, Loader2, RotateCcw, X } from 'lucide-react';
import type { AnalyticsQueryResult, AnalyticsSessionSummary } from '../contracts/analytics';
import type { SerializedResultValue } from '../contracts/resultSet';
import {
  MAX_CHART_POINTS,
  MAX_HEATMAP_AXIS,
  MAX_SCATTER_POINTS,
  buildChartAggregationSql,
  buildChartData,
  buildHeatmapData,
  buildScatterData,
  buildStackedSegments,
  chartDataToMarkdown,
  chartScale,
  detectNumericColumnIndexes,
  hasSeriesScaleMismatch,
  heatmapIntensity,
  isChartRejection,
  labelStride,
  stackedChartScale,
  suggestChartSpec,
  type ChartAggregateFn,
  type ChartRejection,
  type ChartType
} from '../utils/resultChart';
import {
  PNG_SCALE,
  rasterizeSvgToPngBase64,
  readCssColor,
  serializeSvgWithInlineStyles
} from '../utils/svgExport';
import { describeError } from '../utils/describeError';
import { useLanguageStore } from '../stores/languageStore';
import { SegmentedControl } from './FormControls';
import type { TranslationKey } from '../i18n/translate';
import { isImeKeyEvent } from '../utils/shortcuts';

/** 分类色板的槽位，按固定次序分配、不循环。取值见 index.css 的 --dm-series-* */
const SERIES_SLOTS = 8;

/** 超过这么多序列就不画了。第 9 条不去生成新颜色——那必然撞上已有的槽位 */
const MAX_SERIES = SERIES_SLOTS;

const REJECTION_KEYS: Readonly<Record<ChartRejection, TranslationKey>> = {
  'no-rows': 'chart.reject.noRows',
  'no-numeric-column': 'chart.reject.noNumericColumn',
  'too-many-rows': 'chart.reject.tooManyRows'
};

const AGG_FNS: readonly ChartAggregateFn[] = ['SUM', 'AVG', 'COUNT', 'MAX', 'MIN'];
const TOP_N_OPTIONS = [10, 20, 50] as const;

const PLOT = { width: 760, height: 320, left: 72, right: 20, top: 16, bottom: 48 };

interface AggregatedDataset {
  readonly columns: readonly string[];
  readonly rows: ReadonlyArray<readonly SerializedResultValue[]>;
  readonly fn: ChartAggregateFn;
  readonly groupColumn: string;
  readonly metricColumn: string;
  readonly topN: number;
  readonly sourceRows: number;
}

interface ResultChartDialogProps {
  columns: readonly string[];
  rows: ReadonlyArray<readonly SerializedResultValue[]>;
  initialChartType?: ChartType;
  onClose: () => void;
}

export function ResultChartDialog({
  columns,
  rows,
  initialChartType,
  onClose
}: ResultChartDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const svgRef = useRef<SVGSVGElement | null>(null);
  const mountedRef = useRef(true);
  const activeAggSessionRef = useRef<string | null>(null);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      const activeId = activeAggSessionRef.current;
      if (activeId) {
        void invoke('analytics_cancel', { analyticsId: activeId }).catch(() => {});
      }
    };
  }, []);

  const [type, setType] = useState<ChartType>(initialChartType ?? 'bar');
  /** null 表示还没动过，用推荐的那一组 */
  const [chosenValues, setChosenValues] = useState<readonly number[] | null>(null);
  const [scatterX, setScatterX] = useState<number | null>(null);
  const [scatterY, setScatterY] = useState<number | null>(null);
  const [heatX, setHeatX] = useState<number | null>(null);
  const [heatY, setHeatY] = useState<number | null>(null);
  const [heatValue, setHeatValue] = useState<number | 'count' | null>(null);

  const sourceNumericIndexes = useMemo(
    () => detectNumericColumnIndexes(columns, rows as SerializedResultValue[][]),
    [columns, rows]
  );
  const hasNumericSource = sourceNumericIndexes.length > 0;

  // >200 行或无数值列时的显式内存 DuckDB 聚合状态
  const [aggregated, setAggregated] = useState<AggregatedDataset | null>(null);
  const [aggGroup, setAggGroup] = useState<string>(columns[0] ?? '');
  const [aggMetric, setAggMetric] = useState<string>('');
  const [aggFn, setAggFn] = useState<ChartAggregateFn>(() => (hasNumericSource ? 'SUM' : 'COUNT'));
  const [aggTopN, setAggTopN] = useState<number>(20);
  const [aggregating, setAggregating] = useState(false);
  const [aggError, setAggError] = useState<string | null>(null);

  // 导出与复制反馈
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportNotice, setExportNotice] = useState<string | null>(null);
  const [copiedMd, setCopiedMd] = useState(false);

  const activeColumns = aggregated ? aggregated.columns : columns;
  const activeRows = aggregated ? aggregated.rows : rows;

  useEffect(() => {
    if (columns.length > 0 && !columns.includes(aggGroup)) {
      const firstNonNumeric = columns.find((_, idx) => !sourceNumericIndexes.includes(idx));
      setAggGroup(firstNonNumeric ?? columns[0]);
    }
    if (sourceNumericIndexes.length === 0) {
      if (aggFn !== 'COUNT') {
        setAggFn('COUNT');
      }
    } else if (aggMetric === '') {
      setAggMetric(columns[sourceNumericIndexes[0]] ?? '');
    }
  }, [columns, sourceNumericIndexes, aggGroup, aggMetric, aggFn]);

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

  const suggestion = useMemo(
    () => suggestChartSpec(activeColumns, activeRows as SerializedResultValue[][]),
    [activeColumns, activeRows]
  );

  const handleRunDuckDbAggregate = async () => {
    if (aggregating || !aggGroup) {
      return;
    }
    setAggregating(true);
    setAggError(null);
    let sessionId: string | null = null;
    try {
      const loaded = await invoke<AnalyticsSessionSummary>('analytics_load_result', {
        columns: columns.map((name) => ({ name })),
        rows
      });
      sessionId = loaded.analyticsId;
      activeAggSessionRef.current = sessionId;
      if (!mountedRef.current) {
        return;
      }
      const metricCol = aggFn === 'COUNT' || aggMetric === '' ? null : aggMetric;
      const sql = buildChartAggregationSql({
        groupColumn: aggGroup,
        metricColumn: metricCol,
        fn: aggFn,
        topN: aggTopN
      });
      const res = await invoke<AnalyticsQueryResult>('analytics_query', {
        analyticsId: loaded.analyticsId,
        sql,
        limit: MAX_CHART_POINTS,
        timeoutSeconds: 30
      });
      if (!mountedRef.current) {
        return;
      }
      setAggregated({
        columns: res.columns,
        rows: res.rows,
        fn: aggFn,
        groupColumn: aggGroup,
        metricColumn: metricCol ?? '*',
        topN: aggTopN,
        sourceRows: rows.length
      });
      setChosenValues(null);
      setType('bar');
    } catch (err) {
      if (mountedRef.current) {
        setAggError(describeError(err));
      }
    } finally {
      if (sessionId) {
        if (activeAggSessionRef.current === sessionId) {
          activeAggSessionRef.current = null;
        }
        void invoke('analytics_close', { analyticsId: sessionId }).catch(() => {});
      }
      if (mountedRef.current) {
        setAggregating(false);
      }
    }
  };

  if (isChartRejection(suggestion)) {
    const canAggregate =
      (suggestion === 'too-many-rows' || suggestion === 'no-numeric-column') && columns.length > 0;
    const availableAggFns: readonly ChartAggregateFn[] = hasNumericSource ? AGG_FNS : ['COUNT'];
    return (
      <ChartShell onClose={onClose} title={t('chart.title')}>
        <div className="space-y-4 px-5 py-5">
          <p className="text-sm text-fg-muted">
            {t(REJECTION_KEYS[suggestion], { limit: MAX_CHART_POINTS, rows: activeRows.length })}
          </p>
          {canAggregate && (
            <div className="space-y-3 rounded-control border border-line bg-surface p-4">
              <p className="text-xs font-medium text-fg">{t('chart.aggregateHeading')}</p>
              <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                <label className="flex flex-col gap-1 text-xs text-fg-muted">
                  <span>{t('chart.aggregateGroup')}</span>
                  <select
                    value={aggGroup}
                    onChange={(event) => setAggGroup(event.target.value)}
                    className="rounded-control border border-line-strong bg-surface-raised px-2 py-1 text-xs text-fg"
                  >
                    {columns.map((col) => (
                      <option key={col} value={col}>
                        {col}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex flex-col gap-1 text-xs text-fg-muted">
                  <span>{t('chart.aggregateFn')}</span>
                  <select
                    value={aggFn}
                    onChange={(event) => setAggFn(event.target.value as ChartAggregateFn)}
                    className="rounded-control border border-line-strong bg-surface-raised px-2 py-1 text-xs text-fg"
                  >
                    {availableAggFns.map((fn) => (
                      <option key={fn} value={fn}>
                        {fn}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex flex-col gap-1 text-xs text-fg-muted">
                  <span>{t('chart.aggregateMetric')}</span>
                  <select
                    value={aggFn === 'COUNT' ? '' : aggMetric}
                    disabled={aggFn === 'COUNT'}
                    onChange={(event) => setAggMetric(event.target.value)}
                    className="rounded-control border border-line-strong bg-surface-raised px-2 py-1 text-xs text-fg disabled:opacity-50"
                  >
                    {aggFn === 'COUNT' ? (
                      <option value="">{t('chart.countMetric')}</option>
                    ) : (
                      sourceNumericIndexes.map((idx) => (
                        <option key={columns[idx]} value={columns[idx]}>
                          {columns[idx]}
                        </option>
                      ))
                    )}
                  </select>
                </label>
                <label className="flex flex-col gap-1 text-xs text-fg-muted">
                  <span>{t('chart.aggregateTopN')}</span>
                  <select
                    value={aggTopN}
                    onChange={(event) => setAggTopN(Number(event.target.value))}
                    className="rounded-control border border-line-strong bg-surface-raised px-2 py-1 text-xs text-fg"
                  >
                    {TOP_N_OPTIONS.map((n) => (
                      <option key={n} value={n}>
                        Top {n}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              {aggError && <p className="text-xs text-danger">{aggError}</p>}
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => void handleRunDuckDbAggregate()}
                  disabled={aggregating || (aggFn !== 'COUNT' && aggMetric === '')}
                  className="flex items-center gap-1.5 rounded-control bg-accent px-3 py-1.5 text-xs font-medium text-fg-on-accent hover:opacity-90 disabled:opacity-50"
                >
                  {aggregating && <Loader2 size={12} className="animate-spin" />}
                  <span>{t('chart.aggregateRun')}</span>
                </button>
              </div>
            </div>
          )}
        </div>
      </ChartShell>
    );
  }

  const numericIndexes = suggestion.valueIndexes;
  const allNumericIndexes = detectNumericColumnIndexes(activeColumns, activeRows as SerializedResultValue[][]);
  const canScatter = allNumericIndexes.length >= 2;
  const canHeatmap = activeColumns.length >= 2;
  const effectiveType: ChartType =
    type === 'scatter' && !canScatter
      ? 'bar'
      : type === 'heatmap' && !canHeatmap
        ? 'bar'
        : type;

  const selected = (chosenValues ?? numericIndexes.slice(0, 1)).slice(0, MAX_SERIES);
  const spec = { type: effectiveType, categoryIndex: suggestion.categoryIndex, valueIndexes: selected };
  const data = buildChartData(spec, activeColumns, activeRows as SerializedResultValue[][]);
  const scale =
    effectiveType === 'stacked-bar' ? stackedChartScale(data) : chartScale(data);

  const resolvedScatterX =
    scatterX !== null && allNumericIndexes.includes(scatterX) ? scatterX : (allNumericIndexes[0] ?? 0);
  const resolvedScatterY =
    scatterY !== null && allNumericIndexes.includes(scatterY)
      ? scatterY
      : (allNumericIndexes[1] ?? allNumericIndexes[0] ?? 0);
  const scatterData =
    effectiveType === 'scatter'
      ? buildScatterData(
          activeColumns,
          activeRows as SerializedResultValue[][],
          resolvedScatterX,
          resolvedScatterY,
          suggestion.categoryIndex
        )
      : null;

  const resolvedHeatX = heatX !== null && heatX < activeColumns.length ? heatX : 0;
  const resolvedHeatY =
    heatY !== null && heatY < activeColumns.length
      ? heatY
      : activeColumns.length > 1
        ? 1
        : 0;
  const resolvedHeatVal: number | null =
    heatValue === 'count'
      ? null
      : heatValue !== null && allNumericIndexes.includes(heatValue)
        ? heatValue
        : (numericIndexes[0] ?? null);
  const heatmapData =
    effectiveType === 'heatmap'
      ? buildHeatmapData(
          activeColumns,
          activeRows as SerializedResultValue[][],
          resolvedHeatX,
          resolvedHeatY,
          resolvedHeatVal
        )
      : null;

  const toggleValue = (index: number) => {
    setChosenValues((current) => {
      const base = current ?? numericIndexes.slice(0, 1);
      const next = base.includes(index)
        ? base.filter((candidate) => candidate !== index)
        : [...base, index].sort((left, right) => left - right);
      return next.length === 0 ? base : next;
    });
  };

  const handleExport = async (format: 'svg' | 'png') => {
    const svgEl = svgRef.current;
    if (!svgEl || exporting) {
      return;
    }
    setExportError(null);
    setExportNotice(null);
    try {
      const chosen = await save({
        defaultPath: `chart.${format}`,
        filters: [
          {
            name: format === 'svg' ? t('chart.svgFilterName') : t('chart.pngFilterName'),
            extensions: [format]
          }
        ]
      });
      if (!chosen || !mountedRef.current) {
        return;
      }
      setExporting(true);
      const markup = serializeSvgWithInlineStyles(svgEl, {
        background: readCssColor('--dm-surface-raised', '#ffffff'),
        width: PLOT.width,
        height: PLOT.height
      });
      if (format === 'svg') {
        await invoke('write_text_file', { path: chosen, contents: markup });
      } else {
        const contentsBase64 = await rasterizeSvgToPngBase64(
          markup,
          PLOT.width,
          PLOT.height,
          PNG_SCALE
        );
        await invoke('write_binary_file', { path: chosen, contentsBase64 });
      }
      if (mountedRef.current) {
        setExportNotice(t('chart.exportedNotice'));
        window.setTimeout(() => {
          if (mountedRef.current) {
            setExportNotice(null);
          }
        }, 2000);
      }
    } catch (err) {
      if (mountedRef.current) {
        setExportError(describeError(err));
      }
    } finally {
      if (mountedRef.current) {
        setExporting(false);
      }
    }
  };

  const handleCopyMarkdown = () => {
    const categoryHeader =
      spec.categoryIndex === null ? '#' : (activeColumns[spec.categoryIndex] ?? 'Category');
    const md = chartDataToMarkdown(data, categoryHeader);
    navigator.clipboard
      .writeText(md)
      .then(() => {
        setCopiedMd(true);
        window.setTimeout(() => setCopiedMd(false), 1400);
      })
      .catch(() => {});
  };

  const typeOptions: { value: ChartType; label: string }[] = [
    { value: 'bar', label: t('chart.type.bar') },
    { value: 'stacked-bar', label: t('chart.type.stackedBar') },
    { value: 'line', label: t('chart.type.line') },
    { value: 'area', label: t('chart.type.area') },
    ...(canScatter ? [{ value: 'scatter' as const, label: t('chart.type.scatter') }] : []),
    ...(canHeatmap ? [{ value: 'heatmap' as const, label: t('chart.type.heatmap') }] : [])
  ];

  const innerWidth = PLOT.width - PLOT.left - PLOT.right;
  const innerHeight = PLOT.height - PLOT.top - PLOT.bottom;
  const toY = (value: number) =>
    PLOT.top + innerHeight - ((value - scale.min) / (scale.max - scale.min)) * innerHeight;
  const bandWidth = innerWidth / Math.max(1, data.categories.length);
  const stride = labelStride(data.categories.length, Math.max(2, Math.floor(innerWidth / 56)));
  const baseline = toY(Math.max(scale.min, Math.min(0, scale.max)));
  const stackedSegments = effectiveType === 'stacked-bar' ? buildStackedSegments(data) : [];

  const isCartesianSeries =
    effectiveType === 'bar'
    || effectiveType === 'stacked-bar'
    || effectiveType === 'line'
    || effectiveType === 'area';

  return (
    <ChartShell
      onClose={onClose}
      title={t('chart.title')}
      headerActions={
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            onClick={handleCopyMarkdown}
            className="flex items-center gap-1 rounded-control border border-line px-2 py-1 text-xs text-fg-muted hover:bg-surface-hover hover:text-fg"
          >
            {copiedMd ? <Check size={12} className="text-success" /> : <Copy size={12} />}
            <span>{copiedMd ? t('chart.copiedMarkdown') : t('chart.copyMarkdown')}</span>
          </button>
          <button
            type="button"
            onClick={() => void handleExport('svg')}
            disabled={exporting}
            className="flex items-center gap-1 rounded-control border border-line px-2 py-1 text-xs text-fg-muted hover:bg-surface-hover hover:text-fg disabled:opacity-50"
          >
            <Download size={12} />
            <span>{t('chart.exportSvg')}</span>
          </button>
          <button
            type="button"
            onClick={() => void handleExport('png')}
            disabled={exporting}
            className="flex items-center gap-1 rounded-control border border-line px-2 py-1 text-xs text-fg-muted hover:bg-surface-hover hover:text-fg disabled:opacity-50"
          >
            <ImageIcon size={12} />
            <span>{t('chart.exportPng')}</span>
          </button>
        </div>
      }
    >
      {/* Aggregated badge */}
      {aggregated && (
        <div className="flex items-center justify-between gap-2 border-b border-accent-line bg-accent-soft px-5 py-1.5 text-xs text-accent">
          <span>
            {t('chart.aggregatedBadge', {
              fn: aggregated.fn,
              metric: aggregated.metricColumn,
              group: aggregated.groupColumn,
              topN: aggregated.topN,
              total: aggregated.sourceRows.toLocaleString()
            })}
          </span>
          <button
            type="button"
            onClick={() => {
              setAggregated(null);
              setChosenValues(null);
            }}
            className="flex shrink-0 items-center gap-1 rounded px-1.5 py-0.5 text-xs hover:bg-surface-hover"
          >
            <RotateCcw size={11} />
            <span>{t('chart.aggregateReset')}</span>
          </button>
        </div>
      )}

      {/* Controls */}
      <div className="flex flex-wrap items-center gap-x-5 gap-y-2 border-b border-line px-5 py-2.5">
        <SegmentedControl
          value={effectiveType}
          onChange={setType}
          options={typeOptions}
        />

        {isCartesianSeries && (
          <>
            <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
              <span className="text-xs text-fg-muted">{t('chart.values')}</span>
              {numericIndexes.map((index) => (
                <label key={index} className="flex cursor-pointer items-center gap-1.5 text-xs text-fg">
                  <input
                    type="checkbox"
                    checked={selected.includes(index)}
                    onChange={() => toggleValue(index)}
                    className="h-3.5 w-3.5 accent-[var(--dm-accent)]"
                  />
                  <span className="max-w-[10rem] truncate font-mono">{activeColumns[index]}</span>
                </label>
              ))}
            </div>
            <span className="text-xs text-fg-subtle">
              {spec.categoryIndex === null
                ? t('chart.categoryIsRowNumber')
                : t('chart.categoryIs', { column: activeColumns[spec.categoryIndex] ?? '' })}
            </span>
          </>
        )}

        {effectiveType === 'scatter' && (
          <div className="flex flex-wrap items-center gap-3 text-xs text-fg-muted">
            <label className="flex items-center gap-1.5">
              <span>{t('chart.xColumn')}</span>
              <select
                value={resolvedScatterX}
                onChange={(event) => setScatterX(Number(event.target.value))}
                className="rounded-control border border-line-strong bg-surface px-2 py-0.5 font-mono text-xs text-fg"
              >
                {allNumericIndexes.map((idx) => (
                  <option key={idx} value={idx}>
                    {activeColumns[idx]}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-1.5">
              <span>{t('chart.yColumn')}</span>
              <select
                value={resolvedScatterY}
                onChange={(event) => setScatterY(Number(event.target.value))}
                className="rounded-control border border-line-strong bg-surface px-2 py-0.5 font-mono text-xs text-fg"
              >
                {allNumericIndexes.map((idx) => (
                  <option key={idx} value={idx}>
                    {activeColumns[idx]}
                  </option>
                ))}
              </select>
            </label>
          </div>
        )}

        {effectiveType === 'heatmap' && (
          <div className="flex flex-wrap items-center gap-3 text-xs text-fg-muted">
            <label className="flex items-center gap-1.5">
              <span>{t('chart.xColumn')}</span>
              <select
                value={resolvedHeatX}
                onChange={(event) => setHeatX(Number(event.target.value))}
                className="rounded-control border border-line-strong bg-surface px-2 py-0.5 font-mono text-xs text-fg"
              >
                {activeColumns.map((col, idx) => (
                  <option key={`${col}-${idx}`} value={idx}>
                    {col}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-1.5">
              <span>{t('chart.yColumn')}</span>
              <select
                value={resolvedHeatY}
                onChange={(event) => setHeatY(Number(event.target.value))}
                className="rounded-control border border-line-strong bg-surface px-2 py-0.5 font-mono text-xs text-fg"
              >
                {activeColumns.map((col, idx) => (
                  <option key={`${col}-${idx}`} value={idx}>
                    {col}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-1.5">
              <span>{t('chart.valueColumn')}</span>
              <select
                value={resolvedHeatVal === null ? 'count' : String(resolvedHeatVal)}
                onChange={(event) =>
                  setHeatValue(event.target.value === 'count' ? 'count' : Number(event.target.value))
                }
                className="rounded-control border border-line-strong bg-surface px-2 py-0.5 font-mono text-xs text-fg"
              >
                <option value="count">{t('chart.countMetric')}</option>
                {allNumericIndexes.map((idx) => (
                  <option key={idx} value={idx}>
                    {activeColumns[idx]}
                  </option>
                ))}
              </select>
            </label>
          </div>
        )}
      </div>

      {exportError && <p className="px-5 pt-2 text-xs text-danger">{exportError}</p>}
      {exportNotice && <p className="px-5 pt-2 text-xs text-success">{exportNotice}</p>}

      {isCartesianSeries && hasSeriesScaleMismatch(data) && (
        <p className="px-5 pt-2.5 text-xs text-warning">{t('chart.scaleMismatch')}</p>
      )}
      {scatterData?.truncatedByCap && (
        <p className="px-5 pt-2.5 text-xs text-warning">
          {t('chart.scatterCapped', { limit: MAX_SCATTER_POINTS.toLocaleString() })}
        </p>
      )}
      {heatmapData?.clippedAxes && (
        <p className="px-5 pt-2.5 text-xs text-warning">
          {t('chart.heatmapClipped', { limit: MAX_HEATMAP_AXIS })}
        </p>
      )}

      {isCartesianSeries && data.series.length > 1 && (
        <ul className="flex flex-wrap items-center gap-x-4 gap-y-1 px-5 pt-2.5">
          {data.series.map((series, slot) => (
            <li key={series.name} className="flex items-center gap-1.5 text-xs text-fg">
              <span
                aria-hidden
                className="h-2.5 w-2.5 shrink-0 rounded-[2px]"
                style={{ background: seriesColor(slot) }}
              />
              <span className="max-w-[12rem] truncate font-mono">{series.name}</span>
            </li>
          ))}
        </ul>
      )}

      <div className="overflow-x-auto px-5 py-3">
        <svg
          ref={svgRef}
          viewBox={`0 0 ${PLOT.width} ${PLOT.height}`}
          className="h-auto w-full min-w-[540px]"
          role="img"
          aria-label={t('chart.ariaLabel', {
            series: data.series.map((series) => series.name).join(', '),
            count: data.categories.length
          })}
        >
          {/* 1) 柱状 / 堆叠柱状 / 折线 / 面积图 */}
          {isCartesianSeries && (
            <>
              {scale.ticks.map((tick) => (
                <g key={tick}>
                  <line
                    x1={PLOT.left}
                    x2={PLOT.width - PLOT.right}
                    y1={toY(tick)}
                    y2={toY(tick)}
                    stroke="var(--dm-line)"
                    strokeWidth={1}
                  />
                  <text
                    x={PLOT.left - 8}
                    y={toY(tick)}
                    textAnchor="end"
                    dominantBaseline="middle"
                    className="fill-[var(--dm-fg-subtle)] text-[10px]"
                  >
                    {formatTick(tick)}
                  </text>
                </g>
              ))}

              {scale.min < 0 && scale.max > 0 && (
                <line
                  x1={PLOT.left}
                  x2={PLOT.width - PLOT.right}
                  y1={baseline}
                  y2={baseline}
                  stroke="var(--dm-line-strong)"
                  strokeWidth={1}
                />
              )}

              {effectiveType === 'bar' &&
                data.series.map((series, slot) =>
                  series.points.map((point, pointIndex) => {
                    if (point === null) {
                      return null;
                    }
                    const groupWidth = bandWidth * 0.72;
                    const barWidth = Math.max(1, groupWidth / data.series.length - 2);
                    const x =
                      PLOT.left
                      + pointIndex * bandWidth
                      + (bandWidth - groupWidth) / 2
                      + slot * (groupWidth / data.series.length);
                    return (
                      <path
                        key={`${series.name}-${pointIndex}`}
                        d={barPath(x, barWidth, toY(point), baseline)}
                        fill={seriesColor(slot)}
                      >
                        <title>{`${data.categories[pointIndex]} · ${series.name}: ${point}`}</title>
                      </path>
                    );
                  })
                )}

              {effectiveType === 'stacked-bar' &&
                stackedSegments.map((seg) => {
                  const barWidth = Math.max(2, bandWidth * 0.64);
                  const x = PLOT.left + seg.categoryIndex * bandWidth + (bandWidth - barWidth) / 2;
                  const yTop = Math.min(toY(seg.y0), toY(seg.y1));
                  const height = Math.max(1, Math.abs(toY(seg.y1) - toY(seg.y0)));
                  return (
                    <rect
                      key={`${seg.seriesName}-${seg.categoryIndex}`}
                      x={x}
                      y={yTop}
                      width={barWidth}
                      height={height}
                      fill={seriesColor(seg.seriesIndex)}
                    >
                      <title>{`${data.categories[seg.categoryIndex]} · ${seg.seriesName}: ${seg.value}`}</title>
                    </rect>
                  );
                })}

              {(effectiveType === 'line' || effectiveType === 'area') &&
                data.series.map((series, slot) => {
                  const toX = (index: number) => PLOT.left + (index + 0.5) * bandWidth;
                  return (
                    <g key={series.name}>
                      {effectiveType === 'area' &&
                        areaPaths(series.points, toX, toY, baseline).map((path, segment) => (
                          <path
                            key={`area-${segment}`}
                            d={path}
                            fill={seriesColor(slot)}
                            opacity={0.22}
                          />
                        ))}
                      {linePaths(series.points, toX, toY).map((path, segment) => (
                        <path
                          key={`line-${segment}`}
                          d={path}
                          fill="none"
                          stroke={seriesColor(slot)}
                          strokeWidth={2}
                          strokeLinecap="round"
                        />
                      ))}
                      {series.points.map((point, pointIndex) =>
                        point === null ? null : (
                          <circle
                            key={pointIndex}
                            cx={toX(pointIndex)}
                            cy={toY(point)}
                            r={3.5}
                            fill={seriesColor(slot)}
                            stroke="var(--dm-surface-raised)"
                            strokeWidth={1.5}
                          >
                            <title>{`${data.categories[pointIndex]} · ${series.name}: ${point}`}</title>
                          </circle>
                        )
                      )}
                    </g>
                  );
                })}

              {data.categories.map((category, index) =>
                index % stride === 0 || index === data.categories.length - 1 ? (
                  <text
                    key={index}
                    x={PLOT.left + (index + 0.5) * bandWidth}
                    y={PLOT.height - PLOT.bottom + 16}
                    textAnchor="middle"
                    className="fill-[var(--dm-fg-muted)] text-[10px]"
                  >
                    {truncate(category, 10)}
                  </text>
                ) : null
              )}
            </>
          )}

          {/* 2) 散点图 */}
          {effectiveType === 'scatter' && scatterData && (() => {
            const toSX = (v: number) =>
              PLOT.left
              + ((v - scatterData.xScale.min)
                / Math.max(Number.EPSILON, scatterData.xScale.max - scatterData.xScale.min))
                * innerWidth;
            const toSY = (v: number) =>
              PLOT.top
              + innerHeight
              - ((v - scatterData.yScale.min)
                / Math.max(Number.EPSILON, scatterData.yScale.max - scatterData.yScale.min))
                * innerHeight;
            return (
              <>
                {scatterData.yScale.ticks.map((tick) => (
                  <g key={`sy-${tick}`}>
                    <line
                      x1={PLOT.left}
                      x2={PLOT.width - PLOT.right}
                      y1={toSY(tick)}
                      y2={toSY(tick)}
                      stroke="var(--dm-line)"
                      strokeWidth={1}
                    />
                    <text
                      x={PLOT.left - 8}
                      y={toSY(tick)}
                      textAnchor="end"
                      dominantBaseline="middle"
                      className="fill-[var(--dm-fg-subtle)] text-[10px]"
                    >
                      {formatTick(tick)}
                    </text>
                  </g>
                ))}
                {scatterData.xScale.ticks.map((tick) => (
                  <g key={`sx-${tick}`}>
                    <line
                      x1={toSX(tick)}
                      x2={toSX(tick)}
                      y1={PLOT.top}
                      y2={PLOT.top + innerHeight}
                      stroke="var(--dm-line)"
                      strokeWidth={1}
                    />
                    <text
                      x={toSX(tick)}
                      y={PLOT.height - PLOT.bottom + 16}
                      textAnchor="middle"
                      className="fill-[var(--dm-fg-muted)] text-[10px]"
                    >
                      {formatTick(tick)}
                    </text>
                  </g>
                ))}
                {scatterData.points.map((pt, idx) => (
                  <circle
                    key={idx}
                    cx={toSX(pt.x)}
                    cy={toSY(pt.y)}
                    r={3.5}
                    fill={seriesColor(0)}
                    opacity={0.78}
                  >
                    <title>{`${pt.label} · ${scatterData.xName}: ${pt.x}, ${scatterData.yName}: ${pt.y}`}</title>
                  </circle>
                ))}
              </>
            );
          })()}

          {/* 3) 二维矩阵热力图 */}
          {effectiveType === 'heatmap' && heatmapData && (() => {
            const cols = Math.max(1, heatmapData.xLabels.length);
            const rowsCount = Math.max(1, heatmapData.yLabels.length);
            const cellW = innerWidth / cols;
            const cellH = innerHeight / rowsCount;
            const xStride = labelStride(cols, Math.max(2, Math.floor(innerWidth / 56)));
            const yStride = labelStride(rowsCount, Math.max(2, Math.floor(innerHeight / 18)));
            return (
              <>
                {heatmapData.cells.map((cell) => {
                  const x = PLOT.left + cell.xIdx * cellW + 1;
                  const y = PLOT.top + cell.yIdx * cellH + 1;
                  return (
                    <rect
                      key={`${cell.xIdx}-${cell.yIdx}`}
                      x={x}
                      y={y}
                      width={Math.max(2, cellW - 2)}
                      height={Math.max(2, cellH - 2)}
                      rx={2}
                      fill="var(--dm-accent)"
                      opacity={heatmapIntensity(cell.value, heatmapData.min, heatmapData.max)}
                    >
                      <title>{`${heatmapData.xLabels[cell.xIdx]} × ${heatmapData.yLabels[cell.yIdx]} · ${heatmapData.valueName}: ${cell.value}`}</title>
                    </rect>
                  );
                })}
                {heatmapData.xLabels.map((label, idx) =>
                  idx % xStride === 0 || idx === cols - 1 ? (
                    <text
                      key={`hx-${idx}`}
                      x={PLOT.left + (idx + 0.5) * cellW}
                      y={PLOT.height - PLOT.bottom + 16}
                      textAnchor="middle"
                      className="fill-[var(--dm-fg-muted)] text-[10px]"
                    >
                      {truncate(label, 9)}
                    </text>
                  ) : null
                )}
                {heatmapData.yLabels.map((label, idx) =>
                  idx % yStride === 0 || idx === rowsCount - 1 ? (
                    <text
                      key={`hy-${idx}`}
                      x={PLOT.left - 8}
                      y={PLOT.top + (idx + 0.5) * cellH}
                      textAnchor="end"
                      dominantBaseline="middle"
                      className="fill-[var(--dm-fg-subtle)] text-[10px]"
                    >
                      {truncate(label, 10)}
                    </text>
                  ) : null
                )}
              </>
            );
          })()}
        </svg>
      </div>
    </ChartShell>
  );
}

function ChartShell({
  title,
  headerActions,
  children,
  onClose
}: {
  title: string;
  headerActions?: React.ReactNode;
  children: React.ReactNode;
  onClose: () => void;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="result-chart-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[88vh] w-[860px] max-w-[calc(100vw-2rem)] flex-col overflow-auto rounded-panel bg-surface-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between gap-2 border-b border-line px-5 py-3">
          <h3 id="result-chart-title" className="text-sm font-semibold text-fg">
            {title}
          </h3>
          <div className="flex items-center gap-2">
            {headerActions}
            <button
              type="button"
              onClick={onClose}
              aria-label="Close"
              className="rounded-control p-1 text-fg-muted transition-colors hover:bg-surface-hover"
            >
              <X size={16} />
            </button>
          </div>
        </div>
        {children}
      </div>
    </div>
  );
}

function barPath(x: number, width: number, valueY: number, baselineY: number): string {
  const radius = Math.min(4, width / 2, Math.abs(valueY - baselineY));
  const right = x + width;

  if (valueY <= baselineY) {
    return [
      `M${x} ${baselineY}`,
      `L${x} ${valueY + radius}`,
      `Q${x} ${valueY} ${x + radius} ${valueY}`,
      `L${right - radius} ${valueY}`,
      `Q${right} ${valueY} ${right} ${valueY + radius}`,
      `L${right} ${baselineY}`,
      'Z'
    ].join(' ');
  }

  return [
    `M${x} ${baselineY}`,
    `L${x} ${valueY - radius}`,
    `Q${x} ${valueY} ${x + radius} ${valueY}`,
    `L${right - radius} ${valueY}`,
    `Q${right} ${valueY} ${right} ${valueY - radius}`,
    `L${right} ${baselineY}`,
    'Z'
  ].join(' ');
}

function seriesColor(slot: number): string {
  return `var(--dm-series-${Math.min(slot, SERIES_SLOTS - 1) + 1})`;
}

function linePaths(
  points: readonly (number | null)[],
  toX: (index: number) => number,
  toY: (value: number) => number
): string[] {
  const paths: string[] = [];
  let current: string[] = [];

  points.forEach((point, index) => {
    if (point === null) {
      if (current.length > 1) {
        paths.push(current.join(' '));
      }
      current = [];
      return;
    }
    current.push(`${current.length === 0 ? 'M' : 'L'}${toX(index)} ${toY(point)}`);
  });

  if (current.length > 1) {
    paths.push(current.join(' '));
  }
  return paths;
}

function areaPaths(
  points: readonly (number | null)[],
  toX: (index: number) => number,
  toY: (value: number) => number,
  baselineY: number
): string[] {
  const paths: string[] = [];
  let segment: { x: number; y: number }[] = [];

  const flush = () => {
    if (segment.length > 1) {
      const first = segment[0];
      const last = segment[segment.length - 1];
      const top = segment.map((pt, i) => `${i === 0 ? 'M' : 'L'}${pt.x} ${pt.y}`).join(' ');
      paths.push(`${top} L${last.x} ${baselineY} L${first.x} ${baselineY} Z`);
    }
    segment = [];
  };

  points.forEach((point, index) => {
    if (point === null) {
      flush();
      return;
    }
    segment.push({ x: toX(index), y: toY(point) });
  });
  flush();
  return paths;
}

function formatTick(value: number): string {
  const absolute = Math.abs(value);
  if (absolute >= 1_000_000) {
    return `${trimZero(value / 1_000_000)}M`;
  }
  if (absolute >= 1_000) {
    return `${trimZero(value / 1_000)}k`;
  }
  return String(value);
}

function trimZero(value: number): string {
  return String(Number(value.toFixed(1)));
}

function truncate(text: string, limit: number): string {
  return text.length > limit ? `${text.slice(0, limit - 1)}…` : text;
}
