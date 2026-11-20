/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLanguageStore } from '../stores/languageStore';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const { DataProfileDialog } = await import('./DataProfileDialog');

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('DataProfileDialog', () => {
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

  it('loads result set, renders numeric histogram + NoSQL presence, and closes session on unmount', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'analytics_load_result') {
        return {
          analyticsId: 'sess-1',
          sourceKind: 'resultSet',
          rowCount: 3,
          columnCount: 2,
          truncated: false,
          columns: [
            { name: 'amount', duckdbType: 'DOUBLE', inferredType: 'numeric', precisionDegraded: false },
            { name: 'status', duckdbType: 'VARCHAR', inferredType: 'text', precisionDegraded: false }
          ]
        };
      }
      if (cmd === 'analytics_profile') {
        return {
          analyticsId: 'sess-1',
          rowCount: 3,
          columnCount: 2,
          elapsedMs: 12,
          columns: [
            {
              name: 'amount',
              inferredType: 'numeric',
              duckdbType: 'DOUBLE',
              precisionDegraded: false,
              totalCount: 3,
              nullCount: 0,
              nullRatio: 0,
              distinctCount: 3,
              distinctRatio: 1,
              min: '10',
              max: '30',
              mean: 20,
              stddev: 10,
              p25: 15,
              p50: 20,
              p75: 25,
              p95: 29,
              bins: [
                { lower: 10, upper: 20, count: 1 },
                { lower: 20, upper: 30, count: 2 }
              ],
              topValues: []
            },
            {
              name: 'status',
              inferredType: 'text',
              duckdbType: 'VARCHAR',
              precisionDegraded: false,
              totalCount: 3,
              nullCount: 1,
              nullRatio: 1 / 3,
              distinctCount: 1,
              distinctRatio: 0.5,
              presentCount: 2,
              presentRatio: 2 / 3,
              explicitNullCount: 0,
              minLength: 4,
              maxLength: 4,
              bins: [],
              topValues: [{ value: 'PAID', count: 2, ratio: 1 }]
            }
          ]
        };
      }
      return true;
    });

    await act(async () => {
      root.render(
        <DataProfileDialog
          source={{
            kind: 'resultSet',
            columns: [{ name: 'amount' }, { name: 'status' }],
            rows: [
              [10, 'PAID'],
              [20, 'PAID'],
              [30, null]
            ]
          }}
          onClose={vi.fn()}
        />
      );
    });
    await flush();

    expect(container.textContent).toContain('amount');
    expect(container.textContent).toContain('status');
    expect(container.textContent).toContain('PAID');
    expect(container.textContent).toContain('Schema 漂移');
    expect(container.textContent).toContain('唯一键');
    expect(container.textContent).toContain('分组');
    expect(container.textContent).toContain('点击复制 SQL');

    await act(async () => {
      root.unmount();
    });
    expect(invoke).toHaveBeenCalledWith('analytics_close', { analyticsId: 'sess-1' });
  });

  it('allows cancelling an in-flight profile calculation', async () => {
    let rejectProfile: ((reason?: unknown) => void) | null = null;
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'analytics_load_json_docs') {
        return Promise.resolve({
          analyticsId: 'sess-cancel',
          sourceKind: 'jsonDocuments',
          rowCount: 100,
          columnCount: 1,
          truncated: false,
          columns: []
        });
      }
      if (cmd === 'analytics_profile') {
        return new Promise((_, reject) => {
          rejectProfile = reject;
        });
      }
      return Promise.resolve(true);
    });

    await act(async () => {
      root.render(
        <DataProfileDialog
          source={{ kind: 'jsonDocuments', documents: [{ a: 1 }] }}
          onClose={vi.fn()}
        />
      );
    });
    await flush();

    const cancelBtn = Array.from(container.querySelectorAll('button')).find(
      (btn) => btn.textContent?.includes('取消分析')
    );
    expect(cancelBtn).toBeTruthy();

    await act(async () => {
      cancelBtn?.click();
    });
    expect(invoke).toHaveBeenCalledWith('analytics_cancel', { analyticsId: 'sess-cancel' });

    await act(async () => {
      rejectProfile?.({ code: 'DATAOMNI_ANALYTICS_CANCELLED', message: 'DATAOMNI_ANALYTICS_CANCELLED' });
    });
    await flush();
    expect(container.textContent).toContain('分析已取消。');
  });

  it('closes on Escape key when not composing IME', async () => {
    const onClose = vi.fn();
    await act(async () => {
      root.render(
        <DataProfileDialog
          source={{ kind: 'resultSet', columns: [], rows: [] }}
          onClose={onClose}
        />
      );
    });

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', isComposing: true }));
    });
    expect(onClose).not.toHaveBeenCalled();

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
