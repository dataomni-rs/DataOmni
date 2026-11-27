/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DatabaseType, type ConnectionProfile } from '../contracts/connection';
import { useLanguageStore } from '../stores/languageStore';
import { useQueryStore } from '../stores/queryStore';
import { useSettingsStore } from '../stores/settingsStore';
import { useTableEditStore } from '../stores/tableEditStore';
import { useWorkspaceStore } from '../stores/workspaceStore';
import { workspaceTabId } from '../contracts/workspace';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args), Channel: class {} }));

const readQuery = vi.fn();
vi.mock('../stores/queryStore', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../stores/queryStore')>()),
  runReadQuery: (sql: string) => readQuery(sql)
}));

const { default: TableDataViewer } = await import('./TableDataViewer');

const connection: ConnectionProfile = {
  id: 'ch',
  name: 'ch',
  db_type: DatabaseType.ClickHouse,
  host: '127.0.0.1',
  port: 8123,
  username: 'default',
  password: '',
  ssl: false,
  save_password: false,
  options: {},
  tags: [],
  environment: 'development',
  created_at: '',
  updated_at: ''
};

const COLUMNS = [
  { column_name: 'id', data_type: 'UInt32', is_nullable: false, is_primary_key: true, primary_key_ordinal: 1 },
  { column_name: 'fs', data_type: 'FixedString(4)', is_nullable: false, is_primary_key: false },
  { column_name: 'note', data_type: 'String', is_nullable: false, is_primary_key: false }
];

let container: HTMLDivElement;
let root: Root;

function byTitle(title: string): HTMLButtonElement {
  const found = container.querySelector<HTMLButtonElement>(`button[title="${title}"]`);
  if (!found) throw new Error(`没有「${title}」按钮`);
  return found;
}

function openContextMenuOn(text: string) {
  const cell = [...container.querySelectorAll('td')].find((td) => td.textContent === text);
  if (!cell) throw new Error(`没有内容是「${text}」的格子`);
  cell.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 10, clientY: 10 }));
}

function menuLabels(): string[] {
  return [...document.querySelectorAll('div.fixed button')].map((button) => button.firstElementChild?.textContent ?? '');
}

function menuItem(label: string): HTMLButtonElement {
  const found = [...document.querySelectorAll<HTMLButtonElement>('div.fixed button')]
    .find((button) => button.firstElementChild?.textContent === label);
  if (!found) throw new Error(`菜单里没有「${label}」，只有 ${menuLabels().join(' / ')}`);
  return found;
}

