import type { SerializedResultValue } from '../contracts/resultSet';
import { selectionRect, type CellSelection } from './cellSelection';
import { compareDecimalStrings } from './resultSorting';
import { isTaggedResultValue } from './resultValues';

export interface SelectionSummary {
  cellCount: number;
  nonNullCount: number;
  distinctCount: number;
  numericCount: number;
  sum: string | null;
  avg: string | null;
  min: string | null;
  max: string | null;
}

interface ParsedDecimal {
  negative: boolean;
  digits: string;
  scale: number;
  normalized: string;
}

const DECIMAL_PATTERN = /^\s*([+-]?)(\d+)(?:\.(\d+))?\s*$/;

function plainNumberText(value: number): string | null {
  if (!Number.isFinite(value)) {
    return null;
  }
  const text = String(value);
  const match = /^(-?)(\d+)(?:\.(\d+))?e([+-]\d+)$/i.exec(text);
  if (!match) {
    return text;
  }
  const [, sign, integer, fraction = '', exponent] = match;
  const digits = integer + fraction;
  const point = integer.length + Number(exponent);
  if (point <= 0) {
    return `${sign}0.${'0'.repeat(-point)}${digits}`;
  }
  if (point >= digits.length) {
    return `${sign}${digits}${'0'.repeat(point - digits.length)}`;
  }
  return `${sign}${digits.slice(0, point)}.${digits.slice(point)}`;
}

function parseNumericValue(value: SerializedResultValue): ParsedDecimal | null {
  let text: string | null = null;
  if (typeof value === 'number') {
    text = plainNumberText(value);
  } else if (isTaggedResultValue(value) && (value.type === 'bigint' || value.type === 'decimal')) {
    text = value.value;
  }
  if (text === null) {
    return null;
  }

  const match = DECIMAL_PATTERN.exec(text);
  if (!match) {
    return null;
  }
  const negative = match[1] === '-';
  const integer = match[2].replace(/^0+(?=\d)/, '') || '0';
  const rawFraction = (match[3] ?? '').replace(/0+$/, '');
  const isZero = integer === '0' && rawFraction === '';
  const sign = negative && !isZero ? '-' : '';
  const normalized = rawFraction.length > 0 ? `${sign}${integer}.${rawFraction}` : `${sign}${integer}`;
  return {
    negative: negative && !isZero,
    digits: integer + rawFraction,
    scale: rawFraction.length,
    normalized
  };
}

function formatScaledBigInt(value: bigint, scale: number): string {
  const negative = value < 0n;
  const abs = negative ? -value : value;
  const digits = abs.toString();
  if (scale <= 0) {
    return `${negative ? '-' : ''}${digits}`;
  }
  const padded = digits.padStart(scale + 1, '0');
  const integer = padded.slice(0, padded.length - scale);
  const fraction = padded.slice(padded.length - scale).replace(/0+$/, '');
  const sign = negative ? '-' : '';
  return fraction.length > 0 ? `${sign}${integer}.${fraction}` : `${sign}${integer}`;
}

function formatAverage(sumScaled: bigint, scale: number, count: number): string {
  const extraDigits = 6;
  const divisor = BigInt(count);
  const negative = sumScaled < 0n;
  const absScaled = (negative ? -sumScaled : sumScaled) * 10n ** BigInt(extraDigits);
  // 半入法四舍五入，避免均值总向零截断
  const rounded = (absScaled + divisor / 2n) / divisor;
  return formatScaledBigInt(negative && rounded !== 0n ? -rounded : rounded, scale + extraDigits);
}

/**
 * 计算当前矩形选区的实时统计，模仿 Excel 状态栏的即时聚合：
 * - 单格不展示（避免每次点选都刷状态栏）
 * - 文本与 ID 列可立即看到计数、非空、去重数（一眼发现重复值）
 * - 含数值时用 BigInt 定点算术求和与均值，避免 0.1 + 0.2 浮点误差与 BIGINT 精度丢失
 */
export function summarizeSelection(
  rows: readonly (readonly SerializedResultValue[])[],
  selection: CellSelection | null
): SelectionSummary | null {
  if (!selection || rows.length === 0) {
    return null;
  }

  const rect = selectionRect(selection);
  const top = Math.max(0, rect.top);
  const bottom = Math.min(rows.length - 1, rect.bottom);
  if (top > bottom) {
    return null;
  }

  let cellCount = 0;
  let nonNullCount = 0;
  const distinctKeys = new Set<string>();
  const numerics: ParsedDecimal[] = [];

  for (let rowIndex = top; rowIndex <= bottom; rowIndex += 1) {
    const row = rows[rowIndex];
    if (!row) {
      continue;
    }
    const left = Math.max(0, rect.left);
    const right = Math.min(row.length - 1, rect.right);
    for (let colIndex = left; colIndex <= right; colIndex += 1) {
      cellCount += 1;
      const cell = row[colIndex] ?? null;
      if (cell === null) {
        continue;
      }
      nonNullCount += 1;
      distinctKeys.add(JSON.stringify(cell));
      const parsed = parseNumericValue(cell);
      if (parsed) {
        numerics.push(parsed);
      }
    }
  }

  if (cellCount < 2) {
    return null;
  }

  if (numerics.length === 0) {
    return {
      cellCount,
      nonNullCount,
      distinctCount: distinctKeys.size,
      numericCount: 0,
      sum: null,
      avg: null,
      min: null,
      max: null
    };
  }

  const maxScale = numerics.reduce((acc, item) => Math.max(acc, item.scale), 0);
  let sumScaled = 0n;
  let min = numerics[0].normalized;
  let max = numerics[0].normalized;

  for (const item of numerics) {
    const factor = 10n ** BigInt(maxScale - item.scale);
    const magnitude = BigInt(item.digits) * factor;
    sumScaled += item.negative ? -magnitude : magnitude;

    if (compareDecimalStrings(item.normalized, min) < 0) {
      min = item.normalized;
    }
    if (compareDecimalStrings(item.normalized, max) > 0) {
      max = item.normalized;
    }
  }

  return {
    cellCount,
    nonNullCount,
    distinctCount: distinctKeys.size,
    numericCount: numerics.length,
    sum: formatScaledBigInt(sumScaled, maxScale),
    avg: formatAverage(sumScaled, maxScale, numerics.length),
    min,
    max
  };
}
