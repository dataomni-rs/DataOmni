import type {
  ColumnProfile,
  HistogramBin,
  ProfileColumnType,
  TimeHistogramBin
} from '../contracts/analytics';
import type { MongoDocumentRow } from './mongoDocuments';
import { quoteSqlIdentifier, type SqlIdentifierDialect } from './sqlIdentifiers';

export type ProfileSortMode = 'order' | 'nulls' | 'distinct' | 'drift';
export type NullSeverity = 'normal' | 'warn' | 'high';
export type ColumnCardinalityHint = 'all-null' | 'constant' | 'unique-key';

/** 识别列的直觉特征：全空 / 常量列 / 唯一键候选（单行样本不做常量/唯一判断） */
export function columnCardinalityHint(profile: ColumnProfile): ColumnCardinalityHint | null {
  if (profile.totalCount <= 0) {
    return null;
  }
  if (profile.nullCount >= profile.totalCount) {
    return 'all-null';
  }
  if (profile.totalCount >= 2 && profile.nullCount === 0) {
    if (profile.distinctCount === 1) {
      return 'constant';
    }
    if (profile.distinctCount === profile.totalCount) {
      return 'unique-key';
    }
  }
  return null;
}

/** 空值率告警分级：<5% 正常，5%–30% 提示，>=30% 高空值 */
export function nullSeverity(nullRatio: number): NullSeverity {
  if (nullRatio >= 0.3) {
    return 'high';
  }
  if (nullRatio >= 0.05) {
    return 'warn';
  }
  return 'normal';
}

/** 文档模式中字段未在所有文档里出现（存在 Schema 漂移） */
export function hasSchemaDrift(profile: ColumnProfile): boolean {
  return profile.presentRatio !== undefined && profile.presentRatio < 1;
}

/** 比率格式化为易读百分比；极小非零值显示 `<0.1%`，避免把 1/10000 误印成 `0%` */
export function formatProfileRatio(ratio: number): string {
  if (!Number.isFinite(ratio) || ratio <= 0) {
    return '0%';
  }
  if (ratio >= 1) {
    return '100%';
  }
  const pct = ratio * 100;
  if (pct < 0.1) {
    return '<0.1%';
  }
  return `${Number(pct.toFixed(1))}%`;
}

/** 统计数值的紧凑显示 */
export function formatProfileNumber(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value)) {
    return '—';
  }
  const abs = Math.abs(value);
  if (abs === 0) {
    return '0';
  }
  if (abs >= 1_000_000_000 || abs < 0.0001) {
    return value.toExponential(2);
  }
  if (Number.isInteger(value)) {
    return value.toLocaleString('en-US');
  }
  return Number(value.toFixed(abs >= 100 ? 2 : 4)).toLocaleString('en-US', {
    maximumFractionDigits: 4
  });
}

/** 直方图桶高度归一化到 [0, 1]；非零桶保底一个可见高度由组件决定 */
export function histogramHeights(bins: readonly HistogramBin[]): number[] {
  const max = bins.reduce((best, bin) => Math.max(best, bin.count), 0);
  if (max === 0) {
    return bins.map(() => 0);
  }
  return bins.map((bin) => bin.count / max);
}

/** 按关键字筛选并按指定维度排序列画像（不改原数组） */
export function filterAndSortProfiles(
  columns: readonly ColumnProfile[],
  query: string,
  sortMode: ProfileSortMode
): ColumnProfile[] {
  const needle = query.trim().toLowerCase();
  const filtered = needle === ''
    ? [...columns]
    : columns.filter(
        (col) =>
          col.name.toLowerCase().includes(needle)
          || col.duckdbType.toLowerCase().includes(needle)
      );

  if (sortMode === 'order') {
    return filtered;
  }

  return filtered.sort((a, b) => {
    if (sortMode === 'nulls') {
      return b.nullRatio - a.nullRatio || b.nullCount - a.nullCount;
    }
    if (sortMode === 'distinct') {
      return b.distinctCount - a.distinctCount;
    }
    // drift：存在率越低越靠前（漂移最明显），非文档模式按空值率兜底
    const aPresence = a.presentRatio ?? 1;
    const bPresence = b.presentRatio ?? 1;
    return aPresence - bPresence || b.nullRatio - a.nullRatio;
  });
}

