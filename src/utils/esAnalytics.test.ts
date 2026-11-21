import { describe, expect, it } from 'vitest';
import {
  buildEsDateHistogramBody,
  buildEsNumericStatsBody,
  buildEsTermsAggBody,
  esBucketsToChartTable,
  parseEsDateHistogramResponse,
  parseEsIndexHealth,
  parseEsNumericStatsResponse,
  parseEsTermsResponse,
  summarizeEsMapping
} from './esAnalytics';
import type { MappingField } from './esMapping';

describe('esAnalytics', () => {
  it('detects mapping explosion (>1000 fields), dual text+keyword storage, and categorizes fields', () => {
    const fields: MappingField[] = [
      { path: 'created_at', depth: 0, type: 'date', multiField: false, details: [] },
      { path: '@timestamp', depth: 0, type: 'date', multiField: false, details: [] },
      { path: 'title', depth: 0, type: 'text', multiField: false, details: [] },
      { path: 'title.keyword', depth: 1, type: 'keyword', multiField: true, details: [] },
      { path: 'latency_ms', depth: 0, type: 'double', multiField: false, details: [] }
    ];
    for (let i = 0; i < 1_000; i += 1) {
      fields.push({ path: `dyn_${i}`, depth: 0, type: 'keyword', multiField: false, details: [] });
    }
    const summary = summarizeEsMapping(fields);
    expect(summary.totalFields).toBe(1_005);
    expect(summary.explosionSeverity).toBe('critical');
    expect(summary.dualTextKeywordCount).toBe(1);
    expect(summary.dateFields[0]).toBe('@timestamp');
    expect(summary.numericFields).toEqual(['latency_ms']);
  });

  it('builds size:0 native aggs and parses ES 8 / OpenSearch responses into chart tables', () => {
    expect(JSON.parse(buildEsDateHistogramBody('@timestamp', '1h')).size).toBe(0);
    expect(JSON.parse(buildEsTermsAggBody('level', 10)).aggs.missing_docs.missing.field).toBe('level');
    expect(JSON.parse(buildEsNumericStatsBody('latency_ms')).aggs.pcts.percentiles.percents).toContain(95);

    const hist = parseEsDateHistogramResponse(
      JSON.stringify({
        aggregations: {
          by_time: {
            buckets: [
              { key_as_string: '2026-11-20T00:00:00Z', doc_count: 120 },
              { key_as_string: '2026-11-20T01:00:00Z', doc_count: 340 }
            ]
          }
        }
      })
    );
    expect(esBucketsToChartTable('@timestamp', hist)).toEqual({
      columns: ['@timestamp', 'doc_count'],
      rows: [
        ['2026-11-20T00:00:00Z', 120],
        ['2026-11-20T01:00:00Z', 340]
      ]
    });

    const terms = parseEsTermsResponse(
      JSON.stringify({
        hits: { total: { value: 500 } },
        aggregations: {
          top_terms: { buckets: [{ key: 'ERROR', doc_count: 42 }] },
          missing_docs: { doc_count: 5 }
        }
      })
    );
    expect(terms).toEqual({
      totalHits: 500,
      missingCount: 5,
      buckets: [{ key: 'ERROR', count: 42 }]
    });

    const stats = parseEsNumericStatsResponse(
      JSON.stringify({
        aggregations: {
          stats: { count: 100, min: 1, max: 950, avg: 45.2, std_deviation: 18.1 },
          pcts: { values: { '25.0': 10, '50.0': 30, '75.0': 60, '95.0': 210, '99.0': 800 } }
        }
      })
    );
    expect(stats.p95).toBe(210);
    expect(stats.avg).toBe(45.2);
  });

  it('degrades _cat health cleanly on HTTP 403 and flags shard/delete anomalies when authorized', () => {
    const denied = parseEsIndexHealth(
      { status: 403, body: '{"error":"security_exception"}' },
      { status: 403, body: '{"error":"security_exception"}' }
    );
    expect(denied.permissionDenied).toBe(true);

    const healthy = parseEsIndexHealth(
      {
        status: 200,
        body: JSON.stringify([
          {
            health: 'yellow',
            pri: '3',
            rep: '1',
            'docs.count': '700',
            'docs.deleted': '300',
            'store.size': '2.4gb'
          }
        ])
      },
      {
        status: 200,
        body: JSON.stringify([
          { state: 'STARTED', store: '60gb' },
          { state: 'STARTED', store: '12mb' },
          { state: 'UNASSIGNED', store: null }
        ])
      }
    );
    expect(healthy.permissionDenied).toBe(false);
    expect(healthy.health).toBe('yellow');
    expect(healthy.deletedRatio).toBeCloseTo(0.3, 5);
    expect(healthy.oversizedShards).toBe(1);
    expect(healthy.undersizedShards).toBe(1);
    expect(healthy.unassignedShards).toBe(1);
  });
});
