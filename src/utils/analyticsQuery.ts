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
export const MAX_ATTACHED_TABLES = 4;

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

/** 生成下一个可用的附加对账表名（ref_1 .. ref_4） */
export function nextAttachTableName(existing: readonly string[]): string {
  const used = new Set(existing.map((name) => name.toLowerCase()));
  for (let i = 1; i <= MAX_ATTACHED_TABLES + 1; i += 1) {
    const candidate = `ref_${i}`;
    if (!used.has(candidate)) {
      return candidate;
    }
  }
  return `ref_${existing.length + 1}`;
}

function findJoinKeyPair(
  primaryColumns: readonly AnalyticsColumnMeta[],
  refColumns: readonly AnalyticsColumnMeta[]
): [string, string] | null {
  if (primaryColumns.length === 0 || refColumns.length === 0) {
    return null;
  }
  const refByLower = new Map(refColumns.map((col) => [col.name.toLowerCase(), col.name]));
  // 优先匹配同名列（常见如 id / user_id / order_id）
  for (const col of primaryColumns) {
    const matched = refByLower.get(col.name.toLowerCase());
    if (matched) {
      return [col.name, matched];
    }
  }
  return [primaryColumns[0].name, refColumns[0].name];
}

/** 对账模板 1：LEFT JOIN 关联补全 */
export function buildLeftJoinTemplate(
  primaryColumns: readonly AnalyticsColumnMeta[],
  refTableName: string,
  refColumns: readonly AnalyticsColumnMeta[]
): string {
  const pair = findJoinKeyPair(primaryColumns, refColumns);
  const refIdent = quoteDuckIdentifier(refTableName);
  if (!pair) {
    return `SELECT a.*, b.*\nFROM result_set a\nLEFT JOIN ${refIdent} b ON 1 = 0\nLIMIT 200`;
  }
  const [leftKey, rightKey] = pair;
  return [
    'SELECT a.*, b.*',
    'FROM result_set a',
    `LEFT JOIN ${refIdent} b`,
    `  ON a.${quoteDuckIdentifier(leftKey)} = b.${quoteDuckIdentifier(rightKey)}`,
    'LIMIT 200'
  ].join('\n');
}

/** 对账模板 2：EXCEPT 差异排查（存在于主结果但不在参考表中的键） */
export function buildExceptDiffTemplate(
  primaryColumns: readonly AnalyticsColumnMeta[],
  refTableName: string,
  refColumns: readonly AnalyticsColumnMeta[]
): string {
  const pair = findJoinKeyPair(primaryColumns, refColumns);
  const refIdent = quoteDuckIdentifier(refTableName);
  if (!pair) {
    return `SELECT * FROM result_set\nEXCEPT\nSELECT * FROM ${refIdent}\nLIMIT 200`;
  }
  const [leftKey, rightKey] = pair;
  return [
    `SELECT ${quoteDuckIdentifier(leftKey)}`,
    'FROM result_set',
    'EXCEPT',
    `SELECT ${quoteDuckIdentifier(rightKey)}`,
    `FROM ${refIdent}`,
    'LIMIT 200'
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
