import { describe, expect, it } from 'vitest';
import type { SerializedResultValue } from '../contracts/resultSet';
import {
  cellMatchesQuery,
  filterResultRows,
  findGridMatches,
  formatFilterPreview,
  upsertResultCellFilter,
  type ResultCellFilter
} from './gridSearch';

describe('gridSearch', () => {
  const rows: SerializedResultValue[][] = [
    [1, 'alice', 'active'],
    [2, 'Bob', 'paused'],
    [3, null, 'active']
  ];

  it('不区分大小写查找所有命中坐标并支持查找 NULL', () => {
    expect(findGridMatches(rows, 'ALI')).toEqual([{ row: 0, column: 1 }]);
    expect(findGridMatches(rows, 'active')).toEqual([
      { row: 0, column: 2 },
      { row: 2, column: 2 }
    ]);
    expect(findGridMatches(rows, 'null')).toEqual([{ row: 2, column: 1 }]);
    expect(findGridMatches(rows, '   ')).toEqual([]);
    expect(cellMatchesQuery(null, 'NULL')).toBe(true);
  });

  it('客户端结果集筛选支持关键词 + 多列值筛选且排除保留 NULL 行', () => {
    const eqFilter: ResultCellFilter = {
      id: 'f1',
      columnIndex: 2,
      columnName: 'status',
      operator: 'eq',
      value: 'active'
    };
    expect(filterResultRows(rows, { query: '', filterByQuery: true, cellFilters: [eqFilter] })).toEqual([
      [1, 'alice', 'active'],
      [3, null, 'active']
    ]);

    const neFilter: ResultCellFilter = {
      id: 'f2',
      columnIndex: 1,
      columnName: 'name',
      operator: 'ne',
      value: 'alice'
    };
    expect(filterResultRows(rows, { query: '', filterByQuery: true, cellFilters: [neFilter] })).toEqual([
      [2, 'Bob', 'paused'],
      [3, null, 'active']
    ]);

    expect(filterResultRows(rows, { query: 'bob', filterByQuery: true, cellFilters: [] })).toEqual([
      [2, 'Bob', 'paused']
    ]);
  });

  it('upsertResultCellFilter 对 eq 覆盖同列、对 ne 叠加并去重', () => {
    const first: ResultCellFilter = { id: '1', columnIndex: 2, columnName: 'status', operator: 'ne', value: 'paused' };
    const second: ResultCellFilter = { id: '2', columnIndex: 2, columnName: 'status', operator: 'ne', value: 'deleted' };
    const duplicate: ResultCellFilter = { id: '3', columnIndex: 2, columnName: 'status', operator: 'ne', value: 'paused' };
    const replace: ResultCellFilter = { id: '4', columnIndex: 2, columnName: 'status', operator: 'eq', value: 'active' };

    let list = upsertResultCellFilter([], first);
    list = upsertResultCellFilter(list, second);
    list = upsertResultCellFilter(list, duplicate);
    expect(list).toHaveLength(2);

    list = upsertResultCellFilter(list, replace);
    expect(list).toEqual([replace]);
  });

  it('formatFilterPreview 压缩空白并截断长文本', () => {
    expect(formatFilterPreview(null)).toBe('NULL');
    expect(formatFilterPreview('')).toBe('""');
    expect(formatFilterPreview('line1\n  line2')).toBe('line1 line2');
    expect(formatFilterPreview('abcdefghijklmnopqrstuvwxyz', 10)).toBe('abcdefghij…');
  });
});
