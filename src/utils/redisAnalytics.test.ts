import { describe, expect, it } from 'vitest';
import {
  aggregateRedisKeyspace,
  classifyRedisTtlBucket,
  extractRedisKeyPrefix,
  formatRedisBytes,
  rankRedisBigKeys,
  redisValueToAnalyticsRows,
  type RedisKeyspaceAnalysis,
  type RedisSampledKey
} from './redisAnalytics';

function makeKey(
  text: string,
  overrides: Partial<Omit<RedisSampledKey, 'key'>> = {}
): RedisSampledKey {
  return {
    key: { raw: btoa(text), text, binary: false },
    kind: 'string',
    ttlMs: -1,
    memoryBytes: 256,
    length: 64,
    ...overrides
  };
}

describe('redisAnalytics', () => {
  it('classifies TTL milliseconds across all 5 health tiers and skips expired (-2)', () => {
    expect(classifyRedisTtlBucket(-2)).toBeNull();
    expect(classifyRedisTtlBucket(-1)).toBe('no-expire');
    expect(classifyRedisTtlBucket(0)).toBe('lt-1h');
    expect(classifyRedisTtlBucket(3_599_999)).toBe('lt-1h');
    expect(classifyRedisTtlBucket(3_600_000)).toBe('1h-24h');
    expect(classifyRedisTtlBucket(86_400_000)).toBe('1d-7d');
    expect(classifyRedisTtlBucket(7 * 86_400_000)).toBe('gt-7d');
  });

  it('extracts configurable prefix depth and root keys', () => {
    expect(extractRedisKeyPrefix('session:tenant1:user:42', ':', 1)).toBe('session:*');
    expect(extractRedisKeyPrefix('session:tenant1:user:42', ':', 2)).toBe('session:tenant1:*');
    expect(extractRedisKeyPrefix('cache/v1/item', '/', 1)).toBe('cache/*');
    expect(extractRedisKeyPrefix('standalone_key', ':', 1)).toBe('(root)');
  });

  it('aggregates prefix memory, TTL distribution, and ranks BigKeys with filters', () => {
    const analysis: RedisKeyspaceAnalysis = {
      sampledCount: 4,
      partial: false,
      cancelled: false,
      memoryUsageAvailable: true,
      elapsedMs: 42,
      keys: [
        makeKey('session:u1', { memoryBytes: 1000, ttlMs: -1, kind: 'string', length: 900 }),
        makeKey('session:u2', { memoryBytes: 500, ttlMs: 120_000, kind: 'string', length: 400 }),
        makeKey('cache:big', { memoryBytes: 4000, ttlMs: 7_200_000, kind: 'hash', length: 12_000 }),
        makeKey('lone', { memoryBytes: 100, ttlMs: 10 * 86_400_000, kind: 'zset', length: 300 })
      ]
    };

    const summary = aggregateRedisKeyspace(analysis, ':', 1);
    expect(summary.totalKeys).toBe(4);
    expect(summary.totalBytes).toBe(5600);
    expect(summary.noExpireCount).toBe(1);
    expect(summary.prefixes[0]).toMatchObject({
      prefix: 'cache:*',
      keyCount: 1,
      totalBytes: 4000,
      topKind: 'hash'
    });
    expect(summary.prefixes[1]).toMatchObject({
      prefix: 'session:*',
      keyCount: 2,
      totalBytes: 1500,
      noExpireCount: 1,
      noExpireRatio: 0.5
    });

    const topByLen = rankRedisBigKeys(analysis.keys, { sortBy: 'length', limit: 2 });
    expect(topByLen.map((k) => k.key.text)).toEqual(['cache:big', 'session:u1']);

    const onlyNoExpire = rankRedisBigKeys(analysis.keys, {
      sortBy: 'memory',
      ttlBucket: 'no-expire'
    });
    expect(onlyNoExpire.map((k) => k.key.text)).toEqual(['session:u1']);
  });

  it('formats bytes and converts Hash/ZSet values for DuckDB secondary analytics', () => {
    expect(formatRedisBytes(0)).toBe('0 B');
    expect(formatRedisBytes(2048)).toBe('2.0 KB');
    expect(formatRedisBytes(3 * 1024 * 1024)).toBe('3.00 MB');

    const zsetRows = redisValueToAnalyticsRows({
      kind: 'zset',
      length: 2,
      offset: 0,
      entries: [
        [{ raw: 'YQ==', text: 'alice', binary: false }, '98.5'],
        [{ raw: 'Yg==', text: 'bob', binary: false }, '72']
      ],
      next: null
    });
    expect(zsetRows).toEqual({
      columns: [
        { name: 'member', logicalType: 'text' },
        { name: 'score', logicalType: 'numeric' }
      ],
      rows: [
        ['alice', 98.5],
        ['bob', 72]
      ]
    });
  });
});
