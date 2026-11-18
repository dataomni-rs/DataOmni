/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
const getVersionMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: () => getVersionMock(),
}));

const { UpdateNotice } = await import('./UpdateNotice');
const { useLanguageStore } = await import('../stores/languageStore');

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  localStorage.clear();
  invokeMock.mockReset();
  getVersionMock.mockReset();
  getVersionMock.mockResolvedValue('1.2.1');
  useLanguageStore.getState().setPreference('zh');
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe('UpdateNotice', () => {
  it('Homebrew Cask 安装检测到新版本时提供一键复制 brew upgrade 命令，不覆盖包管理器状态', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });

    invokeMock.mockImplementation((command: string) => {
      if (command === 'latest_release') return Promise.resolve('1.3.0');
      if (command === 'install_channel') return Promise.resolve('homebrew');
      return Promise.resolve();
    });

    await act(async () => {
      root.render(<UpdateNotice />);
    });

    expect(container.textContent).toContain('DataOmni 1.3.0 已发布。');
    const copyButton = [...container.querySelectorAll('button')].find((button) =>
      (button.textContent ?? '').includes('brew upgrade'),
    );
    expect(copyButton).toBeDefined();

    await act(async () => {
      copyButton?.click();
    });
    expect(writeText).toHaveBeenCalledWith('brew upgrade --cask dataomni');
    expect(container.textContent).toContain('已复制升级命令');
  });

  it('独立安装包只显示查看发布说明，不渲染 brew upgrade 按钮', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'latest_release') return Promise.resolve('1.3.0');
      if (command === 'install_channel') return Promise.resolve('standalone');
      return Promise.resolve();
    });

    await act(async () => {
      root.render(<UpdateNotice />);
    });

    expect(container.textContent).toContain('DataOmni 1.3.0 已发布。');
    expect(container.textContent).not.toContain('brew upgrade');
  });
});
