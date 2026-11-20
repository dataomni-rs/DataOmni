import type { CompletionRelation, DatabaseType } from '../contracts';
import type { SerializedResultValue } from '../contracts/resultSet';
import type { CachedTableStructure } from '../stores/appStore';
import { tableStructureKey } from '../stores/appStore';
import { showsSchemaLevel } from './databaseObjects';
import type { ForeignKeyInfo } from './schemaObjects';
import type { SqlIdentifierDialect } from './sqlIdentifiers';

export type JumpTargetSource = 'foreign-key' | 'sql-join' | 'catalog-match' | 'source-table';

export interface ColumnJumpTarget {
  schema: string | null;
  table: string;
  targetColumns: readonly string[];
  sourceColumns: readonly string[];
  source: JumpTargetSource;
  /** 是否足够明确，可以直接在单元格上画悬停快捷跳转图标 */
  inline: boolean;
}

export interface ResolvedCellJump {
  schema: string | null;
  table: string;
  filters: Array<{ column: string; value: string }>;
  source: JumpTargetSource;
  /** 用于菜单与提示展示，例如 `users (id)` 或 `public.orders (id)` */
  targetLabel: string;
}

export interface TableRef {
  schema: string | null;
  table: string;
}

export interface SqlJoinPair {
  left: TableRef;
  leftColumn: string;
  right: TableRef;
  rightColumn: string;
}

export type SqlProjectionItem =
  | { kind: 'column'; outputName: string; qualifier: string | null; column: string }
  | { kind: 'star'; qualifier: string | null }
  | { kind: 'other' };

export interface SqlJumpContext {
  aliases: Map<string, TableRef>;
  tables: TableRef[];
  joinPairs: SqlJoinPair[];
  projections: SqlProjectionItem[];
}

const REFERENCE_COL_SUFFIX = /(?:_id|_uuid|_guid)$/i;
const CAMEL_ID_SUFFIX = /[a-z0-9]Id$/;

const SQL_CLAUSE_KEYWORDS = new Set([
  'where', 'on', 'using', 'join', 'inner', 'left', 'right', 'full', 'cross', 'outer', 'natural',
  'group', 'order', 'having', 'limit', 'offset', 'union', 'intersect', 'except', 'set', 'values',
  'for', 'fetch', 'window', 'into', 'as'
]);

const IDENTIFIER_PATTERN = String.raw`(?:"(?:[^"]|"")+"|\`(?:[^\`]|\`\` )+\`|\[(?:[^\]]|\]\])+\]|[A-Za-z_][A-Za-z0-9_$]*)`;

/** 是否像一个引用外键 / 业务 ID 列名（不含裸 `id`） */
export function isReferenceColumnName(columnName: string): boolean {
  const trimmed = columnName.trim();
  if (trimmed.length <= 2) {
    return false;
  }
  return REFERENCE_COL_SUFFIX.test(trimmed) || CAMEL_ID_SUFFIX.test(trimmed);
}

/** 是否是主键 `id` 或外键/业务 ID 列 */
export function isPrimaryLikeIdColumn(columnName: string): boolean {
  const trimmed = columnName.trim();
  return /^id$/i.test(trimmed) || isReferenceColumnName(trimmed);
}

/**
 * 把单元格的值转成用于表数据页 `=` 筛选的纯文本。
 * NULL、空串、布尔、二进制或复合 JSON 不参与外键跳转。
 */
export function serializeJumpFilterValue(cell: SerializedResultValue): string | null {
  if (cell === null || cell === undefined || typeof cell === 'boolean') {
    return null;
  }
  if (typeof cell === 'string') {
    return cell.trim() === '' ? null : cell;
  }
  if (typeof cell === 'number') {
    return Number.isFinite(cell) ? String(cell) : null;
  }
  if (typeof cell === 'object' && (cell.type === 'bigint' || cell.type === 'decimal')) {
    return cell.value.trim() === '' ? null : cell.value;
  }
  return null;
}

