import { describe, expect, it } from 'vitest';
import { DatabaseType } from '../contracts/connection';
import {
  buildSessionActionSql,
  filterActiveSessions,
  formatSessionDuration,
  normalizeActiveSessionRows,
  shouldFallbackSessionsQuery,
  summarizeActiveSessions,
  supportsActiveSessions,
  truncateSqlPreview
} from './activeSessions';

describe('activeSessions', () => {
  it('supports only server relational dialects', () => {
    expect(supportsActiveSessions(DatabaseType.MySQL)).toBe(true);
    expect(supportsActiveSessions(DatabaseType.PostgreSQL)).toBe(true);
    expect(supportsActiveSessions(DatabaseType.SqlServer)).toBe(true);
    expect(supportsActiveSessions(DatabaseType.Oracle)).toBe(true);
    expect(supportsActiveSessions(DatabaseType.ClickHouse)).toBe(true);

    expect(supportsActiveSessions(DatabaseType.SQLite)).toBe(false);
    expect(supportsActiveSessions(DatabaseType.DuckDB)).toBe(false);
    expect(supportsActiveSessions(DatabaseType.MongoDB)).toBe(false);
    expect(supportsActiveSessions(DatabaseType.Redis)).toBe(false);
    expect(supportsActiveSessions(DatabaseType.Neo4j)).toBe(false);
    expect(supportsActiveSessions(DatabaseType.Elasticsearch)).toBe(false);
  });

  it('normalizes heterogeneous driver rows and prioritizes blocked then active then duration', () => {
    const rows = normalizeActiveSessionRows([
      {
        session_id: 101,
        username: 'app',
        database_name: 'prod',
        client_addr: '10.0.0.5',
        state: 'idle',
        duration_seconds: 900,
        wait_event: null,
        blocking_pid: null,
        sql_text: 'SELECT 1',
        is_current: false
      },
      {
        session_id: '102',
        username: 'worker',
        database_name: 'prod',
        client_addr: '10.0.0.6',
        state: 'active',
        duration_seconds: '45',
        wait_event: 'Lock:relation',
        blocking_pid: '103',
        sql_text: 'UPDATE orders SET status = 1 WHERE id = 9',
        is_current: 0
      },
      {
        session_id: { value: '103' },
        username: 'admin',
        database_name: 'prod',
        client_addr: '10.0.0.1',
        state: 'idle in transaction',
        duration_seconds: { value: 120 },
        wait_event: null,
        blocking_pid: '',
        sql_text: 'BEGIN; UPDATE orders SET status = 2 WHERE id = 9',
        is_current: '1'
      },
      {
        // 空 session_id 应被跳过
        session_id: '',
        username: 'ghost',
        state: 'active'
      }
    ], DatabaseType.PostgreSQL);

    expect(rows.map((row) => row.sessionId)).toEqual(['102', '103', '101']);
    expect(rows[0].isBlocked).toBe(true);
    expect(rows[0].isActive).toBe(true);
    expect(rows[1].isCurrent).toBe(true);
    expect(rows[1].isActive).toBe(true);
    expect(rows[2].isActive).toBe(false);

    const summary = summarizeActiveSessions(rows);
    expect(summary).toEqual({
      total: 3,
      active: 2,
      blocked: 1,
      maxDurationSeconds: 900
    });
  });

  it('detects MySQL metadata lock waits even when blocking_pid is null', () => {
    const rows = normalizeActiveSessionRows([
      {
        SESSION_ID: '88',
        USERNAME: 'root',
        DATABASE_NAME: 'shop',
        CLIENT_ADDR: '127.0.0.1:50010',
        STATE: 'Query',
        DURATION_SECONDS: 18,
        WAIT_EVENT: 'Waiting for table metadata lock',
        BLOCKING_PID: null,
        SQL_TEXT: 'ALTER TABLE orders ADD COLUMN note TEXT',
        IS_CURRENT: 0
      },
      {
        SESSION_ID: '89',
        USERNAME: 'root',
        DATABASE_NAME: 'shop',
        CLIENT_ADDR: '127.0.0.1:50011',
        STATE: 'Sleep',
        DURATION_SECONDS: 400,
        WAIT_EVENT: null,
        BLOCKING_PID: null,
        SQL_TEXT: '',
        IS_CURRENT: 1
      }
    ], DatabaseType.MySQL);

    expect(rows[0].sessionId).toBe('88');
    expect(rows[0].isBlocked).toBe(true);
    expect(rows[0].isActive).toBe(true);
    expect(rows[1].isActive).toBe(false);
    expect(rows[1].isCurrent).toBe(true);
  });

  it('filters by search keyword, active-only, blocked-only, and hide-current flags', () => {
    const rows = normalizeActiveSessionRows([
      {
        session_id: '10',
        username: 'alice',
        database_name: 'billing',
        client_addr: '10.0.0.1',
        state: 'active',
        duration_seconds: 12,
        wait_event: 'Lock:tuple',
        blocking_pid: '11',
        sql_text: 'UPDATE invoices SET paid = true',
        is_current: false
      },
      {
        session_id: '11',
        username: 'bob',
        database_name: 'analytics',
        client_addr: '10.0.0.2',
        state: 'active',
        duration_seconds: 3,
        wait_event: null,
        blocking_pid: null,
        sql_text: 'SELECT count(*) FROM events',
        is_current: true
      }
    ], DatabaseType.PostgreSQL);

    expect(filterActiveSessions(rows, {
      search: 'invoices',
      onlyActive: false,
      onlyBlocked: false,
      hideCurrent: false
    }).map((r) => r.sessionId)).toEqual(['10']);

    expect(filterActiveSessions(rows, {
      search: '',
      onlyActive: true,
      onlyBlocked: true,
      hideCurrent: false
    }).map((r) => r.sessionId)).toEqual(['10']);

    expect(filterActiveSessions(rows, {
      search: '',
      onlyActive: false,
      onlyBlocked: false,
      hideCurrent: true
    }).map((r) => r.sessionId)).toEqual(['10']);
  });

  it('builds safe action statements and rejects malformed or injected session IDs', () => {
    expect(buildSessionActionSql(DatabaseType.MySQL, 'cancel', '42')).toBe('KILL QUERY 42');
    expect(buildSessionActionSql(DatabaseType.MySQL, 'terminate', '42')).toBe('KILL CONNECTION 42');
    expect(buildSessionActionSql(DatabaseType.MySQL, 'terminate', '42; DROP TABLE t')).toBeNull();
    expect(buildSessionActionSql(DatabaseType.MySQL, 'cancel', '0')).toBeNull();

    expect(buildSessionActionSql(DatabaseType.PostgreSQL, 'cancel', '1234'))
      .toBe('SELECT pg_cancel_backend(1234)');
    expect(buildSessionActionSql(DatabaseType.PostgreSQL, 'terminate', '1234'))
      .toBe('SELECT pg_terminate_backend(1234)');

    expect(buildSessionActionSql(DatabaseType.SqlServer, 'terminate', '55')).toBe('KILL 55');
    expect(buildSessionActionSql(DatabaseType.SqlServer, 'cancel', '55')).toBeNull();

    expect(buildSessionActionSql(DatabaseType.Oracle, 'terminate', '128,4567'))
      .toBe("ALTER SYSTEM KILL SESSION '128,4567' IMMEDIATE");
    expect(buildSessionActionSql(DatabaseType.Oracle, 'cancel', '128,4567')).toBeNull();
    expect(buildSessionActionSql(DatabaseType.Oracle, 'terminate', "128,1'; DROP USER x")).toBeNull();

    expect(buildSessionActionSql(DatabaseType.ClickHouse, 'cancel', 'q-123_abc:1'))
      .toBe("KILL QUERY WHERE query_id = 'q-123_abc:1' SYNC");
    expect(buildSessionActionSql(DatabaseType.ClickHouse, 'terminate', 'q-123')).toBeNull();
    expect(buildSessionActionSql(DatabaseType.ClickHouse, 'cancel', "q' OR '1'='1")).toBeNull();

    expect(buildSessionActionSql(DatabaseType.SQLite, 'terminate', '1')).toBeNull();
  });

  it('formats durations, truncates SQL previews, and classifies fallback-eligible errors', () => {
    expect(formatSessionDuration(0)).toBe('0s');
    expect(formatSessionDuration(59)).toBe('59s');
    expect(formatSessionDuration(65)).toBe('1m 05s');
    expect(formatSessionDuration(3661)).toBe('1h 01m');

    expect(truncateSqlPreview('SELECT   a,\n  b  FROM t', 100)).toBe('SELECT a, b FROM t');
    expect(truncateSqlPreview('SELECT 1234567890', 10)).toBe('SELECT 12…');

    expect(shouldFallbackSessionsQuery('ERROR: function pg_blocking_pids(integer) does not exist (42883)')).toBe(true);
    expect(shouldFallbackSessionsQuery('column "backend_type" does not exist (42703)')).toBe(true);
    expect(shouldFallbackSessionsQuery('Code: 46. DB::Exception: Unknown function queryID')).toBe(true);
    expect(shouldFallbackSessionsQuery('permission denied for view pg_stat_activity')).toBe(false);
  });
});
