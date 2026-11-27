import type { SerializedResultValue } from '../contracts/resultSet';
import { selectionRect, type CellSelection } from './cellSelection';
import { formatResultValue, isTaggedResultValue } from './resultValues';

export interface RowInspectorColumn {
  name: string;
  dataType?: string | null;
}

export interface RowInspectorField {
  index: number;
  name: string;
  dataType: string | null;
  valueA: SerializedResultValue;
  valueB?: SerializedResultValue;
  formattedA: string;
  formattedB?: string;
  prettyJsonA: string | null;
  prettyJsonB: string | null;
  differs: boolean;
}

export function formatPrettyJson(value: SerializedResultValue): string | null {
  if (value === null) {
    return null;
  }
  if (typeof value === 'object' && !isTaggedResultValue(value)) {
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return null;
    }
  }
  const raw = isTaggedResultValue(value)
    ? (value.type === 'json' ? value.value : null)
    : (typeof value === 'string' ? value : null);
  if (!raw) {
    return null;
  }
  const trimmed = raw.trim();
  if (!(trimmed.startsWith('{') && trimmed.endsWith('}')) && !(trimmed.startsWith('[') && trimmed.endsWith(']'))) {
    return null;
  }
  try {
    const parsed: unknown = JSON.parse(trimmed);
    if (parsed !== null && typeof parsed === 'object') {
      return JSON.stringify(parsed, null, 2);
    }
    return null;
  } catch {
    return null;
  }
}

function canonicalFieldValue(value: SerializedResultValue): string {
  return JSON.stringify(value ?? null);
}

/**
 * 把一行（或两行对比）转置为「列名 / 类型 / 值」清单，方便在宽表上垂直查阅或逐列对比差异。
 */
export function buildRowInspection(
  columns: readonly RowInspectorColumn[],
  rowA: readonly SerializedResultValue[],
  rowB?: readonly SerializedResultValue[]
): RowInspectorField[] {
  return columns.map((column, index) => {
    const valueA = rowA[index] ?? null;
    const valueB = rowB ? (rowB[index] ?? null) : undefined;
    const formattedA = valueA === null ? 'NULL' : formatResultValue(valueA);
    const formattedB = rowB ? (valueB === null ? 'NULL' : formatResultValue(valueB ?? null)) : undefined;
    const differs = rowB
      ? canonicalFieldValue(valueA) !== canonicalFieldValue(valueB ?? null)
      : false;
    return {
      index,
      name: column.name,
      dataType: column.dataType ?? null,
      valueA,
      valueB,
      formattedA,
      formattedB,
      prettyJsonA: formatPrettyJson(valueA),
      prettyJsonB: rowB ? formatPrettyJson(valueB ?? null) : null,
      differs
    };
  });
}

export function filterRowInspection(
  fields: readonly RowInspectorField[],
  options: { query: string; onlyDifferences: boolean }
): RowInspectorField[] {
  const needle = options.query.trim().toLowerCase();
  return fields.filter((field) => {
    if (options.onlyDifferences && !field.differs) {
      return false;
    }
    if (needle.length === 0) {
      return true;
    }
    return field.name.toLowerCase().includes(needle)
      || field.formattedA.toLowerCase().includes(needle)
      || (field.formattedB?.toLowerCase().includes(needle) ?? false);
  });
}

/**
 * 选区恰好跨越相邻 2 行时返回这两行下标，用于右键提供「对比选中的 2 行」。
 */
export function selectedTwoRows(selection: CellSelection | null): [number, number] | null {
  if (!selection) {
    return null;
  }
  const rect = selectionRect(selection);
  return rect.bottom - rect.top === 1 ? [rect.top, rect.bottom] : null;
}