beforeEach(() => {
  useLanguageStore.getState().setPreference('en');
  useTableEditStore.setState({ changes: {} });
  const select = vi.fn(async (sql: string) => (sql === 'COLUMNS' ? COLUMNS : []));
  useQueryStore.setState({ connectionId: 'ch', database: { select } as never });
  invoke.mockReset();
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_schema_metadata_queries') {
      return { columns: 'COLUMNS', indexes: 'INDEXES', foreign_keys: 'FKS', check_constraints: null, ddl: null, triggers: 'TRIGGERS', parameter_count: 2 };
    }
    return null;
  });
  readQuery.mockReset();
  readQuery.mockImplementation(async (sql: string) => (
    sql.startsWith('SELECT COUNT(*)')
      ? [{ total: 1 }]
      : [{ id: 1, fs: { type: 'binary', value: 'ff000102' }, note: 'x' }]
  ));
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe('TableDataViewer（ClickHouse）', () => {
  // 打包版上撞到的：编辑前值已经拆了包，rowKeyOf 看不出哪一列是二进制，
  // 定位条件里照样带着 fs = 'ff000102'，服务端报 TOO_LARGE_STRING_SIZE
  it('改一行时，显示成二进制的值不进定位条件', async () => {
    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });
    await act(async () => {
      byTitle('Edit').click();
    });
    const note = [...container.querySelectorAll('input')].find((input) => input.value === 'x');
    if (!note) throw new Error('没有 note 的输入框');
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
      setter?.call(note, 'y');
      note.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await act(async () => {
      byTitle('Save').click();
    });

    const [change] = Object.values(useTableEditStore.getState().changes).flat();
    expect(change?.kind).toBe('update');
    expect(change && 'key' in change ? change.key.columns : null).toEqual(['id', 'note']);
  });

  it('删一行时同样', async () => {
    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });
    await act(async () => {
      byTitle('Delete').click();
    });

    const [change] = Object.values(useTableEditStore.getState().changes).flat();
    expect(change?.kind).toBe('delete');
    expect(change && 'key' in change ? change.key.columns : null).toEqual(['id', 'note']);
  });

  // 用户反馈：操作列在最右边，列一多就被挤出屏幕，右键菜单里又只有复制，找不到怎么改一行
  it('右键菜单里可以编辑这一行', async () => {
    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });
    expect(container.querySelectorAll('input[value="x"]').length).toBe(0);
    await act(async () => {
      openContextMenuOn('x');
    });
    await act(async () => {
      menuItem('Edit this row').click();
    });

    const inputs = [...container.querySelectorAll('input')].map((input) => input.value);
    expect(inputs).toContain('x');
  });

  it('已经排进待提交的行，右键菜单不给编辑', async () => {
    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });
    await act(async () => {
      byTitle('Delete').click();
    });
    await act(async () => {
      openContextMenuOn('x');
    });

    expect(menuLabels()).toContain('Copy row');
    expect(menuLabels()).not.toContain('Edit this row');
  });

  // 打包版上看到的：四个翻页按钮只有图标，读屏与自动化都只拿到一个空名字
  it('翻页按钮有名字', async () => {
    readQuery.mockImplementation(async (sql: string) => (
      sql.startsWith('SELECT COUNT(*)') ? [{ total: 120 }] : [{ id: 1, fs: 'a', note: 'x' }]
    ));
    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });

    const labels = [...container.querySelectorAll('button[aria-label]')].map((button) => button.getAttribute('aria-label'));
    expect(labels).toEqual(expect.arrayContaining(['First page', 'Previous page', 'Next page', 'Last page']));
  });

  // 打包版上看到的：设成「紧凑」后表数据页的行高纹丝不动——操作列那一格写死了 py-1，
  // 里面的图标按钮把整行撑住了。结果网格的操作列跟着设置走，这里也要
  it('行高设置也管操作列', async () => {
    useSettingsStore.setState({ gridDensity: 'compact' });
    readQuery.mockImplementation(async (sql: string) => (
      sql.startsWith('SELECT COUNT(*)') ? [{ total: 1 }] : [{ id: 1, fs: 'a', note: 'x' }]
    ));
    try {
      await act(async () => {
        root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
      });
      const cells = [...container.querySelectorAll('tbody tr:first-child > td')];
      expect(cells.length).toBeGreaterThan(1);
      expect(cells.filter((cell) => cell.classList.contains('py-1'))).toEqual([]);
    } finally {
      useSettingsStore.setState({ gridDensity: 'default' });
    }
  });

  // 打包版上撞到的：AggregateFunction 列点了排序，查询报错、行清空，表头跟着没了，
  // 刷新还带着这个排序——只能关掉标签重开
  it('按某列排序失败时取消排序重读，报错仍然看得到', async () => {
    readQuery.mockImplementation(async (sql: string) => {
      if (sql.startsWith('SELECT COUNT(*)')) return [{ total: 1 }];
      if (sql.includes('ORDER BY `note`')) throw new Error('Data type is not allowed in ORDER BY keys');
      return [{ id: 1, fs: 'a', note: 'x' }];
    });
    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });
    await act(async () => {
      byTitle('note: click to sort ascending (Shift+click to add secondary sort)').click();
    });

    expect(container.textContent).toContain('not allowed in ORDER BY keys');
    expect(container.textContent).toContain('note');
    // 行回来了，表头还在、排序已取消
    expect([...container.querySelectorAll('input, td')].some((cell) => cell.textContent === 'x')).toBe(true);
    byTitle('note: click to sort ascending (Shift+click to add secondary sort)');
    const dataQueries = readQuery.mock.calls.map(([sql]) => sql as string).filter((sql) => !sql.startsWith('SELECT COUNT(*)'));
    expect(dataQueries[dataQueries.length - 1]).not.toContain('ORDER BY `note`');
  });

  // 回归时看到的：「已提交 1 项」在翻页、排序、改完结构之后还挂着，像是刚又提交了一次
  it('「已提交」只说刚才那一次：之后重读了表就收起', async () => {
    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });
    await act(async () => {
      byTitle('Delete').click();
    });
    const commit = [...container.querySelectorAll('button')].find((button) => button.textContent === 'Commit 1');
    if (!commit) throw new Error('没有提交按钮');
    await act(async () => {
      commit.click();
    });
    expect(container.textContent).toContain('Committed 1 change.');

    await act(async () => {
      byTitle('note: click to sort ascending (Shift+click to add secondary sort)').click();
    });
    expect(container.textContent).not.toContain('Committed 1 change.');
  });
});

