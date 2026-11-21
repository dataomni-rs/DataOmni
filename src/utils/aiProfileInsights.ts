import type { ColumnProfile } from '../contracts/analytics';
import { extractJsonObject, type AiPromptMessages, type ParseResult } from './aiSqlAssist';
import { formatProfileNumber, formatProfileRatio, summarizeProfileFindings } from './analyticsProfile';

/**
 * 基于脱敏统计摘要的 AI 画像体检解读（`rfcs/multi-engine-analytics.md` AN10）：
 * - 隐私硬门禁：只发送聚合统计指标（类型、空值率、唯一率、数值分位数、空串/空格计数、时间跨度），
 *   绝不发送任何原始行数据、绝不发送 `topValues` 样本值、绝不发送文本/JSON/二进制列的 `min`/`max` 样本值。
 * - 返回结构化中文/英文解读与只读建议 SQL（前端仅复制，绝不自动执行）。
 */

export const MAX_AI_PROFILE_COLUMNS = 40;

export type AiProfileInsightSeverity = 'high' | 'medium' | 'low';

export interface AiProfileInsightItem {
  readonly column?: string;
  readonly severity: AiProfileInsightSeverity;
  readonly title: string;
  readonly detail: string;
  readonly suggestedSql?: string;
}

export interface AiProfileInsights {
  readonly summary: string;
  readonly findings: readonly AiProfileInsightItem[];
}

function selectColumnsForAi(columns: readonly ColumnProfile[]): ColumnProfile[] {
  if (columns.length <= MAX_AI_PROFILE_COLUMNS) {
    return [...columns];
  }
  const flaggedNames = new Set(summarizeProfileFindings(columns).map((f) => f.column));
  const flagged = columns.filter((col) => flaggedNames.has(col.name));
  const rest = columns.filter((col) => !flaggedNames.has(col.name));
  return [...flagged, ...rest].slice(0, MAX_AI_PROFILE_COLUMNS);
}

/** 序列化单列脱敏聚合指标（严禁输出文本样本值与 topValues） */
function formatRedactedColumnLine(col: ColumnProfile): string {
  const parts: string[] = [
    `${col.name} (${col.duckdbType})`,
    `null=${formatProfileRatio(col.nullRatio)} (${col.nullCount}/${col.totalCount})`,
    `distinct=${col.distinctCount} (${formatProfileRatio(col.distinctRatio)})`
  ];
  if (col.inferredType === 'integer' || col.inferredType === 'numeric') {
    if (col.min !== undefined) {
      parts.push(
        `min=${col.min}`,
        `p25=${formatProfileNumber(col.p25)}`,
        `p50=${formatProfileNumber(col.p50)}`,
        `p75=${formatProfileNumber(col.p75)}`,
        `p95=${formatProfileNumber(col.p95)}`,
        `max=${col.max ?? 'n/a'}`,
        `mean=${formatProfileNumber(col.mean)}`,
        `stddev=${formatProfileNumber(col.stddev)}`
      );
    }
  } else if (
    col.inferredType === 'date'
    || col.inferredType === 'time'
    || col.inferredType === 'datetime'
  ) {
    if (col.min !== undefined) {
      parts.push(`range=${col.min}..${col.max ?? 'n/a'}`);
    }
  } else if (col.minLength !== undefined && col.maxLength !== undefined) {
    // 文本 / JSON / 二进制列只发长度区间与空串/空格计数，绝不发 min/max 真实值或 topValues
    parts.push(`len=${col.minLength}..${col.maxLength}`);
  }
  if ((col.emptyStringCount ?? 0) > 0) {
    parts.push(`empty_strings=${col.emptyStringCount}`);
  }
  if ((col.paddedWhitespaceCount ?? 0) > 0) {
    parts.push(`padded_whitespace=${col.paddedWhitespaceCount}`);
  }
  if (col.presentRatio !== undefined && col.presentRatio < 1) {
    parts.push(`present=${formatProfileRatio(col.presentRatio)}`);
  }
  return `- ${parts.join(', ')}`;
}

