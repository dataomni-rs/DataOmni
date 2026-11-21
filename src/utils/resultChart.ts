import type { SerializedResultValue } from '../contracts/resultSet';

/**
 * 「把这份结果画成图」需要的全部判断。
 *
 * 只做纯函数：选哪几列、每个点的值是多少、纵轴刻度落在哪。组件只负责把算好的
 * 数字摆成 SVG。这样「为什么这一列被当成了数值」这类问题有测试可查，
 * 而不是要在界面上反复试。
 */

/** 一次最多画多少个点。超过就不画，也不悄悄采样 */
export const MAX_CHART_POINTS = 200;

/** 散点图单次渲染上限（超限时显式提示用 DuckDB 聚合或切热力图） */
export const MAX_SCATTER_POINTS = 2000;

/** 热力图每根轴最多展示的唯一标签数 */
export const MAX_HEATMAP_AXIS = 24;

export type ChartType = 'bar' | 'stacked-bar' | 'line' | 'area' | 'scatter' | 'heatmap';

export interface ChartSpec {
  readonly type: ChartType;
  /** 横轴那一列的下标；null 表示按行号排，没有合适的分类列时如此 */
  readonly categoryIndex: number | null;
  /** 纵轴的一列或多列下标，按原始列序 */
  readonly valueIndexes: readonly number[];
}

/** 画不了的原因。给出原因而不是灰掉按钮：用户想知道差在哪 */
export type ChartRejection = 'no-rows' | 'no-numeric-column' | 'too-many-rows';

export interface ChartSeries {
  readonly name: string;
  /** 与 categories 一一对应；不是数字的格子是 null，不当成 0 */
  readonly points: readonly (number | null)[];
}

export interface ChartData {
  readonly categories: readonly string[];
  readonly series: readonly ChartSeries[];
}

/**
 * 把一个结果格子读成数字。
 *
 * `bigint` / `decimal` 是带类型标签的对象（后端为了不丢精度序列化成字符串），
 * 画图时转成 double 是可以接受的——图上一个像素远粗于 double 的精度损失。
 * 但布尔**不**转：`true` 变成 1 会让一列状态位画出一张看着像数据的图。
 */
