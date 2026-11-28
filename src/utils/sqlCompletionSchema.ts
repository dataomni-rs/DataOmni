import type { Completion, CompletionContext, CompletionResult, CompletionSource } from '@codemirror/autocomplete';
import {
  MSSQL,
  PLSQL,
  MySQL,
  PostgreSQL,
  SQLite,
  StandardSQL,
  type SQLDialect,
  type SQLNamespace
} from '@codemirror/lang-sql';
import { DatabaseType } from '../contracts/connection';
import { SQL_DIALECT_TRAITS, sqlDialectFromDatabaseType } from '../contracts/sqlDialectTraits';
import type {
  CompletionColumn,
  CompletionRelation,
  RelationKind
} from '../contracts/databaseMetadata';
import { showsSchemaLevel } from './databaseObjects';
import { identifierDialectFor, sqlIdentifierAsTyped, type SqlIdentifierDialect } from './sqlIdentifiers';

export type { CompletionColumn, CompletionRelation, RelationKind };

/** `boost` 的取值范围是 -99..99，两端都用满 */
const MAX_COLUMN_BOOST = 99;

/**
 * 表 / 视图 / Schema 的类型说明。由调用方翻译好再传进来——这个模块是纯的，
 * 在里面读当前语言会让它依赖 store，测试也要跟着起一个。
 */
export interface CompletionLabels {
  table: string;
  view: string;
  schema: string;
}

export interface CompletionSchemaResult {
  schema: SQLNamespace;
  /**
   * 这个 Schema 下的表可以不带前缀补全。只有 PostgreSQL 有值，且只在库里
   * 真有 `public` 时才给——`search_path` 默认就是它。
   */
  defaultSchema?: string;
}

/**
 * 把后端 `get_completion_catalog_query` 查出来的扁平行按关系归组。
 *
 * 保持行本身的顺序：SQL 已经 `ORDER BY` 过表内列序，补全列表照抄这个次序。
 * 按字母排会把 `id` 冲到中间去，而绝大多数时候想选的就是最前面那几列。
 */
export function normalizeCompletionRows(
  rows: ReadonlyArray<Record<string, unknown>>
): CompletionRelation[] {
  const byRelation = new Map<string, CompletionRelation>();

  for (const row of rows) {
    const name = text(row.relation_name);
    const column = text(row.column_name);
    if (!name || !column) {
      continue;
    }

    const schema = text(row.relation_schema) || null;
    // 同名不同 Schema 是两张表，键里必须带 Schema。分隔符用 NUL：
    // 它不可能出现在标识符里，用点号会让 `a.b` 和 schema `a` 的表 `b` 撞车。
    const key = `${schema ?? ''}\u0000${name}`;
    const existing = byRelation.get(key);
    if (existing) {
      existing.columns.push({ name: column, dataType: text(row.data_type) });
      continue;
    }

    byRelation.set(key, {
      schema,
      name,
      kind: text(row.relation_kind) === 'view' ? 'view' : 'table',
      columns: [{ name: column, dataType: text(row.data_type) }]
    });
  }

  return [...byRelation.values()];
}

/**
 * 把真实的库结构翻成 `@codemirror/lang-sql` 的补全命名空间。
 *
 * 层级跟着对象树走：PostgreSQL 保留 Schema 一层（一个连接能横跨多个 Schema，
 * 抹掉它会让两张同名表挤在一起），MySQL 与 SQLite 不保留（连接时已经选定了库）。
 */
