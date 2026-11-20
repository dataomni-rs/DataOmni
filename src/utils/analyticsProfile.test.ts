import { describe, expect, it } from 'vitest';
import type { ColumnProfile } from '../contracts/analytics';
import {
  buildProfileFilterSql,
  buildProfileGroupBySql,
  buildProfileNullFilterSql,
  columnCardinalityHint,
  extractEsJsonDocs,
  filterAndSortProfiles,
  formatProfileNumber,
  formatProfileRatio,
  hasSchemaDrift,
  histogramHeights,
  mongoRowsToJsonDocs,
  nullSeverity
} from './analyticsProfile';

function makeColumn(overrides: Partial<ColumnProfile> & { name: string }): ColumnProfile {
  return {
    inferredType: 'text',
    duckdbType: 'VARCHAR',
    precisionDegraded: false,
    totalCount: 100,
    nullCount: 0,
    nullRatio: 0,
    distinctCount: 10,
    distinctRatio: 0.1,
    bins: [],
    topValues: [],
    ...overrides
  };
}

describe('analyticsProfile', () => {
  it('classifies null ratio severity thresholds', () => {
    expect(nullSeverity(0)).toBe('normal');
    expect(nullSeverity(0.049)).toBe('normal');
    expect(nullSeverity(0.05)).toBe('warn');
    expect(nullSeverity(0.299)).toBe('warn');
    expect(nullSeverity(0.3)).toBe('high');
  });

  it('formats ratios and numbers cleanly without hiding tiny non-zero rates', () => {
    expect(formatProfileRatio(0)).toBe('0%');
    expect(formatProfileRatio(1)).toBe('100%');
    expect(formatProfileRatio(0.0001)).toBe('<0.1%');
    expect(formatProfileRatio(0.125)).toBe('12.5%');
    expect(formatProfileNumber(undefined)).toBe('—');
    expect(formatProfileNumber(0)).toBe('0');
    expect(formatProfileNumber(12345)).toBe('12,345');
    expect(formatProfileNumber(3.1415926)).toBe('3.1416');
  });

  it('detects schema drift and sorts by nulls, distinct count, or drift', () => {
    const a = makeColumn({ name: 'a', nullRatio: 0.1, distinctCount: 50, presentRatio: 1 });
    const b = makeColumn({ name: 'b', nullRatio: 0.5, distinctCount: 5, presentRatio: 0.4 });
    const c = makeColumn({ name: 'c', nullRatio: 0.2, distinctCount: 90, presentRatio: 0.8 });

    expect(hasSchemaDrift(a)).toBe(false);
    expect(hasSchemaDrift(b)).toBe(true);

    expect(filterAndSortProfiles([a, b, c], '', 'nulls').map((col) => col.name)).toEqual(['b', 'c', 'a']);
    expect(filterAndSortProfiles([a, b, c], '', 'distinct').map((col) => col.name)).toEqual(['c', 'a', 'b']);
    expect(filterAndSortProfiles([a, b, c], '', 'drift').map((col) => col.name)).toEqual(['b', 'c', 'a']);
    expect(filterAndSortProfiles([a, b, c], 'b', 'order').map((col) => col.name)).toEqual(['b']);
  });

  it('normalizes histogram heights safely', () => {
    expect(
      histogramHeights([
        { lower: 0, upper: 10, count: 5 },
        { lower: 10, upper: 20, count: 20 },
        { lower: 20, upper: 30, count: 0 }
      ])
    ).toEqual([0.25, 1, 0]);
    expect(histogramHeights([{ lower: 0, upper: 1, count: 0 }])).toEqual([0]);
  });

  it('generates safely quoted DuckDB filter SQL for text, numbers, booleans, and nulls', () => {
    expect(buildProfileFilterSql('status', 'text', "O'Brien")).toBe(
      `SELECT * FROM result_set WHERE "status" = 'O''Brien'`
    );
    expect(buildProfileFilterSql('col"name', 'integer', '42')).toBe(
      `SELECT * FROM result_set WHERE "col""name" = 42`
    );
    expect(buildProfileFilterSql('active', 'boolean', 'true')).toBe(
      `SELECT * FROM result_set WHERE "active" = true`
    );
    expect(buildProfileNullFilterSql('email')).toBe(
      `SELECT * FROM result_set WHERE "email" IS NULL`
    );
    expect(buildProfileGroupBySql('user_id')).toContain('GROUP BY "user_id"');
    expect(buildProfileGroupBySql('user_id')).toContain('COUNT(*) AS row_count');
  });

  it('detects all-null, constant, and unique-key column cardinality hints', () => {
    expect(columnCardinalityHint(makeColumn({ name: 'x', totalCount: 6, nullCount: 6, distinctCount: 0 }))).toBe('all-null');
    expect(columnCardinalityHint(makeColumn({ name: 'x', totalCount: 6, nullCount: 0, distinctCount: 1 }))).toBe('constant');
    expect(columnCardinalityHint(makeColumn({ name: 'x', totalCount: 6, nullCount: 0, distinctCount: 6 }))).toBe('unique-key');
    expect(columnCardinalityHint(makeColumn({ name: 'x', totalCount: 6, nullCount: 1, distinctCount: 5 }))).toBeNull();
    expect(columnCardinalityHint(makeColumn({ name: 'x', totalCount: 1, nullCount: 0, distinctCount: 1 }))).toBeNull();
  });

  it('converts MongoDB rows preserving missing keys vs explicit nulls', () => {
    const docs = mongoRowsToJsonDocs([
      {
        id: '1',
        fields: [
          ['age', { kind: 'int', text: '30', truncated: false }],
          ['active', { kind: 'bool', text: 'true', truncated: false }],
          ['email', { kind: 'null', text: 'null', truncated: false }]
        ]
      },
      {
        id: '2',
        fields: [['age', { kind: 'int', text: '25', truncated: false }]]
      }
    ]);
    expect(docs).toEqual([
      { age: 30, active: true, email: null },
      { age: 25 }
    ]);
    expect('email' in docs[0]).toBe(true);
    expect('email' in docs[1]).toBe(false);
  });

  it('extracts Elasticsearch _source hits and ignores invalid JSON', () => {
    const body = JSON.stringify({
      hits: {
        hits: [
          { _id: '1', _source: { level: 'ERROR', ms: 120 } },
          { _id: '2', _source: { level: 'INFO' } }
        ]
      }
    });
    expect(extractEsJsonDocs(body)).toEqual([
      { level: 'ERROR', ms: 120 },
      { level: 'INFO' }
    ]);
    expect(extractEsJsonDocs('not json')).toEqual([]);
  });
});
