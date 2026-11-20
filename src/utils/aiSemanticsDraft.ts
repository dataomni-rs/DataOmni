import type { ErLink, ErTable } from './erLayout';
import { tableKey } from './erLayout';
import { extractJsonObject, type AiPromptMessages, type ParseResult } from './aiSqlAssist';

/**
 * 数据字典 AI 业务语义注释草拟（`rfcs/ai-sql-assist.md` AI2，与 `CM4` `semantics/<id>.md` 闭环）。
 *
 * - 按表分批（每批最多 20 张表），只发目录元数据（表名、列、类型、主外键），零数据行外发；
 * - 所有草拟输出固定带 `<!-- AI-inferred: review before trusting -->` 与 `[AI 推测]` / `[AI-inferred]` 标记；
 * - 生成结果使用 `### <table>` 分节，与后端 `connection_semantics::extract_table_section` 完全对齐。
 */

export const SEMANTICS_BATCH_SIZE = 20;
export const MAX_SEMANTICS_BYTES = 32 * 1024;
export const AI_INFERRED_MARKER = '<!-- AI-inferred: review before trusting -->';

export interface SemanticsColumnDraft {
  name: string;
  meaning: string;
}

export interface SemanticsTableDraft {
  table: string;
  summary: string;
  columns: SemanticsColumnDraft[];
}

export interface SemanticsBatchDraft {
  overview: string;
  tables: SemanticsTableDraft[];
}

export function chunkTablesForSemantics(
  tables: readonly ErTable[],
  batchSize = SEMANTICS_BATCH_SIZE
): ErTable[][] {
  const step = Math.max(1, batchSize);
  const batches: ErTable[][] = [];
  for (let i = 0; i < tables.length; i += step) {
    batches.push(tables.slice(i, i + step));
  }
  return batches;
}

export function buildSemanticsDraftPrompt(input: {
  dialect: string;
  tables: readonly ErTable[];
  links: readonly ErLink[];
  language: 'zh' | 'en';
}): AiPromptMessages {
  const langRule = input.language === 'zh'
    ? 'Write overview, table summaries, and column meanings in concise Simplified Chinese.'
    : 'Write overview, table summaries, and column meanings in concise English.';

  const system = [
    `You are a database documentation assistant for ${input.dialect}.`,
    'Draft concise business semantic descriptions for the listed tables and key non-obvious columns based strictly on schema metadata (names, types, primary keys, foreign keys).',
    'Mark uncertain inferences honestly and avoid inventing business rules that are not implied by column names.',
    langRule,
    'Return ONLY one valid JSON object with exact keys: {"overview": string, "tables": [{"table": string, "summary": string, "columns": [{"name": string, "meaning": string}]}]}.'
  ].join(' ');

  const batchKeys = new Set(input.tables.map((table) => tableKey(table)));
  const relevantLinks = input.links.filter(
    (link) => batchKeys.has(link.from.table) || batchKeys.has(link.to.table)
  );

  const tableLines = input.tables.map((table) => {
    const key = tableKey(table);
    const cols = table.columns
      .map((col) => {
        const flags = [col.isPrimaryKey ? 'PK' : '', col.isNullable ? 'NULL' : 'NOT NULL']
          .filter(Boolean)
          .join(' ');
        return `${col.name} ${col.dataType} [${flags}]`;
      })
      .join(', ');
    return `- ${key}: ${cols}`;
  });

  const sections = [
    `Dialect: ${input.dialect}`,
    `Tables (metadata only, no row data):\n${tableLines.join('\n')}`
  ];

  if (relevantLinks.length > 0) {
    const linkLines = relevantLinks.map(
      (link) => `- ${link.from.table}.${link.from.column} -> ${link.to.table}.${link.to.column}`
    );
    sections.push(`Foreign keys:\n${linkLines.join('\n')}`);
  }

  return { system, user: sections.join('\n\n') };
}

export function parseSemanticsDraftReply(raw: string): ParseResult<SemanticsBatchDraft> {
  const parsed = extractJsonObject(raw);
  if (!parsed || !Array.isArray(parsed.tables)) {
    return { ok: false, raw };
  }

  const tables: SemanticsTableDraft[] = parsed.tables
    .filter((item): item is Record<string, unknown> => Boolean(item) && typeof item === 'object' && !Array.isArray(item))
    .map((item) => {
      const table = typeof item.table === 'string' ? item.table.trim() : '';
      const summary = typeof item.summary === 'string' ? item.summary.trim() : '';
      const columns: SemanticsColumnDraft[] = Array.isArray(item.columns)
        ? item.columns
          .filter((col): col is Record<string, unknown> => Boolean(col) && typeof col === 'object' && !Array.isArray(col))
          .map((col) => ({
            name: typeof col.name === 'string' ? col.name.trim() : '',
            meaning: typeof col.meaning === 'string' ? col.meaning.trim() : ''
          }))
          .filter((col) => col.name !== '' && col.meaning !== '')
        : [];
      return { table, summary, columns };
    })
    .filter((item) => item.table !== '' && (item.summary !== '' || item.columns.length > 0));

  if (tables.length === 0) {
    return { ok: false, raw };
  }

  return {
    ok: true,
    value: {
      overview: typeof parsed.overview === 'string' ? parsed.overview.trim() : '',
      tables
    }
  };
}

/**
 * 将一批或多批 AI 草拟结果合成为标准 `semantics/<id>.md` 格式，固定带 `[AI 推测]` 标头与 `### <table>` 小节。
 */
export function renderSemanticsMarkdown(
  batches: readonly SemanticsBatchDraft[],
  language: 'zh' | 'en'
): string {
  const badge = language === 'zh' ? '[AI 推测]' : '[AI-inferred]';
  const disclaimer =
    language === 'zh'
      ? `> ${badge} 以下业务语义说明由 AI 根据表结构草拟，保存前请结合真实业务口径审阅修改。`
      : `> ${badge} Drafted by AI from schema metadata only. Review and edit before saving.`;

  const lines: string[] = [AI_INFERRED_MARKER, disclaimer];

  const overviews = batches.map((b) => b.overview.trim()).filter((text) => text !== '');
  if (overviews.length > 0) {
    lines.push('', overviews.join('\n\n'));
  }

  const seenTables = new Set<string>();
  for (const batch of batches) {
    for (const table of batch.tables) {
      const key = table.table.toLowerCase();
      if (seenTables.has(key)) {
        continue;
      }
      seenTables.add(key);
      lines.push('', `### ${table.table}`);
      if (table.summary) {
        lines.push(`- ${badge} ${table.summary}`);
      }
      for (const col of table.columns) {
        lines.push(`- \`${col.name}\`: ${col.meaning}`);
      }
    }
  }

  return `${lines.join('\n')}\n`;
}