export function buildCompletionSchema(
  relations: readonly CompletionRelation[],
  dbType: DatabaseType,
  labels: CompletionLabels,
  /** 登录用户。Oracle 不带前缀的名字落在用户自己的 schema 里（名字大写） */
  username?: string,
  /** 连接上填的库。ClickHouse 不带前缀的名字落在当前库里，空着是 `default` */
  database?: string | null
): CompletionSchemaResult {
  const dialect = identifierDialectFor(dbType);
  if (!showsSchemaLevel(dbType)) {
    const flat: Record<string, SQLNamespace> = {};
    for (const relation of relations) {
      flat[relation.name] = relationNamespace(relation, labels, dialect);
    }
    return { schema: flat };
  }

  const bySchema = new Map<string, Record<string, SQLNamespace>>();
  for (const relation of relations) {
    const schema = relation.schema ?? '';
    const group = bySchema.get(schema) ?? {};
    group[relation.name] = relationNamespace(relation, labels, dialect);
    bySchema.set(schema, group);
  }

  const schema: Record<string, SQLNamespace> = {};
  for (const [name, children] of bySchema) {
    schema[name] = {
      self: completionName(name, dialect, { type: 'type', detail: labels.schema }),
      children
    };
  }

  const sqlDialect = sqlDialectFromDatabaseType(dbType);
  const schemaRule = sqlDialect ? SQL_DIALECT_TRAITS[sqlDialect].defaultSchemaRule : 'none';
  return {
    schema,
    // 不带前缀的名字落在哪个 schema：由 SQL_DIALECT_TRAITS 统一声明（含 Oracle 与达梦的大写用户名）
    defaultSchema: [
      ...(schemaRule === 'database-or-default' ? [database?.trim() || 'default'] : []),
      'public',
      'dbo',
      ...(schemaRule === 'main' ? ['main'] : []),
      ...(schemaRule === 'uppercase-username' && username ? [username.toUpperCase()] : [])
    ].find((name) => bySchema.has(name))
  };
}

/**
 * 方言决定补全出哪些关键字与内建类型：`AUTO_INCREMENT` 只在 MySQL 有，
 * `RETURNING` 只在 PostgreSQL 有。认不出的类型退回标准 SQL——给一份通用
 * 关键字，比一个都不给强。
 */
export function sqlDialectFor(dbType: DatabaseType): SQLDialect {
  const sqlDialect = sqlDialectFromDatabaseType(dbType);
  if (!sqlDialect) {
    return StandardSQL;
  }
  switch (SQL_DIALECT_TRAITS[sqlDialect].completionDialect) {
    case 'mysql':
      return MySQL;
    case 'postgresql':
      return PostgreSQL;
    case 'sqlite':
      return SQLite;
    case 'mssql':
      return MSSQL;
    case 'plsql':
      return PLSQL;
  }
}

/**
 * 表内顺序在补全列表里的权重。
 *
 * CodeMirror 默认按字母排，`created_at` 会被顶到最前、`id` 埋进中间。而表内
 * 顺序是有意义的：主键在前、审计列在后。`boost` 直接加在匹配得分上，而匹配
 * 质量的档位差至少 100（见 autocomplete 的 Penalty），所以 ±99 的范围只在
 * **匹配质量相同时**决定次序，不会让一个更差的匹配插队。
 *
 * 列数超过 199 的表，尾部的列会挤在同一档上退回字母序——那时候靠的已经是
 * 输入前缀而不是浏览列表了。
 */
function columnBoost(index: number): number {
  return Math.max(MAX_COLUMN_BOOST - index, -MAX_COLUMN_BOOST);
}

function relationNamespace(
  relation: CompletionRelation,
  labels: CompletionLabels,
  dialect: SqlIdentifierDialect
): SQLNamespace {
  const self = completionName(relation.name, dialect, {
    type: 'type',
    detail: relation.kind === 'view' ? labels.view : labels.table
  });

  // 类型放在 detail 里：选列的时候最想知道的就是它是什么类型
  const children: Completion[] = relation.columns.map((column, index) => completionName(
    column.name,
    dialect,
    { type: 'property', detail: column.dataType, boost: columnBoost(index) }
  ));

  return { self, children };
}

/**
 * 按名字筛（`label` 是原名），插进去的是这个方言里能执行的写法。
 *
 * lang-sql 只给**字符串**形式的候选自动加引号，而我们传的是带说明的对象，
 * 不写 `apply` 就插裸名字：PostgreSQL 的 `Orders`、叫 `order` 的列补进去就报错。
 * 已经敲了左引号时 lang-sql 会丢掉 `apply`、自己补右引号，不会引两层
 */
function completionName(
  name: string,
  dialect: SqlIdentifierDialect,
  rest: Omit<Completion, 'label' | 'apply'>
): Completion {
  const typed = sqlIdentifierAsTyped(name, dialect);
  return typed === name ? { label: name, ...rest } : { label: name, apply: typed, ...rest };
}

function text(value: unknown): string {
  return typeof value === 'string' ? value : value == null ? '' : String(value);
}

