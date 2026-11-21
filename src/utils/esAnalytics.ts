import type { SerializedResultValue } from '../contracts/resultSet';
import type { MappingField } from './esMapping';

export const ES_MAPPING_WARN_FIELDS = 800;
export const ES_MAPPING_LIMIT_FIELDS = 1_000;
export const ES_DATE_INTERVAL_OPTIONS = ['15m', '1h', '1d', '7d'] as const;
export type EsDateInterval = (typeof ES_DATE_INTERVAL_OPTIONS)[number];

export interface EsMappingDiagnostics {
  readonly totalFields: number;
  readonly explosionSeverity: 'normal' | 'warn' | 'critical';
  readonly dualTextKeywordCount: number;
  readonly dateFields: readonly string[];
  readonly keywordFields: readonly string[];
  readonly numericFields: readonly string[];
}

export interface EsTermsBucket {
  readonly key: string;
  readonly count: number;
}

export interface EsTermsResult {
  readonly totalHits: number;
  readonly missingCount: number;
  readonly buckets: readonly EsTermsBucket[];
}

export interface EsNumericStatsResult {
  readonly count: number;
  readonly min: number | null;
  readonly max: number | null;
  readonly avg: number | null;
  readonly stdDeviation: number | null;
  readonly p25: number | null;
  readonly p50: number | null;
  readonly p75: number | null;
  readonly p95: number | null;
  readonly p99: number | null;
}

export interface EsIndexHealthSummary {
  readonly permissionDenied: boolean;
  readonly health: string | null;
  readonly primaryShards: number | null;
  readonly replicaShards: number | null;
  readonly docsCount: number | null;
  readonly docsDeleted: number | null;
  readonly deletedRatio: number | null;
  readonly storeSize: string | null;
  readonly shardCount: number;
  readonly unassignedShards: number;
  readonly oversizedShards: number;
  readonly undersizedShards: number;
}

const DATE_TYPES = new Set(['date', 'date_nanos']);
const KEYWORD_TYPES = new Set(['keyword', 'constant_keyword', 'wildcard', 'boolean', 'ip', 'version']);
const NUMERIC_TYPES = new Set([
  'long',
  'integer',
  'short',
  'byte',
  'double',
  'float',
  'half_float',
  'scaled_float',
  'unsigned_long'
]);

/** 从 MappingField 清单汇总字段膨胀预警与三类可聚合字段（date / keyword / numeric） */
export function summarizeEsMapping(fields: readonly MappingField[]): EsMappingDiagnostics {
  const leafFields = fields.filter((f) => f.type !== 'object' && f.type !== 'nested');
  const totalFields = leafFields.length;
  const explosionSeverity: 'normal' | 'warn' | 'critical' =
    totalFields >= ES_MAPPING_LIMIT_FIELDS
      ? 'critical'
      : totalFields >= ES_MAPPING_WARN_FIELDS
        ? 'warn'
        : 'normal';

  const textPaths = new Set(fields.filter((f) => f.type === 'text').map((f) => f.path));
  let dualTextKeywordCount = 0;
  const dateFields: string[] = [];
  const keywordFields: string[] = [];
  const numericFields: string[] = [];

  for (const field of leafFields) {
    if (DATE_TYPES.has(field.type)) {
      dateFields.push(field.path);
    } else if (KEYWORD_TYPES.has(field.type)) {
      keywordFields.push(field.path);
      if (field.multiField) {
        const parent = field.path.slice(0, Math.max(0, field.path.lastIndexOf('.')));
        if (textPaths.has(parent)) {
          dualTextKeywordCount += 1;
        }
      }
    } else if (NUMERIC_TYPES.has(field.type)) {
      numericFields.push(field.path);
    }
  }

  // 把 @timestamp 排在时间字段首位，符合日志排查直觉
  dateFields.sort((a, b) => {
    if (a === '@timestamp') return -1;
    if (b === '@timestamp') return 1;
    return a.localeCompare(b);
  });

  return {
    totalFields,
    explosionSeverity,
    dualTextKeywordCount,
    dateFields,
    keywordFields,
    numericFields
  };
}

