import type { ColumnInfo } from '../contracts';
import type { SerializedResultValue } from '../contracts/resultSet';
import { cellInputFromValue, type BoundValue, type CellInput } from './cellInput';
import { selectionRect, type CellSelection } from './cellSelection';
import { columnEditorKind } from './columnEditors';
import { canStageAnother, pendingForRow, rowIdOf, stageUpdate, type PendingChange } from './pendingChanges';
import { editStartValue } from './resultValues';
import type { RowKey } from './rowStatements';
import type { SqlIdentifierDialect } from './sqlIdentifiers';

export type BatchFillMode = 'fill-down' | 'set-null';

export interface BatchFillRequest {
  mode: BatchFillMode;
  selection: CellSelection;
  rows: readonly Record<string, unknown>[];
  columns: readonly ColumnInfo[];
  visibleIndexes: readonly number[];
  keyColumnSet: ReadonlySet<string>;
  dialect: SqlIdentifierDialect;
  existingChanges: readonly PendingChange[];
  rowKeyFrom: (row: Record<string, unknown>) => RowKey;
}

export interface BatchFillResult {
  nextChanges: PendingChange[];
  stagedCellCount: number;
  skippedReadOnlyColumns: string[];
  skippedNotNullColumns: string[];
  blockedBySingleRowLimit: boolean;
}

function originalRowValues(
  row: Record<string, unknown>,
  columns: readonly ColumnInfo[],
  dialect: SqlIdentifierDialect
): Record<string, BoundValue> {
  const byName = new Map(columns.map((column) => [column.name, column.data_type]));
  return Object.fromEntries(
    Object.entries(row).map(([name, value]) => [
      name,
      editStartValue(
        value as SerializedResultValue,
        columnEditorKind(byName.get(name) ?? '', dialect)
      )
    ])
  );
}

/**
 * 选区批量填充（向下填充 / 批量设 NULL），全部转化为标准的 `stageUpdate` 进暂存队列：
 * - 键列、生成列自动跳过（不破坏行定位语义）
 * - `set-null` 自动跳过 `NOT NULL` 列，避免提交时整批回滚
 * - 已排删除的行跳过；ClickHouse 仍受「一次改一行」门禁保护
 */
export function planBatchCellFill(request: BatchFillRequest): BatchFillResult {
  const rect = selectionRect(request.selection);
  const skippedReadOnlyColumns: string[] = [];
  const skippedNotNullColumns: string[] = [];
  const targetColumns: Array<{ visiblePos: number; column: ColumnInfo }> = [];

  for (let visiblePos = rect.left; visiblePos <= rect.right; visiblePos += 1) {
    const schemaIndex = request.visibleIndexes[visiblePos];
    const column = schemaIndex !== undefined ? request.columns[schemaIndex] : undefined;
    if (!column) {
      continue;
    }
    if (request.keyColumnSet.has(column.name) || column.is_generated) {
      skippedReadOnlyColumns.push(column.name);
      continue;
    }
    if (request.mode === 'set-null' && !column.is_nullable) {
      skippedNotNullColumns.push(column.name);
      continue;
    }
    targetColumns.push({ visiblePos, column });
  }

  let nextChanges = [...request.existingChanges];
  let stagedCellCount = 0;
  let blockedBySingleRowLimit = false;

  if (targetColumns.length === 0) {
    return {
      nextChanges,
      stagedCellCount,
      skippedReadOnlyColumns,
      skippedNotNullColumns,
      blockedBySingleRowLimit
    };
  }

  const topRow = request.rows[rect.top];
  if (request.mode === 'fill-down' && (!topRow || rect.bottom <= rect.top)) {
    return {
      nextChanges,
      stagedCellCount,
      skippedReadOnlyColumns,
      skippedNotNullColumns,
      blockedBySingleRowLimit
    };
  }

  const topRowValues = topRow ? originalRowValues(topRow, request.columns, request.dialect) : {};
  const startRow = request.mode === 'fill-down' ? rect.top + 1 : rect.top;

  for (let rowIndex = startRow; rowIndex <= rect.bottom; rowIndex += 1) {
    const row = request.rows[rowIndex];
    if (!row) {
      continue;
    }
    const key = request.rowKeyFrom(row);
    const rowId = rowIdOf(key);
    const existing = pendingForRow(nextChanges, rowId);
    if (existing?.kind === 'delete') {
      continue;
    }
    if (!canStageAnother(nextChanges, rowId, request.dialect)) {
      blockedBySingleRowLimit = true;
      break;
    }

    const original = originalRowValues(row, request.columns, request.dialect);
    const assignments: Record<string, CellInput> = {};

    for (const { column } of targetColumns) {
      if (request.mode === 'set-null') {
        assignments[column.name] = { kind: 'null' };
      } else {
        const sourceValue = topRowValues[column.name] ?? null;
        if (sourceValue === null && !column.is_nullable) {
          if (!skippedNotNullColumns.includes(column.name)) {
            skippedNotNullColumns.push(column.name);
          }
          continue;
        }
        assignments[column.name] = cellInputFromValue(sourceValue);
      }
    }

    if (Object.keys(assignments).length === 0) {
      continue;
    }

    const updated = stageUpdate(nextChanges, key, original, assignments);
    const nextPending = pendingForRow(updated, rowId);
    if (nextPending !== existing) {
      stagedCellCount += Object.keys(assignments).length;
    }
    nextChanges = updated;
  }

  return {
    nextChanges,
    stagedCellCount,
    skippedReadOnlyColumns,
    skippedNotNullColumns,
    blockedBySingleRowLimit
  };
}
