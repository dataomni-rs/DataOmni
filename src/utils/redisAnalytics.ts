import type { RedisBytes, RedisValue } from './redisKeys';

export type RedisPrefixSeparator = ':' | '/' | '.';
export const REDIS_PREFIX_SEPARATORS: readonly RedisPrefixSeparator[] = [':', '/', '.'];
export const REDIS_SAMPLE_SIZE_OPTIONS = [1_000, 5_000, 20_000] as const;
export const DEFAULT_REDIS_SAMPLE_SIZE = 5_000;

export type RedisTtlBucketId = 'no-expire' | 'lt-1h' | '1h-24h' | '1d-7d' | 'gt-7d';

export interface RedisSampledKey {
  readonly key: RedisBytes;
  readonly kind: string;
  readonly ttlMs: number;
  readonly memoryBytes?: number;
  readonly length?: number;
}

export interface RedisKeyspaceAnalysis {
  readonly sampledCount: number;
  readonly partial: boolean;
  readonly cancelled: boolean;
  readonly memoryUsageAvailable: boolean;
  readonly elapsedMs: number;
  readonly keys: readonly RedisSampledKey[];
}

export interface RedisPrefixStat {
  readonly prefix: string;
  readonly keyCount: number;
  readonly keyRatio: number;
  readonly totalBytes: number;
  readonly byteRatio: number;
  readonly noExpireCount: number;
  readonly noExpireRatio: number;
  readonly topKind: string;
}

export interface RedisTtlBucketStat {
  readonly id: RedisTtlBucketId;
  readonly keyCount: number;
  readonly keyRatio: number;
  readonly totalBytes: number;
}

export interface RedisKindStat {
  readonly kind: string;
  readonly keyCount: number;
  readonly keyRatio: number;
  readonly totalBytes: number;
}

export interface RedisAnalyticsSummary {
  readonly totalKeys: number;
  readonly totalBytes: number;
  readonly noExpireCount: number;
  readonly prefixes: readonly RedisPrefixStat[];
  readonly ttlBuckets: readonly RedisTtlBucketStat[];
  readonly kinds: readonly RedisKindStat[];
}

const HOUR_MS = 3_600_000;
const DAY_MS = 24 * HOUR_MS;
const WEEK_MS = 7 * DAY_MS;

/** 将 PTTL 毫秒值归入 5 档健康度分桶；已过期（-2）返回 null */
export function classifyRedisTtlBucket(ttlMs: number): RedisTtlBucketId | null {
  if (ttlMs === -2) {
    return null;
  }
  if (ttlMs < 0) {
    return 'no-expire';
  }
  if (ttlMs < HOUR_MS) {
    return 'lt-1h';
  }
  if (ttlMs < DAY_MS) {
    return '1h-24h';
  }
  if (ttlMs < WEEK_MS) {
    return '1d-7d';
  }
  return 'gt-7d';
}

/** 按指定分隔符提取 1–2 级命名空间前缀（无分隔符归为 `(root)`） */
export function extractRedisKeyPrefix(
  keyText: string,
  separator: RedisPrefixSeparator,
  depth: 1 | 2 = 1
): string {
  const parts = keyText.split(separator).filter((segment) => segment.length > 0);
  if (parts.length <= 1) {
    return '(root)';
  }
  const take = Math.min(depth, parts.length - 1);
  return `${parts.slice(0, take).join(separator)}${separator}*`;
}

/** 估算单键字节权重：优先真实 `MEMORY USAGE`，ACL 禁用时回退至元素数/字符串长度 */
export function effectiveKeyWeight(key: RedisSampledKey): number {
  if (key.memoryBytes !== undefined) {
    return key.memoryBytes;
  }
  return key.length ?? 0;
}