/**
 * 归一化目标表的 schema，与侧边栏打开的 `TableWorkspaceTab` id 保持一致：
 * MySQL / SQLite 同库表用 `schema = null`，PostgreSQL / SQL Server / Oracle 保留 schema。
 */
export function normalizeJumpSchema(
  targetSchema: string | null | undefined,
  dbType: DatabaseType,
  currentDatabase?: string | null
): string | null {
  if (!targetSchema) {
    return null;
  }
  if (!showsSchemaLevel(dbType)) {
    if (currentDatabase && targetSchema.toLowerCase() === currentDatabase.toLowerCase()) {
      return null;
    }
    return targetSchema;
  }
  return targetSchema;
}

function unquoteSqlIdent(raw: string, dialect: SqlIdentifierDialect): string {
  const trimmed = raw.trim();
  if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
    return trimmed.slice(1, -1).replace(/""/g, '"');
  }
  if (trimmed.startsWith('`') && trimmed.endsWith('`')) {
    return trimmed.slice(1, -1).replace(/``/g, '`');
  }
  if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
    return trimmed.slice(1, -1).replace(/\]\]/g, ']');
  }
  if (dialect === 'oracle') {
    return trimmed.toUpperCase();
  }
  return dialect === 'postgresql' ? trimmed.toLowerCase() : trimmed;
}

/** 用空格抹平注释与单引号字符串，保留字符位置和换行结构以便后续安全切分 */
function stripCommentsAndStrings(sql: string): string {
  let out = '';
  let i = 0;
  while (i < sql.length) {
    const ch = sql[i];
    const next = sql[i + 1];
    if (ch === '-' && next === '-') {
      out += '  ';
      i += 2;
      while (i < sql.length && sql[i] !== '\n') {
        out += ' ';
        i += 1;
      }
      continue;
    }
    if (ch === '/' && next === '*') {
      out += '  ';
      i += 2;
      while (i < sql.length && !(sql[i] === '*' && sql[i + 1] === '/')) {
        out += sql[i] === '\n' ? '\n' : ' ';
        i += 1;
      }
      if (i < sql.length) {
        out += '  ';
        i += 2;
      }
      continue;
    }
    if (ch === "'") {
      out += ' ';
      i += 1;
      while (i < sql.length) {
        if (sql[i] === "'" && sql[i + 1] === "'") {
          out += '  ';
          i += 2;
          continue;
        }
        if (sql[i] === '\\' && i + 1 < sql.length) {
          out += '  ';
          i += 2;
          continue;
        }
        if (sql[i] === "'") {
          out += ' ';
          i += 1;
          break;
        }
        out += sql[i] === '\n' ? '\n' : ' ';
        i += 1;
      }
      continue;
    }
    out += ch;
    i += 1;
  }
  return out;
}

/** 提取 `SELECT ... FROM` 之间顶层的投影片段 */
function extractTopLevelSelectSegment(cleanedSql: string): string | null {
  const selectMatch = /^\s*select\b/i.exec(cleanedSql);
  if (!selectMatch) {
    return null;
  }
  let pos = selectMatch[0].length;
  // 跳过 DISTINCT / ALL / TOP n
  const prefixMatch = /^\s+(?:(?:distinct|all)\b\s*)?(?:top\s+(?:\(\s*\d+\s*\)|\d+)\s+)?/i.exec(
    cleanedSql.slice(pos)
  );
  if (prefixMatch) {
    pos += prefixMatch[0].length;
  }

  let depth = 0;
  let quote: string | null = null;
  for (let i = pos; i < cleanedSql.length; i += 1) {
    const ch = cleanedSql[i];
    if (quote) {
      if (ch === quote) {
        quote = null;
      }
      continue;
    }
    if (ch === '"' || ch === '`') {
      quote = ch;
      continue;
    }
    if (ch === '(') {
      depth += 1;
      continue;
    }
    if (ch === ')') {
      depth = Math.max(0, depth - 1);
      continue;
    }
    if (depth === 0 && /\bfrom\b/i.test(cleanedSql.slice(i, i + 5))) {
      const prev = i > 0 ? cleanedSql[i - 1] : ' ';
      if (!/[A-Za-z0-9_$]/.test(prev) && /^from(?![A-Za-z0-9_$])/i.test(cleanedSql.slice(i))) {
        return cleanedSql.slice(pos, i).trim();
      }
    }
  }
  return null;
}

