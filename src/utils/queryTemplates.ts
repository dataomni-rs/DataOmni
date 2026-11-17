/**
 * 本地参数化查询模板持久化与筛选（rfcs/query-templates.md）。
 *
 * 参数语法直接复用 D2（`:name` 绑定参数），不发明模板宏；落盘遵守 R1 前后向兼容与
 * 5MB localStorage 同源配额保护。
 */

import { DatabaseType } from '../contracts/connection';
import { speaksSql, SUPPORTED_DATABASE_TYPES } from '../contracts/databaseSupport';
import { dialectSupportsParameters, parameterNames } from './queryParameters';
import { identifierDialectFor } from './sqlIdentifiers';
import { splitSqlStatements } from './sqlStatements';

export const QUERY_TEMPLATES_STORAGE_KEY = 'dataomni.query-templates';
const SNAPSHOT_VERSION = 1;

export const MAX_QUERY_TEMPLATES = 200;
export const MAX_QUERY_TEMPLATE_STORAGE_CHARS = 512_000;

export const SQL_TEMPLATE_DB_TYPES: readonly DatabaseType[] = [
  ...SUPPORTED_DATABASE_TYPES,
].filter((type) => speaksSql(type));

const KNOWN_SQL_DB_TYPES: ReadonlySet<string> = new Set(SQL_TEMPLATE_DB_TYPES);

let newerTemplatesOnDisk = false;

export interface QueryTemplate {
  id: string;
  title: string;
  sql: string;
  /** 限定适用的关系库类型；缺省表示通用 SQL 模板 */
  dbType?: DatabaseType;
  updatedAt: string;
}

export interface QueryTemplateDraft {
  id?: string;
  title: string;
  sql: string;
  dbType?: DatabaseType;
}

export type QueryTemplateScope = 'compatible' | 'all';

interface QueryTemplatesSnapshot {
  version: number;
  templates: unknown[];
}

export function resetQueryTemplatesDiskGuardForTest(): void {
  newerTemplatesOnDisk = false;
}

function normalizeTemplateEntry(raw: unknown): QueryTemplate | null {
  if (typeof raw !== 'object' || raw === null) {
    return null;
  }
  const candidate = raw as Record<string, unknown>;
  if (
    typeof candidate.id !== 'string' ||
    !candidate.id.trim() ||
    typeof candidate.title !== 'string' ||
    !candidate.title.trim() ||
    typeof candidate.sql !== 'string' ||
    !candidate.sql.trim() ||
    typeof candidate.updatedAt !== 'string'
  ) {
    return null;
  }

  // 未来版本或未知 dbType 降级为通用模板，绝不丢弃用户保存的 SQL 正文
  const dbType =
    typeof candidate.dbType === 'string' && KNOWN_SQL_DB_TYPES.has(candidate.dbType)
      ? (candidate.dbType as DatabaseType)
      : undefined;

  return {
    id: candidate.id,
    title: candidate.title,
    sql: candidate.sql,
    ...(dbType ? { dbType } : {}),
    updatedAt: candidate.updatedAt,
  };
}

function sortNewestFirst(templates: readonly QueryTemplate[]): QueryTemplate[] {
  return [...templates].sort((left, right) => {
    const leftTime = Date.parse(left.updatedAt);
    const rightTime = Date.parse(right.updatedAt);
    if (Number.isNaN(leftTime) || Number.isNaN(rightTime)) {
      return 0;
    }
    return rightTime - leftTime;
  });
}

export function loadQueryTemplates(): QueryTemplate[] {
  let stored: unknown;
  try {
    const raw = localStorage.getItem(QUERY_TEMPLATES_STORAGE_KEY);
    stored = raw ? JSON.parse(raw) : null;
  } catch {
    return [];
  }

  if (typeof stored !== 'object' || stored === null) {
    return [];
  }

  const snapshot = stored as Partial<QueryTemplatesSnapshot>;
  newerTemplatesOnDisk =
    typeof snapshot.version === 'number' && snapshot.version > SNAPSHOT_VERSION;
  if (snapshot.version !== SNAPSHOT_VERSION || !Array.isArray(snapshot.templates)) {
    return [];
  }

  const valid = snapshot.templates
    .map(normalizeTemplateEntry)
    .filter((entry): entry is QueryTemplate => entry !== null);
  return sortNewestFirst(valid);
}

