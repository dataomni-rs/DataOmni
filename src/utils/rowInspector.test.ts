import { describe, expect, it } from 'vitest';
import {
  buildRowInspection,
  filterRowInspection,
  formatPrettyJson,
  selectedTwoRows
} from './rowInspector';

describe('rowInspector', () => {
  it('把单行或双行转置为字段列表并准确标记差异列', () => {
    const columns = [
      { name: 'id', dataType: 'bigint' },
      { name: 'payload', dataType: 'json' },
      { name: 'status', dataType: 'varchar' }
    ];
    const rowA = [1, '{"a":1}', 'active'];
    const rowB = [2, '{"a":1}', 'paused'];

    const fields = buildRowInspection(columns, rowA, rowB);
    expect(fields.map((f) => [f.name, f.differs])).toEqual([
      ['id', true],
      ['payload', false],
      ['status', true]
    ]);
    expect(fields[1].prettyJsonA).toBe('{\n  "a": 1\n}');

    const onlyDiffs = filterRowInspection(fields, { query: '', onlyDifferences: true });
    expect(onlyDiffs.map((f) => f.name)).toEqual(['id', 'status']);

    const byName = filterRowInspection(fields, { query: 'pay', onlyDifferences: false });
    expect(byName.map((f) => f.name)).toEqual(['payload']);
  });

  it('formatPrettyJson 仅格式化对象或数组 JSON，普通文本返回 null', () => {
    expect(formatPrettyJson('{"k":1}')).toBe('{\n  "k": 1\n}');
    expect(formatPrettyJson({ type: 'json', value: '[1,2]' })).toBe('[\n  1,\n  2\n]');
    expect(formatPrettyJson('plain text')).toBeNull();
    expect(formatPrettyJson('{broken')).toBeNull();
    expect(formatPrettyJson(null)).toBeNull();
  });

  it('selectedTwoRows 仅在选区恰好跨越 2 行时返回行号对', () => {
    expect(selectedTwoRows(null)).toBeNull();
    expect(selectedTwoRows({ anchor: { row: 0, column: 0 }, focus: { row: 0, column: 2 } })).toBeNull();
    expect(selectedTwoRows({ anchor: { row: 3, column: 1 }, focus: { row: 2, column: 0 } })).toEqual([2, 3]);
    expect(selectedTwoRows({ anchor: { row: 0, column: 0 }, focus: { row: 2, column: 0 } })).toBeNull();
  });
});
