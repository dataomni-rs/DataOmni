import type { ConnectionEnvironment } from '../contracts';
import type { CompletionRelation } from '../contracts/databaseMetadata';
import type { QueryExecutionError } from '../contracts/queryExecution';

/**
 * SQL 编辑器三把克制型 AI 手术刀（`rfcs/ai-sql-assist.md` AI1）：
 * 1. 自然语言生成 / 改写 SQL（手动选表上限 15 张，PROD 默认 0 张）；
 * 2. 报错一键诊断与修复（只发报错结构 + 出错语句 + 涉及表列）；
 * 3. EXPLAIN 执行计划瓶颈解读（原文截断至 16 KB，建议只复制/回填，绝不执行）。
 *
 * 类型约束：**所有输入接口严格不接受任何表数据行（ResultRow）**，仅接受目录元数据与 SQL/计划文本。
 */

export const MAX_AI_CONTEXT_TABLES = 15;
export const MAX_EXPLAIN_RAW_CHARS = 16 * 1024;
export const MAX_SEMANTICS_PROMPT_CHARS = 4 * 1024;

export interface AiPromptMessages {
  system: string;
  user: string;
}

export interface NlToSqlPromptInput {
  dialect: string;
  requirement: string;
  existingSql?: string;
  selectedRelations: readonly CompletionRelation[];
  semanticsNotes?: string;
  language: 'zh' | 'en';
}

export interface NlToSqlDraft {
  sql: string;
  explanation: string;
  warnings: string[];
}

export interface ErrorFixPromptInput {
  dialect: string;
  sql: string;
  errorMessage: string;
  details?: QueryExecutionError;
  relations: readonly CompletionRelation[];
  language: 'zh' | 'en';
}

export interface ErrorFixDiagnosis {
  cause: string;
  fixedSql: string | null;
  notes: string[];
}

export interface ExplainAnalysisPromptInput {
  dialect: string;
  sql: string;
  planRaw: string;
  worstEstimateSummary?: string;
  relations: readonly CompletionRelation[];
  language: 'zh' | 'en';
}

export interface ExplainSuggestion {
  title: string;
  sql?: string;
  rationale: string;
}

export interface ExplainAnalysisResult {
  summary: string;
  bottlenecks: string[];
  suggestions: ExplainSuggestion[];
}

export type ParseResult<T> = { ok: true; value: T } | { ok: false; raw: string };

export function relationKey(relation: Pick<CompletionRelation, 'schema' | 'name'>): string {
  return `${relation.schema ?? ''}\u0000${relation.name}`;
}

export function relationDisplayName(relation: Pick<CompletionRelation, 'schema' | 'name'>): string {
  return relation.schema ? `${relation.schema}.${relation.name}` : relation.name;
}

/**
 * 从当前 SQL 中粗提取已出现的已知表/视图键（用于默认上下文预选，上限 `MAX_AI_CONTEXT_TABLES`）。
 */
export function extractReferencedRelationKeys(
  sql: string,
  relations: readonly CompletionRelation[],
  extraTableName?: string | null
): string[] {
  const lowered = sql.toLowerCase();
  const extraLower = extraTableName?.trim().toLowerCase() ?? '';
  const tokens = new Set(lowered.match(/[a-z_][a-z0-9_$]*/g) ?? []);
  const matched: string[] = [];

  for (const relation of relations) {
    const bare = relation.name.toLowerCase();
    const qualified = relationDisplayName(relation).toLowerCase();
    const hit =
      (extraLower !== '' && (extraLower === bare || extraLower === qualified))
      || tokens.has(bare)
      || (relation.schema ? lowered.includes(qualified) : false);
    if (hit) {
      matched.push(relationKey(relation));
      if (matched.length >= MAX_AI_CONTEXT_TABLES) {
        break;
      }
    }
  }
  return matched;
}

/**
 * 默认勾选给 AI 的上下文表：
 * - `production` 环境永远默认 0 张表（必须由用户显式勾选）；
 * - 非生产环境优先选 SQL 已引用的表，否则最多预选前 5 张以控制发包体量。
 */
export function defaultSelectedRelationKeys(
  sql: string,
  relations: readonly CompletionRelation[],
  environment: ConnectionEnvironment
): string[] {
  if (environment === 'production') {
    return [];
  }
  const referenced = extractReferencedRelationKeys(sql, relations);
  if (referenced.length > 0) {
    return referenced;
  }
  return relations.slice(0, Math.min(5, MAX_AI_CONTEXT_TABLES)).map(relationKey);
}

/**
 * 把选中的表目录压成精简 DDL 片段（最多 `MAX_AI_CONTEXT_TABLES` 张表，绝不含任何行数据）。
 */
