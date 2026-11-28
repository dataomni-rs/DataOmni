import { describe, expect, it } from 'vitest';
import type { CompletionRelation } from '../contracts/databaseMetadata';
import {
  MAX_AI_CONTEXT_TABLES,
  MAX_EXPLAIN_RAW_CHARS,
  buildErrorFixPrompt,
  buildExplainAnalysisPrompt,
  buildNlToSqlPrompt,
  defaultSelectedRelationKeys,
  extractReferencedRelationKeys,
  formatRelationSchemaSnippet,
  parseErrorFixReply,
  parseExplainAnalysisReply,
  parseNlToSqlReply,
  relationKey
} from './aiSqlAssist';

function makeRelations(count: number): CompletionRelation[] {
  return Array.from({ length: count }, (_, idx) => ({
    schema: 'public',
    name: `table_${idx + 1}`,
    kind: 'table',
    columns: [
      { name: 'id', dataType: 'bigint' },
      { name: 'status', dataType: 'varchar(32)' },
      { name: 'created_at', dataType: 'timestamp' }
    ]
  }));
}

describe('aiSqlAssist', () => {
  it('enforces PROD 0-table default and caps selected context at 15 tables (E1 budget check)', () => {
    const thirtyTables = makeRelations(30);
    // PROD 永远默认 0 张表
    expect(defaultSelectedRelationKeys('SELECT * FROM table_1', thirtyTables, 'production')).toEqual([]);

    // 非生产环境优先自动提取 SQL 涉及的表
    const devSelected = defaultSelectedRelationKeys(
      'SELECT * FROM table_2 JOIN table_9 ON table_2.id = table_9.id',
      thirtyTables,
      'development'
    );
    expect(devSelected).toEqual([relationKey(thirtyTables[1]), relationKey(thirtyTables[8])]);

    // 即使强行传入 30 张表，发包片段也严格截断到 15 张表
    const snippet = formatRelationSchemaSnippet(thirtyTables);
    expect(snippet.split('\n')).toHaveLength(MAX_AI_CONTEXT_TABLES);
    expect(snippet).toContain('public.table_15');
    expect(snippet).not.toContain('public.table_16');
  });

  it('extracts referenced tables and error detail table for error diagnosis without sending whole catalog (E2)', () => {
    const relations = makeRelations(25);
    const messages = buildErrorFixPrompt({
      dialect: 'PostgreSQL',
      sql: 'SELECT bad_col FROM table_3 WHERE id = 1',
      errorMessage: 'column "bad_col" does not exist',
      details: {
        message: 'column "bad_col" does not exist',
        code: '42703',
        table: 'table_7',
        position: 8
      },
      relations,
      language: 'zh'
    });
    expect(messages.user).toContain('public.table_3');
    expect(messages.user).toContain('public.table_7');
    expect(messages.user).not.toContain('public.table_1 ');
    expect(messages.user).toContain('Code: 42703');
  });

  it('truncates oversized EXPLAIN raw plans at 16 KB (E3)', () => {
    const hugePlan = 'Seq Scan on table_1\n'.repeat(2000);
    expect(hugePlan.length).toBeGreaterThan(MAX_EXPLAIN_RAW_CHARS);
    const messages = buildExplainAnalysisPrompt({
      dialect: 'PostgreSQL',
      sql: 'SELECT * FROM table_1',
      planRaw: hugePlan,
      worstEstimateSummary: 'Seq Scan on table_1: estimated 10, actual 100000',
      relations: makeRelations(3),
      language: 'en'
    });
    expect(messages.user).toContain('[truncated at 16384 chars]');
    expect(messages.user).toContain('Worst row-estimate divergence');
    expect(messages.user).toContain('public.table_1');
  });

  it('builds NL-to-SQL prompts with optional semantics and existing SQL without row data', () => {
    const relations = makeRelations(2);
    const prompt = buildNlToSqlPrompt({
      dialect: 'MySQL',
      requirement: '统计近 7 天按状态分组的订单数',
      existingSql: 'SELECT * FROM table_1',
      selectedRelations: relations,
      semanticsNotes: '### table_1\n- status: 1=paid',
      language: 'zh'
    });
    expect(prompt.system).toContain('MySQL');
    expect(prompt.user).toContain('Existing SQL to revise or extend');
    expect(prompt.user).toContain('Local business semantics notes');
    expect(prompt.user).toContain('status: 1=paid');
    expect(extractReferencedRelationKeys('SELECT 1', relations)).toEqual([]);
  });

  it('parses bare JSON, fenced JSON, prose-wrapped JSON, fixedSql: null, and rejects invalid replies', () => {
    // 裸 JSON
    expect(
      parseNlToSqlReply('{"sql":"SELECT count(*) FROM orders","explanation":"统计总数","warnings":[]}')
    ).toEqual({
      ok: true,
      value: { sql: 'SELECT count(*) FROM orders', explanation: '统计总数', warnings: [] }
    });

    // ```json 代码块包裹
    expect(
      parseErrorFixReply('```json\n{"cause":"权限不足","fixedSql":null,"notes":["请联系 DBA"]}\n```')
    ).toEqual({
      ok: true,
      value: { cause: '权限不足', fixedSql: null, notes: ['请联系 DBA'] }
    });

    // 前后带解释文字的 EXPLAIN 解读
    expect(
      parseExplainAnalysisReply(
        'Here is the analysis:\n{"summary":"Full scan on orders","bottlenecks":["Seq Scan on orders"],"suggestions":[{"title":"Add index","sql":"CREATE INDEX idx_orders_status ON orders(status);","rationale":"Filters by status"}]}\nDone.'
      )
    ).toEqual({
      ok: true,
      value: {
        summary: 'Full scan on orders',
        bottlenecks: ['Seq Scan on orders'],
        suggestions: [
          {
            title: 'Add index',
            sql: 'CREATE INDEX idx_orders_status ON orders(status);',
            rationale: 'Filters by status'
          }
        ]
      }
    });

    // 非 JSON / 缺必填字段
    expect(parseNlToSqlReply('not json at all')).toEqual({ ok: false, raw: 'not json at all' });
    expect(parseNlToSqlReply('{"sql":"   "}')).toEqual({ ok: false, raw: '{"sql":"   "}' });
    expect(parseErrorFixReply('{"fixedSql":"SELECT 1"}')).toEqual({
      ok: false,
      raw: '{"fixedSql":"SELECT 1"}'
    });
    expect(parseExplainAnalysisReply('{"bottlenecks":[]}')).toEqual({
      ok: false,
      raw: '{"bottlenecks":[]}'
    });
  });

  it('组装 SQL/报错/EXPLAIN 提示词前自动脱敏口令、连接串与 API Token（H15 / S8）', () => {
    const nl = buildNlToSqlPrompt({
      dialect: 'MySQL',
      requirement: 'Fix token Bearer secret_token_123456',
      existingSql: "CREATE USER app IDENTIFIED BY 'SuperSecret#99'",
      selectedRelations: [],
      language: 'zh'
    });
    expect(nl.user).not.toContain('SuperSecret#99');
    expect(nl.user).not.toContain('secret_token_123456');

    const fix = buildErrorFixPrompt({
      dialect: 'PostgreSQL',
      sql: "SELECT * FROM users WHERE password = 'hunter2'",
      errorMessage: 'connection failed for postgres://admin:dbpass99@prod.internal:5432/app',
      details: {
        message: 'duplicate key value violates unique constraint',
        detail: 'Key (api_key)=(sk-live1234567890abcd) already exists.'
      },
      relations: [],
      language: 'en'
    });
    expect(fix.user).not.toContain('hunter2');
    expect(fix.user).not.toContain('dbpass99');
    expect(fix.user).not.toContain('sk-live1234567890abcd');

    const explain = buildExplainAnalysisPrompt({
      dialect: 'PostgreSQL',
      sql: "SELECT * FROM accounts WHERE access_token = 'tok_xyz_secret'",
      planRaw: "Seq Scan on accounts (Filter: (password = 'planSecret77'))",
      relations: [],
      language: 'en'
    });
    expect(explain.user).not.toContain('tok_xyz_secret');
    expect(explain.user).not.toContain('planSecret77');
  });
});
