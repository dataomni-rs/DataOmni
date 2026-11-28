import { describe, expect, it } from 'vitest';
import { PLSQL } from '@codemirror/lang-sql';
import { DatabaseType } from './connection';
import { SQL_DIALECT_TRAITS, sqlDialectFromDatabaseType } from './sqlDialectTraits';
import { redactSqlForHistory } from '../utils/historyRedaction';
import { parseSingleTableSelect } from '../utils/resultEditability';
import { sqlFormatterLanguage } from '../utils/formatSql';
import { buildCompletionSchema, sqlDialectFor } from '../utils/sqlCompletionSchema';
import { buildInsertStatement } from '../utils/rowStatements';

describe('SQL_DIALECT_TRAITS 与 5 处达梦方言收口（H10 / F2）', () => {
  it('达梦 CREATE/ALTER USER IDENTIFIED BY 口令在历史脱敏中被掩码', () => {
    const out = redactSqlForHistory('CREATE USER demo IDENTIFIED BY "Dameng#Pass123"', 'dameng');
    expect(out.redacted).toBe(true);
    expect(out.sql).not.toContain('Dameng#Pass123');
    expect(out.sql).toContain("IDENTIFIED BY '***'");

    const unquoted = redactSqlForHistory('ALTER USER demo IDENTIFIED BY SecretPass1', 'dameng');
    expect(unquoted.redacted).toBe(true);
    expect(unquoted.sql).not.toContain('SecretPass1');
    expect(unquoted.sql).toContain("IDENTIFIED BY '***'");
  });

  it('达梦无引号单表 SELECT 折叠为大写表名，行内编辑可命中目录', () => {
    const parsed = parseSingleTableSelect('SELECT id, name FROM orders WHERE id = 1', 'dameng');
    expect(parsed).not.toBeNull();
    expect(parsed?.table).toBe('ORDERS');
    const quoted = parseSingleTableSelect('SELECT id FROM "mixed_Case"', 'dameng');
    expect(quoted?.table).toBe('mixed_Case');
  });

  it('达梦映射到 plsql 格式化语言与 CodeMirror PLSQL 补全方言，默认 Schema 折叠为大写用户名', () => {
    expect(sqlFormatterLanguage(DatabaseType.Dameng)).toBe('plsql');
    expect(sqlDialectFor(DatabaseType.Dameng)).toBe(PLSQL);

    const built = buildCompletionSchema(
      [{ schema: 'SYSDBA', name: 'ORDERS', kind: 'table', columns: [{ name: 'ID', dataType: 'INT' }] }],
      DatabaseType.Dameng,
      { schema: 'Schema', table: 'Table', view: 'View' },
      'sysdba'
    );
    expect(built.defaultSchema).toBe('SYSDBA');
  });

  it('达梦全默认值插入生成显式列 DEFAULT 语法', () => {
    const stmt = buildInsertStatement(
      {
        dialect: 'dameng',
        schema: 'SYSDBA',
        table: 'ORDERS',
        columns: [
          { name: 'ID', data_type: 'INT', is_nullable: false, is_primary_key: true }
        ]
      },
      { ID: { kind: 'default' } }
    );
    expect(stmt.sql).toBe('INSERT INTO "SYSDBA"."ORDERS" ("ID") VALUES (DEFAULT)');
    expect(stmt.params).toEqual([]);
  });

  it('所有支持 SQL 的 DatabaseType 均映射到 SQL_DIALECT_TRAITS 中已声明的方言', () => {
    for (const dbType of [
      DatabaseType.MySQL,
      DatabaseType.PostgreSQL,
      DatabaseType.SQLite,
      DatabaseType.SqlServer,
      DatabaseType.Oracle,
      DatabaseType.Dameng,
      DatabaseType.DuckDB,
      DatabaseType.ClickHouse
    ]) {
      const dialect = sqlDialectFromDatabaseType(dbType);
      expect(dialect, dbType).not.toBeNull();
      expect(SQL_DIALECT_TRAITS[dialect!]).toBeDefined();
    }
    expect(sqlDialectFromDatabaseType(DatabaseType.MongoDB)).toBeNull();
    expect(sqlDialectFromDatabaseType(DatabaseType.Redis)).toBeNull();
  });
});