function splitTopLevelCommas(segment: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let quote: string | null = null;
  let start = 0;
  for (let i = 0; i < segment.length; i += 1) {
    const ch = segment[i];
    if (quote) {
      if (ch === quote) {
        quote = null;
      }
      continue;
    }
    if (ch === '"' || ch === '`') {
      quote = ch;
      continue;
    }
    if (ch === '(') {
      depth += 1;
    } else if (ch === ')') {
      depth = Math.max(0, depth - 1);
    } else if (ch === ',' && depth === 0) {
      parts.push(segment.slice(start, i).trim());
      start = i + 1;
    }
  }
  const tail = segment.slice(start).trim();
  if (tail) {
    parts.push(tail);
  }
  return parts;
}

/**
 * 解析 SELECT 语句里的表别名、JOIN 等值关联对与投影列来源。
 * 故意保持轻量确定性，只认看得懂的结构；复杂子查询或表达式退回空而不误报。
 */
export function parseSqlJumpContext(sql: string, dialect: SqlIdentifierDialect): SqlJumpContext {
  const cleaned = stripCommentsAndStrings(sql);
  const aliases = new Map<string, TableRef>();
  const tables: TableRef[] = [];
  const joinPairs: SqlJoinPair[] = [];
  const projections: SqlProjectionItem[] = [];

  const ident = IDENTIFIER_PATTERN.replace(' )', ')');

  // 1. 解析 FROM / JOIN 的表与别名
  const tableClauseRe = new RegExp(
    String.raw`\b(?:from|join)\s+(${ident})(?:\s*\.\s*(${ident}))?(?:\s+(?:as\s+)?(${ident}))?`,
    'gi'
  );
  let match: RegExpExecArray | null;
  while ((match = tableClauseRe.exec(cleaned)) !== null) {
    const [, first, second, rawAlias] = match;
    // 跳过 FROM (subquery) 这种情况：first 若是左括号不会匹配 ident，但还要防止误把关键字当表名
    if (SQL_CLAUSE_KEYWORDS.has(first.toLowerCase())) {
      continue;
    }
    const ref: TableRef = second
      ? { schema: unquoteSqlIdent(first, dialect), table: unquoteSqlIdent(second, dialect) }
      : { schema: null, table: unquoteSqlIdent(first, dialect) };
    tables.push(ref);
    aliases.set(ref.table.toLowerCase(), ref);
    if (rawAlias && !SQL_CLAUSE_KEYWORDS.has(rawAlias.toLowerCase())) {
      aliases.set(unquoteSqlIdent(rawAlias, dialect).toLowerCase(), ref);
    }
  }

  // 2. 解析 ON / WHERE 里的 a.col1 = b.col2 等值关联对
  const eqPairRe = new RegExp(
    String.raw`(${ident})\s*\.\s*(${ident})\s*=\s*(${ident})\s*\.\s*(${ident})`,
    'g'
  );
  while ((match = eqPairRe.exec(cleaned)) !== null) {
    const [, q1, c1, q2, c2] = match;
    const left = aliases.get(unquoteSqlIdent(q1, dialect).toLowerCase());
    const right = aliases.get(unquoteSqlIdent(q2, dialect).toLowerCase());
    if (!left || !right) {
      continue;
    }
    if (left.table.toLowerCase() === right.table.toLowerCase() && left.schema === right.schema) {
      continue;
    }
    joinPairs.push({
      left,
      leftColumn: unquoteSqlIdent(c1, dialect),
      right,
      rightColumn: unquoteSqlIdent(c2, dialect)
    });
  }

  // 3. 解析 SELECT 投影列
  const selectSegment = extractTopLevelSelectSegment(cleaned);
  if (selectSegment) {
    const qualifiedStarRe = new RegExp(String.raw`^(?:(${ident})\s*\.\s*)?\*$`);
    const colProjectionRe = new RegExp(
      String.raw`^(?:(${ident})\s*\.\s*)?(${ident})(?:\s+(?:as\s+)?(${ident}))?$`,
      'i'
    );
    for (const item of splitTopLevelCommas(selectSegment)) {
      const starMatch = qualifiedStarRe.exec(item);
      if (starMatch) {
        projections.push({
          kind: 'star',
          qualifier: starMatch[1] ? unquoteSqlIdent(starMatch[1], dialect) : null
        });
        continue;
      }
      const colMatch = colProjectionRe.exec(item);
      if (colMatch) {
        const [, rawQualifier, rawColumn, rawAlias] = colMatch;
        const column = unquoteSqlIdent(rawColumn, dialect);
        const alias = rawAlias && !SQL_CLAUSE_KEYWORDS.has(rawAlias.toLowerCase())
          ? unquoteSqlIdent(rawAlias, dialect)
          : null;
        projections.push({
          kind: 'column',
          outputName: alias ?? column,
          qualifier: rawQualifier ? unquoteSqlIdent(rawQualifier, dialect) : null,
          column
        });
        continue;
      }
      projections.push({ kind: 'other' });
    }
  }

  return { aliases, tables, joinPairs, projections };
}