function quoteDuckIdent(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

function quoteDuckString(value: string): string {
  return `'${value.replace(/'/g, "''")}'`;
}

/** 生成点击高频值时的一键二次过滤 SQL（面向内存表 `result_set`） */
export function buildProfileFilterSql(
  columnName: string,
  inferredType: ProfileColumnType,
  value: string
): string {
  const col = quoteDuckIdent(columnName);
  if ((inferredType === 'integer' || inferredType === 'numeric') && /^-?\d+(\.\d+)?$/.test(value.trim())) {
    return `SELECT * FROM result_set WHERE ${col} = ${value.trim()}`;
  }
  if (inferredType === 'boolean' && (value === 'true' || value === 'false')) {
    return `SELECT * FROM result_set WHERE ${col} = ${value}`;
  }
  return `SELECT * FROM result_set WHERE ${col} = ${quoteDuckString(value)}`;
}

/** 生成点击空值告警时的一键查空值 SQL */
export function buildProfileNullFilterSql(columnName: string): string {
  return `SELECT * FROM result_set WHERE ${quoteDuckIdent(columnName)} IS NULL`;
}

/** 生成按指定列分组统计频次的 SQL（面向内存表 `result_set`） */
export function buildProfileGroupBySql(columnName: string): string {
  const col = quoteDuckIdent(columnName);
  return [
    `SELECT ${col},`,
    '  COUNT(*) AS row_count',
    'FROM result_set',
    `GROUP BY ${col}`,
    'ORDER BY row_count DESC',
    'LIMIT 50'
  ].join('\n');
}

/**
 * 识别噪音大于信息的高基数唯一列：全唯一（或 >= 5 行且去重率 >= 98% 且 Top-5 全为出现 1 次）时，
 * 默认折叠 Top-5 示例列表，让用户一眼聚焦在空值率、枚举分布和异常列上。
 */
export function shouldCollapseTopValues(profile: ColumnProfile): boolean {
  if (profile.topValues.length === 0) {
    return false;
  }
  if (columnCardinalityHint(profile) === 'unique-key') {
    return true;
  }
  if (
    profile.totalCount >= 5
    && profile.distinctRatio >= 0.98
    && profile.topValues.every((entry) => entry.count <= 1)
  ) {
    return true;
  }
  // 已有时间分布迷你图的高基数时间列（如 20,000 行里偶发同秒碰撞），精确到秒的 Top-5 列表噪音大于信息
  if (
    profile.timeBins
    && profile.timeBins.length >= 2
    && profile.distinctRatio >= 0.9
    && profile.topValues.every((entry) => entry.ratio < 0.01)
  ) {
    return true;
  }
  return false;
}

export interface NumericOutlierSuggestion {
  readonly kind: 'negative' | 'high';
  readonly threshold: string;
  readonly sql: string;
}

/**
 * 数值列离群值快速下钻建议：
 * - 优先抓意外负值（min < 0 且 p25 >= 0，常见于金额/数量/耗时脏数据）；
 * - 否则在存在显著高位拖尾（max > p95）时给出一键 `> P95` 过滤 SQL。
 */
export function numericOutlierFilter(profile: ColumnProfile): NumericOutlierSuggestion | null {
  if (profile.inferredType !== 'integer' && profile.inferredType !== 'numeric') {
    return null;
  }
  if (profile.totalCount < 5 || profile.min === undefined || profile.max === undefined) {
    return null;
  }
  const col = quoteDuckIdent(profile.name);
  const minVal = Number(profile.min);
  const maxVal = Number(profile.max);
  if (Number.isFinite(minVal) && minVal < 0 && profile.p25 !== undefined && profile.p25 >= 0) {
    return {
      kind: 'negative',
      threshold: '0',
      sql: `SELECT * FROM result_set WHERE ${col} < 0`
    };
  }
  if (
    columnCardinalityHint(profile) !== 'unique-key'
    && profile.p95 !== undefined
    && Number.isFinite(profile.p95)
    && Number.isFinite(maxVal)
    && maxVal > profile.p95
  ) {
    // 排除近似均匀递增序列（如 1..50，P50=25.5, P95=47.55, Max=50，右尾宽度仅 11%）；
    // 仅当存在明显右偏长尾时提示高位离群下钻。
    const spread =
      profile.p50 !== undefined && Number.isFinite(profile.p50) && profile.p95 > profile.p50
        ? profile.p95 - profile.p50
        : 0;
    if (spread === 0 || maxVal - profile.p95 > spread * 0.25) {
      const thresholdNum = Number(profile.p95.toFixed(4));
      return {
        kind: 'high',
        threshold: formatProfileNumber(profile.p95),
        sql: `SELECT * FROM result_set WHERE ${col} > ${thresholdNum} ORDER BY ${col} DESC`
      };
    }
  }
  return null;
}

/** 生成可粘贴回原数据库查询框的方言 `WHERE` 条件片段 */
export function buildSourceWhereClause(
  columnName: string,
  inferredType: ProfileColumnType,
  value: string | null,
  dialect: SqlIdentifierDialect
): string {
  const col = quoteSqlIdentifier(columnName, dialect);
  if (value === null) {
    return `${col} IS NULL`;
  }
  if ((inferredType === 'integer' || inferredType === 'numeric') && /^-?\d+(\.\d+)?$/.test(value.trim())) {
    return `${col} = ${value.trim()}`;
  }
  if (inferredType === 'boolean' && (value === 'true' || value === 'false')) {
    return `${col} = ${value.toUpperCase()}`;
  }
  return `${col} = ${quoteDuckString(value)}`;
}

export interface SparklinePoint {
  readonly x: number;
  readonly y: number;
  readonly label: string;
  readonly count: number;
}

/** 将时间分桶计数映射为内联 SVG Sparkline 坐标点 */
export function timeSparklinePoints(
  bins: readonly TimeHistogramBin[],
  width = 200,
  height = 32,
  padX = 4,
  padY = 4
): SparklinePoint[] {
  if (bins.length < 2) {
    return [];
  }
  const maxCount = bins.reduce((best, bin) => Math.max(best, bin.count), 0);
  const innerW = Math.max(1, width - padX * 2);
  const innerH = Math.max(1, height - padY * 2);
  return bins.map((bin, idx) => {
    const x = padX + (innerW * idx) / (bins.length - 1);
    const ratio = maxCount > 0 ? bin.count / maxCount : 0;
    const y = padY + innerH * (1 - ratio);
    return { x, y, label: bin.label, count: bin.count };
  });
}

/**
 * 将 MongoDB 集合页的 `MongoDocumentRow[]` 转为普通 JSON 对象数组供内存 DuckDB 画像：
 * 缺失字段不写入键（保 `presentCount` 精确），`kind === 'null'` 写入 `null`（保 `explicitNullCount` 精确）。
 */
export function mongoRowsToJsonDocs(
  documents: readonly MongoDocumentRow[]
): Record<string, unknown>[] {
  return documents.map((doc) => {
    const obj: Record<string, unknown> = {};
    for (const [key, cell] of doc.fields) {
      if (cell.kind === 'null') {
        obj[key] = null;
      } else if (cell.kind === 'bool') {
        obj[key] = cell.text === 'true';
      } else if (cell.kind === 'int' || cell.kind === 'double') {
        const parsed = Number(cell.text);
        obj[key] = Number.isFinite(parsed) ? parsed : cell.text;
      } else {
        obj[key] = cell.text;
      }
    }
    return obj;
  });
}

/**
 * 从 Elasticsearch 原始 JSON 响应文本提取文档对象列表（供内存 DuckDB 异构文档画像）。
 * 支持 `_search` 返回的 `hits.hits[]._source` 与顶层对象数组；不满足时返回空数组。
 */
export function extractEsJsonDocs(rawBody: string): Record<string, unknown>[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(rawBody);
  } catch {
    return [];
  }
  if (Array.isArray(parsed)) {
    return parsed.filter(
      (item): item is Record<string, unknown> =>
        item !== null && typeof item === 'object' && !Array.isArray(item)
    );
  }
  if (parsed !== null && typeof parsed === 'object') {
    const hits = (parsed as { hits?: { hits?: unknown } }).hits?.hits;
    if (Array.isArray(hits)) {
      const docs: Record<string, unknown>[] = [];
      for (const hit of hits) {
        if (hit !== null && typeof hit === 'object') {
          const record = hit as Record<string, unknown>;
          const source = record._source;
          if (source !== null && typeof source === 'object' && !Array.isArray(source)) {
            docs.push(source as Record<string, unknown>);
          } else {
            docs.push(record);
          }
        }
      }
      return docs;
    }
  }
  return [];
}
