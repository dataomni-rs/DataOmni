import { describe, expect, it } from 'vitest';
import type { ColumnProfile } from '../contracts/analytics';
import {
  buildProfileEmptyStringFilterSql,
  buildProfileFilterSql,
  buildProfileGroupBySql,
  buildProfileNullFilterSql,
  buildProfilePaddedWhitespaceFilterSql,
  buildSourceWhereClause,
  columnCardinalityHint,
  extractEsJsonDocs,
  filterAndSortProfiles,
  formatProfileMarkdownReport,
  formatProfileNumber,
  formatProfileRatio,
  hasSchemaDrift,
  histogramHeights,
  mongoRowsToJsonDocs,
  nullSeverity,
  numericOutlierFilter,
  shouldCollapseTopValues,
  summarizeProfileFindings,
  timeSparklinePoints
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

  it('collapses top values only for unique / high-cardinality single-hit columns', () => {
    const uniqueId = makeColumn({
      name: 'user_id',
      totalCount: 6,
      nullCount: 0,
      distinctCount: 6,
      distinctRatio: 1,
      topValues: [{ value: 'u1', count: 1, ratio: 0.167 }]
    });
    const statusEnum = makeColumn({
      name: 'status',
      totalCount: 100,
      nullCount: 0,
      distinctCount: 3,
      distinctRatio: 0.03,
      topValues: [{ value: 'ok', count: 80, ratio: 0.8 }]
    });
    expect(shouldCollapseTopValues(uniqueId)).toBe(true);
    expect(shouldCollapseTopValues(statusEnum)).toBe(false);

    // 高基数时间列（20,000 行中偶发 2 条同秒碰撞，但已有 timeBins 迷你图）默认折叠 Top-5
    const timestampCol = makeColumn({
      name: 'created_at',
      inferredType: 'datetime',
      totalCount: 20000,
      nullCount: 0,
      distinctCount: 19972,
      distinctRatio: 0.9986,
      timeBins: [
        { label: '2026-01', count: 1500 },
        { label: '2026-06', count: 4200 }
      ],
      topValues: [
        { value: '2026-06-05 08:39:10', count: 2, ratio: 0.0001 }
      ]
    });
    expect(shouldCollapseTopValues(timestampCol)).toBe(true);
  });

  it('suggests numeric outlier filters for negative anomalies and >P95 tails', () => {
    const negativeAmount = makeColumn({
      name: 'amount',
      inferredType: 'numeric',
      totalCount: 50,
      min: '-12.5',
      max: '500',
      p25: 10,
      p95: 400
    });
    expect(numericOutlierFilter(negativeAmount)).toEqual({
      kind: 'negative',
      threshold: '0',
      operator: 'lt',
      filterValue: '0',
      sql: 'SELECT * FROM result_set WHERE "amount" < 0'
    });

    const highTail = makeColumn({
      name: 'latency_ms',
      inferredType: 'integer',
      totalCount: 100,
      min: '10',
      max: '9500',
      p25: 25,
      p95: 180
    });
    expect(numericOutlierFilter(highTail)).toEqual({
      kind: 'high',
      threshold: '180',
      operator: 'gt',
      filterValue: '180',
      sql: 'SELECT * FROM result_set WHERE "latency_ms" > 180 ORDER BY "latency_ms" DESC'
    });

    const flat = makeColumn({
      name: 'score',
      inferredType: 'integer',
      totalCount: 10,
      min: '5',
      max: '5',
      p25: 5,
      p95: 5
    });
    expect(numericOutlierFilter(flat)).toBeNull();

    // 100% 唯一自增主键 id（1..50）不误报高位异常
    const autoId = makeColumn({
      name: 'id',
      inferredType: 'integer',
      totalCount: 50,
      nullCount: 0,
      distinctCount: 50,
      distinctRatio: 1,
      min: '1',
      max: '50',
      p25: 13,
      p50: 25.5,
      p95: 47.55
    });
    expect(numericOutlierFilter(autoId)).toBeNull();
  });

  it('builds dialect-aware source WHERE clauses and sparkline coordinates', () => {
    expect(buildSourceWhereClause('status', 'text', "o'k", 'mysql')).toBe("`status` = 'o''k'");
    expect(buildSourceWhereClause('amount', 'numeric', '42.5', 'sqlserver')).toBe('[amount] = 42.5');
    expect(buildSourceWhereClause('deleted_at', 'datetime', null, 'postgresql')).toBe('"deleted_at" IS NULL');

    const pts = timeSparklinePoints([
      { label: '2026-11-01', count: 0 },
      { label: '2026-11-02', count: 10 },
      { label: '2026-11-03', count: 5 }
    ], 200, 32, 4, 4);
    expect(pts).toHaveLength(3);
    expect(pts[0].y).toBe(28);
    expect(pts[1].y).toBe(4);
    expect(pts[2].y).toBe(16);
  });

  it('summarizes profile health findings sorted by severity and builds whitespace filter SQL', () => {
    const findings = summarizeProfileFindings([
      makeColumn({
        name: 'status',
        inferredType: 'text',
        totalCount: 100,
        nullCount: 5,
        nullRatio: 0.05,
        emptyStringCount: 3,
        paddedWhitespaceCount: 2
      }),
      makeColumn({
        name: 'amount',
        inferredType: 'numeric',
        totalCount: 100,
        min: '-10',
        max: '500',
        p25: 20,
        p95: 300
      })
    ]);
    expect(findings[0]).toMatchObject({ column: 'amount', kind: 'negative', severity: 'high' });
    expect(findings.map((f) => f.kind)).toEqual([
      'negative',
      'high-nulls',
      'empty-strings',
      'padded-whitespace'
    ]);
    expect(buildProfileEmptyStringFilterSql('status')).toBe('SELECT * FROM result_set WHERE "status" = \'\'');
    expect(buildProfilePaddedWhitespaceFilterSql('status')).toContain('TRIM("status") <> "status"');
  });

  it('formats Markdown profile report with findings and strips raw sample values', () => {
    const md = formatProfileMarkdownReport({
      title: 'orders',
      scopeLabel: 'Sampled · first 5,000 rows',
      language: 'zh',
      profile: {
        rowCount: 5000,
        columnCount: 2,
        elapsedMs: 18,
        columns: [
          makeColumn({
            name: 'email|raw',
            inferredType: 'text',
            duckdbType: 'VARCHAR',
            totalCount: 5000,
            nullCount: 250,
            nullRatio: 0.05,
            minLength: 0,
            maxLength: 40,
            emptyStringCount: 4,
            topValues: [{ value: 'secret-user@example.com', count: 10, ratio: 0.002 }]
          }),
          makeColumn({
            name: 'amount',
            inferredType: 'numeric',
            duckdbType: 'DOUBLE',
            totalCount: 5000,
            min: '-12',
            max: '9999',
            p25: 30,
            p50: 100,
            p95: 500
          })
        ]
      }
    });
    expect(md).toContain('## 数据画像体检报告 · orders');
    expect(md).toContain('Sampled · first 5,000 rows');
    expect(md).toContain('`amount`: 存在负值 (< 0)');
    expect(md).toContain('`email\\|raw`');
    expect(md).toContain('空串 4');
    // 隐私保护：不泄露 topValues 里的具体样本值
    expect(md).not.toContain('secret-user@example.com');
  });
});
