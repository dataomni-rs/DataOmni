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

const { MongoSchemaAnalyticsDialog } = await import('./MongoSchemaAnalyticsDialog');

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('MongoSchemaAnalyticsDialog', () => {
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

  it('displays nested type drift, unused indexes, and applies $type filter on click', async () => {
    invoke.mockResolvedValue({
      sampledDocs: 500,
      truncatedPathsCount: 12,
      elapsedMs: 34,
      paths: [
        {
          path: 'user.age',
          depth: 2,
          presentCount: 500,
          presentRatio: 1,
          missingCount: 0,
          explicitNullCount: 0,
          hasTypeDrift: true,
          types: [
            { bsonType: 'int32', count: 480, ratio: 0.96 },
            { bsonType: 'string', count: 20, ratio: 0.04 }
          ],
          sampleValues: ['25', '30']
        }
      ],
      storage: {
        count: 10_000,
        avgObjSize: 512,
        storageSize: 4 * 1024 * 1024,
        totalIndexSize: 1024 * 1024
      },
      indexUsage: [
        { name: '_id_', ops: 1200, since: '2026-11-01T00:00:00Z', unused: false },
        { name: 'legacy_status_1', ops: 0, since: '2026-11-01T00:00:00Z', unused: true }
      ]
    });
    const onApplyFilter = vi.fn();
    const onClose = vi.fn();

    await act(async () => {
      root.render(
        <MongoSchemaAnalyticsDialog
          connectionString="mongodb://localhost:27017"
          database="app"
          collection="orders"
          filter=""
          timeoutMs={30000}
          onApplyFilter={onApplyFilter}
          onClose={onClose}
        />
      );
    });
    await flush();

    expect(container.textContent).toContain('user.age');
    expect(container.textContent).toContain('类型漂移');

    // Switch to indexes tab and confirm zombie index highlight
    const indexTab = Array.from(container.querySelectorAll<HTMLButtonElement>('button[role="tab"]')).find(
      (btn) => btn.textContent?.includes('存储与索引')
    );
    await act(async () => {
      indexTab?.click();
    });
    expect(container.textContent).toContain('legacy_status_1');
    expect(container.textContent).toContain('0 命中');

    // Switch back and click the string type pill
    const schemaTab = Array.from(container.querySelectorAll<HTMLButtonElement>('button[role="tab"]')).find(
      (btn) => btn.textContent?.includes('嵌套 Schema')
    );
    await act(async () => {
      schemaTab?.click();
    });
    const stringBtn = Array.from(container.querySelectorAll('button')).find((btn) =>
      btn.textContent?.startsWith('string ')
    );
    await act(async () => {
      stringBtn?.click();
    });
    expect(onApplyFilter).toHaveBeenCalledWith('{ "user.age": { $type: "string" } }');
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
