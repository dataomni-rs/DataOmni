/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLanguageStore } from '../stores/languageStore';
import { MAX_CHART_POINTS } from '../utils/resultChart';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const invoke = vi.fn();
const save = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: (...args: unknown[]) => save(...args) }));

const { ResultChartDialog } = await import('./ResultChartDialog');

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('ResultChartDialog', () => {
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

  it('switches across stacked-bar, area, scatter, heatmap and exports SVG with background rect', async () => {
    save.mockResolvedValue('/tmp/out.svg');
    invoke.mockResolvedValue(undefined);

    await act(async () => {
      root.render(
        <ResultChartDialog
          columns={['region', 'orders', 'revenue']}
          rows={[
            ['North', 10, 100],
            ['South', 20, 250]
          ]}
          onClose={vi.fn()}
        />
      );
    });

    for (const label of ['堆叠柱图', '面积图', '散点图', '热力图']) {
      const btn = Array.from(container.querySelectorAll('button')).find(
        (b) => b.textContent?.trim() === label
      );
      expect(btn, `missing ${label}`).toBeTruthy();
      await act(async () => {
        btn?.click();
      });
      expect(container.querySelector('svg')).toBeTruthy();
    }

    const exportSvgBtn = Array.from(container.querySelectorAll('button')).find(
      (b) => b.textContent?.includes('导出 SVG')
    );
    await act(async () => {
      exportSvgBtn?.click();
    });
    await flush();

    expect(invoke).toHaveBeenCalledWith(
      'write_text_file',
      expect.objectContaining({
        path: '/tmp/out.svg',
        contents: expect.stringContaining('<rect width="100%" height="100%" fill=')
      })
    );
  });

  it('offers explicit DuckDB aggregation when rows exceed MAX_CHART_POINTS and closes temp session', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'analytics_load_result') {
        return {
          analyticsId: 'chart-agg-1',
          sourceKind: 'resultSet',
          rowCount: MAX_CHART_POINTS + 5,
          columnCount: 2,
          truncated: false,
          columns: []
        };
      }
      if (cmd === 'analytics_query') {
        return {
          columns: ['region', 'sum_amount'],
          columnMetadata: [],
          rows: [
            ['North', 1200],
            ['South', 800]
          ],
          rowCount: 2,
          truncated: false,
          truncationReason: null,
          elapsedMs: 3
        };
      }
      return true;
    });

    const bigRows = Array.from({ length: MAX_CHART_POINTS + 5 }, (_, i) => [
      i % 2 === 0 ? 'North' : 'South',
      i + 1
    ]);

    await act(async () => {
      root.render(
        <ResultChartDialog
          columns={['region', 'amount']}
          rows={bigRows}
          onClose={vi.fn()}
        />
      );
    });

    expect(container.textContent).toContain('使用内置内存 DuckDB 显式聚合');
    const aggBtn = Array.from(container.querySelectorAll('button')).find(
      (b) => b.textContent?.includes('聚合画图')
    );
    expect(aggBtn).toBeTruthy();

    await act(async () => {
      aggBtn?.click();
    });
    await flush();

    expect(invoke).toHaveBeenCalledWith('analytics_load_result', expect.anything());
    expect(invoke).toHaveBeenCalledWith(
      'analytics_query',
      expect.objectContaining({ analyticsId: 'chart-agg-1' })
    );
    expect(invoke).toHaveBeenCalledWith('analytics_close', { analyticsId: 'chart-agg-1' });
    expect(container.textContent).toContain('SUM(amount) BY region');
    expect(container.querySelector('svg')).toBeTruthy();
  });

  it('offers COUNT DuckDB aggregation when result has no numeric columns', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'analytics_load_result') {
        return {
          analyticsId: 'chart-count-1',
          sourceKind: 'resultSet',
          rowCount: 3,
          columnCount: 2,
          truncated: false,
          columns: []
        };
      }
      if (cmd === 'analytics_query') {
        return {
          columns: ['global_skill_id', 'row_count'],
          columnMetadata: [],
          rows: [
            ['skill-a', 2],
            ['skill-b', 1]
          ],
          rowCount: 2,
          truncated: false,
          truncationReason: null,
          elapsedMs: 2
        };
      }
      return true;
    });

    await act(async () => {
      root.render(
        <ResultChartDialog
          columns={['global_skill_id', 'updated_at']}
          rows={[
            ['skill-a', '2026-10-26 14:15:35'],
            ['skill-a', '2026-10-26 14:15:45'],
            ['skill-b', '2026-11-03 15:36:33']
          ]}
          onClose={vi.fn()}
        />
      );
    });

    expect(container.textContent).toContain('使用内置内存 DuckDB 显式聚合');
    const selects = Array.from(container.querySelectorAll('select'));
    const fnSelect = selects[1];
    expect(fnSelect?.value).toBe('COUNT');
    expect(Array.from(fnSelect?.options ?? []).map((o) => o.value)).toEqual(['COUNT']);

    const aggBtn = Array.from(container.querySelectorAll('button')).find(
      (b) => b.textContent?.includes('聚合画图')
    );
    expect(aggBtn).toBeTruthy();
    expect(aggBtn?.disabled).toBe(false);

    await act(async () => {
      aggBtn?.click();
    });
    await flush();

    expect(invoke).toHaveBeenCalledWith(
      'analytics_query',
      expect.objectContaining({
        analyticsId: 'chart-count-1',
        sql: expect.stringContaining('COUNT(*)')
      })
    );
    expect(invoke).toHaveBeenCalledWith('analytics_close', { analyticsId: 'chart-count-1' });
    expect(container.textContent).toContain('COUNT(*) BY global_skill_id');
    expect(container.querySelector('svg')).toBeTruthy();
  });
});
