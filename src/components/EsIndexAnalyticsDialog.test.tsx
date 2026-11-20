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

const { EsIndexAnalyticsDialog } = await import('./EsIndexAnalyticsDialog');

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('EsIndexAnalyticsDialog', () => {
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

  it('runs size:0 timeline agg even when _cat/indices returns 403', async () => {
    invoke.mockImplementation(async (_cmd: string, args: { path: string }) => {
      if (args.path.startsWith('/_cat/')) {
        return { status: 403, body: '{"error":"security_exception"}' };
      }
      return {
        status: 200,
        body: JSON.stringify({
          aggregations: {
            by_time: {
              buckets: [
                { key_as_string: '2026-11-20T00:00:00Z', doc_count: 150 },
                { key_as_string: '2026-11-20T01:00:00Z', doc_count: 420 }
              ]
            }
          }
        })
      };
    });

    await act(async () => {
      root.render(
        <EsIndexAnalyticsDialog
          connectionString="http://localhost:9200"
          index="app-logs"
          mappingFields={[
            { path: '@timestamp', depth: 0, type: 'date', multiField: false, details: [] },
            { path: 'level', depth: 0, type: 'keyword', multiField: false, details: [] },
            { path: 'latency_ms', depth: 0, type: 'double', multiField: false, details: [] }
          ]}
          timeoutMs={15000}
          onClose={vi.fn()}
        />
      );
    });
    await flush();

    expect(container.textContent).toContain('403');
    expect(container.textContent).toContain('2026-11-20T00:00:00Z');
    expect(container.textContent).toContain('420');
  });
});
