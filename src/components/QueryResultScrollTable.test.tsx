/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { QueryResult } from '../contracts/query';
import { useLanguageStore } from '../stores/languageStore';
import { useQueryStore } from '../stores/queryStore';
import { useWorkspaceStore } from '../stores/workspaceStore';
import { workspaceTabId } from '../contracts/workspace';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), Channel: class {} }));

const { QueryResultScrollTable } = await import('./QueryResultScrollTable');

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  useLanguageStore.getState().setPreference('en');
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

async function render(result: QueryResult) {
  await act(async () => {
    root.render(
      <QueryResultScrollTable result={result} statementId="s1" formatExecutionTime={(ms) => `${ms}ms`} />
    );
  });
}

const buttonLabels = () => [...container.querySelectorAll('button')].map((button) => button.textContent?.trim());

describe('结果头部', () => {
  // 打包版上看到的：DROP / CREATE / INSERT 的结果上也印着「选中单元格后 ⌘C 复制」，
  // 旁边还有两个灰掉的「图表」「导出」——没有格子可选，也没有东西可画、可导
  it('不返回行的语句只说影响了几行，不出复制提示、图表与导出', async () => {
    await render({ columns: [], rows: [], affected_rows: 120, execution_time: 5 });

    expect(container.textContent).toContain('Rows affected: 120');
    expect(container.textContent).not.toContain('to copy');
    expect(buttonLabels()).not.toContain('Chart');
    expect(buttonLabels()).not.toContain('Export');
  });

  it('返回行的语句照旧', async () => {
    await render({ columns: ['n'], rows: [[1]], affected_rows: 1, execution_time: 5 });

    expect(container.textContent).toContain('to copy');
    expect(buttonLabels()).toEqual(expect.arrayContaining(['Chart', 'Export']));
  });
});

describe('一条语句的第几个结果集', () => {
  it('标签写在结果头最前面', async () => {
    await act(async () => {
      root.render(
        <QueryResultScrollTable
          result={{ columns: ['n'], rows: [[1]], affected_rows: 1, execution_time: 0 }}
          statementId="s1#2"
          label="Result set 2 / 3"
          formatExecutionTime={(ms) => `${ms}ms`}
        />
      );
    });

    expect(container.textContent?.startsWith('Result set 2 / 3')).toBe(true);
  });

  // 打包版上看到的：第二个起只读却没有锁和说明，而且印着整条语句的耗时，像是它自己花了这么久
  it('后面的结果集说明只读，不印耗时', async () => {
    await act(async () => {
      root.render(
        <QueryResultScrollTable
          result={{
            columns: ['n'],
            rows: [[1]],
            affected_rows: 1,
            execution_time: 134,
            editability: { editable: false, reason: 'later-result-set' }
          }}
          statementId="s1#2"
          label="Result set 2 / 2"
        />
      );
    });

    expect(container.textContent).toContain('Result sets after the first are read-only');
    expect(container.textContent).not.toContain('134');
  });
});

describe('表头的列类型', () => {
  // MariaDB 的 JSON 列是 LONGTEXT + utf8mb4_bin。sqlx 只凭 BINARY 标志起类型名，拿不到字符集，
  // 报成 LONGBLOB；表数据页照目录显示 longtext
  const mariadbJson: QueryResult = {
    columns: ['doc'],
    column_metadata: [{ name: 'doc', ordinal: 0, database_type: 'LONGBLOB', logical_type: 'binary', nullable: true }],
    rows: [['{"a":1}']],
    affected_rows: 1,
    execution_time: 5
  };

  it('认出了来源表时显示目录里的声明类型', async () => {
    await render({
      ...mariadbJson,
      tableColumns: [{ name: 'doc', data_type: 'longtext', is_nullable: true, is_primary_key: false }]
    });

    expect(container.textContent).toContain('longtext');
    expect(container.textContent).not.toContain('LONGBLOB');
  });

  it('认不出来源表时照驱动给的', async () => {
    await render(mariadbJson);

    expect(container.textContent).toContain('LONGBLOB');
  });
});

describe('外键与来源表点击跳转', () => {
  it('点击 JOIN 结果里的关联列跳转按钮，打开目标表并预填等值筛选', async () => {
    useQueryStore.setState({ connectionId: 'conn-1' });
    useWorkspaceStore.setState({ tabs: [], activeTabId: null, closedTabs: [], pendingTableJumps: {} });

    await act(async () => {
      root.render(
        <QueryResultScrollTable
          result={{
            columns: ['skill_id', 'global_skill_id'],
            rows: [[7, 'smart-ring-analyst']],
            affected_rows: 1,
            execution_time: 4
          }}
          statementId="s-join"
          resultSql="SELECT us.skill_id, s.global_skill_id FROM claw_user_skill us JOIN claw_skill_hub s ON s.id = us.skill_id"
          formatExecutionTime={(ms) => `${ms}ms`}
        />
      );
    });

    const jumpBtn = container.querySelector('button[aria-label="Jump to claw_skill_hub (id)"]') as HTMLButtonElement | null;
    expect(jumpBtn).not.toBeNull();

    await act(async () => {
      jumpBtn?.click();
    });

    const tabId = workspaceTabId('conn-1', 'table-data', { schema: null, table: 'claw_skill_hub' });
    expect(useWorkspaceStore.getState().activeTabId).toBe(tabId);
    expect(useWorkspaceStore.getState().pendingTableJumps[tabId]?.filters).toEqual([
      { column: 'id', value: '7' }
    ]);
  });
});