interface SqlToken {
  kind: 'word' | 'quoted' | 'punct';
  /** 词素原文；对 quoted 是去掉外层引号并还原双写转义后的标识符名 */
  value: string;
  start: number;
  end: number;
}

const TABLE_CLAUSE_KEYWORDS = new Set(['FROM', 'JOIN', 'UPDATE', 'INTO']);
const TABLE_POSITION_KEYWORDS = new Set(['FROM', 'JOIN', 'UPDATE', 'INTO', 'TABLE']);
const FROM_LIST_STOP_KEYWORDS = new Set([
  'WHERE', 'GROUP', 'ORDER', 'LIMIT', 'OFFSET', 'FETCH', 'HAVING', 'WINDOW',
  'UNION', 'INTERSECT', 'EXCEPT', 'SET', 'VALUES', 'ON', 'USING', 'RETURNING',
  'INNER', 'LEFT', 'RIGHT', 'FULL', 'CROSS', 'NATURAL', 'STRAIGHT_JOIN', 'JOIN',
  'FOR', 'LOCK', 'INTO', 'QUALIFY', 'SETTINGS', 'FORMAT'
]);
const BARE_WORD_PATTERN = /[\p{L}\p{N}_$]+$/u;
const VALID_BARE_WORD = /^[\p{L}\p{N}_$]*$/u;

/**
 * 词法扫描光标所在的单条 SQL 语句（按顶层 `;` 隔离），同时判断光标是否位于字符串或注释内。
 */
function tokenizeCurrentStatement(
  doc: string,
  cursorPos: number,
  hashLineComment: boolean
): { tokens: SqlToken[]; cursorInStringOrComment: boolean } {
  const len = doc.length;
  let stmtTokens: SqlToken[] = [];
  let cursorFound = false;
  let cursorInStringOrComment = false;
  let i = 0;

  const isIdentStart = (ch: string) => /[\p{L}_$]/u.test(ch);
  const isIdentPart = (ch: string) => /[\p{L}\p{N}_$]/u.test(ch);

  while (i < len) {
    const ch = doc[i];
    const next = i + 1 < len ? doc[i + 1] : '';

    // 空白
    if (ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r') {
      i++;
      continue;
    }

    // 行注释：`--` 或 MySQL/ClickHouse 的 `#`
    if ((ch === '-' && next === '-') || (hashLineComment && ch === '#')) {
      const start = i;
      i += ch === '-' ? 2 : 1;
      while (i < len && doc[i] !== '\n') i++;
      if (cursorPos > start && cursorPos <= i) {
        cursorInStringOrComment = true;
      }
      continue;
    }

    // 块注释 `/* ... */`
    if (ch === '/' && next === '*') {
      const start = i;
      i += 2;
      while (i + 1 < len && !(doc[i] === '*' && doc[i + 1] === '/')) i++;
      i = i + 1 < len ? i + 2 : len;
      if (cursorPos > start && cursorPos < i) {
        cursorInStringOrComment = true;
      }
      continue;
    }

    // 单引号字符串 `'...'`（支持 `''` 与 `\'`）
    if (ch === "'") {
      const start = i;
      i++;
      while (i < len) {
        if (doc[i] === '\\' && i + 1 < len) {
          i += 2;
          continue;
        }
        if (doc[i] === "'") {
          if (i + 1 < len && doc[i + 1] === "'") {
            i += 2;
            continue;
          }
          i++;
          break;
        }
        i++;
      }
      if (cursorPos > start && cursorPos < i) {
        cursorInStringOrComment = true;
      }
      continue;
    }

    // 引用标识符：`"..."`、`` `...` ``、`[...]`
    if (ch === '"' || ch === '`' || ch === '[') {
      const start = i;
      const close = ch === '[' ? ']' : ch;
      i++;
      let value = '';
      while (i < len) {
        if (doc[i] === close) {
          if (i + 1 < len && doc[i + 1] === close) {
            value += close;
            i += 2;
            continue;
          }
          i++;
          break;
        }
        value += doc[i];
        i++;
      }
      stmtTokens.push({ kind: 'quoted', value, start, end: i });
      continue;
    }

    // 语句分隔符 `;`
    if (ch === ';') {
      if (cursorPos <= i) {
        cursorFound = true;
        break;
      }
      stmtTokens = [];
      i++;
      continue;
    }

    // 普通标识符 / 关键字
    if (isIdentStart(ch)) {
      const start = i;
      i++;
      while (i < len && isIdentPart(doc[i])) i++;
      stmtTokens.push({ kind: 'word', value: doc.slice(start, i), start, end: i });
      continue;
    }

    // 标点符号
    stmtTokens.push({ kind: 'punct', value: ch, start: i, end: i + 1 });
    i++;
  }

  if (!cursorFound && cursorPos > len) {
    return { tokens: stmtTokens, cursorInStringOrComment };
  }
  return { tokens: stmtTokens, cursorInStringOrComment };
}

