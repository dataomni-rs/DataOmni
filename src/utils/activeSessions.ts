import { DatabaseType } from '../contracts/connection';

/** `get_active_sessions_query` 后端返回结构（snake_case） */
export interface ActiveSessionsQueryDescriptor {
  sql: string;
  fallback_sql: string | null;
  supports_cancel: boolean;
  supports_terminate: boolean;
}

export type SessionAction = 'cancel' | 'terminate';

/** `terminate_database_session` 后端返回结构（camelCase） */
export interface TerminateSessionResult {
  applied: boolean;
  statement: string;
}

/** 归一化后的关系库活跃会话记录 */
export interface ActiveSessionRow {
  readonly sessionId: string;
  readonly username: string;
  readonly databaseName: string;
  readonly clientAddr: string;
  readonly state: string;
  readonly durationSeconds: number;
  readonly waitEvent: string | null;
  readonly blockingPid: string | null;
  readonly sqlText: string;
  readonly isCurrent: boolean;
  readonly isActive: boolean;
  readonly isBlocked: boolean;
}

export interface ActiveSessionsSummary {
  readonly total: number;
  readonly active: number;
  readonly blocked: number;
  readonly maxDurationSeconds: number;
}

export interface ActiveSessionsFilter {
  readonly search: string;
  readonly onlyActive: boolean;
  readonly onlyBlocked: boolean;
  readonly hideCurrent: boolean;
}

const SUPPORTED_TYPES: ReadonlySet<string> = new Set([
  DatabaseType.MySQL,
  DatabaseType.PostgreSQL,
  DatabaseType.SqlServer,
  DatabaseType.Oracle,
  DatabaseType.Dameng,
  DatabaseType.ClickHouse
]);

/** 仅服务端关系库提供活跃会话与锁等待诊断（SQLite/DuckDB 为本地文件库，非 SQL 引擎另有专用诊断） */
export function supportsActiveSessions(dbType: string): boolean {
  return SUPPORTED_TYPES.has(dbType);
}

/** 兼容不同驱动返回的标量或 `{ value }` 包装对象 */
function unwrapScalar(value: unknown): unknown {
  if (value !== null && typeof value === 'object' && 'value' in (value as Record<string, unknown>)) {
    return (value as Record<string, unknown>).value;
  }
  return value;
}

function asString(value: unknown): string {
  const scalar = unwrapScalar(value);
  if (scalar === null || scalar === undefined) {
    return '';
  }
  return String(scalar).trim();
}

function asNullableString(value: unknown): string | null {
  const text = asString(value);
  return text.length > 0 ? text : null;
}

function asDurationSeconds(value: unknown): number {
  const scalar = unwrapScalar(value);
  if (typeof scalar === 'number' && Number.isFinite(scalar)) {
    return Math.max(0, Math.floor(scalar));
  }
  if (typeof scalar === 'string') {
    const parsed = Number(scalar);
    if (Number.isFinite(parsed)) {
      return Math.max(0, Math.floor(parsed));
    }
  }
  return 0;
}

function asBoolean(value: unknown): boolean {
  const scalar = unwrapScalar(value);
  if (typeof scalar === 'boolean') {
    return scalar;
  }
  if (typeof scalar === 'number') {
    return scalar !== 0;
  }
  if (typeof scalar === 'string') {
    const normalized = scalar.trim().toLowerCase();
    return normalized === '1' || normalized === 'true' || normalized === 't' || normalized === 'yes';
  }
  return false;
}

/** 从一行结果中按大小写不敏感键名取值，容错不同驱动的列大小写行为 */
function readColumn(row: Record<string, unknown>, key: string): unknown {
  if (key in row) {
    return row[key];
  }
  const upper = key.toUpperCase();
  if (upper in row) {
    return row[upper];
  }
  const lower = key.toLowerCase();
  for (const [candidate, value] of Object.entries(row)) {
    if (candidate.toLowerCase() === lower) {
      return value;
    }
  }
  return undefined;
}

/** 判断一条会话当前是否处于活跃执行状态 */
export function isActiveSessionState(state: string, dbType: DatabaseType): boolean {
  const normalized = state.trim().toLowerCase();
  if (!normalized) {
    return false;
  }
  if (dbType === DatabaseType.MySQL) {
    return normalized !== 'sleep' && normalized !== 'daemon' && normalized !== 'binlog dump';
  }
  if (dbType === DatabaseType.PostgreSQL) {
    return normalized === 'active' || normalized.startsWith('idle in transaction');
  }
  if (dbType === DatabaseType.SqlServer) {
    return normalized !== 'sleeping' && normalized !== 'dormant';
  }
  if (dbType === DatabaseType.Oracle || dbType === DatabaseType.Dameng) {
    return normalized === 'active';
  }
  return true;
}

/** 判断一条会话是否处于阻塞或锁等待状态 */
export function isBlockedSession(blockingPid: string | null, waitEvent: string | null): boolean {
  if (blockingPid && blockingPid.trim().length > 0) {
    return true;
  }
  if (!waitEvent) {
    return false;
  }
  return /\block\b|lock:|metadata lock|waiting for .*lock|lck_m_/i.test(waitEvent);
}

