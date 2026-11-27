import type { SerializedResultValue } from '../contracts/resultSet';
import { formatResultValue } from './resultValues';

export interface GridCellMatch {
  row: number;
  column: number;
}

export type ResultCellFilterOperator = 'eq' | 'ne' | 'is-null' | 'is-not-null';

export interface ResultCellFilter {
  id: string;
  /** 按列下标定位，避免 `SELECT a.id, b.id` 这类重名列互相串列 */
  columnIndex: number;
  columnName: string;
  operator: ResultCellFilterOperator;
  value: string;
}

export function formatCellSearchText(value: SerializedResultValue): string {
  return value === null ? 'NULL' : formatResultValue(value);
}

export function cellMatchesQuery(value: SerializedResultValue, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (needle.length === 0) {
    return false;
  }
  return formatCellSearchText(value).toLowerCase().includes(needle);
}

/**
 * 扫描当前网格中的所有命中格（按行优先顺序），供高亮和上下跳转使用。
 */
export function findGridMatches(
  rows: readonly (readonly SerializedResultValue[])[],
  query: string
): GridCellMatch[] {
  const needle = query.trim().toLowerCase();
  if (needle.length === 0) {
    return [];
  }

  const matches: GridCellMatch[] = [];
  for (let row = 0; row < rows.length; row += 1) {
    const cells = rows[row];
    for (let column = 0; column < cells.length; column += 1) {
      if (formatCellSearchText(cells[column] ?? null).toLowerCase().includes(needle)) {
        matches.push({ row, column });
      }
    }
  }
  return matches;
}

/**
 * 右键菜单里展示当前单元格值的精简预览，换行压成空格并截断超长文本。
 */
export function formatFilterPreview(value: SerializedResultValue, maxLength = 24): string {
  if (value === null) {
    return 'NULL';
  }
  const singleLine = formatResultValue(value).replace(/\s+/g, ' ').trim();
  if (singleLine.length === 0) {
    return '""';
  }
  return singleLine.length > maxLength ? `${singleLine.slice(0, maxLength)}…` : singleLine;
}

/**
 * 客户端结果集快速筛选的合并规则：
 * - `eq` / `is-null` / `is-not-null` 覆盖同一列已有筛选（同一列同时 `= A AND = B` 恒为空集）
 * - `ne` 允许在同一列叠加多个排除值（像 Excel 取消勾选多个值），但去重完全相同的条目
 */
export function upsertResultCellFilter(
  existing: readonly ResultCellFilter[],
  next: ResultCellFilter
): ResultCellFilter[] {
  if (next.operator === 'ne') {
    const duplicate = existing.some(
      (item) => item.columnIndex === next.columnIndex
        && item.operator === 'ne'
        && item.value === next.value
    );
    return duplicate ? [...existing] : [...existing, next];
  }
  return [
    ...existing.filter((item) => item.columnIndex !== next.columnIndex),
    next
  ];
}

export function matchesResultCellFilter(
  cell: SerializedResultValue,
  filter: ResultCellFilter
): boolean {
  switch (filter.operator) {
    case 'is-null':
      return cell === null;
    case 'is-not-null':
      return cell !== null;
    case 'eq':
      return cell !== null && formatResultValue(cell) === filter.value;
    case 'ne':
      return cell === null || formatResultValue(cell) !== filter.value;
  }
}

export function filterResultRows(
  rows: readonly (readonly SerializedResultValue[])[],
  options: {
    query: string;
    filterByQuery: boolean;
    cellFilters: readonly ResultCellFilter[];
  }
): (readonly SerializedResultValue[])[] {
  const needle = options.filterByQuery ? options.query.trim().toLowerCase() : '';
  if (needle.length === 0 && options.cellFilters.length === 0) {
    return [...rows];
  }

  return rows.filter((row) => {
    for (const filter of options.cellFilters) {
      if (!matchesResultCellFilter(row[filter.columnIndex] ?? null, filter)) {
        return false;
      }
    }
    if (needle.length === 0) {
      return true;
    }
    return row.some((cell) => formatCellSearchText(cell ?? null).toLowerCase().includes(needle));
  });
}