function stemFromReferenceColumn(columnName: string): string {
  if (REFERENCE_COL_SUFFIX.test(columnName)) {
    return columnName.replace(REFERENCE_COL_SUFFIX, '').toLowerCase();
  }
  if (CAMEL_ID_SUFFIX.test(columnName)) {
    return columnName
      .slice(0, -2)
      .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
      .toLowerCase();
  }
  return columnName.toLowerCase();
}

function stemVariants(stem: string): string[] {
  const variants = new Set<string>([stem, `${stem}s`, `${stem}es`]);
  if (stem.endsWith('y') && stem.length > 1) {
    variants.add(`${stem.slice(0, -1)}ies`);
  }
  return [...variants];
}

function matchCatalogCandidates(
  columnName: string,
  relations: readonly CompletionRelation[],
  dbType: DatabaseType,
  currentDatabase: string | null | undefined,
  excludeTable?: { schema: string | null; table: string } | null
): Array<{ target: ColumnJumpTarget; score: number }> {
  if (!isReferenceColumnName(columnName) || relations.length === 0) {
    return [];
  }

  const colLower = columnName.toLowerCase();
  const stem = stemFromReferenceColumn(columnName);
  if (!stem) {
    return [];
  }
  const variants = stemVariants(stem);
  const tokens = stem.split('_').filter((token) => token.length >= 3);
  const lastToken = tokens[tokens.length - 1] ?? '';
  const lastTokenVariants = lastToken && lastToken !== stem ? stemVariants(lastToken) : [];

  const scored: Array<{ target: ColumnJumpTarget; score: number }> = [];

  for (const rel of relations) {
    const relSchema = normalizeJumpSchema(rel.schema, dbType, currentDatabase);
    if (
      excludeTable
      && rel.name.toLowerCase() === excludeTable.table.toLowerCase()
      && (relSchema ?? '') === (excludeTable.schema ?? '')
    ) {
      continue;
    }

    const relTableLower = rel.name.toLowerCase();
    const idCol = rel.columns.find((c) => c.name.toLowerCase() === 'id');
    const sameColIndex = rel.columns.findIndex((c) => c.name.toLowerCase() === colLower);
    const sameCol = sameColIndex >= 0 ? rel.columns[sameColIndex] : undefined;

    let score = 0;
    let targetColumn: string | null = null;

    if (variants.includes(relTableLower)) {
      if (idCol) {
        score = 100;
        targetColumn = idCol.name;
      } else if (sameCol) {
        score = 95;
        targetColumn = sameCol.name;
      }
    } else if (variants.some((v) => relTableLower.endsWith(`_${v}`))) {
      if (idCol) {
        score = 90;
        targetColumn = idCol.name;
      } else if (sameCol) {
        score = 86;
        targetColumn = sameCol.name;
      }
    } else if (sameCol) {
      const sharesToken = tokens.some((token) => relTableLower.includes(token));
      if (sharesToken && sameColIndex <= 2) {
        score = 86;
      } else if (sharesToken) {
        score = 80;
      } else if (sameColIndex === 0) {
        score = 72;
      } else {
        score = 55;
      }
      targetColumn = sameCol.name;
    } else if (lastTokenVariants.length > 0 && lastTokenVariants.includes(relTableLower) && idCol) {
      score = 76;
      targetColumn = idCol.name;
    }

    if (score > 0 && targetColumn) {
      // 表优先于视图
      const adjustedScore = rel.kind === 'table' ? score : score - 4;
      scored.push({
        score: adjustedScore,
        target: {
          schema: relSchema,
          table: rel.name,
          targetColumns: [targetColumn],
          sourceColumns: [columnName],
          source: 'catalog-match',
          inline: false
        }
      });
    }
  }

  scored.sort(
    (a, b) =>
      b.score - a.score
      || a.target.table.length - b.target.table.length
      || a.target.table.localeCompare(b.target.table)
  );

  // 去重并只留前 5 个候选
  const seen = new Set<string>();
  const top: Array<{ target: ColumnJumpTarget; score: number }> = [];
  for (const item of scored) {
    const key = `${item.target.schema ?? ''}\0${item.target.table.toLowerCase()}\0${item.target.targetColumns.join(',').toLowerCase()}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    top.push(item);
    if (top.length >= 5) {
      break;
    }
  }

  // 仅当榜首高置信且显著领先时才在单元格上画快捷跳转图标；其余留在右键菜单
  if (top.length > 0 && top[0].score >= 80) {
    const unambiguous = top.length === 1 || top[0].score - top[1].score >= 8;
    if (unambiguous) {
      top[0] = {
        ...top[0],
        target: { ...top[0].target, inline: true }
      };
    }
  }

  return top;
}

function dedupeTargets(targets: readonly ColumnJumpTarget[]): ColumnJumpTarget[] {
  const seen = new Set<string>();
  const out: ColumnJumpTarget[] = [];
  for (const target of targets) {
    const key = `${target.schema ?? ''}\0${target.table.toLowerCase()}\0${target.targetColumns.join(',').toLowerCase()}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    out.push(target);
  }
  return out;
}

