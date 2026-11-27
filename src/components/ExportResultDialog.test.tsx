/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useLanguageStore } from '../stores/languageStore';
import { useTaskStore } from '../stores/taskStore';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const save = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: (...args: unknown[]) => save(...args) }));

const { ExportResultDialog } = await import('./ExportResultDialog');

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  useLanguageStore.getState().setPreference('en');
  invoke.mockReset();
  invoke.mockResolvedValue(42);
  save.mockReset();
  save.mockResolvedValue('/tmp/out.csv');
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

function buttonByText(text: string): HTMLButtonElement {
  const found = [...container.querySelectorAll<HTMLButtonElement>('button')]
    .find((button) => button.textContent?.trim() === text);
  if (!found) {
    throw new Error(`找不到按钮「${text}」`);
  }
  return found;
}

describe('ExportResultDialog（单列与选列、去重）', () => {
  const columns = ['id', 'user_id', 'claw_id'];
  const rows = [
    [1, 'u1', 100],
    [2, 'u2', 200],
    [3, 'u1', 300]
  ];

  it('单列预选时预览只含该列，默认文件名带上列名，开启去重后只写出不重复行', async () => {
    await act(async () => {
      root.render(
        <ExportResultDialog
          columns={columns}
          rows={rows}
          sourceName="claw_user"
          initialColumnIndexes={[1]}
          onClose={() => {}}
        />
      );
    });

    expect(container.textContent).toContain('user_id only (1 of 3)');
    expect(container.textContent).toContain('Rows: 3 · columns: 1');
    expect(container.querySelector('pre')?.textContent).toBe('user_id\nu1\nu2\nu1');

    // 勾选去重后，预览与行列数立即更新为 2 行
    const distinctCheckbox = [...container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')]
      .find((input) => input.closest('label')?.textContent?.includes('DISTINCT'));
    if (!distinctCheckbox) throw new Error('没有去重复选框');
    await act(async () => {
      distinctCheckbox.click();
    });
    expect(container.textContent).toContain('Rows: 2 · columns: 1');
    expect(container.querySelector('pre')?.textContent).toBe('user_id\nu1\nu2');

    // 点击导出：建议文件名带列名，写盘内容为去重后的单列 CSV
    await act(async () => {
      buttonByText('Choose a location and export').click();
    });
    expect(save).toHaveBeenCalledWith(expect.objectContaining({
      defaultPath: expect.stringMatching(/^claw_user-user_id-\d{8}-\d{6}\.csv$/)
    }));
    expect(invoke).toHaveBeenCalledWith('write_text_file', {
      path: '/tmp/out.csv',
      contents: 'user_id\nu1\nu2'
    });
  });

  it('展开列选择器可一键「Only」切到单列，并联动 buildSql 发起整表流式导出', async () => {
    const startTask = vi.fn();
    const originalStart = useTaskStore.getState().start;
    useTaskStore.setState({ start: startTask });

    const buildSql = vi.fn((selected: readonly string[], opts: { distinct: boolean }) => (
      `SELECT ${opts.distinct ? 'DISTINCT ' : ''}${selected.join(', ')} FROM claw_user`
    ));

    try {
      await act(async () => {
        root.render(
          <ExportResultDialog
            columns={columns}
            rows={rows}
            sourceName="claw_user"
            connectionId="c1"
            scopes={[
              { id: 'page', label: 'Current page' },
              { id: 'table', label: 'Whole table', buildSql }
            ]}
            onClose={() => {}}
          />
        );
      });

      // 展开列选择器并点 claw_id 旁的 Only
      const toggle = container.querySelector<HTMLButtonElement>('button[aria-label="Toggle column picker"]');
      if (!toggle) throw new Error('没有列选择开关');
      await act(async () => {
        toggle.click();
      });

      const onlyButtons = [...container.querySelectorAll<HTMLButtonElement>('button')]
        .filter((button) => button.textContent?.trim() === 'Only');
      expect(onlyButtons).toHaveLength(3);
      await act(async () => {
        onlyButtons[2].click();
      });
      expect(container.textContent).toContain('claw_id only (1 of 3)');

      // 切到整张表 + 开启去重，验证动态流式 SQL 只投影 claw_id 且带 DISTINCT
      await act(async () => {
        buttonByText('Whole table').click();
      });
      const distinctCheckbox = [...container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')]
        .find((input) => input.closest('label')?.textContent?.includes('DISTINCT'));
      await act(async () => {
        distinctCheckbox?.click();
      });
      expect(container.textContent).toContain('Deduplication is on');

      await act(async () => {
        buttonByText('Choose a location and export').click();
      });
      expect(startTask).toHaveBeenCalledWith(expect.objectContaining({
        kind: 'export',
        payload: expect.objectContaining({
          connectionId: 'c1',
          sql: 'SELECT DISTINCT claw_id FROM claw_user',
          path: '/tmp/out.csv'
        })
      }));
    } finally {
      useTaskStore.setState({ start: originalStart });
    }
  });
});