/** 构造 `size: 0` 的原生 `date_histogram` 聚合请求体 */
export function buildEsDateHistogramBody(field: string, interval: EsDateInterval): string {
  return JSON.stringify({
    size: 0,
    track_total_hits: true,
    aggs: {
      by_time: {
        date_histogram: {
          field,
          fixed_interval: interval,
          min_doc_count: 0
        }
      }
    }
  });
}

/** 构造 `size: 0` 的原生 `terms + missing` 聚合请求体 */
export function buildEsTermsAggBody(field: string, size = 20): string {
  return JSON.stringify({
    size: 0,
    track_total_hits: true,
    aggs: {
      top_terms: {
        terms: {
          field,
          size: Math.min(100, Math.max(1, size))
        }
      },
      missing_docs: {
        missing: { field }
      }
    }
  });
}

/** 构造 `size: 0` 的原生 `extended_stats + percentiles` 聚合请求体 */
export function buildEsNumericStatsBody(field: string): string {
  return JSON.stringify({
    size: 0,
    track_total_hits: false,
    aggs: {
      stats: { extended_stats: { field } },
      pcts: { percentiles: { field, percents: [25, 50, 75, 95, 99] } }
    }
  });
}

function readTotalHits(parsed: Record<string, unknown>): number {
  const hits = parsed.hits as { total?: number | { value?: number } } | undefined;
  if (typeof hits?.total === 'number') {
    return hits.total;
  }
  return typeof hits?.total?.value === 'number' ? hits.total.value : 0;
}

/** 解析 ES 8 / OpenSearch 的 `date_histogram` 响应 */
export function parseEsDateHistogramResponse(rawBody: string): EsTermsBucket[] {
  const parsed = JSON.parse(rawBody) as {
    aggregations?: { by_time?: { buckets?: Array<{ key_as_string?: string; key?: unknown; doc_count?: number }> } };
  };
  const buckets = parsed.aggregations?.by_time?.buckets ?? [];
  return buckets.map((b) => ({
    key: String(b.key_as_string ?? b.key ?? ''),
    count: typeof b.doc_count === 'number' ? b.doc_count : 0
  }));
}

/** 解析 ES 8 / OpenSearch 的 `terms + missing` 响应 */
export function parseEsTermsResponse(rawBody: string): EsTermsResult {
  const parsed = JSON.parse(rawBody) as Record<string, unknown> & {
    aggregations?: {
      top_terms?: { buckets?: Array<{ key_as_string?: string; key?: unknown; doc_count?: number }> };
      missing_docs?: { doc_count?: number };
    };
  };
  const buckets = (parsed.aggregations?.top_terms?.buckets ?? []).map((b) => ({
    key: String(b.key_as_string ?? b.key ?? ''),
    count: typeof b.doc_count === 'number' ? b.doc_count : 0
  }));
  return {
    totalHits: readTotalHits(parsed),
    missingCount: parsed.aggregations?.missing_docs?.doc_count ?? 0,
    buckets
  };
}

function finiteOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

/** 解析 ES 8 / OpenSearch 的 `extended_stats + percentiles` 响应 */
export function parseEsNumericStatsResponse(rawBody: string): EsNumericStatsResult {
  const parsed = JSON.parse(rawBody) as {
    aggregations?: {
      stats?: Record<string, unknown>;
      pcts?: { values?: Record<string, unknown> };
    };
  };
  const stats = parsed.aggregations?.stats ?? {};
  const pcts = parsed.aggregations?.pcts?.values ?? {};
  return {
    count: typeof stats.count === 'number' ? stats.count : 0,
    min: finiteOrNull(stats.min),
    max: finiteOrNull(stats.max),
    avg: finiteOrNull(stats.avg),
    stdDeviation: finiteOrNull(stats.std_deviation),
    p25: finiteOrNull(pcts['25.0'] ?? pcts['25']),
    p50: finiteOrNull(pcts['50.0'] ?? pcts['50']),
    p75: finiteOrNull(pcts['75.0'] ?? pcts['75']),
    p95: finiteOrNull(pcts['95.0'] ?? pcts['95']),
    p99: finiteOrNull(pcts['99.0'] ?? pcts['99'])
  };
}