export function buildProfileInsightsPrompt(input: {
  title?: string;
  dialect?: string;
  scopeLabel?: string;
  profile: {
    readonly rowCount: number;
    readonly columnCount: number;
    readonly columns: readonly ColumnProfile[];
  };
  language: 'zh' | 'en';
}): AiPromptMessages {
  const langRule =
    input.language === 'zh'
      ? 'Write summary, finding titles, and details in concise Simplified Chinese.'
      : 'Write summary, finding titles, and details in concise English.';

  const system = [
    'You are a pragmatic database quality analyst.',
    'Analyze ONLY the provided redacted column profile statistics (no raw row data or sample values are included).',
    'Identify the highest-signal data quality risks, skewness, anomalies, or cleanup checks and provide optional read-only SELECT verification SQL.',
    'Never suggest mutating SQL (no UPDATE/DELETE/DROP/ALTER).',
    langRule,
    'Return ONLY one valid JSON object with exact keys: {"summary": string, "findings": [{"column": string | null, "severity": "high" | "medium" | "low", "title": string, "detail": string, "suggestedSql": string | null}]}.'
  ].join(' ');

  const selected = selectColumnsForAi(input.profile.columns);
  const findings = summarizeProfileFindings(input.profile.columns);
  const headerLines = [
    `Dataset: ${input.title ?? 'result_set'}`,
    ...(input.dialect ? [`Dialect: ${input.dialect}`] : []),
    ...(input.scopeLabel ? [`Scope: ${input.scopeLabel}`] : []),
    `Shape: ${input.profile.rowCount} rows x ${input.profile.columnCount} columns (showing ${selected.length} columns; aggregated metrics only, zero raw values)`
  ];

  const sections = [
    headerLines.join('\n'),
    `Column statistics:\n${selected.map(formatRedactedColumnLine).join('\n')}`
  ];

  if (findings.length > 0) {
    const flagLines = findings.slice(0, 15).map((f) => {
      const extra =
        f.threshold !== undefined
          ? ` (> ${f.threshold})`
          : f.ratio !== undefined
            ? ` (${formatProfileRatio(f.ratio)})`
            : f.count !== undefined
              ? ` (${f.count})`
              : '';
      return `- [${f.severity}] ${f.column}: ${f.kind}${extra}`;
    });
    sections.push(`Pre-detected flags:\n${flagLines.join('\n')}`);
  }

  return { system, user: sections.join('\n\n') };
}

function normalizeSeverity(value: unknown): AiProfileInsightSeverity {
  if (value === 'high' || value === 'medium' || value === 'low') {
    return value;
  }
  return 'medium';
}

export function parseProfileInsightsReply(raw: string): ParseResult<AiProfileInsights> {
  const parsed = extractJsonObject(raw);
  if (!parsed || typeof parsed.summary !== 'string') {
    return { ok: false, raw };
  }
  const rawFindings = Array.isArray(parsed.findings) ? parsed.findings : [];
  const findings: AiProfileInsightItem[] = [];
  for (const item of rawFindings) {
    if (item === null || typeof item !== 'object') {
      continue;
    }
    const rec = item as Record<string, unknown>;
    const title = typeof rec.title === 'string' ? rec.title.trim() : '';
    const detail = typeof rec.detail === 'string' ? rec.detail.trim() : '';
    if (!title && !detail) {
      continue;
    }
    const column =
      typeof rec.column === 'string' && rec.column.trim().length > 0
        ? rec.column.trim()
        : undefined;
    const suggestedSql =
      typeof rec.suggestedSql === 'string' && rec.suggestedSql.trim().length > 0
        ? rec.suggestedSql.trim()
        : undefined;
    findings.push({
      ...(column ? { column } : {}),
      severity: normalizeSeverity(rec.severity),
      title: title || column || 'Finding',
      detail,
      ...(suggestedSql ? { suggestedSql } : {})
    });
  }
  return {
    ok: true,
    value: {
      summary: parsed.summary.trim(),
      findings
    }
  };
}