describe('TableDataViewer（ClickHouse MergeTree 表按物理位置定位）', () => {
  it('读表时带上位置、网格里不显示，删一行时键里带着它', async () => {
    const merge = COLUMNS.map((column) => ({ ...column, table_engine: 'MergeTree' }));
    const select = vi.fn(async (sql: string) => (sql === 'COLUMNS' ? merge : []));
    useQueryStore.setState({ connectionId: 'ch', database: { select } as never });
    const sent: string[] = [];
    readQuery.mockImplementation(async (sql: string) => {
      sent.push(sql);
      return sql.startsWith('SELECT COUNT(*)')
        ? [{ total: 1 }]
        : [{ id: 1, fs: 'a', note: 'x', __dataomni_part: 'all_1_1_0', __dataomni_part_offset: { type: 'bigint', value: '3' } }];
    });
    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="tpos" schema="db" initialTab="data" />);
    });

    expect(sent.find((sql) => sql.includes('_part_offset AS `__dataomni_part_offset`'))).toBeDefined();
    expect(container.textContent).not.toContain('__dataomni');
    expect(container.textContent).not.toContain('all_1_1_0');
    await act(async () => {
      byTitle('Delete').click();
    });
    const [change] = Object.values(useTableEditStore.getState().changes).flat();
    expect(change && 'key' in change ? change.key.position : null).toEqual({ part: 'all_1_1_0', offset: '3' });
  });

  it('不是 MergeTree 的表照旧只读表自己的列', async () => {
    const memory = COLUMNS.map((column) => ({ ...column, table_engine: 'Memory' }));
    const select = vi.fn(async (sql: string) => (sql === 'COLUMNS' ? memory : []));
    useQueryStore.setState({ connectionId: 'ch', database: { select } as never });
    const sent: string[] = [];
    readQuery.mockImplementation(async (sql: string) => {
      sent.push(sql);
      return sql.startsWith('SELECT COUNT(*)') ? [{ total: 1 }] : [{ id: 1, fs: 'a', note: 'x' }];
    });
    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t2" schema="db" initialTab="data" />);
    });
    expect(sent.some((sql) => sql.includes('_part'))).toBe(false);
  });
});