describe('单列复制与单列导出', () => {
  it('右键表头可复制整列纯值、SQL IN，单列导出默认只选该列且覆盖全部已加载行', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true
    });

    // 30 行超过默认每页 25 行，验证单列导出覆盖全部 30 行而不只是当前页
    const rows = Array.from({ length: 30 }, (_, index) => [
      index + 1,
      `u_${index % 2}`
    ]);
    await render({
      columns: ['id', 'user_id'],
      rows,
      affected_rows: 30,
      execution_time: 3
    });

    const userHeader = [...container.querySelectorAll('th')].find((th) => th.textContent?.includes('user_id'));
    if (!userHeader) throw new Error('没有 user_id 列头');

    await act(async () => {
      userHeader.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 15, clientY: 15 }));
    });
    const findMenu = (label: string) => {
      const btn = [...document.querySelectorAll<HTMLButtonElement>('div.fixed button')]
        .find((button) => button.firstElementChild?.textContent === label);
      if (!btn) throw new Error(`没有菜单项 ${label}`);
      return btn;
    };

    await act(async () => {
      findMenu('Copy column as SQL IN (…)').click();
    });
    expect(writeText).toHaveBeenLastCalledWith("('u_0', 'u_1')");

    await act(async () => {
      userHeader.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 15, clientY: 15 }));
    });
    await act(async () => {
      findMenu('Export column user_id…').click();
    });

    const dialog = document.querySelector('[role="dialog"]');
    expect(dialog?.textContent).toContain('user_id only (1 of 2)');
    expect(dialog?.textContent).toContain('Rows: 30 · columns: 1');
  });

  it('支持查询结果右键快速筛选、即时查找过滤、选区聚合与行详情对比', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true
    });

    await render({
      columns: ['id', 'status', 'amount'],
      rows: [
        [1, 'paid', 10.5],
        [2, 'failed', 20.5],
        [3, 'paid', 30]
      ],
      affected_rows: 3,
      execution_time: 2
    });

    // 1. 选区实时统计（框选 amount 列 3 行）
    const amountHeader = [...container.querySelectorAll('th')].find((th) => th.textContent?.includes('amount'));
    if (!amountHeader) throw new Error('没有 amount 列头');
    await act(async () => {
      amountHeader.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 10, clientY: 10 }));
    });
    const findMenu = (label: string) => {
      const btn = [...document.querySelectorAll<HTMLButtonElement>('div.fixed button')]
        .find((button) => button.firstElementChild?.textContent === label);
      if (!btn) throw new Error(`没有菜单项 ${label}`);
      return btn;
    };
    await act(async () => {
      findMenu('Select column').click();
    });
    const statsBar = container.querySelector('[data-testid="grid-selection-stats"]');
    expect(statsBar?.textContent).toContain('3 cells');
    expect(statsBar?.textContent).toContain('Sum: 61');

    // 点击求和即可复制数值
    const sumBtn = [...(statsBar?.querySelectorAll('button') ?? [])]
      .find((btn) => btn.textContent?.includes('Sum: 61'));
    if (!sumBtn) throw new Error('没有求和按钮');
    await act(async () => {
      sumBtn.click();
    });
    expect(writeText).toHaveBeenLastCalledWith('61');

    // 2. 右键单元格「筛选 = paid」即时筛选加载结果并展示筛选标签
    const paidCell = container.querySelectorAll('tbody tr')[0]?.querySelectorAll('td')[1];
    if (!paidCell) throw new Error('没有 status 单元格');
    await act(async () => {
      paidCell.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 12, clientY: 12 }));
    });
    await act(async () => {
      findMenu('Filter = paid').click();
    });
    expect(container.querySelectorAll('tbody tr')).toHaveLength(2);
    expect(container.querySelector('[data-testid="result-quick-filters"]')?.textContent).toContain('Filtered 2 / 3');

    // 3. 点「Clear filters」恢复全部 3 行，再打开 Find 栏按关键词过滤
    const clearBtn = [...container.querySelectorAll('button')].find((btn) => btn.textContent === 'Clear filters');
    if (!clearBtn) throw new Error('没有 Clear filters 按钮');
    await act(async () => {
      clearBtn.click();
    });
    expect(container.querySelectorAll('tbody tr')).toHaveLength(3);

    const findBtn = [...container.querySelectorAll('button')].find((btn) => btn.textContent === 'Find');
    if (!findBtn) throw new Error('没有 Find 按钮');
    await act(async () => {
      findBtn.click();
    });
    const findInput = container.querySelector<HTMLInputElement>('[data-testid="grid-find-bar"] input');
    if (!findInput) throw new Error('没有查找输入框');
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
    await act(async () => {
      setter?.call(findInput, 'failed');
      findInput.dispatchEvent(new Event('input', { bubbles: true }));
    });
    expect(container.querySelectorAll('tbody tr')).toHaveLength(1);
    expect(container.querySelector('[data-testid="grid-find-bar"]')?.textContent).toContain('1 / 3 matched');
  });
});
