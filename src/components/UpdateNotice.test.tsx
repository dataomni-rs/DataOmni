/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
const getVersionMock = vi.fn();
const listenMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: () => getVersionMock(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

const { UpdateNotice } = await import('./UpdateNotice');
const { useLanguageStore } = await import('../stores/languageStore');

let container: HTMLDivElement;
let root: Root;

function findButton(labelSubstring: string): HTMLButtonElement | undefined {
  return [...document.body.querySelectorAll('button')].find((button) =>
    (button.textContent ?? '').includes(labelSubstring),
  );
}

beforeEach(() => {
  localStorage.clear();
  invokeMock.mockReset();
  getVersionMock.mockReset();
  listenMock.mockReset();
  getVersionMock.mockResolvedValue('1.2.1');
  listenMock.mockResolvedValue(() => {});
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
  it('Homebrew Cask 安装检测到新版本时提供一键复制 brew upgrade 命令，不显示自覆盖升级入口', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });

    invokeMock.mockImplementation((command: string) => {
      if (command === 'latest_release') return Promise.resolve('1.3.0');
      if (command === 'install_channel') return Promise.resolve('homebrew');
      return Promise.resolve(null);
    });

    await act(async () => {
      root.render(<UpdateNotice />);
    });

    expect(container.textContent).toContain('DataOmni 1.3.0 已发布。');
    expect(findButton('立即升级')).toBeUndefined();
    expect(invokeMock).not.toHaveBeenCalledWith('check_app_update');

    const copyButton = findButton('brew upgrade');
    expect(copyButton).toBeDefined();

    await act(async () => {
      copyButton?.click();
    });
    expect(writeText).toHaveBeenCalledWith('brew upgrade --cask dataomni');
    expect(container.textContent).toContain('已复制升级命令');
  });

  it('独立安装包若远端尚无 latest.json 签名包时优雅回退为仅查看发布说明', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'latest_release') return Promise.resolve('1.3.0');
      if (command === 'install_channel') return Promise.resolve('standalone');
      if (command === 'check_app_update') return Promise.resolve(null);
      return Promise.resolve();
    });

    await act(async () => {
      root.render(<UpdateNotice />);
    });

    expect(container.textContent).toContain('DataOmni 1.3.0 已发布。');
    expect(findButton('立即升级')).toBeUndefined();
    expect(container.textContent).not.toContain('brew upgrade');
  });

  it('独立安装包走三步显式升级：绝不自动下载或静默重启，且未提交事务守卫拒绝时中止安装', async () => {
    const onBeforeRestart = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);

    invokeMock.mockImplementation((command: string) => {
      if (command === 'latest_release') return Promise.resolve('1.3.0');
      if (command === 'install_channel') return Promise.resolve('standalone');
      if (command === 'check_app_update') {
        return Promise.resolve({
          version: '1.3.0',
          currentVersion: '1.2.1',
          notes: '- 新增应用内签名平滑更新',
          pubDate: '2026-11-19T08:00:00Z',
        });
      }
      if (command === 'download_app_update') {
        return Promise.resolve({ version: '1.3.0', bytesLen: 1024 });
      }
      if (command === 'install_app_update') {
        return Promise.resolve();
      }
      return Promise.resolve();
    });

    await act(async () => {
      root.render(<UpdateNotice onBeforeRestart={onBeforeRestart} />);
    });

    // 第一步：只展示横幅与「立即升级」，绝不后台静默下载
    expect(invokeMock).not.toHaveBeenCalledWith('download_app_update');
    expect(invokeMock).not.toHaveBeenCalledWith('install_app_update');

    const upgradeButton = findButton('立即升级');
    expect(upgradeButton).toBeDefined();

    // 第二步：打开确认框并显式点击下载与 Ed25519 校验
    await act(async () => {
      upgradeButton?.click();
    });
    expect(document.body.textContent).toContain('升级到 DataOmni 1.3.0');
    expect(document.body.textContent).toContain('新增应用内签名平滑更新');
    expect(invokeMock).not.toHaveBeenCalledWith('download_app_update');

    await act(async () => {
      findButton('下载并校验更新')?.click();
    });
    expect(invokeMock).toHaveBeenCalledWith('download_app_update');
    // 验签完成后停在确认安装态，绝不自动调用 install_app_update 重启
    expect(document.body.textContent).toContain('更新包已通过 Ed25519 签名校验');
    expect(invokeMock).not.toHaveBeenCalledWith('install_app_update');

    // 第三步（守卫拦截）：用户有未提交事务并点了取消，不调 install_app_update 且保留已验签状态
    await act(async () => {
      findButton('立即重启完成更新')?.click();
    });
    expect(onBeforeRestart).toHaveBeenCalledTimes(1);
    expect(invokeMock).not.toHaveBeenCalledWith('install_app_update');
    expect(findButton('立即重启完成更新')).toBeDefined();

    // 第三步（守卫放行）：事务已处理、快照已冲刷后，正式执行安装重启
    await act(async () => {
      findButton('立即重启完成更新')?.click();
    });
    expect(onBeforeRestart).toHaveBeenCalledTimes(2);
    expect(invokeMock).toHaveBeenCalledWith('install_app_update');
  });

  it('Ed25519 签名校验失败时展示明确拒绝提示，绝不进入可安装状态', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'latest_release') return Promise.resolve('1.3.0');
      if (command === 'install_channel') return Promise.resolve('standalone');
      if (command === 'check_app_update') {
        return Promise.resolve({
          version: '1.3.0',
          currentVersion: '1.2.1',
          notes: null,
          pubDate: null,
        });
      }
      if (command === 'download_app_update') {
        return Promise.reject(
          new Error('DATAOMNI_UPDATE_SIGNATURE_INVALID: signature verification failed'),
        );
      }
      return Promise.resolve();
    });

    await act(async () => {
      root.render(<UpdateNotice />);
    });

    await act(async () => {
      findButton('立即升级')?.click();
    });
    await act(async () => {
      findButton('下载并校验更新')?.click();
    });

    expect(document.body.textContent).toContain('更新包 Ed25519 签名校验未通过，已拒绝安装');
    expect(findButton('立即重启完成更新')).toBeUndefined();
    expect(invokeMock).not.toHaveBeenCalledWith('install_app_update');
  });
});