export function formatRelationSchemaSnippet(relations: readonly CompletionRelation[]): string {
  const admitted = relations.slice(0, MAX_AI_CONTEXT_TABLES);
  if (admitted.length === 0) {
    return '(no tables selected)';
  }
  return admitted
    .map((relation) => {
      const cols = relation.columns
        .map((column) => (column.dataType ? `${column.name} ${column.dataType}` : column.name))
        .join(', ');
      return `${relation.kind.toUpperCase()} ${relationDisplayName(relation)} (${cols})`;
    })
    .join('\n');
}

function truncateText(text: string, maxChars: number): string {
  const trimmed = text.trim();
  if (trimmed.length <= maxChars) {
    return trimmed;
  }
  return `${trimmed.slice(0, maxChars)}\n... [truncated at ${maxChars} chars]`;
}

/** 构造「自然语言生成 / 改写 SQL」提示词 */
export function buildNlToSqlPrompt(input: NlToSqlPromptInput): AiPromptMessages {
  const langRule = input.language === 'zh'
    ? 'Write explanation and warnings in concise Simplified Chinese.'
    : 'Write explanation and warnings in concise English.';

  const system = [
    `You are a restrained SQL assistant for ${input.dialect}.`,
    'Generate ONE accurate SQL query matching the exact dialect and provided schema only.',
    'Do not invent tables or columns that are not listed unless explicitly asked.',
    langRule,
    'Return ONLY one valid JSON object with exact keys: {"sql": string, "explanation": string, "warnings": string[]}.',
    'No markdown prose outside the JSON object.'
  ].join(' ');

  const sections: string[] = [
    `Dialect: ${input.dialect}`,
    `Requirement:\n${input.requirement.trim()}`
  ];

  if (input.existingSql && input.existingSql.trim() !== '') {
    sections.push(`Existing SQL to revise or extend:\n${input.existingSql.trim()}`);
  }

  sections.push(`Selected schema (metadata only, no row data):\n${formatRelationSchemaSnippet(input.selectedRelations)}`);

  if (input.semanticsNotes && input.semanticsNotes.trim() !== '') {
    sections.push(
      `Local business semantics notes:\n${truncateText(input.semanticsNotes, MAX_SEMANTICS_PROMPT_CHARS)}`
    );
  }

  return { system, user: sections.join('\n\n') };
}

/** 构造「报错一键诊断与修复」提示词 */
export function buildErrorFixPrompt(input: ErrorFixPromptInput): AiPromptMessages {
  const langRule = input.language === 'zh'
    ? 'Write cause and notes in concise Simplified Chinese.'
    : 'Write cause and notes in concise English.';

  const system = [
    `You are a ${input.dialect} SQL error diagnostic assistant.`,
    'Analyze the database error and the failing SQL.',
    'If the error can be fixed by rewriting SQL, provide `fixedSql`.',
    'If it is a permission, connection, lock, or data-constraint conflict that SQL rewriting cannot safely fix, set `fixedSql` to null.',
    langRule,
    'Return ONLY one valid JSON object with exact keys: {"cause": string, "fixedSql": string | null, "notes": string[]}.'
  ].join(' ');

  const referencedKeys = new Set(
    extractReferencedRelationKeys(input.sql, input.relations, input.details?.table)
  );
  const relevantRelations = input.relations.filter((relation) =>
    referencedKeys.has(relationKey(relation))
  );

  const errorLines = [`Message: ${input.errorMessage}`];
  if (input.details?.code) errorLines.push(`Code: ${input.details.code}`);
  if (input.details?.position) errorLines.push(`Position: ${input.details.position}`);
  if (input.details?.detail) errorLines.push(`Detail: ${input.details.detail}`);
  if (input.details?.hint) errorLines.push(`Hint: ${input.details.hint}`);
  if (input.details?.constraint) errorLines.push(`Constraint: ${input.details.constraint}`);
  if (input.details?.table) errorLines.push(`Table: ${input.details.table}`);

  const sections = [
    `Dialect: ${input.dialect}`,
    `Database error:\n${errorLines.join('\n')}`,
    `Failing SQL:\n${input.sql.trim()}`
  ];

  if (relevantRelations.length > 0) {
    sections.push(`Referenced table metadata:\n${formatRelationSchemaSnippet(relevantRelations)}`);
  }

  return { system, user: sections.join('\n\n') };
}