function targetsFromPhysicalFks(
  foreignKeys: readonly ForeignKeyInfo[],
  dbType: DatabaseType,
  currentDatabase: string | null | undefined
): Map<string, ColumnJumpTarget[]> {
  const byColumn = new Map<string, ColumnJumpTarget[]>();
  for (const fk of foreignKeys) {
    if (!fk.referencedTable || fk.columns.length === 0) {
      continue;
    }
    const targetColumns = fk.referencedColumns.filter((c): c is string => Boolean(c));
    if (targetColumns.length !== fk.columns.length) {
      continue;
    }
    const target: ColumnJumpTarget = {
      schema: normalizeJumpSchema(fk.referencedSchema, dbType, currentDatabase),
      table: fk.referencedTable,
      targetColumns,
      sourceColumns: [...fk.columns],
      source: 'foreign-key',
      inline: true
    };
    for (const col of fk.columns) {
      const key = col.toLowerCase();
      const list = byColumn.get(key) ?? [];
      list.push(target);
      byColumn.set(key, list);
    }
  }
  return byColumn;
}

/**
 * 计算表数据视图 (`TableDataViewer`) 中每一列可跳转的目标表列表（按小写列名索引）。
 * 优先物理外键；无物理外键时，用已缓存的库目录为 `*_id` 列补全候选。
 */
