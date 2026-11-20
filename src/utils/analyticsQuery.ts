import type { AnalyticsColumnMeta } from '../contracts/analytics';

export const LOCAL_DATA_FILE_EXTENSIONS = [
  'parquet',
  'pq',
  'csv',
  'tsv',
  'json',
  'ndjson',
  'jsonl'
] as const;

export const ANALYTICS_QUERY_LIMIT_OPTIONS = [200, 1000, 10000] as const;

export function quoteDuckIdentifier(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

function isNumericMeta(col: AnalyticsColumnMeta): boolean {
  return col.inferredType === 'integer' || col.inferredType === 'numeric';
}

function isCategoricalMeta(col: AnalyticsColumnMeta): boolean {
  return (
    col.inferredType === 'text'
    || col.inferredType === 'boolean'
    || col.inferredType === 'date'
    || col.inferredType === 'datetime'
  );
}

/** 快捷模板 1：按维度列分组汇总（GROUP BY） */
export function buildGroupByTemplate(columns: readonly AnalyticsColumnMeta[]): string {
  if (columns.length === 0) {
    return 'SELECT COUNT(*) AS row_count\nFROM result_set';
  }
  const groupCol = columns.find(isCategoricalMeta) ?? columns[0];
  const metricCol = columns.find((col) => isNumericMeta(col) && col.name !== groupCol.name);
  const groupIdent = quoteDuckIdentifier(groupCol.name);

  if (metricCol) {
    const metricIdent = quoteDuckIdentifier(metricCol.name);
    return [
      `SELECT ${groupIdent},`,
      '  COUNT(*) AS row_count,',
      `  SUM(${metricIdent}) AS total_value,`,
      `  ROUND(AVG(${metricIdent}), 2) AS avg_value`,
      'FROM result_set',
      `GROUP BY ${groupIdent}`,
      'ORDER BY row_count DESC',
      'LIMIT 50'
    ].join('\n');
  }

  return [
    `SELECT ${groupIdent},`,
    '  COUNT(*) AS row_count',
    'FROM result_set',
    `GROUP BY ${groupIdent}`,
    'ORDER BY row_count DESC',
    'LIMIT 50'
  ].join('\n');
}

/** 快捷模板 2：DuckDB 原生交叉透视（PIVOT） */
export function buildPivotTemplate(columns: readonly AnalyticsColumnMeta[]): string {
  if (columns.length === 0) {
    return 'SELECT * FROM result_set LIMIT 100';
  }
  const categoricals = columns.filter(isCategoricalMeta);
  const rowCol = categoricals[0] ?? columns[0];
  const pivotCol =
    categoricals.find((col) => col.name !== rowCol.name)
    ?? columns.find((col) => col.name !== rowCol.name)
    ?? rowCol;
  const metricCol = columns.find(
    (col) => isNumericMeta(col) && col.name !== rowCol.name && col.name !== pivotCol.name
  );

  const usingExpr = metricCol
    ? `SUM(${quoteDuckIdentifier(metricCol.name)})`
    : 'COUNT(*)';

  if (pivotCol.name === rowCol.name) {
    return `PIVOT result_set\nON ${quoteDuckIdentifier(pivotCol.name)}\nUSING ${usingExpr}`;
  }

  return [
    'PIVOT result_set',
    `ON ${quoteDuckIdentifier(pivotCol.name)}`,
    `USING ${usingExpr}`,
    `GROUP BY ${quoteDuckIdentifier(rowCol.name)}`,
    `ORDER BY ${quoteDuckIdentifier(rowCol.name)}`
  ].join('\n');
}

/** 快捷模板 3：重复键 / 空值排查 */
export function buildDuplicateCheckTemplate(columns: readonly AnalyticsColumnMeta[]): string {
  if (columns.length === 0) {
    return 'SELECT * FROM result_set LIMIT 100';
  }
  const target = columns[0];
  const ident = quoteDuckIdentifier(target.name);
  return [
    `SELECT ${ident},`,
    '  COUNT(*) AS duplicate_count',
    'FROM result_set',
    `GROUP BY ${ident}`,
    `HAVING COUNT(*) > 1 OR ${ident} IS NULL`,
    'ORDER BY duplicate_count DESC',
    'LIMIT 100'
  ].join('\n');
}

/** 从跨平台文件路径提取文件名 */
export function localFileBasename(filePath: string): string {
  const normalized = filePath.replace(/\\/g, '/');
  const idx = normalized.lastIndexOf('/');
  return idx >= 0 ? normalized.slice(idx + 1) : normalized;
}

/** 构造导出 Parquet 的默认文件名 */
export function defaultParquetFileName(sourceName: string): string {
  const base = sourceName
    .replace(/\.(parquet|pq|csv|tsv|json|ndjson|jsonl)$/i, '')
    .replace(/[^\w.-]+/g, '_')
    .replace(/^_+|_+$/g, '');
  return `${base || 'result_set'}.parquet`;
}