/** 将后端原始查询行归一化并按「阻塞优先 → 活跃优先 → 耗时降序」排序 */
export function normalizeActiveSessionRows(
  rows: readonly Record<string, unknown>[],
  dbType: DatabaseType
): ActiveSessionRow[] {
  const normalized: ActiveSessionRow[] = [];
  for (const row of rows) {
    const sessionId = asString(readColumn(row, 'session_id'));
    if (!sessionId) {
      continue;
    }
    const state = asString(readColumn(row, 'state'));
    const waitEvent = asNullableString(readColumn(row, 'wait_event'));
    const blockingPid = asNullableString(readColumn(row, 'blocking_pid'));
    normalized.push({
      sessionId,
      username: asString(readColumn(row, 'username')),
      databaseName: asString(readColumn(row, 'database_name')),
      clientAddr: asString(readColumn(row, 'client_addr')),
      state,
      durationSeconds: asDurationSeconds(readColumn(row, 'duration_seconds')),
      waitEvent,
      blockingPid,
      sqlText: asString(readColumn(row, 'sql_text')),
      isCurrent: asBoolean(readColumn(row, 'is_current')),
      isActive: isActiveSessionState(state, dbType),
      isBlocked: isBlockedSession(blockingPid, waitEvent)
    });
  }

  return normalized.sort((left, right) => {
    if (left.isBlocked !== right.isBlocked) {
      return left.isBlocked ? -1 : 1;
    }
    if (left.isActive !== right.isActive) {
      return left.isActive ? -1 : 1;
    }
    if (left.durationSeconds !== right.durationSeconds) {
      return right.durationSeconds - left.durationSeconds;
    }
    return left.sessionId.localeCompare(right.sessionId, undefined, { numeric: true });
  });
}

export function summarizeActiveSessions(rows: readonly ActiveSessionRow[]): ActiveSessionsSummary {
  let active = 0;
  let blocked = 0;
  let maxDurationSeconds = 0;
  for (const row of rows) {
    if (row.isActive) active += 1;
    if (row.isBlocked) blocked += 1;
    if (row.durationSeconds > maxDurationSeconds) {
      maxDurationSeconds = row.durationSeconds;
    }
  }
  return { total: rows.length, active, blocked, maxDurationSeconds };
}

export function filterActiveSessions(
  rows: readonly ActiveSessionRow[],
  filter: ActiveSessionsFilter
): ActiveSessionRow[] {
  const keyword = filter.search.trim().toLowerCase();
  return rows.filter((row) => {
    if (filter.onlyActive && !row.isActive) return false;
    if (filter.onlyBlocked && !row.isBlocked) return false;
    if (filter.hideCurrent && row.isCurrent) return false;
    if (!keyword) return true;
    const haystack = [
      row.sessionId,
      row.username,
      row.databaseName,
      row.clientAddr,
      row.state,
      row.waitEvent ?? '',
      row.blockingPid ?? '',
      row.sqlText
    ].join('\n').toLowerCase();
    return haystack.includes(keyword);
  });
}

/** 将秒数格式化为紧凑易读的持续时长 */
export function formatSessionDuration(seconds: number): string {
  const clamped = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0));
  if (clamped < 60) {
    return `${clamped}s`;
  }
  const hours = Math.floor(clamped / 3600);
  const minutes = Math.floor((clamped % 3600) / 60);
  const secs = clamped % 60;
  if (hours === 0) {
    return `${minutes}m ${String(secs).padStart(2, '0')}s`;
  }
  return `${hours}h ${String(minutes).padStart(2, '0')}m`;
}

/** 生成用于表格单行展示的压缩 SQL 摘要（避免长查询撑破表格行高） */
export function truncateSqlPreview(sql: string, maxLength = 120): string {
  const singleLine = sql.replace(/\s+/g, ' ').trim();
  if (singleLine.length <= maxLength) {
    return singleLine;
  }
  return `${singleLine.slice(0, Math.max(1, maxLength - 1))}…`;
}

/**
 * 构造在二次确认弹窗里向用户展示的精确管理 SQL，并与后端白名单规则保持一致。
 * 校验不通过或方言不支持该动作时返回 `null`。
 */
export function buildSessionActionSql(
  dbType: DatabaseType,
  action: SessionAction,
  sessionId: string
): string | null {
  const trimmed = sessionId.trim();
  if (!trimmed) {
    return null;
  }
  switch (dbType) {
    case DatabaseType.MySQL:
      if (!/^[1-9]\d*$/.test(trimmed)) return null;
      return action === 'cancel' ? `KILL QUERY ${trimmed}` : `KILL CONNECTION ${trimmed}`;
    case DatabaseType.PostgreSQL:
      if (!/^[1-9]\d*$/.test(trimmed)) return null;
      return action === 'cancel'
        ? `SELECT pg_cancel_backend(${trimmed})`
        : `SELECT pg_terminate_backend(${trimmed})`;
    case DatabaseType.SqlServer:
      if (action !== 'terminate' || !/^[1-9]\d*$/.test(trimmed)) return null;
      return `KILL ${trimmed}`;
    case DatabaseType.Oracle:
      if (action !== 'terminate' || !/^[1-9]\d*,\d+$/.test(trimmed)) return null;
      return `ALTER SYSTEM KILL SESSION '${trimmed}' IMMEDIATE`;
    case DatabaseType.Dameng:
      if (action !== 'terminate' || !/^[1-9]\d*$/.test(trimmed)) return null;
      return `CALL SP_CLOSE_SESSION(${trimmed})`;
    case DatabaseType.ClickHouse:
      if (action !== 'cancel' || trimmed.length > 128 || !/^[A-Za-z0-9._:@/-]+$/.test(trimmed)) {
        return null;
      }
      return `KILL QUERY WHERE query_id = '${trimmed}' SYNC`;
    default:
      return null;
  }
}

/**
 * 判断主查询失败是否属于兼容库/旧版本缺少列或函数，值得切到 `fallback_sql` 重试一次。
 * 权限拒绝或断连类错误不盲目回退，避免掩盖真实原因。
 */
export function shouldFallbackSessionsQuery(errorMessage: string): boolean {
  return /42703|42883|unknown function|does not exist|no such column|undefined column|undefined function|unknown identifier/i
    .test(errorMessage);
}
