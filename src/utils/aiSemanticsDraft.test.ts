import { describe, expect, it } from 'vitest';
import type { ErLink, ErTable } from './erLayout';
import {
  AI_INFERRED_MARKER,
  SEMANTICS_BATCH_SIZE,
  buildSemanticsDraftPrompt,
  chunkTablesForSemantics,
  parseSemanticsDraftReply,
  renderSemanticsMarkdown
} from './aiSemanticsDraft';

function makeTables(count: number): ErTable[] {
  return Array.from({ length: count }, (_, idx) => ({
    schema: 'public',
    name: `tbl_${idx + 1}`,
    columns: [
      { name: 'id', dataType: 'bigint', isPrimaryKey: true, isNullable: false },
      { name: 'status', dataType: 'smallint', isPrimaryKey: false, isNullable: false }
    ]
  }));
}

describe('aiSemanticsDraft', () => {
  it('splits large catalogs into batches of at most 20 tables', () => {
    const batches = chunkTablesForSemantics(makeTables(45));
    expect(batches.map((b) => b.length)).toEqual([SEMANTICS_BATCH_SIZE, SEMANTICS_BATCH_SIZE, 5]);
  });

  it('builds metadata-only prompts with PK/FK structure and zero row data', () => {
    const tables = makeTables(2);
    const links: ErLink[] = [
      {
        constraintName: 'fk_tbl2_tbl1',
        from: { table: 'public.tbl_2', column: 'id' },
        to: { table: 'public.tbl_1', column: 'id' }
      }
    ];
    const prompt = buildSemanticsDraftPrompt({
      dialect: 'PostgreSQL',
      tables,
      links,
      language: 'zh'
    });
    expect(prompt.user).toContain('public.tbl_1');
    expect(prompt.user).toContain('PK NOT NULL');
    expect(prompt.user).toContain('public.tbl_2.id -> public.tbl_1.id');
    expect(prompt.user).toContain('no row data');
  });

  it('parses fenced or bare JSON and renders CM4-compatible Markdown with AI-inferred marker', () => {
    const raw = '```json\n{"overview":"订单核心库","tables":[{"table":"orders","summary":"客户订单主表","columns":[{"name":"status","meaning":"订单状态码"}]}]}\n```';
    const parsed = parseSemanticsDraftReply(raw);
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;

    const md = renderSemanticsMarkdown([parsed.value], 'zh');
    expect(md.startsWith(AI_INFERRED_MARKER)).toBe(true);
    expect(md).toContain('[AI 推测]');
    expect(md).toContain('### orders');
    expect(md).toContain('- `status`: 订单状态码');

    expect(parseSemanticsDraftReply('not json').ok).toBe(false);
    expect(parseSemanticsDraftReply('{"overview":"x","tables":[]}').ok).toBe(false);
  });
});
