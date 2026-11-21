import { describe, expect, it } from 'vitest';
import type { ColumnInfo } from '../contracts';
import type { SqlIdentifierDialect } from './sqlIdentifiers';
import {
  buildTableSampleQuery,
  DEFAULT_TABLE_SAMPLE_SIZE,
  MAX_TABLE_SAMPLE_SIZE
} from './tableSampleQuery';

const COLUMNS: ColumnInfo[] = [
  { name: 'id', data_type: 'integer', is_nullable: false, is_primary_key: true },
  { name: 'status', data_type: 'text', is_nullable: false, is_primary_key: false },
  { name: 'secret', data_type: 'text', is_nullable: true, is_primary_key: false }
];

describe('buildTableSampleQuery', () => {
  it.each<[SqlIdentifierDialect, string]>([
    ['mysql', 'SELECT `id`, `status` FROM `orders` LIMIT 5000'],
    ['postgresql', 'SELECT "id", "status" FROM "public"."orders" LIMIT 5000'],
    ['sqlite', 'SELECT "id", "status" FROM "orders" LIMIT 5000'],
    ['duckdb', 'SELECT "id", "status" FROM "main"."orders" LIMIT 5000'],
    ['clickhouse', 'SELECT `id`, `status` FROM `analytics`.`orders` LIMIT 5000'],
    ['sqlserver', 'SELECT TOP (5000) [id], [status] FROM [dbo].[orders]'],
    ['oracle', 'SELECT "id", "status" FROM "HR"."orders" FETCH FIRST 5000 ROWS ONLY']
  ])('builds no-sort bounded sample SQL for %s', (dialect, expected) => {
    const schemaByDialect: Partial<Record<SqlIdentifierDialect, string>> = {
      postgresql: 'public',
      duckdb: 'main',
      clickhouse: 'analytics',
      sqlserver: 'dbo',
      oracle: 'HR'
    };
    const sql = buildTableSampleQuery({
      schema: schemaByDialect[dialect] ?? null,
      table: 'orders',
      columns: COLUMNS,
      visibleColumns: ['id', 'status'],
      filters: [],
      limit: DEFAULT_TABLE_SAMPLE_SIZE,
      dialect
    });
    expect(sql).toBe(expected);
    expect(sql?.toUpperCase()).not.toContain('ORDER BY');
    expect(sql?.toUpperCase()).not.toContain('RAND');
  });

  it('inherits active filters and omits hidden columns', () => {
    const sql = buildTableSampleQuery({
      schema: 'public',
      table: 'orders',
      columns: COLUMNS,
      visibleColumns: ['id', 'status'],
      filters: [{ id: 'f1', column: 'status', operator: 'eq', value: 'paid' }],
      limit: 1000,
      dialect: 'postgresql'
    });
    expect(sql).toBe(
      `SELECT "id", "status" FROM "public"."orders" WHERE "status" = 'paid' LIMIT 1000`
    );
    expect(sql).not.toContain('secret');
  });

  it('clamps limit to [1, MAX_TABLE_SAMPLE_SIZE] and returns null when no visible columns', () => {
    const clamped = buildTableSampleQuery({
      schema: null,
      table: 'orders',
      columns: COLUMNS,
      visibleColumns: ['id'],
      filters: [],
      limit: 999_999,
      dialect: 'sqlite'
    });
    expect(clamped).toBe(`SELECT "id" FROM "orders" LIMIT ${MAX_TABLE_SAMPLE_SIZE}`);

    const empty = buildTableSampleQuery({
      schema: null,
      table: 'orders',
      columns: COLUMNS,
      visibleColumns: [],
      filters: [],
      limit: 1000,
      dialect: 'sqlite'
    });
    expect(empty).toBeNull();
  });
});
