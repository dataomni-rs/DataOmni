import { describe, expect, it } from 'vitest';
import { summarizeSelection } from './selectionStats';
import type { SerializedResultValue } from '../contracts/resultSet';

describe('summarizeSelection', () => {
  it('单格或无选区返回 null', () => {
    const rows: SerializedResultValue[][] = [[10, 20], [30, 40]];
    expect(summarizeSelection(rows, null)).toBeNull();
    expect(summarizeSelection(rows, { anchor: { row: 0, column: 0 }, focus: { row: 0, column: 0 } })).toBeNull();
  });

  it('文本列返回计数、非空与去重数，数值指标为 null', () => {
    const rows: SerializedResultValue[][] = [
      ['alice'],
      ['bob'],
      ['alice'],
      [null]
    ];
    const summary = summarizeSelection(rows, {
      anchor: { row: 0, column: 0 },
      focus: { row: 3, column: 0 }
    });
    expect(summary).toEqual({
      cellCount: 4,
      nonNullCount: 3,
      distinctCount: 2,
      numericCount: 0,
      sum: null,
      avg: null,
      min: null,
      max: null
    });
  });

  it('精确计算十进制小数与超大 BIGINT，无浮点精度误差', () => {
    const rows: SerializedResultValue[][] = [
      [0.1],
      [0.2],
      [{ type: 'decimal', value: '0.30' }]
    ];
    const decimalSummary = summarizeSelection(rows, {
      anchor: { row: 0, column: 0 },
      focus: { row: 2, column: 0 }
    });
    expect(decimalSummary?.sum).toBe('0.6');
    expect(decimalSummary?.avg).toBe('0.2');
    expect(decimalSummary?.min).toBe('0.1');
    expect(decimalSummary?.max).toBe('0.3');

    const bigintRows: SerializedResultValue[][] = [
      [{ type: 'bigint', value: '65253938877693959' }],
      [{ type: 'bigint', value: '65253938877693961' }]
    ];
    const bigintSummary = summarizeSelection(bigintRows, {
      anchor: { row: 0, column: 0 },
      focus: { row: 1, column: 0 }
    });
    expect(bigintSummary?.sum).toBe('130507877755387920');
    expect(bigintSummary?.avg).toBe('65253938877693960');
    expect(bigintSummary?.min).toBe('65253938877693959');
    expect(bigintSummary?.max).toBe('65253938877693961');
  });

  it('支持负数、混合列与反向拖拽选区', () => {
    const rows: SerializedResultValue[][] = [
      ['a', -10],
      ['b', 20],
      ['c', null]
    ];
    const summary = summarizeSelection(rows, {
      anchor: { row: 2, column: 1 },
      focus: { row: 0, column: 0 }
    });
    expect(summary).toEqual({
      cellCount: 6,
      nonNullCount: 5,
      distinctCount: 5,
      numericCount: 2,
      sum: '10',
      avg: '5',
      min: '-10',
      max: '20'
    });
  });
});