/** 构造「EXPLAIN 执行计划瓶颈解读」提示词 */
export function buildExplainAnalysisPrompt(input: ExplainAnalysisPromptInput): AiPromptMessages {
  const langRule = input.language === 'zh'
    ? 'Write summary, bottlenecks, titles, and rationales in concise Simplified Chinese.'
    : 'Write summary, bottlenecks, titles, and rationales in concise English.';

  const system = [
    `You are a ${input.dialect} query execution plan reviewer.`,
    'Identify concrete bottlenecks citing actual plan operators, table names, and columns from the provided EXPLAIN plan.',
    'Do not invent generic advice unrelated to the plan.',
    'Optional candidate DDL (such as CREATE INDEX) may be put in suggestion.sql for human review only.',
    langRule,
    'Return ONLY one valid JSON object with exact keys: {"summary": string, "bottlenecks": string[], "suggestions": [{"title": string, "sql"?: string, "rationale": string}]}.'
  ].join(' ');

  const referencedKeys = new Set(extractReferencedRelationKeys(input.sql, input.relations));
  const relevantRelations = input.relations.filter((relation) =>
    referencedKeys.has(relationKey(relation))
  );

  const sections = [
    `Dialect: ${input.dialect}`,
    `SQL:\n${input.sql.trim()}`
  ];
  if (input.worstEstimateSummary) {
    sections.push(`Worst row-estimate divergence:\n${input.worstEstimateSummary}`);
  }
  sections.push(`EXPLAIN output:\n${truncateText(input.planRaw, MAX_EXPLAIN_RAW_CHARS)}`);
  if (relevantRelations.length > 0) {
    sections.push(`Referenced table metadata:\n${formatRelationSchemaSnippet(relevantRelations)}`);
  }

  return { system, user: sections.join('\n\n') };
}

/** 容忍裸 JSON、```json 代码块包裹或前后附带解释文字，提取第一个合法 JSON 对象 */
export function extractJsonObject(raw: string): Record<string, unknown> | null {
  const trimmed = raw.trim();
  const candidates: string[] = [trimmed];

  const fenceMatch = /```(?:json)?\s*([\s\S]*?)```/i.exec(trimmed);
  if (fenceMatch?.[1]) {
    candidates.unshift(fenceMatch[1].trim());
  }

  const firstBrace = trimmed.indexOf('{');
  const lastBrace = trimmed.lastIndexOf('}');
  if (firstBrace !== -1 && lastBrace > firstBrace) {
    candidates.push(trimmed.slice(firstBrace, lastBrace + 1));
  }

  for (const candidate of candidates) {
    try {
      const parsed = JSON.parse(candidate) as unknown;
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return parsed as Record<string, unknown>;
      }
    } catch {
      // 尝试下一个候选切片
    }
  }
  return null;
}

function stringList(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string => typeof item === 'string' && item.trim() !== '');
}

export function parseNlToSqlReply(raw: string): ParseResult<NlToSqlDraft> {
  const parsed = extractJsonObject(raw);
  if (!parsed || typeof parsed.sql !== 'string' || parsed.sql.trim() === '') {
    return { ok: false, raw };
  }
  return {
    ok: true,
    value: {
      sql: parsed.sql.trim(),
      explanation: typeof parsed.explanation === 'string' ? parsed.explanation.trim() : '',
      warnings: stringList(parsed.warnings)
    }
  };
}

export function parseErrorFixReply(raw: string): ParseResult<ErrorFixDiagnosis> {
  const parsed = extractJsonObject(raw);
  if (!parsed || typeof parsed.cause !== 'string' || parsed.cause.trim() === '') {
    return { ok: false, raw };
  }
  const fixedSql =
    typeof parsed.fixedSql === 'string' && parsed.fixedSql.trim() !== ''
      ? parsed.fixedSql.trim()
      : null;
  return {
    ok: true,
    value: {
      cause: parsed.cause.trim(),
      fixedSql,
      notes: stringList(parsed.notes)
    }
  };
}

export function parseExplainAnalysisReply(raw: string): ParseResult<ExplainAnalysisResult> {
  const parsed = extractJsonObject(raw);
  if (!parsed || typeof parsed.summary !== 'string' || parsed.summary.trim() === '') {
    return { ok: false, raw };
  }
  const suggestions: ExplainSuggestion[] = Array.isArray(parsed.suggestions)
    ? parsed.suggestions
      .filter((item): item is Record<string, unknown> => Boolean(item) && typeof item === 'object' && !Array.isArray(item))
      .map((item) => ({
        title: typeof item.title === 'string' ? item.title.trim() : '',
        sql: typeof item.sql === 'string' && item.sql.trim() !== '' ? item.sql.trim() : undefined,
        rationale: typeof item.rationale === 'string' ? item.rationale.trim() : ''
      }))
      .filter((item) => item.title !== '' || item.rationale !== '')
    : [];

  return {
    ok: true,
    value: {
      summary: parsed.summary.trim(),
      bottlenecks: stringList(parsed.bottlenecks),
      suggestions
    }
  };
}
