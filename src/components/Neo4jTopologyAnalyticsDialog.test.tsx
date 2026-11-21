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

const { Neo4jTopologyAnalyticsDialog } = await import('./Neo4jTopologyAnalyticsDialog');

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('Neo4jTopologyAnalyticsDialog', () => {
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

  it('loads O(1) counts, runs bounded supernode sampling in read-only mode, and emits read-only neighborhood Cypher', async () => {
    invoke.mockImplementation(async (_cmd: string, args: { query: string; readOnly?: boolean }) => {
      expect(args.readOnly).toBe(true);
      if (args.query.includes('db.labels')) {
        return { columns: ['label'], rows: [[{ kind: 'string', value: 'Person' }]] };
      }
      if (args.query.includes('db.relationshipTypes')) {
        return { columns: ['relationshipType'], rows: [[{ kind: 'string', value: 'KNOWS' }]] };
      }
      if (args.query.includes('RETURN count(')) {
        return { columns: ['count'], rows: [[{ kind: 'integer', value: '120' }]] };
      }
      return {
        columns: ['id', 'labels', 'degree', 'propKeys'],
        rows: [
          [
            { kind: 'string', value: '4:db:99' },
            { kind: 'list', items: [{ kind: 'string', value: 'Person' }] },
            { kind: 'integer', value: '450' },
            { kind: 'list', items: [{ kind: 'string', value: 'name' }] }
          ]
        ]
      };
    });
    const onRunCypher = vi.fn();
    const onClose = vi.fn();

    await act(async () => {
      root.render(
        <Neo4jTopologyAnalyticsDialog
          connectionString="bolt://localhost:7687"
          timeoutMs={15000}
          onRunCypher={onRunCypher}
          onClose={onClose}
        />
      );
    });
    await flush();

    expect(container.textContent).toContain('Person');
    expect(container.textContent).toContain('KNOWS');

    const topoTab = Array.from(container.querySelectorAll<HTMLButtonElement>('button[role="tab"]')).find(
      (btn) => btn.textContent?.includes('超级节点')
    );
    await act(async () => {
      topoTab?.click();
    });
    const sampleBtn = Array.from(container.querySelectorAll('button')).find((btn) =>
      btn.textContent?.includes('开始采样体检')
    );
    await act(async () => {
      sampleBtn?.click();
    });
    await flush();

    expect(container.textContent).toContain('4:db:99');
    expect(container.textContent).toContain('450');

    const row = Array.from(container.querySelectorAll('tbody tr')).find((tr) =>
      tr.textContent?.includes('4:db:99')
    );
    await act(async () => {
      row?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onRunCypher).toHaveBeenCalledWith(expect.stringContaining('elementId(n) = "4:db:99"'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
