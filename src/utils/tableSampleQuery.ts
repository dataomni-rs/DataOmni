import type { ColumnInfo } from '../contracts';
import { unhandledDialect } from '../contracts/queryExecution';
import type { ColumnFilter } from './tableFilters';
import { buildFilterClause } from './tableFilters';
import type { SqlIdentifierDialect } from './sqlIdentifiers';
import { quoteQualifiedSqlIdentifier, quoteSqlIdentifier } from './sqlIdentifiers';
import { projectedColumn } from './tablePagination';

export const TABLE_SAMPLE_SIZE_OPTIONS = [1_000, 5_000, 20_000] as const;
export const DEFAULT_TABLE_SAMPLE_SIZE = 5_000;
export const MAX_TABLE_SAMPLE_SIZE = 20_000;

export interface TableSampleQueryRequest {
  readonly schema: string | null | undefined;
  readonly table: string;
  readonly columns: readonly ColumnInfo[];
  readonly visibleColumns: readonly string[];
  readonly filters: readonly ColumnFilter[];
  readonly limit: number;
  readonly dialect: SqlIdentifierDialect;
}

/**
 * 关系库表级抽样 SQL（AN0-E1 拍板）：
 * - 严禁 `ORDER BY RAND()` / `random()`，且默认不拼 `ORDER BY`，避免在大表上触发全表排序；
 * - 继承当前 `TableFilterBar` 的 `WHERE` 筛选与可见列投影（含不可读类型序列化）；
 * - 按 7 类关系方言生成对应的限流子句（MySQL/PG/SQLite/DuckDB/ClickHouse 用 `LIMIT`，
 *   SQL Server 用 `SELECT TOP (N)`，Oracle 用 `FETCH FIRST N ROWS ONLY`）。
 */
export function buildTableSampleQuery(request: TableSampleQueryRequest): string | null {
  const safeLimit = Math.min(MAX_TABLE_SAMPLE_SIZE, Math.max(1, Math.trunc(request.limit)));
  const projection = request.visibleColumns
    .filter((name) => name.length > 0)
    .map((name) => {
      const column = request.columns.find((candidate) => candidate.name === name);
      return column ? projectedColumn(column, request.dialect) : quoteSqlIdentifier(name, request.dialect);
    });
  if (projection.length === 0) {
    return null;
  }

  const tableReference = quoteQualifiedSqlIdentifier(
    request.schema ? [request.schema, request.table] : [request.table],
    request.dialect
  );
  const whereClause = buildFilterClause(request.filters, request.columns, request.dialect);
  const selectCols = projection.join(', ');

  switch (request.dialect) {
    case 'sqlserver':
      return [`SELECT TOP (${safeLimit}) ${selectCols}`, `FROM ${tableReference}`, whereClause]
        .filter((part) => part.trim().length > 0)
        .join(' ');
    case 'oracle':
      return [
        `SELECT ${selectCols}`,
        `FROM ${tableReference}`,
        whereClause,
        `FETCH FIRST ${safeLimit} ROWS ONLY`
      ]
        .filter((part) => part.trim().length > 0)
        .join(' ');
    case 'mysql':
    case 'postgresql':
    case 'sqlite':
    case 'duckdb':
    case 'clickhouse':
    case 'dameng':
      return [`SELECT ${selectCols}`, `FROM ${tableReference}`, whereClause, `LIMIT ${safeLimit}`]
        .filter((part) => part.trim().length > 0)
        .join(' ');
    default:
      return unhandledDialect(request.dialect);
  }
}
