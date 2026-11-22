/**
 * @vitest-environment happy-dom
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DatabaseType, type ConnectionProfile } from '../contracts';
import type { DatabaseHandle } from '../utils/databaseHandle';
import { useLanguageStore } from '../stores/languageStore';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const { ActiveSessionsDialog } = await import('./ActiveSessionsDialog');

const PROD_PG_CONNECTION: ConnectionProfile = {
  id: 'conn-pg-prod',
  name: 'Orders Prod',
  db_type: DatabaseType.PostgreSQL,
  host: '10.0.0.10',
  port: 5432,
  database: 'orders',
  username: 'dba',
  password: '',
  ssl: true,
  save_password: true,
  options: {},
  tags: [],
  created_at: '2026-11-22T00:00:00Z',
  updated_at: '2026-11-22T00:00:00Z',
  environment: 'production'
};

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('ActiveSessionsDialog', () => {
  beforeEach(() => {
    useLanguageStore.getState().setPreference('zh');
    invoke.mockReset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => {
      root.unmount();
    });
    container.remove();
  });

  it('never registers background polling intervals', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/components/ActiveSessionsDialog.tsx'),
      'utf8'
    );
    expect(source).not.toMatch(/\bsetInterval\s*\(/);
  });

  it('renders sessions, blocks self-termination, and requires two-step confirmation before terminating', async () => {
    const select = vi.fn().mockResolvedValue([
      {
        session_id: '201',
        username: 'worker',
        database_name: 'orders',
        client_addr: '10.0.0.21',
        state: 'active',
        duration_seconds: 95,
        wait_event: 'Lock:relation',
        blocking_pid: '202',
        sql_text: 'UPDATE orders SET status = 1 WHERE id = 42',
        is_current: false
      },
      {
        session_id: '202',
        username: 'dba',
        database_name: 'orders',
        client_addr: '10.0.0.1',
        state: 'active',
        duration_seconds: 1,
        wait_event: null,
        blocking_pid: null,
        sql_text: 'SELECT * FROM pg_stat_activity',
        is_current: true
      }
    ]);
    const database: DatabaseHandle = {
      select,
      close: vi.fn().mockResolvedValue(true)
    };

    invoke.mockImplementation(async (command: string) => {
      if (command === 'get_active_sessions_query') {
        return {
          sql: 'SELECT primary',
          fallback_sql: 'SELECT fallback',
          supports_cancel: true,
          supports_terminate: true
        };
      }
      if (command === 'terminate_database_session') {
        return {
          applied: true,
          statement: 'SELECT pg_terminate_backend(201)'
        };
      }
      throw new Error(`unexpected command ${command}`);
    });

    await act(async () => {
      root.render(
        <ActiveSessionsDialog
          connection={PROD_PG_CONNECTION}
          database={database}
          onClose={vi.fn()}
        />
      );
    });
    await flush();

    expect(container.textContent).toContain('Orders Prod');
    expect(container.textContent).toContain('生产');
    expect(container.textContent).toContain('被 #202 阻塞');
    expect(container.textContent).toContain('当前');
    expect(container.textContent).toContain('1m 35s');

    // Only one terminate button should render (current session 202 has none)
    const terminateButtons = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).filter(
      (btn) => btn.textContent?.includes('断开会话')
    );
    expect(terminateButtons).toHaveLength(1);

    // Click terminate: opens confirmation prompt without invoking terminate yet
    await act(async () => {
      terminateButtons[0].click();
    });
    await flush();

    expect(invoke).not.toHaveBeenCalledWith('terminate_database_session', expect.anything());
    expect(container.textContent).toContain('SELECT pg_terminate_backend(201)');

    // Confirm inside DestructiveStatementPrompt
    const confirmButton = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).find(
      (btn) => btn.className.includes('bg-danger-solid')
    );
    expect(confirmButton).toBeTruthy();

    await act(async () => {
      confirmButton?.click();
    });
    await flush();

    expect(invoke).toHaveBeenCalledWith('terminate_database_session', {
      request: {
        connectionId: 'conn-pg-prod',
        action: 'terminate',
        sessionId: '201'
      }
    });
    // Refreshes once manually after termination
    expect(select).toHaveBeenCalledTimes(2);
    expect(container.textContent).toContain('已执行：SELECT pg_terminate_backend(201)');
  });

  it('automatically falls back to basic query when server lacks pg_blocking_pids', async () => {
    const select = vi.fn()
      .mockRejectedValueOnce(new Error('function pg_blocking_pids(integer) does not exist (42883)'))
      .mockResolvedValueOnce([
        {
          session_id: '500',
          username: 'sm3_user',
          database_name: 'dataomni_test',
          client_addr: '127.0.0.1',
          state: 'active',
          duration_seconds: 2,
          wait_event: null,
          blocking_pid: null,
          sql_text: 'SELECT 1',
          is_current: true
        }
      ]);
    const database: DatabaseHandle = {
      select,
      close: vi.fn().mockResolvedValue(true)
    };

    invoke.mockResolvedValue({
      sql: 'SELECT primary_with_blocking_pids',
      fallback_sql: 'SELECT basic_pg_stat_activity',
      supports_cancel: true,
      supports_terminate: true
    });

    await act(async () => {
      root.render(
        <ActiveSessionsDialog
          connection={PROD_PG_CONNECTION}
          database={database}
          onClose={vi.fn()}
        />
      );
    });
    await flush();

    expect(select).toHaveBeenNthCalledWith(1, 'SELECT primary_with_blocking_pids');
    expect(select).toHaveBeenNthCalledWith(2, 'SELECT basic_pg_stat_activity');
    expect(container.textContent).toContain('sm3_user');
    expect(container.textContent).toContain('已自动回退到基础会话视图');
  });
});
