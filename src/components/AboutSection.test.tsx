/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const { AboutSection } = await import('./AboutSection');
const { useLanguageStore } = await import('../stores/languageStore');

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'diagnostics') {
      return Promise.resolve({
        app_version: '1.1.0',
        os: 'macos',
        arch: 'aarch64',
        webview_version: '619.1',
        ai: true,
        log_file: '/tmp/dataomni.log',
      });
    }
    return Promise.resolve();
  });
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

function buttonByText(label: string): HTMLButtonElement | undefined {
  return [...container.querySelectorAll('button')].find(
    (button) => (button.textContent ?? '').trim() === label,
  );
}

describe('AboutSection', () => {
  it('opens the English website and GitHub issues through fixed backend link targets', async () => {
    await act(async () => {
      root.render(<AboutSection />);
    });

    const website = buttonByText('Website');
    const issues = buttonByText('Report an issue');
    expect(website).toBeDefined();
    expect(issues).toBeDefined();

    await act(async () => {
      website?.click();
    });
    expect(invokeMock).toHaveBeenCalledWith('open_project_link', { target: 'website' });

    await act(async () => {
      issues?.click();
    });
    expect(invokeMock).toHaveBeenCalledWith('open_project_link', { target: 'issues' });
  });

  it('opens /zh/ on dataomni.dev when the interface language is Chinese and surfaces link errors', async () => {
    useLanguageStore.getState().setPreference('zh');
    await act(async () => {
      root.render(<AboutSection />);
    });

    const websiteZh = buttonByText('官方网站');
    expect(websiteZh).toBeDefined();

    invokeMock.mockImplementationOnce(() =>
      Promise.reject('DATAOMNI_EXTERNAL_LINK_OPEN_FAILED: XDG_OPEN_ERROR'),
    );
    await act(async () => {
      websiteZh?.click();
    });

    expect(invokeMock).toHaveBeenCalledWith('open_project_link', { target: 'websiteZh' });
    expect(container.textContent).toContain('无法在浏览器中打开链接：XDG_OPEN_ERROR');
  });
});