function isIdentifierToken(token: SqlToken | undefined): token is SqlToken {
  return token !== undefined && (token.kind === 'word' || token.kind === 'quoted');
}

function resolveRelation(
  relations: readonly CompletionRelation[],
  schemaName: string | null,
  tableName: string,
  hasSchemaLevel: boolean,
  defaultSchema?: string
): CompletionRelation | null {
  const targetTable = tableName.toLowerCase();
  const targetSchema = schemaName?.toLowerCase() ?? null;

  if (targetSchema !== null) {
    const exact = relations.find(
      rel => rel.name === tableName && (rel.schema ?? '') === schemaName
    );
    if (exact) return exact;
    return relations.find(
      rel => rel.name.toLowerCase() === targetTable && (rel.schema ?? '').toLowerCase() === targetSchema
    ) ?? null;
  }

  if (!hasSchemaLevel) {
    return (
      relations.find(rel => rel.name === tableName)
      ?? relations.find(rel => rel.name.toLowerCase() === targetTable)
      ?? null
    );
  }

  const sameName = relations.filter(rel => rel.name.toLowerCase() === targetTable);
  if (sameName.length === 0) return null;
  if (defaultSchema) {
    const inDefault = sameName.find(
      rel => (rel.schema ?? '').toLowerCase() === defaultSchema.toLowerCase()
    );
    if (inDefault) return inDefault;
  }
  return sameName.length === 1 ? sameName[0] : null;
}

function extractReferencedRelationsFromTokens(
  tokens: readonly SqlToken[],
  relations: readonly CompletionRelation[],
  dbType: DatabaseType,
  defaultSchema?: string
): CompletionRelation[] {
  const hasSchemaLevel = showsSchemaLevel(dbType);
  const matched: CompletionRelation[] = [];
  const seen = new Set<string>();

  const record = (schemaName: string | null, tableName: string) => {
    const found = resolveRelation(relations, schemaName, tableName, hasSchemaLevel, defaultSchema);
    if (!found) return;
    const key = `${found.schema ?? ''}\u0000${found.name}`;
    if (seen.has(key)) return;
    seen.add(key);
    matched.push(found);
  };

  const parseTableRef = (startIndex: number): number => {
    const first = tokens[startIndex];
    if (!isIdentifierToken(first)) return startIndex;
    // 跳过 `FROM ONLY table` 等修饰符（非关键字则视为表名）
    if (first.kind === 'word' && first.value.toUpperCase() === 'ONLY') {
      return parseTableRef(startIndex + 1);
    }
    let idx = startIndex + 1;
    let schemaName: string | null = null;
    let tableName = first.value;

    // 支持 `schema.table` 与 `catalog.schema.table`
    while (
      tokens[idx]?.kind === 'punct'
      && tokens[idx].value === '.'
      && isIdentifierToken(tokens[idx + 1])
    ) {
      schemaName = tableName;
      tableName = tokens[idx + 1].value;
      idx += 2;
    }

    record(schemaName, tableName);
    return idx;
  };

  let i = 0;
  while (i < tokens.length) {
    const token = tokens[i];
    if (token.kind !== 'word') {
      i++;
      continue;
    }
    const upper = token.value.toUpperCase();
    if (!TABLE_CLAUSE_KEYWORDS.has(upper)) {
      i++;
      continue;
    }

    i = parseTableRef(i + 1);

    // `FROM a, b AS b1` 逗号连接表列表
    if (upper === 'FROM') {
      while (i < tokens.length) {
        const current = tokens[i];
        if (current.kind === 'word' && FROM_LIST_STOP_KEYWORDS.has(current.value.toUpperCase())) {
          break;
        }
        if (current.kind === 'punct' && current.value === ',') {
          i = parseTableRef(i + 1);
          continue;
        }
        if (current.kind === 'punct' && (current.value === '(' || current.value === ')')) {
          break;
        }
        i++;
      }
    }
  }

  return matched;
}