describe('TableDataViewer（CockroachDB 的触发器）', () => {
  // CockroachDB 没有 pg_get_triggerdef()，目录查询报 42883；它的触发器只有 SHOW 看得见
  it('目录查询报缺函数时改走 SHOW，画出触发器原文', async () => {
    const crdb = { ...connection, id: 'crdb', name: 'crdb', db_type: DatabaseType.PostgreSQL };
    const select = vi.fn(async (sql: string) => {
      if (sql === 'COLUMNS') return COLUMNS;
      if (sql === 'TRIGGERS') {
        throw Object.assign(new Error('unknown function: pg_get_triggerdef()'), { code: '42883' });
      }
      if (sql === 'SHOW TRIGGERS FROM "public"."t"') return [{ trigger_name: 'audit', enabled: true }];
      if (sql === 'SHOW CREATE TRIGGER "audit" ON "public"."t"') {
        return [{ trigger_name: 'audit', create_statement: 'CREATE TRIGGER audit BEFORE INSERT ON db.public.t' }];
      }
      return [];
    });
    useQueryStore.setState({ connectionId: 'crdb', database: { select } as never });
    invoke.mockImplementation(async (command: string) => (
      command === 'get_schema_metadata_queries'
        ? {
          columns: 'COLUMNS', indexes: 'INDEXES', foreign_keys: 'FKS', check_constraints: null, ddl: null,
          triggers: 'TRIGGERS',
          triggers_by_show: { list: 'SHOW TRIGGERS FROM {table}', definition: 'SHOW CREATE TRIGGER {trigger} ON {table}' },
          parameter_count: 2
        }
        : null
    ));

    await act(async () => {
      root.render(<TableDataViewer connection={crdb} tableName="t" schema="public" initialTab="schema" />);
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(container.textContent).toContain('CREATE TRIGGER audit BEFORE INSERT ON db.public.t');
    expect(container.textContent).not.toContain('pg_get_triggerdef');
  });
});

describe('TableDataViewer（外键跳转预填筛选）', () => {
  it('消费待执行跳转筛选并在首次查询时直接带上 WHERE 条件', async () => {
    const pgConn = { ...connection, id: 'pg-jump', name: 'pg-jump', db_type: DatabaseType.PostgreSQL };
    useWorkspaceStore.setState({ tabs: [], activeTabId: null, closedTabs: [], pendingTableJumps: {} });
    useWorkspaceStore.getState().jumpToTableData(
      'pg-jump',
      { schema: 'public', table: 'users' },
      [{ column: 'id', value: '42' }]
    );

    const pgColumns = [
      { column_name: 'id', data_type: 'integer', is_nullable: false, is_primary_key: true, primary_key_ordinal: 1 },
      { column_name: 'note', data_type: 'text', is_nullable: true, is_primary_key: false }
    ];
    const select = vi.fn(async (sql: string) => (sql === 'COLUMNS' ? pgColumns : []));
    useQueryStore.setState({ connectionId: 'pg-jump', database: { select } as never });
    const sent: string[] = [];
    readQuery.mockImplementation(async (sql: string) => {
      sent.push(sql);
      return sql.startsWith('SELECT COUNT(*)') ? [{ total: 1 }] : [{ id: 42, fs: 'a', note: 'ok' }];
    });

    await act(async () => {
      root.render(<TableDataViewer connection={pgConn} tableName="users" schema="public" initialTab="data" />);
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const tabId = workspaceTabId('pg-jump', 'table-data', { schema: 'public', table: 'users' });
    expect(useWorkspaceStore.getState().pendingTableJumps[tabId]).toBeUndefined();
    expect(sent.join('\n')).toContain('WHERE');
    expect(sent.join('\n')).toContain('"id" = 42');
  });
});

describe('TableDataViewer（单列复制与单列导出）', () => {
  it('右键表头只显示整列动作，并支持复制纯值、SQL IN 与单列导出', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true
    });
    readQuery.mockImplementation(async (sql: string) => (
      sql.startsWith('SELECT COUNT(*)')
        ? [{ total: 3 }]
        : [
            { id: 1, fs: 'a', note: 'alpha' },
            { id: 2, fs: 'b', note: 'beta' },
            { id: 3, fs: 'c', note: 'alpha' }
          ]
    ));

    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });

    const noteHeader = [...container.querySelectorAll('th')].find((th) => th.textContent?.includes('note'));
    if (!noteHeader) throw new Error('没有 note 列头');

    await act(async () => {
      noteHeader.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 12, clientY: 12 }));
    });
    expect(menuLabels()).toEqual([
      'Select column',
      'Copy column values',
      'Copy column with its name',
      'Copy column as SQL IN (…)',
      'Export column note…'
    ]);

    // 复制整列（仅值）：不带列名表头
    await act(async () => {
      menuItem('Copy column values').click();
    });
    expect(writeText).toHaveBeenLastCalledWith('alpha\nbeta\nalpha');

    // 复制为 SQL IN (...)：去重并按方言加引号
    await act(async () => {
      noteHeader.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 12, clientY: 12 }));
    });
    await act(async () => {
      menuItem('Copy column as SQL IN (…)').click();
    });
    expect(writeText).toHaveBeenLastCalledWith("('alpha', 'beta')");

    // 右键列头导出这一列：打开导出对话框且预选为仅 note 列
    await act(async () => {
      noteHeader.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 12, clientY: 12 }));
    });
    await act(async () => {
      menuItem('Export column note…').click();
    });
    const dialog = document.querySelector('[role="dialog"]');
    expect(dialog?.textContent).toContain('note only (1 of 3)');
    expect(dialog?.querySelector('pre')?.textContent).toBe('note\nalpha\nbeta\nalpha');
  });

  it('支持右键按单元格快速筛选、选区统计条、多列排序与双行对比侧栏', async () => {
    const sent: string[] = [];
    readQuery.mockImplementation(async (sql: string) => {
      sent.push(sql);
      return sql.startsWith('SELECT COUNT(*)')
        ? [{ total: 3 }]
        : [
            { id: 10, fs: 'a', note: 'alpha' },
            { id: 20, fs: 'b', note: 'beta' },
            { id: 30, fs: 'c', note: 'alpha' }
          ];
    });

    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });

    // 1. 选区实时统计（框选 id 列的 10 与 20）
    const rows = [...container.querySelectorAll('tbody tr')];
    const idCell0 = rows[0]?.querySelectorAll('td')[0];
    const idCell1 = rows[1]?.querySelectorAll('td')[0];
    if (!idCell0 || !idCell1) throw new Error('没有找到 id 单元格');
    await act(async () => {
      idCell0.click();
    });
    await act(async () => {
      idCell1.dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: true }));
    });
    const statsBar = container.querySelector('[data-testid="grid-selection-stats"]');
    expect(statsBar?.textContent).toContain('2 cells');
    expect(statsBar?.textContent).toContain('Sum: 30');
    expect(statsBar?.textContent).toContain('Avg: 15');

    // 2. 选中 2 行后右键对比两行差异
    await act(async () => {
      idCell1.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 12, clientY: 12 }));
    });
    await act(async () => {
      menuItem('Compare selected 2 rows').click();
    });
    const inspector = container.querySelector('[data-testid="grid-row-inspector"]');
    expect(inspector?.textContent).toContain('Compare #1 vs #2');
    expect(inspector?.textContent).toContain('3 differing fields');

    // 3. 右键单元格「按当前值快速筛选」直接发服务端 WHERE
    const noteCell0 = rows[0]?.querySelectorAll('td')[2];
    if (!noteCell0) throw new Error('没有 note 单元格');
    await act(async () => {
      noteCell0.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 12, clientY: 12 }));
    });
    await act(async () => {
      menuItem('Filter = alpha').click();
    });
    expect(sent.some((sql) => sql.includes("WHERE `note` = 'alpha'"))).toBe(true);

    // 4. Shift+点击第二列实现多列组合排序
    await act(async () => {
      byTitle('note: click to sort ascending (Shift+click to add secondary sort)').click();
    });
    await act(async () => {
      byTitle('fs: click to sort ascending (Shift+click to add secondary sort)')
        .dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: true }));
    });
    expect(sent.some((sql) => sql.includes('ORDER BY `note` ASC, `fs` ASC'))).toBe(true);
  });

  it('鼠标拖拽可选中任意连续矩形选区，且右键复制选区保留整块选区内容', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true
    });
    readQuery.mockImplementation(async (sql: string) => (
      sql.startsWith('SELECT COUNT(*)')
        ? [{ total: 3 }]
        : [
            { id: 10, fs: 'a1', note: 'n1' },
            { id: 20, fs: 'b2', note: 'n2' },
            { id: 30, fs: 'c3', note: 'n3' }
          ]
    ));

    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });

    const rows = [...container.querySelectorAll('tbody tr')];
    const startCell = rows[0]?.querySelectorAll('td')[1]; // (row 0, col 1: fs = a1)
    const endCell = rows[1]?.querySelectorAll('td')[2];   // (row 1, col 2: note = n2)
    if (!startCell || !endCell) throw new Error('未找到拖拽起止单元格');

    await act(async () => {
      startCell.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0, buttons: 1 }));
    });
    await act(async () => {
      // React 的 onMouseEnter 监听的是 mouseover（或 mouseenter），在 happy-dom 中派发 mouseover
      endCell.dispatchEvent(new MouseEvent('mouseover', { bubbles: true, buttons: 1 }));
    });
    await act(async () => {
      window.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, button: 0, buttons: 0 }));
    });

    // 选区应为 2x2（4 格），且不能把第 0 行的 id 列卷进去
    const statsBar = container.querySelector('[data-testid="grid-selection-stats"]');
    expect(statsBar?.textContent).toContain('4 cells');

    // 右键选区内部格并点「Copy selection」，应复制 2x2 矩形而非单格
    await act(async () => {
      endCell.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 10, clientY: 10 }));
    });
    await act(async () => {
      menuItem('Copy selection').click();
    });
    expect(writeText).toHaveBeenLastCalledWith('a1\tn1\nb2\tn2');
  });

  it('拖动表头可交换列顺序并同步数据格与右键重置列顺序', async () => {
    readQuery.mockImplementation(async (sql: string) => (
      sql.startsWith('SELECT COUNT(*)')
        ? [{ total: 2 }]
        : [
            { id: 10, fs: 'a1', note: 'n1' },
            { id: 20, fs: 'b2', note: 'n2' }
          ]
    ));

    await act(async () => {
      root.render(<TableDataViewer connection={connection} tableName="t" schema="db" initialTab="data" />);
    });

    const headers = [...container.querySelectorAll<HTMLTableCellElement>('thead th[data-col-orig-index]')];
    expect(headers).toHaveLength(3);
    // 给三列表头稳定的几何矩形，供拖动命中检测使用
    headers.forEach((th, idx) => {
      vi.spyOn(th, 'getBoundingClientRect').mockReturnValue({
        left: idx * 100,
        right: (idx + 1) * 100,
        top: 0,
        bottom: 32,
        width: 100,
        height: 32,
        x: idx * 100,
        y: 0,
        toJSON: () => ({})
      });
    });

    // 把第 2 列（note）拖到第 0 列（id）位置
    await act(async () => {
      headers[2].dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0, clientX: 250, clientY: 16 }));
    });
    await act(async () => {
      window.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, buttons: 1, clientX: 40, clientY: 16 }));
    });
    await act(async () => {
      window.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, button: 0, clientX: 40, clientY: 16 }));
    });

    const reorderedCols = [...container.querySelectorAll<HTMLTableCellElement>('thead th[data-col-orig-index]')]
      .map((th) => th.dataset.colOrigIndex);
    expect(reorderedCols).toEqual(['2', '0', '1']);
    const firstRowTexts = [...container.querySelectorAll('tbody tr')[0].querySelectorAll('td')]
      .slice(0, 3)
      .map((td) => td.textContent?.trim());
    expect(firstRowTexts).toEqual(['n1', '10', 'a1']);

    // 右键列头可一键重置列顺序
    await act(async () => {
      container.querySelector('thead th[data-col-orig-index]')?.dispatchEvent(
        new MouseEvent('contextmenu', { bubbles: true, clientX: 12, clientY: 12 })
      );
    });
    await act(async () => {
      menuItem('Reset order').click();
    });
    const resetCols = [...container.querySelectorAll<HTMLTableCellElement>('thead th[data-col-orig-index]')]
      .map((th) => th.dataset.colOrigIndex);
    expect(resetCols).toEqual(['0', '1', '2']);
  });
});
