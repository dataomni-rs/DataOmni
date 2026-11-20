// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentCliSection } from './AgentCliSection';
import { useLanguageStore } from '../stores/languageStore';

const { invokeMock, saveMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  saveMock: vi.fn()
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: saveMock
}));

describe('AgentCliSection', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    useLanguageStore.getState().setPreference('en');
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    invokeMock.mockReset();
    saveMock.mockReset();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('renders detected executable snippets and filters read-only audit records without delete controls', async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === 'agent_cli_info') {
        return Promise.resolve({
          executablePath: '/Applications/DataOmni.app/Contents/MacOS/dataomni',
          onPath: true,
          os: 'macos',
          openConnectionCount: 2,
          auditLogPath: '/Users/demo/Library/Logs/com.dataomni.app/dataomni-cli.log',
          guideMarkdown: '# DataOmni skill'
        });
      }
      if (command === 'read_agent_audit_log') {
        return Promise.resolve([
          {
            timestamp: '2026-11-20T11:00:00+08:00',
            command: 'query',
            connection: 'dev-pg',
            firstKeyword: 'SELECT',
            length: 24,
            sha256: '1111222233334444',
            result: 'ok',
            ms: 12
          },
          {
            timestamp: '2026-11-20T10:59:00+08:00',
            command: 'query',
            connection: 'dev-pg',
            firstKeyword: 'DROP',
            length: 18,
            sha256: '5555666677778888',
            result: 'refused',
            ms: 1
          }
        ]);
      }
      return Promise.resolve(null);
    });

    await act(async () => {
      root.render(<AgentCliSection />);
    });

    expect(container.textContent).toContain('/Applications/DataOmni.app/Contents/MacOS/dataomni');
    expect(container.textContent).toContain('claude mcp add dataomni -- "/Applications/DataOmni.app/Contents/MacOS/dataomni" mcp');
    expect(container.textContent).toContain('"mcpServers"');
    expect(container.textContent).toContain('2 open connections');
    expect(container.textContent).toContain('SELECT');
    expect(container.textContent).toContain('DROP');

    // 按 result 筛选 refused
    const select = container.querySelector('select') as HTMLSelectElement;
    expect(select).not.toBeNull();
    await act(async () => {
      select.value = 'refused';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    expect(container.textContent).toContain('DROP');
    expect(container.textContent).not.toContain('SELECT');
    expect(container.textContent).toContain('1 record');
  });
});
