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
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: (...args: unknown[]) => save(...args) }));

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
});