export function saveQueryTemplates(templates: readonly QueryTemplate[]): void {
  if (newerTemplatesOnDisk) {
    return;
  }

  let candidate = sortNewestFirst(templates).slice(0, MAX_QUERY_TEMPLATES);
  while (candidate.length > 0) {
    const text = JSON.stringify({ version: SNAPSHOT_VERSION, templates: candidate });
    if (text.length <= MAX_QUERY_TEMPLATE_STORAGE_CHARS) {
      try {
        localStorage.setItem(QUERY_TEMPLATES_STORAGE_KEY, text);
        return;
      } catch {
        // 同源配额被占满时继续从最旧条目收缩
      }
    }
    candidate = candidate.slice(0, candidate.length - 1);
  }

  try {
    localStorage.setItem(
      QUERY_TEMPLATES_STORAGE_KEY,
      JSON.stringify({ version: SNAPSHOT_VERSION, templates: [] }),
    );
  } catch {
    // localStorage 整体不可写时静默放弃，不阻断编辑器主流程
  }
}

function makeTemplateId(now: Date): string {
  const rand = Math.random().toString(36).slice(2, 8);
  return `tpl-${now.getTime().toString(36)}-${rand}`;
}

export function upsertQueryTemplate(
  templates: readonly QueryTemplate[],
  draft: QueryTemplateDraft,
  now: Date = new Date(),
): QueryTemplate[] {
  const title = draft.title.trim();
  const sql = draft.sql.trim();
  if (!title || !sql) {
    return [...templates];
  }

  const dbType =
    draft.dbType && KNOWN_SQL_DB_TYPES.has(draft.dbType) ? draft.dbType : undefined;
  const entry: QueryTemplate = {
    id: draft.id?.trim() || makeTemplateId(now),
    title,
    sql,
    ...(dbType ? { dbType } : {}),
    updatedAt: now.toISOString(),
  };

  const remaining = templates.filter((item) => item.id !== entry.id);
  return [entry, ...remaining].slice(0, MAX_QUERY_TEMPLATES);
}

export function removeQueryTemplate(
  templates: readonly QueryTemplate[],
  id: string,
): QueryTemplate[] {
  return templates.filter((item) => item.id !== id);
}

/** 提取模板 SQL 中的 D2 `:name` 参数名（通用模板按 PostgreSQL 词法扫描，兼顾 `::type` 排除） */
export function templateParameterNames(
  sql: string,
  dbType?: DatabaseType,
): string[] {
  const dialect = identifierDialectFor(dbType ?? DatabaseType.PostgreSQL);
  if (!dialectSupportsParameters(dialect)) {
    return [];
  }
  const statements = splitSqlStatements(sql, dialect);
  return parameterNames(statements.length > 0 ? statements : [sql], dialect);
}

export function filterQueryTemplates(
  templates: readonly QueryTemplate[],
  options: {
    query: string;
    currentDbType?: DatabaseType;
    scope: QueryTemplateScope;
  },
): QueryTemplate[] {
  const needle = options.query.trim().toLowerCase().replace(/^:/, '');
  return templates.filter((template) => {
    if (
      options.scope === 'compatible' &&
      options.currentDbType &&
      template.dbType &&
      template.dbType !== options.currentDbType
    ) {
      return false;
    }
    if (!needle) {
      return true;
    }
    if (
      template.title.toLowerCase().includes(needle) ||
      template.sql.toLowerCase().includes(needle)
    ) {
      return true;
    }
    const params = templateParameterNames(
      template.sql,
      template.dbType ?? options.currentDbType,
    );
    return params.some((param) => param.toLowerCase().includes(needle));
  });
}
