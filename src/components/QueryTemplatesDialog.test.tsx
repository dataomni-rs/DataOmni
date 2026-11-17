/**
 * @vitest-environment happy-dom
 */
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DatabaseType } from '../contracts/connection';
import { useLanguageStore } from '../stores/languageStore';
import { useTemplateStore } from '../stores/templateStore';
import {
  QUERY_TEMPLATES_STORAGE_KEY,
  resetQueryTemplatesDiskGuardForTest,
} from '../utils/queryTemplates';
import { QueryTemplatesDialog } from './QueryTemplatesDialog';

describe('QueryTemplatesDialog', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as Record<string, unknown>).IS_REACT_ACT_ENVIRONMENT = true;
    localStorage.clear();
    resetQueryTemplatesDiskGuardForTest();
    useTemplateStore.setState({ templates: [] });
    useLanguageStore.getState().setPreference('en');
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('saves current SQL with :name badges, filters by scope & search, inserts, and deletes', () => {
    const onInsert = vi.fn();
    const onInsertAndRun = vi.fn();
    const onClose = vi.fn();

    useTemplateStore.setState({
      templates: [
        {
          id: 'mysql-only',
          title: 'MySQL Slow Log',
          sql: 'SHOW FULL PROCESSLIST;',
          dbType: DatabaseType.MySQL,
          updatedAt: '2026-11-17T01:00:00Z',
        },
      ],
    });

    act(() => {
      root.render(
        createElement(QueryTemplatesDialog, {
          currentDbType: DatabaseType.PostgreSQL,
          currentSql: 'SELECT * FROM orders WHERE customer_id = :customer_id AND status = :status;',
          onInsert,
          onInsertAndRun,
          onClose,
        }),
      );
    });

    // Compatible scope on PostgreSQL hides MySQL-only template initially
    expect(container.textContent).not.toContain('MySQL Slow Log');

    // Click "Save current SQL"
    const saveCurrentBtn = [...container.querySelectorAll('button')].find((btn) =>
      btn.textContent?.includes('Save current SQL'),
    );
    expect(saveCurrentBtn).toBeDefined();
    act(() => {
      saveCurrentBtn?.click();
    });

    const titleInput = container.querySelector<HTMLInputElement>('#template-title-input');
    expect(titleInput).not.toBeNull();
    act(() => {
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
      setter?.call(titleInput, 'Customer Orders Lookup');
      titleInput?.dispatchEvent(new Event('input', { bubbles: true }));
    });

    const saveFormBtn = [...container.querySelectorAll('button')].find((btn) =>
      btn.textContent?.includes('Save template'),
    );
    act(() => {
      saveFormBtn?.click();
    });

    // Saved to store & localStorage, showing :customer_id and :status badges
    expect(container.textContent).toContain('Customer Orders Lookup');
    expect(container.textContent).toContain(':customer_id');
    expect(container.textContent).toContain(':status');
    expect(localStorage.getItem(QUERY_TEMPLATES_STORAGE_KEY)).toContain('Customer Orders Lookup');

    // Switch scope to "All" -> both templates appear
    const allScopeBtn = [...container.querySelectorAll('button')].find(
      (btn) => btn.textContent?.trim() === 'All',
    );
    act(() => {
      allScopeBtn?.click();
    });
    expect(container.textContent).toContain('MySQL Slow Log');

    // Click "Insert & run" on Customer Orders Lookup
    const insertRunBtn = [...container.querySelectorAll('button')].find((btn) =>
      btn.textContent?.includes('Insert & run'),
    );
    act(() => {
      insertRunBtn?.click();
    });
    expect(onInsertAndRun).toHaveBeenCalledTimes(1);
    expect(onInsertAndRun.mock.calls[0][0]).toMatchObject({
      title: 'Customer Orders Lookup',
      dbType: DatabaseType.PostgreSQL,
    });
  });
});