/**
 * 从光标所在的单条 SQL 语句里提取 `FROM` / `JOIN` / `UPDATE` / `INTO` 引用的已知表或视图。
 * 保持出现顺序并去重，供无表别名时的裸列补全（DC2 / R7）使用。
 */
export function extractReferencedRelations(
  doc: string,
  cursorPos: number,
  relations: readonly CompletionRelation[],
  dbType: DatabaseType,
  defaultSchema?: string
): CompletionRelation[] {
  if (relations.length === 0) return [];
  const hashComment = dbType === DatabaseType.MySQL || dbType === DatabaseType.ClickHouse;
  const { tokens } = tokenizeCurrentStatement(doc, cursorPos, hashComment);
  return extractReferencedRelationsFromTokens(tokens, relations, dbType, defaultSchema);
}

/**
 * 构造无表别名时的裸列补全源（DC2 / R7）。
 *
 * 与 `@codemirror/lang-sql` 的 `schemaCompletionSource` 分工明确：
 * - 光标紧跟 `.`（`o.` / `o.to` / `public.`）或位于字符串/注释内时立即返回 `null`，完全交由 lang-sql；
 * - 光标刚敲完 `FROM` / `JOIN` / `UPDATE` / `INTO` / `TABLE` 时返回 `null`，避免把列名混进表名位置；
 * - 其余位置按当前语句引用的已知表产出列候选，保留表内列序与方言引号，不把全库表列一股脑倒出来。
 */
export function createBareColumnCompletionSource(
  relations: readonly CompletionRelation[],
  dbType: DatabaseType,
  defaultSchema?: string
): CompletionSource {
  const dialect = identifierDialectFor(dbType);
  const hashComment = dbType === DatabaseType.MySQL || dbType === DatabaseType.ClickHouse;

  return (context: CompletionContext): CompletionResult | null => {
    if (relations.length === 0) return null;

    const doc = context.state.doc.toString();
    const pos = context.pos;
    const { tokens, cursorInStringOrComment } = tokenizeCurrentStatement(doc, pos, hashComment);
    if (cursorInStringOrComment) return null;

    const wordMatch = context.matchBefore(BARE_WORD_PATTERN);
    const from = wordMatch ? wordMatch.from : pos;
    if (!wordMatch && !context.explicit) return null;

    // 紧跟成员访问符 `.`（如 `o.`、`o.to`、`billing.inv`）时退让给 lang-sql
    const prevNonSpace = doc.slice(0, from).trimEnd();
    if (prevNonSpace.endsWith('.')) return null;

    // 紧跟表名关键字（如 `FROM |`、`FROM ord|`、`JOIN |`）时不弹列名
    let prevToken: SqlToken | undefined;
    for (let idx = tokens.length - 1; idx >= 0; idx--) {
      if (tokens[idx].end <= from) {
        prevToken = tokens[idx];
        break;
      }
    }
    if (prevToken?.kind === 'word' && TABLE_POSITION_KEYWORDS.has(prevToken.value.toUpperCase())) {
      return null;
    }

    const referenced = extractReferencedRelationsFromTokens(tokens, relations, dbType, defaultSchema);
    if (referenced.length === 0) return null;

    const multiTable = referenced.length > 1;
    const options: Completion[] = [];
    const seenColumns = new Set<string>();

    for (const relation of referenced) {
      relation.columns.forEach((column, index) => {
        const dedupKey = multiTable ? `${relation.name}\u0000${column.name}` : column.name;
        if (seenColumns.has(dedupKey)) return;
        seenColumns.add(dedupKey);

        const detail = multiTable
          ? (column.dataType ? `${relation.name} · ${column.dataType}` : relation.name)
          : column.dataType;

        options.push(
          completionName(column.name, dialect, {
            type: 'property',
            detail,
            boost: columnBoost(index)
          })
        );
      });
    }

    if (options.length === 0) return null;
    return {
      from,
      options,
      validFor: VALID_BARE_WORD
    };
  };
}