/** 将分片大小文本（如 `12.5mb`、`60gb`）转为字节，供单分片过碎/过大诊断 */
export function parseEsByteSize(text: string | null | undefined): number | null {
  if (!text) return null;
  const match = text.trim().toLowerCase().match(/^([\d.]+)\s*(b|kb|mb|gb|tb)$/);
  if (!match) return null;
  const value = Number(match[1]);
  if (!Number.isFinite(value)) return null;
  const scale: Record<string, number> = {
    b: 1,
    kb: 1024,
    mb: 1024 ** 2,
    gb: 1024 ** 3,
    tb: 1024 ** 4
  };
  return value * (scale[match[2]] ?? 1);
}

/** 解析 `_cat/indices` + `_cat/shards` 健康摘要；遇 `403` 时独立降级不抛异常（AN0-E4 门禁） */
export function parseEsIndexHealth(
  indicesResp: { status: number; body: string } | null,
  shardsResp: { status: number; body: string } | null
): EsIndexHealthSummary {
  if (indicesResp?.status === 403 || shardsResp?.status === 403) {
    return {
      permissionDenied: true,
      health: null,
      primaryShards: null,
      replicaShards: null,
      docsCount: null,
      docsDeleted: null,
      deletedRatio: null,
      storeSize: null,
      shardCount: 0,
      unassignedShards: 0,
      oversizedShards: 0,
      undersizedShards: 0
    };
  }

  let health: string | null = null;
  let primaryShards: number | null = null;
  let replicaShards: number | null = null;
  let docsCount: number | null = null;
  let docsDeleted: number | null = null;
  let storeSize: string | null = null;

  if (indicesResp && indicesResp.status >= 200 && indicesResp.status < 300) {
    try {
      const rows = JSON.parse(indicesResp.body) as Array<Record<string, string>>;
      const first = rows[0];
      if (first) {
        health = first.health ?? null;
        primaryShards = first.pri ? Number(first.pri) : null;
        replicaShards = first.rep ? Number(first.rep) : null;
        docsCount = first['docs.count'] ? Number(first['docs.count']) : null;
        docsDeleted = first['docs.deleted'] ? Number(first['docs.deleted']) : null;
        storeSize = first['store.size'] ?? null;
      }
    } catch {
      // ignore malformed optional _cat reply
    }
  }

  let shardCount = 0;
  let unassignedShards = 0;
  let oversizedShards = 0;
  let undersizedShards = 0;

  if (shardsResp && shardsResp.status >= 200 && shardsResp.status < 300) {
    try {
      const shards = JSON.parse(shardsResp.body) as Array<Record<string, string>>;
      shardCount = shards.length;
      for (const s of shards) {
        if ((s.state ?? '').toUpperCase() === 'UNASSIGNED') {
          unassignedShards += 1;
        }
        const bytes = parseEsByteSize(s.store);
        if (bytes !== null) {
          if (bytes > 50 * 1024 ** 3) {
            oversizedShards += 1;
          } else if (bytes > 0 && bytes < 100 * 1024 ** 2) {
            undersizedShards += 1;
          }
        }
      }
    } catch {
      // ignore
    }
  }

  const deletedRatio =
    docsCount !== null && docsDeleted !== null && docsCount + docsDeleted > 0
      ? docsDeleted / (docsCount + docsDeleted)
      : null;

  return {
    permissionDenied: false,
    health,
    primaryShards,
    replicaShards,
    docsCount,
    docsDeleted,
    deletedRatio,
    storeSize,
    shardCount,
    unassignedShards,
    oversizedShards,
    undersizedShards
  };
}

/** 将 ES 聚合桶转为 `ResultChartDialog` 原生 SVG 所需的表格行列 */
export function esBucketsToChartTable(
  labelCol: string,
  buckets: readonly EsTermsBucket[]
): { columns: string[]; rows: SerializedResultValue[][] } {
  return {
    columns: [labelCol, 'doc_count'],
    rows: buckets.map((b) => [b.key, b.count])
  };
}