/** 纯函数聚合 Redis 抽样结果：前缀内存占比、TTL 5 档健康度与数据类型分布 */
export function aggregateRedisKeyspace(
  analysis: RedisKeyspaceAnalysis,
  separator: RedisPrefixSeparator = ':',
  prefixDepth: 1 | 2 = 1
): RedisAnalyticsSummary {
  const totalKeys = analysis.keys.length;
  let totalBytes = 0;
  let noExpireCount = 0;

  const prefixMap = new Map<
    string,
    { keyCount: number; totalBytes: number; noExpireCount: number; kinds: Map<string, number> }
  >();
  const kindMap = new Map<string, { keyCount: number; totalBytes: number }>();
  const ttlMap = new Map<RedisTtlBucketId, { keyCount: number; totalBytes: number }>([
    ['no-expire', { keyCount: 0, totalBytes: 0 }],
    ['lt-1h', { keyCount: 0, totalBytes: 0 }],
    ['1h-24h', { keyCount: 0, totalBytes: 0 }],
    ['1d-7d', { keyCount: 0, totalBytes: 0 }],
    ['gt-7d', { keyCount: 0, totalBytes: 0 }]
  ]);

  for (const row of analysis.keys) {
    const weight = effectiveKeyWeight(row);
    totalBytes += weight;
    const isNoExpire = row.ttlMs === -1;
    if (isNoExpire) {
      noExpireCount += 1;
    }

    const prefix = extractRedisKeyPrefix(row.key.text, separator, prefixDepth);
    let prefixAcc = prefixMap.get(prefix);
    if (!prefixAcc) {
      prefixAcc = { keyCount: 0, totalBytes: 0, noExpireCount: 0, kinds: new Map() };
      prefixMap.set(prefix, prefixAcc);
    }
    prefixAcc.keyCount += 1;
    prefixAcc.totalBytes += weight;
    if (isNoExpire) {
      prefixAcc.noExpireCount += 1;
    }
    prefixAcc.kinds.set(row.kind, (prefixAcc.kinds.get(row.kind) ?? 0) + 1);

    let kindAcc = kindMap.get(row.kind);
    if (!kindAcc) {
      kindAcc = { keyCount: 0, totalBytes: 0 };
      kindMap.set(row.kind, kindAcc);
    }
    kindAcc.keyCount += 1;
    kindAcc.totalBytes += weight;

    const bucket = classifyRedisTtlBucket(row.ttlMs);
    if (bucket) {
      const ttlAcc = ttlMap.get(bucket);
      if (ttlAcc) {
        ttlAcc.keyCount += 1;
        ttlAcc.totalBytes += weight;
      }
    }
  }

  const prefixes: RedisPrefixStat[] = Array.from(prefixMap.entries())
    .map(([prefix, acc]) => {
      let topKind = 'string';
      let topKindCount = -1;
      for (const [k, cnt] of acc.kinds.entries()) {
        if (cnt > topKindCount) {
          topKind = k;
          topKindCount = cnt;
        }
      }
      return {
        prefix,
        keyCount: acc.keyCount,
        keyRatio: totalKeys > 0 ? acc.keyCount / totalKeys : 0,
        totalBytes: acc.totalBytes,
        byteRatio: totalBytes > 0 ? acc.totalBytes / totalBytes : 0,
        noExpireCount: acc.noExpireCount,
        noExpireRatio: acc.keyCount > 0 ? acc.noExpireCount / acc.keyCount : 0,
        topKind
      };
    })
    .sort((a, b) => b.totalBytes - a.totalBytes || b.keyCount - a.keyCount || a.prefix.localeCompare(b.prefix));

  const ttlBuckets: RedisTtlBucketStat[] = Array.from(ttlMap.entries()).map(([id, acc]) => ({
    id,
    keyCount: acc.keyCount,
    keyRatio: totalKeys > 0 ? acc.keyCount / totalKeys : 0,
    totalBytes: acc.totalBytes
  }));

  const kinds: RedisKindStat[] = Array.from(kindMap.entries())
    .map(([kind, acc]) => ({
      kind,
      keyCount: acc.keyCount,
      keyRatio: totalKeys > 0 ? acc.keyCount / totalKeys : 0,
      totalBytes: acc.totalBytes
    }))
    .sort((a, b) => b.totalBytes - a.totalBytes || b.keyCount - a.keyCount);

  return {
    totalKeys,
    totalBytes,
    noExpireCount,
    prefixes,
    ttlBuckets,
    kinds
  };
}

/** 按估算内存或元素数量排 BigKey Top N（可选按 TTL 分档或前缀过滤） */
export function rankRedisBigKeys(
  keys: readonly RedisSampledKey[],
  options: {
    sortBy: 'memory' | 'length';
    limit?: number;
    ttlBucket?: RedisTtlBucketId | null;
    prefix?: string | null;
    separator?: RedisPrefixSeparator;
    prefixDepth?: 1 | 2;
  }
): RedisSampledKey[] {
  const { sortBy, limit = 50, ttlBucket = null, prefix = null, separator = ':', prefixDepth = 1 } = options;
  const filtered = keys.filter((row) => {
    if (ttlBucket && classifyRedisTtlBucket(row.ttlMs) !== ttlBucket) {
      return false;
    }
    if (prefix && extractRedisKeyPrefix(row.key.text, separator, prefixDepth) !== prefix) {
      return false;
    }
    return true;
  });

  return filtered
    .sort((a, b) => {
      if (sortBy === 'length') {
        return (b.length ?? 0) - (a.length ?? 0) || effectiveKeyWeight(b) - effectiveKeyWeight(a);
      }
      return effectiveKeyWeight(b) - effectiveKeyWeight(a) || (b.length ?? 0) - (a.length ?? 0);
    })
    .slice(0, limit);
}

/** 格式化字节数为人类可读单位 */
export function formatRedisBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B';
  }
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${Math.round(bytes)} B`;
}

/** 将单个 Hash / ZSet 集合值转换为可供 DuckDB 二次分析的二维结果集 */
export function redisValueToAnalyticsRows(
  value: RedisValue
): { columns: { name: string; logicalType: string }[]; rows: (string | number | null)[][] } | null {
  if (value.kind === 'hash' && value.entries.length > 0) {
    return {
      columns: [
        { name: 'field', logicalType: 'text' },
        { name: 'value', logicalType: 'text' }
      ],
      rows: value.entries.map(([field, val]) => [field.text, val.text])
    };
  }
  if (value.kind === 'zset' && value.entries.length > 0) {
    return {
      columns: [
        { name: 'member', logicalType: 'text' },
        { name: 'score', logicalType: 'numeric' }
      ],
      rows: value.entries.map(([member, score]) => {
        const numeric = Number(score);
        return [member.text, Number.isFinite(numeric) ? numeric : score];
      })
    };
  }
  return null;
}