export function resolveTableColumnJumps(options: {
  columns: readonly string[];
  foreignKeys?: readonly ForeignKeyInfo[] | null;
  relations?: readonly CompletionRelation[];
  currentTable: string;
  currentSchema?: string | null;
  dbType: DatabaseType;
  currentDatabase?: string | null;
}): Map<string, ColumnJumpTarget[]> {
  const {
    columns,
    foreignKeys,
    relations = [],
    currentTable,
    currentSchema = null,
    dbType,
    currentDatabase
  } = options;

  const physicalByCol = targetsFromPhysicalFks(foreignKeys ?? [], dbType, currentDatabase);
  const exclude = {
    schema: normalizeJumpSchema(currentSchema, dbType, currentDatabase),
    table: currentTable
  };

  const result = new Map<string, ColumnJumpTarget[]>();
  for (const col of columns) {
    const colKey = col.toLowerCase();
    const list: ColumnJumpTarget[] = [...(physicalByCol.get(colKey) ?? [])];
    const hasPhysical = list.length > 0;
    const catalogMatches = matchCatalogCandidates(col, relations, dbType, currentDatabase, exclude);
    for (const { target } of catalogMatches) {
      list.push(hasPhysical ? { ...target, inline: false } : target);
    }
    const deduped = dedupeTargets(list);
    if (deduped.length > 0) {
      result.set(colKey, deduped);
    }
  }
  return result;
}

/**
 * 计算 SQL 查询结果 (`QueryResultScrollTable`) 中每一列可跳转的目标表列表（按列序号索引）。
 * 综合四层信号：物理外键缓存 → SQL JOIN ON 关联 → 库目录匹配 → 来源原表（ID 列）。
 */
