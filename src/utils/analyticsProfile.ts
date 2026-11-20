import type { ColumnProfile, HistogramBin, ProfileColumnType } from '../contracts/analytics';
import type { MongoDocumentRow } from './mongoDocuments';

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
