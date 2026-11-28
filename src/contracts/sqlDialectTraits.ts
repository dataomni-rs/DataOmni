import { DatabaseType } from './connection';
import type { SqlDialect } from './queryExecution';

export type UnquotedIdentifierCase = 'preserve' | 'lower' | 'upper';
export type SqlFormatterLanguageName =
  | 'mysql'
  | 'postgresql'
  | 'sqlite'
  | 'transactsql'
  | 'plsql'
  | 'duckdb'
  | 'clickhouse';
export type SqlCompletionDialectFamily = 'mysql' | 'postgresql' | 'sqlite' | 'mssql' | 'plsql';
export type DefaultSchemaRule =
  | 'none'
  | 'public'
  | 'dbo'
  | 'main'
  | 'uppercase-username'
  | 'database-or-default';
export type EmptyInsertStyle =
  | 'empty-values'
  | 'explicit-default-column'
  | 'default-values'
  | 'unsupported';

export interface SqlDialectTraits {
  /** 无引号标识符在目录中的大小写折叠策略 */
  readonly unquotedIdentifierCase: UnquotedIdentifierCase;
  /** `sql-formatter` 语言名 */
  readonly formatterLanguage: SqlFormatterLanguageName;
  /** CodeMirror SQL 补全方言族 */
  readonly completionDialect: SqlCompletionDialectFamily;
  /** 无显式 schema 时的默认 schema 推断规则 */
  readonly defaultSchemaRule: DefaultSchemaRule;
  /** 是否使用 `IDENTIFIED BY <identifier>` 语法（口令不是字符串字面量，查询历史脱敏必须覆写） */
  readonly usesOracleIdentifiedByPassword: boolean;
  /** 全列取默认值时的 INSERT 语法形态 */
  readonly emptyInsertStyle: EmptyInsertStyle;
}

/**
 * 全部关系型 SQL 方言的声明式能力表（H10 / F2）。
 *
 * 利用 `Record<SqlDialect, SqlDialectTraits>` 强制 TypeScript 穷尽检查：
 * 未来新增任何 SQL 方言时编译器都会要求在一处填齐全部 6 项特性，
 * 避免散落在多处的 `dialect === 'oracle'` 条件分支漏改导致脱敏或编辑退化。
 */
export const SQL_DIALECT_TRAITS: Readonly<Record<SqlDialect, SqlDialectTraits>> = {
  mysql: {
    unquotedIdentifierCase: 'preserve',
    formatterLanguage: 'mysql',
    completionDialect: 'mysql',
    defaultSchemaRule: 'none',
    usesOracleIdentifiedByPassword: false,
    emptyInsertStyle: 'empty-values'
  },
  postgresql: {
    unquotedIdentifierCase: 'lower',
    formatterLanguage: 'postgresql',
    completionDialect: 'postgresql',
    defaultSchemaRule: 'public',
    usesOracleIdentifiedByPassword: false,
    emptyInsertStyle: 'default-values'
  },
  sqlite: {
    unquotedIdentifierCase: 'preserve',
    formatterLanguage: 'sqlite',
    completionDialect: 'sqlite',
    defaultSchemaRule: 'none',
    usesOracleIdentifiedByPassword: false,
    emptyInsertStyle: 'default-values'
  },
  sqlserver: {
    unquotedIdentifierCase: 'preserve',
    formatterLanguage: 'transactsql',
    completionDialect: 'mssql',
    defaultSchemaRule: 'dbo',
    usesOracleIdentifiedByPassword: false,
    emptyInsertStyle: 'default-values'
  },
  oracle: {
    unquotedIdentifierCase: 'upper',
    formatterLanguage: 'plsql',
    completionDialect: 'plsql',
    defaultSchemaRule: 'uppercase-username',
    usesOracleIdentifiedByPassword: true,
    emptyInsertStyle: 'explicit-default-column'
  },
  dameng: {
    unquotedIdentifierCase: 'upper',
    formatterLanguage: 'plsql',
    completionDialect: 'plsql',
    defaultSchemaRule: 'uppercase-username',
    usesOracleIdentifiedByPassword: true,
    emptyInsertStyle: 'explicit-default-column'
  },
  duckdb: {
    unquotedIdentifierCase: 'preserve',
    formatterLanguage: 'duckdb',
    completionDialect: 'postgresql',
    defaultSchemaRule: 'main',
    usesOracleIdentifiedByPassword: false,
    emptyInsertStyle: 'default-values'
  },
  clickhouse: {
    unquotedIdentifierCase: 'preserve',
    formatterLanguage: 'clickhouse',
    completionDialect: 'mysql',
    defaultSchemaRule: 'database-or-default',
    usesOracleIdentifiedByPassword: false,
    emptyInsertStyle: 'unsupported'
  }
};

/** 将 `DatabaseType` 映射到对应的 `SqlDialect`；非 SQL 引擎返回 `null`。 */
export function sqlDialectFromDatabaseType(dbType: DatabaseType): SqlDialect | null {
  switch (dbType) {
    case DatabaseType.MySQL:
      return 'mysql';
    case DatabaseType.PostgreSQL:
      return 'postgresql';
    case DatabaseType.SQLite:
      return 'sqlite';
    case DatabaseType.SqlServer:
      return 'sqlserver';
    case DatabaseType.Oracle:
      return 'oracle';
    case DatabaseType.Dameng:
      return 'dameng';
    case DatabaseType.DuckDB:
      return 'duckdb';
    case DatabaseType.ClickHouse:
      return 'clickhouse';
    default:
      return null;
  }
}
