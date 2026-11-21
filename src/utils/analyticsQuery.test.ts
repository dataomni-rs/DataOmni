import { describe, expect, it } from 'vitest';
import type { AnalyticsColumnMeta } from '../contracts/analytics';
import {
  buildCorrelationMatrixSql,
  buildDuplicateCheckTemplate,
  buildExceptDiffTemplate,
  buildGroupByTemplate,
  buildLeftJoinTemplate,
  buildPivotTemplate,
  defaultParquetFileName,
  localFileBasename,
  MAX_CORRELATION_COLUMNS,
  nextAttachTableName,
  selectCorrelationColumns
} from './analyticsQuery';

const SAMPLE_COLUMNS: AnalyticsColumnMeta[] = [
  { name: 'region', duckdbType: 'VARCHAR', inferredType: 'text', precisionDegraded: false },
  { name: 'quarter', duckdbType: 'VARCHAR', inferredType: 'text', precisionDegraded: false },
  { name: 'sales"amt', duckdbType: 'DOUBLE', inferredType: 'numeric', precisionDegraded: false }
];

describe('analyticsQuery', () => {
  it('generates GROUP BY template using first categorical and numeric columns with safe quoting', () => {
    const sql = buildGroupByTemplate(SAMPLE_COLUMNS);
    expect(sql).toContain('SELECT "region"');
    expect(sql).toContain('SUM("sales""amt") AS total_value');
    expect(sql).toContain('GROUP BY "region"');
  });

  it('generates DuckDB PIVOT template across two dimensions and one numeric measure', () => {
    const sql = buildPivotTemplate(SAMPLE_COLUMNS);
    expect(sql).toContain('PIVOT result_set');
    expect(sql).toContain('ON "quarter"');
    expect(sql).toContain('USING SUM("sales""amt")');
    expect(sql).toContain('GROUP BY "region"');
  });

  it('generates duplicate and null check template', () => {
    const sql = buildDuplicateCheckTemplate(SAMPLE_COLUMNS);
    expect(sql).toContain('HAVING COUNT(*) > 1 OR "region" IS NULL');
  });

  it('extracts cross-platform basenames and builds default Parquet filenames', () => {
    expect(localFileBasename('/Users/hal/data/orders.parquet')).toBe('orders.parquet');
    expect(localFileBasename('C:\\Users\\hal\\data\\events.ndjson')).toBe('events.ndjson');
    expect(defaultParquetFileName('orders.csv')).toBe('orders.parquet');
    expect(defaultParquetFileName('sales report (2026)')).toBe('sales_report_2026.parquet');
  });

  it('allocates sequential ref table names and builds LEFT JOIN / EXCEPT reconciliation SQL using shared join keys', () => {
    expect(nextAttachTableName([])).toBe('ref_1');
    expect(nextAttachTableName(['ref_1', 'REF_2'])).toBe('ref_3');

    const refCols: AnalyticsColumnMeta[] = [
      { name: 'note', duckdbType: 'VARCHAR', inferredType: 'text', precisionDegraded: false },
      { name: 'QUARTER', duckdbType: 'VARCHAR', inferredType: 'text', precisionDegraded: false }
    ];
    const joinSql = buildLeftJoinTemplate(SAMPLE_COLUMNS, 'ref_1', refCols);
    expect(joinSql).toContain('LEFT JOIN "ref_1" b');
    expect(joinSql).toContain('ON a."quarter" = b."QUARTER"');

    const exceptSql = buildExceptDiffTemplate(SAMPLE_COLUMNS, 'ref_1', refCols);
    expect(exceptSql).toContain('SELECT "quarter"\nFROM result_set\nEXCEPT\nSELECT "QUARTER"\nFROM "ref_1"');
  });

  it('builds single-scan correlation matrix SQL for numeric columns capped at 12', () => {
    expect(
      buildCorrelationMatrixSql([
        { name: 'status', inferredType: 'text', distinctCount: 4 },
        { name: 'amount', inferredType: 'numeric', distinctCount: 50 },
        { name: 'const_num', inferredType: 'integer', distinctCount: 1 }
      ])
    ).toBeNull();

    const sql = buildCorrelationMatrixSql([
      { name: 'amount', inferredType: 'numeric', distinctCount: 100 },
      { name: 'discount', inferredType: 'numeric', distinctCount: 10 },
      { name: 'qty', inferredType: 'integer', distinctCount: 5 }
    ]);
    expect(sql).toBeTruthy();
    expect(sql).toContain('WITH _corr_agg AS');
    expect(sql).toContain('corr(CAST("amount" AS DOUBLE), CAST("discount" AS DOUBLE))');
    expect(sql).toContain("'amount' AS col_x, 'amount' AS col_y, 1.0 AS corr");
    expect(sql).toContain("'discount' AS col_x, 'amount' AS col_y, r_0_1 AS corr");

    const many = Array.from({ length: 18 }, (_, i) => ({
      name: `m_${i}`,
      inferredType: 'numeric' as const,
      distinctCount: 20
    }));
    expect(selectCorrelationColumns(many)).toHaveLength(MAX_CORRELATION_COLUMNS);
  });
});
