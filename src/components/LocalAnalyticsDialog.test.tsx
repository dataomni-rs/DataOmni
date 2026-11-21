/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLanguageStore } from '../stores/languageStore';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const invoke = vi.fn();
const save = vi.fn();
const open = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: (...args: unknown[]) => save(...args),
  open: (...args: unknown[]) => open(...args)
}));

const { LocalAnalyticsDialog } = await import('./LocalAnalyticsDialog');

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('LocalAnalyticsDialog', () => {
  beforeEach(() => {
    useLanguageStore.getState().setPreference('zh');
    invoke.mockReset();
    save.mockReset();
    open.mockReset();
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

  it('opens local parquet file, executes preview & GROUP BY template, exports parquet, and closes on unmount', async () => {
    invoke.mockImplementation(async (cmd: string, args: Record<string, unknown>) => {
      if (cmd === 'analytics_open_file') {
        return {
          analyticsId: 'file-sess-1',
          sourceKind: 'localFile',
          fileKind: 'parquet',
          rowCount: 2,
          columnCount: 2,
          truncated: false,
          columns: [
            { name: 'region', duckdbType: 'VARCHAR', inferredType: 'text', precisionDegraded: false },
            { name: 'amount', duckdbType: 'DOUBLE', inferredType: 'numeric', precisionDegraded: false }
          ]
        };
      }
      if (cmd === 'analytics_query') {
        if (String(args.sql).includes('GROUP BY')) {
          return {
            columns: ['region', 'row_count', 'total_value', 'avg_value'],
            columnMetadata: [],
            rows: [['North', 2, 300, 150]],
            rowCount: 1,
            truncated: false,
            truncationReason: null,
            elapsedMs: 4
          };
        }
        return {
          columns: ['region', 'amount'],
          columnMetadata: [],
          rows: [
            ['North', 100],
            ['North', 200]
          ],
          rowCount: 2,
          truncated: false,
          truncationReason: null,
          elapsedMs: 3
        };
      }
      if (cmd === 'analytics_export_parquet') {
        return { rowCount: 1, bytes: 2048 };
      }
      return true;
    });
    save.mockResolvedValue('/tmp/out.parquet');

    await act(async () => {
      root.render(
        <LocalAnalyticsDialog
          source={{ kind: 'localFile', filePath: '/data/orders.parquet' }}
          onClose={vi.fn()}
        />
      );
    });
    await flush();

    expect(container.textContent).toContain('orders.parquet');
    expect(container.textContent).toContain('North');
    // CodeMirror SQL 编辑器已挂载且支持列 chip 光标插入
    expect(container.querySelector('.cm-editor')).not.toBeNull();
    expect(container.querySelector('textarea')).toBeNull();
    const regionChip = Array.from(container.querySelectorAll('button')).find(
      (btn) => btn.textContent === 'region'
    );
    expect(regionChip).toBeTruthy();
    await act(async () => {
      regionChip?.click();
    });
    expect(container.querySelector('.cm-content')?.textContent).toContain('"region"');

    // 点击 GROUP BY 快捷模板
    const groupBtn = Array.from(container.querySelectorAll('button')).find(
      (btn) => btn.textContent?.includes('GROUP BY')
    );
    expect(groupBtn).toBeTruthy();
    await act(async () => {
      groupBtn?.click();
    });
    await flush();
    expect(container.textContent).toContain('300');

    // 点击导出 Parquet
    const exportBtn = Array.from(container.querySelectorAll('button')).find(
      (btn) => btn.textContent?.includes('导出 Parquet')
    );
    await act(async () => {
      exportBtn?.click();
    });
    await flush();
    expect(invoke).toHaveBeenCalledWith(
      'analytics_export_parquet',
      expect.objectContaining({
        analyticsId: 'file-sess-1',
        outputPath: '/tmp/out.parquet'
      })
    );
    expect(container.textContent).toContain('2.0 KB');

    // 卸载时销毁会话
    await act(async () => {
      root.unmount();
    });
    expect(invoke).toHaveBeenCalledWith('analytics_close', { analyticsId: 'file-sess-1' });
  });

  it('attaches a local CSV reconciliation table and runs LEFT JOIN template', async () => {
    open.mockResolvedValue('/tmp/ops_list.csv');
    invoke.mockImplementation(async (cmd: string, args: Record<string, unknown>) => {
      if (cmd === 'analytics_load_result') {
        return {
          analyticsId: 'sess-join',
          sourceKind: 'resultSet',
          rowCount: 2,
          columnCount: 2,
          truncated: false,
          columns: [
            { name: 'order_id', duckdbType: 'BIGINT', inferredType: 'integer', precisionDegraded: false },
            { name: 'amount', duckdbType: 'DOUBLE', inferredType: 'numeric', precisionDegraded: false }
          ]
        };
      }
      if (cmd === 'analytics_attach_file') {
        return {
          tableName: 'ref_1',
          fileKind: 'csv',
          rowCount: 3,
          columnCount: 2,
          truncated: false,
          columns: [
            { name: 'order_id', duckdbType: 'BIGINT', inferredType: 'integer', precisionDegraded: false },
            { name: 'flag', duckdbType: 'VARCHAR', inferredType: 'text', precisionDegraded: false }
          ]
        };
      }
      if (cmd === 'analytics_query') {
        if (String(args.sql).includes('LEFT JOIN')) {
          return {
            columns: ['order_id', 'amount', 'flag'],
            columnMetadata: [],
            rows: [[1, 100, 'vip']],
            rowCount: 1,
            truncated: false,
            truncationReason: null,
            elapsedMs: 2
          };
        }
        return {
          columns: ['order_id', 'amount'],
          columnMetadata: [],
          rows: [[1, 100]],
          rowCount: 1,
          truncated: false,
          truncationReason: null,
          elapsedMs: 1
        };
      }
      return true;
    });

    await act(async () => {
      root.render(
        <LocalAnalyticsDialog
          source={{
            kind: 'resultSet',
            title: 'orders',
            columns: [{ name: 'order_id' }, { name: 'amount' }],
            rows: [[1, 100]]
          }}
          onClose={vi.fn()}
        />
      );
    });
    await flush();

    const attachBtn = Array.from(container.querySelectorAll('button')).find(
      (btn) => btn.textContent?.includes('附加文件对账')
    );
    expect(attachBtn).toBeTruthy();
    await act(async () => {
      attachBtn?.click();
    });
    await flush();

    expect(invoke).toHaveBeenCalledWith('analytics_attach_file', {
      analyticsId: 'sess-join',
      tableName: 'ref_1',
      path: '/tmp/ops_list.csv'
    });
    expect(container.textContent).toContain('ref_1');
    expect(container.textContent).toContain('ops_list.csv');
    // 附加表的列名芯片也应直接展示
    expect(container.textContent).toContain('flag');

    const joinBtn = Array.from(container.querySelectorAll('button')).find(
      (btn) => btn.textContent?.includes('LEFT JOIN 对账')
    );
    expect(joinBtn).toBeTruthy();
    await act(async () => {
      joinBtn?.click();
    });
    await flush();

    expect(container.textContent).toContain('vip');
  });

  it('supports table-level sampling toggle in workbench header when tableSample is provided', async () => {
    const fetchSample = vi.fn().mockResolvedValue({
      columns: [{ name: 'order_id' }],
      rows: [[1], [2], [3]],
      truncated: false
    });
    invoke.mockImplementation(async (cmd: string, args: Record<string, unknown>) => {
      if (cmd === 'analytics_load_result') {
        const rows = (args.rows as unknown[]) ?? [];
        return {
          analyticsId: `sess-${rows.length}`,
          sourceKind: 'resultSet',
          rowCount: rows.length,
          columnCount: 1,
          truncated: false,
          columns: [
            { name: 'order_id', duckdbType: 'BIGINT', inferredType: 'integer', precisionDegraded: false }
          ]
        };
      }
      if (cmd === 'analytics_query') {
        return {
          columns: ['order_id'],
          columnMetadata: [],
          rows: [[1]],
          rowCount: 1,
          truncated: false,
          truncationReason: null,
          elapsedMs: 1
        };
      }
      return true;
    });

    await act(async () => {
      root.render(
        <LocalAnalyticsDialog
          source={{
            kind: 'resultSet',
            title: 'orders',
            columns: [{ name: 'order_id' }],
            rows: [[1]]
          }}
          tableSample={{
            hasActiveFilters: false,
            fetchSample,
            cancelSample: vi.fn()
          }}
          sourceDialect="sqlite"
          onApplyTableFilter={vi.fn()}
          onClose={vi.fn()}
        />
      );
    });
    await flush();

    const sampleBtn = Array.from(container.querySelectorAll('button')).find((btn) =>
      btn.textContent?.includes('表抽样')
    );
    expect(sampleBtn).toBeTruthy();
    await act(async () => {
      sampleBtn?.click();
    });
    await flush();

    expect(fetchSample).toHaveBeenCalledWith(5000, expect.any(String));
    expect(container.textContent).toContain('3 × 1');
  });
});
