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

const { RedisKeyspaceAnalyticsDialog } = await import('./RedisKeyspaceAnalyticsDialog');

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('RedisKeyspaceAnalyticsDialog', () => {
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

  it('renders prefix breakdown, TTL health, degraded MEMORY USAGE notice, and navigates to clicked BigKey', async () => {
    invoke.mockResolvedValue({
      sampledCount: 2,
      partial: false,
      cancelled: false,
      memoryUsageAvailable: false,
      elapsedMs: 18,
      keys: [
        {
          key: { raw: btoa('session:u1'), text: 'session:u1', binary: false },
          kind: 'string',
          ttlMs: -1,
          length: 512
        },
        {
          key: { raw: btoa('cache:orders'), text: 'cache:orders', binary: false },
          kind: 'hash',
          ttlMs: 120_000,
          length: 2048
        }
      ]
    });
    const onSelectKey = vi.fn();
    const onClose = vi.fn();

    await act(async () => {
      root.render(
        <RedisKeyspaceAnalyticsDialog
          connectionString="redis://localhost:6379/0"
          database={0}
          initialPattern="*"
          onSelectKey={onSelectKey}
          onClose={onClose}
        />
      );
    });
    await flush();

    expect(container.textContent).toContain('MEMORY USAGE');
    expect(container.textContent).toContain('cache:*');
    expect(container.textContent).toContain('session:*');

    // Switch to BigKey tab and click top key
    const bigKeyTab = Array.from(container.querySelectorAll<HTMLButtonElement>('button[role="tab"]')).find(
      (btn) => btn.textContent?.includes('BigKey')
    );
    await act(async () => {
      bigKeyTab?.click();
    });
    expect(container.textContent).toContain('cache:orders');

    const row = Array.from(container.querySelectorAll('tbody tr')).find((tr) =>
      tr.textContent?.includes('cache:orders')
    );
    await act(async () => {
      row?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(onSelectKey).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: 'hash',
        key: expect.objectContaining({ text: 'cache:orders' })
      })
    );
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
