import { describe, expect, it } from 'vitest';
import type { AnalyticsColumnMeta } from '../contracts/analytics';
import {
  buildDuplicateCheckTemplate,
  buildGroupByTemplate,
  buildPivotTemplate,
  defaultParquetFileName,
  localFileBasename
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
});