export function resolveQueryColumnJumps(options: {
  sql?: string;
  columns: readonly string[];
  dialect: SqlIdentifierDialect;
  dbType: DatabaseType;
  currentDatabase?: string | null;
  connectionId?: string | null;
  relations?: readonly CompletionRelation[];
  tableStructures?: Readonly<Record<string, CachedTableStructure>>;
}): Map<number, ColumnJumpTarget[]> {
  const {
    sql,
    columns,
    dialect,
    dbType,
    currentDatabase,
    connectionId,
    relations = [],
    tableStructures = {}
  } = options;

  const ctx = sql ? parseSqlJumpContext(sql, dialect) : null;
  const singleTable = ctx && ctx.tables.length === 1 ? ctx.tables[0] : null;
  const hasStar = ctx ? ctx.projections.some((p) => p.kind === 'star') : false;

  const result = new Map<number, ColumnJumpTarget[]>();

  for (let colIndex = 0; colIndex < columns.length; colIndex += 1) {
    const colName = columns[colIndex];
    const list: ColumnJumpTarget[] = [];

    // 1. 判定这一列来自哪张表及原列名
    let sourceTable: TableRef | null = null;
    let sourceColumn = colName;
    if (ctx && !hasStar && colIndex < ctx.projections.length) {
      const proj = ctx.projections[colIndex];
      if (proj.kind === 'column') {
        sourceColumn = proj.column;
        if (proj.qualifier) {
          sourceTable = ctx.aliases.get(proj.qualifier.toLowerCase()) ?? null;
        } else if (singleTable) {
          sourceTable = singleTable;
        }
      }
    } else if (singleTable) {
      sourceTable = singleTable;
    }

    const normalizedSourceTable = sourceTable
      ? {
          schema: normalizeJumpSchema(sourceTable.schema, dbType, currentDatabase),
          table: sourceTable.table
        }
      : null;

    // 2. 查已缓存的源表物理外键
    if (normalizedSourceTable && connectionId) {
      const structKey = tableStructureKey(
        connectionId,
        normalizedSourceTable.schema,
        normalizedSourceTable.table
      );
      const cachedFks = tableStructures[structKey]?.objects?.foreignKeys;
      if (cachedFks && cachedFks.length > 0) {
        const fkMap = targetsFromPhysicalFks(cachedFks, dbType, currentDatabase);
        for (const fkTarget of fkMap.get(sourceColumn.toLowerCase()) ?? []) {
          // SQL 投影里可能没有带齐复合外键的全部列，或改了别名；这里仅在全部源列都存在时启用
          const mappedSources = fkTarget.sourceColumns.map((srcCol) => {
            if (srcCol.toLowerCase() === sourceColumn.toLowerCase()) {
              return colName;
            }
            return columns.find((c) => c.toLowerCase() === srcCol.toLowerCase()) ?? null;
          });
          if (mappedSources.every((s): s is string => s !== null)) {
            list.push({ ...fkTarget, sourceColumns: mappedSources });
          }
        }
      }
    }

    // 3. 从 SQL JOIN ON 条件提取关联跳转（如 `ON s.id = us.skill_id`）
    if (ctx && normalizedSourceTable) {
      for (const pair of ctx.joinPairs) {
        const leftNorm = {
          schema: normalizeJumpSchema(pair.left.schema, dbType, currentDatabase),
          table: pair.left.table
        };
        const rightNorm = {
          schema: normalizeJumpSchema(pair.right.schema, dbType, currentDatabase),
          table: pair.right.table
        };
        if (
          leftNorm.table.toLowerCase() === normalizedSourceTable.table.toLowerCase()
          && pair.leftColumn.toLowerCase() === sourceColumn.toLowerCase()
        ) {
          list.push({
            schema: rightNorm.schema,
            table: rightNorm.table,
            targetColumns: [pair.rightColumn],
            sourceColumns: [colName],
            source: 'sql-join',
            inline: true
          });
        } else if (
          rightNorm.table.toLowerCase() === normalizedSourceTable.table.toLowerCase()
          && pair.rightColumn.toLowerCase() === sourceColumn.toLowerCase()
        ) {
          list.push({
            schema: leftNorm.schema,
            table: leftNorm.table,
            targetColumns: [pair.leftColumn],
            sourceColumns: [colName],
            source: 'sql-join',
            inline: true
          });
        }
      }
    }

    // 4. 库目录启发式匹配（覆盖无物理 FK 的 `user_id`、`claw_id` 等列）
    const catalogMatches = matchCatalogCandidates(
      sourceColumn,
      relations,
      dbType,
      currentDatabase,
      normalizedSourceTable
    );
    const alreadyInline = list.some((t) => t.inline);
    for (const { target } of catalogMatches) {
      list.push({
        ...target,
        sourceColumns: [colName],
        inline: alreadyInline ? false : target.inline
      });
    }

    // 5. 来源原表跳转：对 SQL 查询结果里的 ID 列（如 `s.global_skill_id`、`id`），可直接跳回来源表定位该行
    if (normalizedSourceTable && isPrimaryLikeIdColumn(sourceColumn)) {
      const hasInlineAlready = list.some((t) => t.inline);
      list.push({
        schema: normalizedSourceTable.schema,
        table: normalizedSourceTable.table,
        targetColumns: [sourceColumn],
        sourceColumns: [colName],
        source: 'source-table',
        inline: !hasInlineAlready
      });
    }

    const deduped = dedupeTargets(list);
    if (deduped.length > 0) {
      result.set(colIndex, deduped);
    }
  }

  return result;
}

/** 结合当前行的数据构造一次可执行的表跳转请求；若关键列为 NULL / 不可序列化则返回 null */
export function buildCellJump(
  target: ColumnJumpTarget,
  getCellValue: (columnName: string) => SerializedResultValue
): ResolvedCellJump | null {
  const filters: Array<{ column: string; value: string }> = [];
  for (let i = 0; i < target.sourceColumns.length; i += 1) {
    const raw = getCellValue(target.sourceColumns[i]);
    const serialized = serializeJumpFilterValue(raw);
    if (serialized === null) {
      return null;
    }
    filters.push({
      column: target.targetColumns[i],
      value: serialized
    });
  }
  if (filters.length === 0) {
    return null;
  }
  const qualified = target.schema ? `${target.schema}.${target.table}` : target.table;
  const colSummary = target.targetColumns.join(', ');
  return {
    schema: target.schema,
    table: target.table,
    filters,
    source: target.source,
    targetLabel: `${qualified} (${colSummary})`
  };
}
