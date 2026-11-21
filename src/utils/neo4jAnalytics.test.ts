import { describe, expect, it } from 'vitest';
import { cypherMayWrite } from './cypherRisk';
import {
  buildInspectLabelCypher,
  buildInspectSupernodeCypher,
  buildLabelCountCypher,
  buildNeo4jDegreeSampleCypher,
  buildRelationshipCountCypher,
  neo4jCountsToChartTable,
  summarizeNeo4jDegreeSample
} from './neo4jAnalytics';
import type { CypherValue } from './cypherValue';

describe('neo4jAnalytics', () => {
  it('generates strictly read-only Cypher queries with safe identifier escaping', () => {
    const queries = [
      buildLabelCountCypher('User`Admin'),
      buildRelationshipCountCypher('FOLLOWS'),
      buildNeo4jDegreeSampleCypher('Person', 1000, false),
      buildNeo4jDegreeSampleCypher('Person', 1000, true),
      buildInspectSupernodeCypher('4:abc:99', false),
      buildInspectSupernodeCypher('99', true),
      buildInspectLabelCypher('Person')
    ];
    for (const q of queries) {
      expect(cypherMayWrite(q)).toBe(false);
    }
    expect(queries[0]).toBe('MATCH (n:`User``Admin`) RETURN count(n) AS count');
    expect(queries[2]).toContain('COUNT { (n)--() }');
    expect(queries[3]).toContain('size((n)--())');
  });

  it('summarizes sampled node degree percentiles, isolated ratio, supernodes, and sparse properties', () => {
    const rows: CypherValue[][] = [
      [
        { kind: 'string', value: 'n1' },
        { kind: 'list', items: [{ kind: 'string', value: 'User' }] },
        { kind: 'integer', value: '0' },
        { kind: 'list', items: [{ kind: 'string', value: 'name' }] }
      ],
      [
        { kind: 'string', value: 'n2' },
        { kind: 'list', items: [{ kind: 'string', value: 'User' }] },
        { kind: 'integer', value: '4' },
        { kind: 'list', items: [{ kind: 'string', value: 'name' }, { kind: 'string', value: 'email' }] }
      ],
      [
        { kind: 'string', value: 'n3' },
        { kind: 'list', items: [{ kind: 'string', value: 'User' }, { kind: 'string', value: 'VIP' }] },
        { kind: 'integer', value: '250' },
        { kind: 'list', items: [{ kind: 'string', value: 'name' }] }
      ]
    ];

    const summary = summarizeNeo4jDegreeSample(rows);
    expect(summary.sampledNodes).toBe(3);
    expect(summary.isolatedCount).toBe(1);
    expect(summary.isolatedRatio).toBeCloseTo(1 / 3, 5);
    expect(summary.minDegree).toBe(0);
    expect(summary.p50Degree).toBe(4);
    expect(summary.maxDegree).toBe(250);
    expect(summary.supernodes[0]).toEqual({ id: 'n3', labels: ['User', 'VIP'], degree: 250 });
    expect(summary.propertyCoverage[0]).toMatchObject({ key: 'email', count: 1 });
    expect(summary.propertyCoverage[1]).toMatchObject({ key: 'name', count: 3, ratio: 1 });
  });

  it('adapts count items into ResultChartDialog table format', () => {
    expect(
      neo4jCountsToChartTable('label', [
        { name: 'User', count: 500 },
        { name: 'Order', count: 1200 }
      ])
    ).toEqual({
      columns: ['label', 'count'],
      rows: [
        ['User', 500],
        ['Order', 1200]
      ]
    });
  });
});