export function chartValue(cell: SerializedResultValue): number | null {
  if (typeof cell === 'number') {
    return Number.isFinite(cell) ? cell : null;
  }
  if (typeof cell === 'string') {
    // 空串不是 0：`Number('')` 给 0，会把缺失值画成一个真实的低点
    const trimmed = cell.trim();
    if (trimmed === '') {
      return null;
    }
    const parsed = Number(trimmed);
    return Number.isFinite(parsed) ? parsed : null;
  }
  if (cell !== null && typeof cell === 'object' && (cell.type === 'bigint' || cell.type === 'decimal')) {
    const parsed = Number(cell.value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

/** 分类轴上的标签。null 显示成空而不是 "null"——轴上不需要这个词 */
export function chartLabel(cell: SerializedResultValue): string {
  if (cell === null) {
    return '';
  }
  if (typeof cell === 'object') {
    return cell.value;
  }
  return String(cell);
}

/**
 * 一列能不能当纵轴：抽样看它的**实际值**，不看声明类型。
 *
 * 为什么不用 `column_metadata`：查询结果的列常常是表达式（`count(*)`、
 * `sum(x)/count(*)`、`CASE …`），驱动给的声明类型未必可靠，而聚合列恰好是
 * 最想画的那些。抽样问的是「这一列的值是不是数字」，那正是画图需要的条件。
 */
function isNumericColumn(rows: readonly SerializedResultValue[][], index: number): boolean {
  let seen = 0;
  for (const row of rows) {
    const cell = row[index];
    if (cell === null || cell === undefined) {
      continue;
    }
    // 布尔挡在这里：`chartValue` 不认布尔，但一列全 null 加几个 true 会
    // 「一个非空值都没看到」而落到下面的 false，两条路殊途同归；
    // 显式写出来是为了让意图可读
    if (typeof cell === 'boolean') {
      return false;
    }
    if (chartValue(cell) === null) {
      return false;
    }
    seen += 1;
  }
  return seen > 0;
}

/**
 * 给这份结果挑一组默认的画法。
 *
 * 规则简单到能一句话说清：所有数值列当纵轴，第一个非数值列当横轴（没有就拿第一列）。挑不出
 * 纵轴就不画。**默认用柱状图**——查询结果里最常见的形状是「分组 + 聚合值」，
 * 那是柱状图；折线图要求横轴本身有次序，而一份 `GROUP BY` 的结果没有。
 */
export function suggestChartSpec(
  columns: readonly string[],
  rows: readonly SerializedResultValue[][]
): ChartSpec | ChartRejection {
  if (rows.length === 0) {
    return 'no-rows';
  }
  if (rows.length > MAX_CHART_POINTS) {
    return 'too-many-rows';
  }

  const valueIndexes: number[] = [];
  let categoryIndex: number | null = null;
  for (let index = 0; index < columns.length; index += 1) {
    if (isNumericColumn(rows, index)) {
      valueIndexes.push(index);
    } else if (categoryIndex === null) {
      categoryIndex = index;
    }
  }

  if (valueIndexes.length === 0) {
    return 'no-numeric-column';
  }

  // 全是数值列时第一列当横轴：`SELECT year, count(*) … GROUP BY year`、按月份号或部门 id 分组，
  // 分组键都在最前面且是数字。按行号排的话默认画的是年份本身，量那一列反倒要自己去勾，
  // 勾上了横轴也只有 1、2、3。只剩一列时没得分，照旧按行号
  if (categoryIndex === null && valueIndexes.length > 1) {
    const [first, ...rest] = valueIndexes;
    return { type: 'bar', categoryIndex: first ?? null, valueIndexes: rest };
  }

  return { type: 'bar', categoryIndex, valueIndexes };
}

export function isChartRejection(spec: ChartSpec | ChartRejection): spec is ChartRejection {
  return typeof spec === 'string';
}

export function buildChartData(
  spec: ChartSpec,
  columns: readonly string[],
  rows: readonly SerializedResultValue[][]
): ChartData {
  const categories = rows.map((row, rowIndex) =>
    spec.categoryIndex === null
      ? String(rowIndex + 1)
      : chartLabel(row[spec.categoryIndex] ?? null)
  );

  const series = spec.valueIndexes.map((index) => ({
    name: columns[index] ?? '',
    points: rows.map((row) => chartValue(row[index] ?? null))
  }));

  return { categories, series };
}

export interface ChartScale {
  readonly min: number;
  readonly max: number;
  readonly ticks: readonly number[];
}

/**
 * 纵轴的范围与刻度。
 *
 * 两条刻意的取舍：
 *
 * - **全是正数时从 0 起**。截断的纵轴会让 1000 和 1010 看起来差一倍，
 *   而柱状图的长度本身就是在表达「多少」。有负值时才让下界跟着数据走。
 * - 刻度取 1 / 2 / 5 × 10ⁿ。任意等分会得到 0、3.67、7.34 这样的刻度，
 *   读图的人得先做一次除法才知道一根柱子是多少。
 */
export function chartScale(data: ChartData, tickCount = 4): ChartScale {
  const values = data.series
    .flatMap((series) => series.points)
    .filter((point): point is number => point !== null);

  if (values.length === 0) {
    return { min: 0, max: 1, ticks: [0, 1] };
  }

  const dataMax = Math.max(...values);
  const dataMin = Math.min(...values);
  const lower = dataMin > 0 ? 0 : dataMin;
  const upper = dataMax < 0 ? 0 : dataMax;

  if (lower === upper) {
    // 一条水平线：给它一个非零的跨度，否则后面除以 0
    const magnitude = Math.abs(upper) || 1;
    return { min: upper - magnitude, max: upper + magnitude, ticks: [upper - magnitude, upper, upper + magnitude] };
  }

  const step = niceStep((upper - lower) / tickCount);
  const min = Math.floor(lower / step) * step;
  const max = Math.ceil(upper / step) * step;

  const ticks: number[] = [];
  // 按整数步数累加而不是反复 `+= step`：浮点累加会让 0.1 步长的刻度
  // 变成 0.30000000000000004
  const steps = Math.round((max - min) / step);
  for (let index = 0; index <= steps; index += 1) {
    ticks.push(roundToStep(min + index * step, step));
  }

  return { min, max, ticks };
}

/** 把一个粗略的步长收成 1 / 2 / 5 × 10ⁿ 里最接近的那个（向上取） */
function niceStep(rough: number): number {
  if (!Number.isFinite(rough) || rough <= 0) {
    return 1;
  }
  const magnitude = 10 ** Math.floor(Math.log10(rough));
  const normalized = rough / magnitude;
  const factor = normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10;
  return factor * magnitude;
}

/** 按步长的小数位数收尾，去掉浮点乘法带出来的尾巴 */
function roundToStep(value: number, step: number): number {
  const decimals = Math.max(0, -Math.floor(Math.log10(step)));
  return Number(value.toFixed(Math.min(decimals + 1, 20)));
}

/**
 * 几条序列的量级差到「小的那条在图上看不见」的程度时为真。
 *
 * 这是同轴多序列唯一真实的失效方式：`SELECT region, orders, revenue` 里
 * orders 是十几、revenue 是几千，放在一根纵轴上，orders 那组柱子会被压成
 * 贴着基线的一条细边——它在，但读不出来，而读图的人不会怀疑图。
 *
 * 不因此拒绝画（用户明确勾了这两列），也**不**开第二根纵轴——那样同一个
 * 高度在图上就表示两个不同的量，比看不见更糟。只是把这件事说出来。
 *
 * 阈值 50 倍：再小一些两条都还读得出来，再大就只剩一条。
 */
export const SERIES_SCALE_MISMATCH_RATIO = 50;

export function hasSeriesScaleMismatch(data: ChartData): boolean {
  const magnitudes = data.series
    .map((series) => {
      const values = series.points.filter((point): point is number => point !== null);
      return values.length === 0 ? null : Math.max(...values.map(Math.abs));
    })
    .filter((magnitude): magnitude is number => magnitude !== null && magnitude > 0);

  if (magnitudes.length < 2) {
    return false;
  }

  return Math.max(...magnitudes) / Math.min(...magnitudes) > SERIES_SCALE_MISMATCH_RATIO;
}

/**
 * 横轴标签太密时每隔几个显示一个。
 *
 * 挤在一起的标签是一团糊，比没有标签更糟——没有标签至少不会让人以为读得出来。
 * 保证第一个和最后一个一定显示：那两个决定了图的范围。
 */
export function labelStride(count: number, maxLabels: number): number {
  if (count <= maxLabels || maxLabels <= 0) {
    return 1;
  }
  return Math.ceil(count / maxLabels);
}

/**
 * 找出表格里所有数值列的下标（用于 >200 行时展示显式 DuckDB 聚合面板与散点图选列）。
 */
export function detectNumericColumnIndexes(
  columns: readonly string[],
  rows: readonly SerializedResultValue[][]
): number[] {
  const indexes: number[] = [];
  for (let index = 0; index < columns.length; index += 1) {
    if (isNumericColumn(rows, index)) {
      indexes.push(index);
    }
  }
  return indexes;
}

export interface StackedSegment {
  readonly categoryIndex: number;
  readonly seriesIndex: number;
  readonly seriesName: string;
  readonly value: number;
  readonly y0: number;
  readonly y1: number;
}

/** 堆叠柱状图 / 面积图的分段累加坐标：正负值分别沿 0 轴向上/向下堆叠，互不吞没 */
export function buildStackedSegments(data: ChartData): StackedSegment[] {
  const segments: StackedSegment[] = [];
  for (let catIdx = 0; catIdx < data.categories.length; catIdx += 1) {
    let posAcc = 0;
    let negAcc = 0;
    for (let seriesIdx = 0; seriesIdx < data.series.length; seriesIdx += 1) {
      const series = data.series[seriesIdx];
      const value = series.points[catIdx] ?? null;
      if (value === null) {
        continue;
      }
      if (value >= 0) {
        const y0 = posAcc;
        const y1 = posAcc + value;
        posAcc = y1;
        segments.push({
          categoryIndex: catIdx,
          seriesIndex: seriesIdx,
          seriesName: series.name,
          value,
          y0,
          y1
        });
      } else {
        const y0 = negAcc;
        const y1 = negAcc + value;
        negAcc = y1;
        segments.push({
          categoryIndex: catIdx,
          seriesIndex: seriesIdx,
          seriesName: series.name,
          value,
          y0,
          y1
        });
      }
    }
  }
  return segments;
}

/** 堆叠图纵轴比例尺：按每个分类的正向总和与负向总和定界 */
export function stackedChartScale(data: ChartData, tickCount = 4): ChartScale {
  const sums: number[] = [];
  for (let catIdx = 0; catIdx < data.categories.length; catIdx += 1) {
    let pos = 0;
    let neg = 0;
    let seen = false;
    for (const series of data.series) {
      const value = series.points[catIdx] ?? null;
      if (value === null) continue;
      seen = true;
      if (value >= 0) pos += value;
      else neg += value;
    }
    if (seen) {
      sums.push(pos, neg);
    }
  }
  return chartScale({ categories: data.categories, series: [{ name: 'stack', points: sums }] }, tickCount);
}

export interface ScatterPoint {
  readonly x: number;
  readonly y: number;
  readonly label: string;
}

export interface ScatterData {
  readonly xName: string;
  readonly yName: string;
  readonly points: readonly ScatterPoint[];
  readonly xScale: ChartScale;
  readonly yScale: ChartScale;
  readonly truncatedByCap: boolean;
}

function numericAxisScale(values: readonly number[], tickCount = 4): ChartScale {
  if (values.length === 0) {
    return { min: 0, max: 1, ticks: [0, 1] };
  }
  const dataMin = Math.min(...values);
  const dataMax = Math.max(...values);
  if (dataMin === dataMax) {
    const pad = Math.abs(dataMax) * 0.1 || 1;
    return { min: dataMin - pad, max: dataMax + pad, ticks: [dataMin - pad, dataMin, dataMax + pad] };
  }
  const step = niceStep((dataMax - dataMin) / tickCount);
  const min = Math.floor(dataMin / step) * step;
  const max = Math.ceil(dataMax / step) * step;
  const steps = Math.round((max - min) / step);
  const ticks: number[] = [];
  for (let i = 0; i <= steps; i += 1) {
    ticks.push(roundToStep(min + i * step, step));
  }
  return { min, max, ticks };
}

/** 构建散点图数据集（两数值列，超过 MAX_SCATTER_POINTS 时显式截断并标明） */
export function buildScatterData(
  columns: readonly string[],
  rows: readonly SerializedResultValue[][],
  xIndex: number,
  yIndex: number,
  labelIndex: number | null = null
): ScatterData {
  const points: ScatterPoint[] = [];
  let truncatedByCap = false;

  for (let i = 0; i < rows.length; i += 1) {
    const row = rows[i];
    const x = chartValue(row[xIndex] ?? null);
    const y = chartValue(row[yIndex] ?? null);
    if (x === null || y === null) {
      continue;
    }
    if (points.length >= MAX_SCATTER_POINTS) {
      truncatedByCap = true;
      break;
    }
    const label = labelIndex !== null ? chartLabel(row[labelIndex] ?? null) : `#${i + 1}`;
    points.push({ x, y, label });
  }

  return {
    xName: columns[xIndex] ?? '',
    yName: columns[yIndex] ?? '',
    points,
    xScale: numericAxisScale(points.map((p) => p.x), 5),
    yScale: numericAxisScale(points.map((p) => p.y), 4),
    truncatedByCap
  };
}

export interface HeatmapCell {
  readonly xIdx: number;
  readonly yIdx: number;
  readonly value: number;
  readonly count: number;
}

export interface HeatmapData {
  readonly xName: string;
  readonly yName: string;
  readonly valueName: string;
  readonly xLabels: readonly string[];
  readonly yLabels: readonly string[];
  readonly cells: readonly HeatmapCell[];
  readonly min: number;
  readonly max: number;
  readonly clippedAxes: boolean;
}

/** 构建二维交叉矩阵热力图：按 (xLabel, yLabel) 聚合数值列（或计数），单轴上限 MAX_HEATMAP_AXIS */
export function buildHeatmapData(
  columns: readonly string[],
  rows: readonly SerializedResultValue[][],
  xIndex: number,
  yIndex: number,
  valueIndex: number | null
): HeatmapData {
  const xOrder: string[] = [];
  const xMap = new Map<string, number>();
  const yOrder: string[] = [];
  const yMap = new Map<string, number>();
  let clippedAxes = false;

  const accum = new Map<string, { xIdx: number; yIdx: number; sum: number; count: number }>();

  for (const row of rows) {
    const xLabel = chartLabel(row[xIndex] ?? null) || '(empty)';
    const yLabel = chartLabel(row[yIndex] ?? null) || '(empty)';
    const val = valueIndex === null ? 1 : chartValue(row[valueIndex] ?? null);
    if (val === null) {
      continue;
    }

    let xIdx = xMap.get(xLabel);
    if (xIdx === undefined) {
      if (xOrder.length >= MAX_HEATMAP_AXIS) {
        clippedAxes = true;
        continue;
      }
      xIdx = xOrder.length;
      xOrder.push(xLabel);
      xMap.set(xLabel, xIdx);
    }

    let yIdx = yMap.get(yLabel);
    if (yIdx === undefined) {
      if (yOrder.length >= MAX_HEATMAP_AXIS) {
        clippedAxes = true;
        continue;
      }
      yIdx = yOrder.length;
      yOrder.push(yLabel);
      yMap.set(yLabel, yIdx);
    }

    const key = `${xIdx}:${yIdx}`;
    const existing = accum.get(key);
    if (existing) {
      existing.sum += val;
      existing.count += 1;
    } else {
      accum.set(key, { xIdx, yIdx, sum: val, count: 1 });
    }
  }

  const cells: HeatmapCell[] = Array.from(accum.values()).map((entry) => ({
    xIdx: entry.xIdx,
    yIdx: entry.yIdx,
    value: entry.sum,
    count: entry.count
  }));

  const values = cells.map((c) => c.value);
  const min = values.length > 0 ? Math.min(...values) : 0;
  const max = values.length > 0 ? Math.max(...values) : 0;

  return {
    xName: columns[xIndex] ?? '',
    yName: columns[yIndex] ?? '',
    valueName: valueIndex === null ? 'COUNT(*)' : (columns[valueIndex] ?? ''),
    xLabels: xOrder,
    yLabels: yOrder,
    cells,
    min,
    max,
    clippedAxes
  };
}

/** 热力图单元格归一化透明度 [0.12, 0.92]，0 与最大值的视觉对比清晰 */
export function heatmapIntensity(value: number, min: number, max: number): number {
  if (max <= min) {
    return 0.72;
  }
  const t = Math.max(0, Math.min(1, (value - min) / (max - min)));
  return 0.14 + t * 0.78;
}

export interface CorrelationCellStyle {
  readonly fill: string;
  readonly opacity: number;
  readonly highContrast: boolean;
}

/**
 * Pearson 相关系数（[-1, 1]）双极色阶：
 * - 正相关走 --dm-accent（蓝），负相关走 --dm-danger（红），接近 0 走浅中性底色。
 * - 透明度随 |r| 线性递增；|r| >= 0.58 时文字切换反色以保证对比度。
 */
export function correlationHeatmapStyle(value: number): CorrelationCellStyle {
  const clamped = Math.max(-1, Math.min(1, value));
  const abs = Math.abs(clamped);
  if (abs < 0.005) {
    return { fill: 'var(--dm-fg-muted)', opacity: 0.1, highContrast: false };
  }
  return {
    fill: clamped > 0 ? 'var(--dm-accent)' : 'var(--dm-danger)',
    opacity: 0.1 + abs * 0.78,
    highContrast: abs >= 0.58
  };
}

export type ChartAggregateFn = 'SUM' | 'AVG' | 'COUNT' | 'MAX' | 'MIN';

export interface ChartAggregationSpec {
  readonly groupColumn: string;
  readonly metricColumn: string | null;
  readonly fn: ChartAggregateFn;
  readonly topN: number;
}

function quoteIdent(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

/** 为超过 200 行的结果集生成显式内存 DuckDB 聚合 SQL（口径透明，绝不悄悄采样） */
export function buildChartAggregationSql(spec: ChartAggregationSpec): string {
  const groupCol = quoteIdent(spec.groupColumn);
  const topN = Math.max(1, Math.min(MAX_CHART_POINTS, Math.floor(spec.topN)));
  const expr =
    spec.fn === 'COUNT' || spec.metricColumn === null
      ? 'COUNT(*)'
      : `${spec.fn}(CAST(${quoteIdent(spec.metricColumn)} AS DOUBLE))`;
  const metricAlias =
    spec.fn === 'COUNT' || spec.metricColumn === null
      ? 'count'
      : `${spec.fn.toLowerCase()}_${spec.metricColumn}`;

  return [
    `SELECT COALESCE(CAST(${groupCol} AS VARCHAR), '(NULL)') AS ${groupCol},`,
    `  ${expr} AS ${quoteIdent(metricAlias)}`,
    'FROM result_set',
    `GROUP BY ${groupCol}`,
    `ORDER BY ${quoteIdent(metricAlias)} DESC NULLS LAST`,
    `LIMIT ${topN}`
  ].join('\n');
}

function escapeMdCell(text: string): string {
  return text.replace(/\|/g, '\\|').replace(/\r?\n/g, ' ');
}

/** 将当前图表数据转为 Markdown 表格摘要，方便贴进文档或 PR */
export function chartDataToMarkdown(data: ChartData, categoryHeader: string): string {
  const headers = [categoryHeader || 'Category', ...data.series.map((s) => s.name)];
  const lines = [
    `| ${headers.map(escapeMdCell).join(' | ')} |`,
    `| ${headers.map(() => '---').join(' | ')} |`
  ];
  for (let i = 0; i < data.categories.length; i += 1) {
    const cells = [
      escapeMdCell(data.categories[i] ?? ''),
      ...data.series.map((s) => {
        const val = s.points[i];
        return val === null || val === undefined ? '' : String(val);
      })
    ];
    lines.push(`| ${cells.join(' | ')} |`);
  }
  return lines.join('\n');
}

/** 将二维热力图转为标准二维矩阵 Markdown 表格 */
export function heatmapDataToMarkdown(
  data: HeatmapData,
  formatValue?: (value: number) => string,
  cornerLabel?: string
): string {
  const valueMap = new Map<string, number>();
  for (const cell of data.cells) {
    valueMap.set(`${cell.xIdx}:${cell.yIdx}`, cell.value);
  }
  const headers = [cornerLabel ?? (data.yName || 'Row'), ...data.xLabels];
  const lines = [
    `| ${headers.map(escapeMdCell).join(' | ')} |`,
    `| ${headers.map(() => '---').join(' | ')} |`
  ];
  for (let yIdx = 0; yIdx < data.yLabels.length; yIdx += 1) {
    const rowCells = [
      escapeMdCell(data.yLabels[yIdx] ?? ''),
      ...data.xLabels.map((_, xIdx) => {
        const val = valueMap.get(`${xIdx}:${yIdx}`);
        if (val === undefined) {
          return '—';
        }
        return escapeMdCell(formatValue ? formatValue(val) : String(val));
      })
    ];
    lines.push(`| ${rowCells.join(' | ')} |`);
  }
  return lines.join('\n');
}
