import { describe, expect, it } from 'vitest';
import type { ColumnInfo } from '../contracts';
import { planBatchCellFill } from './batchCellFill';

const columns: ColumnInfo[] = [
  { name: 'id', data_type: 'bigint', is_nullable: false, is_primary_key: true },
  { name: 'status', data_type: 'varchar(32)', is_nullable: false, is_primary_key: false },
  { name: 'note', data_type: 'text', is_nullable: true, is_primary_key: false },
  { name: 'total', data_type: 'int', is_nullable: false, is_primary_key: false, is_generated: true }
];

const rows = [
  { id: 1, status: 'done', note: 'first', total: 10 },
  { id: 2, status: 'pending', note: 'second', total: 20 },
  { id: 3, status: 'pending', note: null, total: 30 }
];

const rowKeyFrom = (row: Record<string, unknown>) => ({
  columns: ['id'],
  values: { id: row.id as number }
});

describe('planBatchCellFill', () => {
  it('向下填充把首行值写入下方行并跳过主键与生成列', () => {
    const result = planBatchCellFill({
      mode: 'fill-down',
      selection: { anchor: { row: 0, column: 0 }, focus: { row: 2, column: 3 } },
      rows,
      columns,
      visibleIndexes: [0, 1, 2, 3],
      keyColumnSet: new Set(['id']),
      dialect: 'mysql',
      existingChanges: [],
      rowKeyFrom
    });

    expect(result.skippedReadOnlyColumns).toEqual(['id', 'total']);
    expect(result.nextChanges).toHaveLength(2);
    expect(result.stagedCellCount).toBe(4);
    expect(result.nextChanges[0].kind).toBe('update');
    if (result.nextChanges[0].kind === 'update') {
      expect(result.nextChanges[0].values).toEqual({
        status: { kind: 'value', value: 'done' },
        note: { kind: 'value', value: 'first' }
      });
    }
  });

  it('批量设为 NULL 自动跳过 NOT NULL 列与键列', () => {
    const result = planBatchCellFill({
      mode: 'set-null',
      selection: { anchor: { row: 0, column: 1 }, focus: { row: 1, column: 2 } },
      rows,
      columns,
      visibleIndexes: [0, 1, 2, 3],
      keyColumnSet: new Set(['id']),
      dialect: 'postgresql',
      existingChanges: [],
      rowKeyFrom
    });

    expect(result.skippedNotNullColumns).toEqual(['status']);
    expect(result.nextChanges).toHaveLength(2);
    if (result.nextChanges[0].kind === 'update') {
      expect(result.nextChanges[0].values).toEqual({ note: { kind: 'null' } });
    }
  });

  it('ClickHouse 遵守一次改一行的限制', () => {
    const result = planBatchCellFill({
      mode: 'fill-down',
      selection: { anchor: { row: 0, column: 1 }, focus: { row: 2, column: 1 } },
      rows,
      columns,
      visibleIndexes: [0, 1, 2, 3],
      keyColumnSet: new Set(['id']),
      dialect: 'clickhouse',
      existingChanges: [],
      rowKeyFrom
    });

    expect(result.nextChanges).toHaveLength(1);
    expect(result.blockedBySingleRowLimit).toBe(true);
  });
});
