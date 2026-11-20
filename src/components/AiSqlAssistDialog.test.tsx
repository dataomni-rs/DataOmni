// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AiSqlAssistDialog } from './AiSqlAssistDialog';
import { useLanguageStore } from '../stores/languageStore';
import { useSettingsStore } from '../stores/settingsStore';
import { DEFAULT_AI_SETTINGS } from '../utils/aiSettings';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn()
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock
}));

describe('AiSqlAssistDialog', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    useLanguageStore.getState().setPreference('en');
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    invokeMock.mockReset();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
  });

  it('blocks cloud endpoints on production connections when local-only-sensitive policy is active', async () => {
    useSettingsStore.setState({
      ai: {
        ...DEFAULT_AI_SETTINGS,
        enabled: true,
        baseUrl: 'https://api.openai.com/v1',
        model: 'gpt-4o-mini',
        environmentPolicy: 'local-only-sensitive',
        localPreset: { protocol: 'openai', baseUrl: 'http://localhost:11434/v1', model: '' }
      }
    });
    invokeMock.mockResolvedValue('');

    await act(async () => {
      root.render(
        <AiSqlAssistDialog
          connectionId="prod-1"
          dialect="PostgreSQL"
          environment="production"
          relations={[
            { schema: 'public', name: 'orders', kind: 'table', columns: [{ name: 'id', dataType: 'bigint' }] }
          ]}
          mode={{ kind: 'nl-to-sql' }}
          onApplySql={() => {}}
          onClose={() => {}}
        />
      );
    });

    expect(container.textContent).toContain('localhost model endpoint');
    expect(container.textContent).toContain('0 / 15');
    // 绝不向外发 ai_complete 请求
    expect(invokeMock).not.toHaveBeenCalledWith('ai_complete', expect.anything());
  });

  it('renders transparent sent payload and inserts drafted SQL without executing it', async () => {
    useSettingsStore.setState({
      ai: {
        ...DEFAULT_AI_SETTINGS,
        enabled: true,
        baseUrl: 'https://api.openai.com/v1',
        model: 'gpt-4o-mini',
        environmentPolicy: 'local-only-sensitive'
      }
    });
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'get_connection_semantics') return Promise.resolve('### orders\n- status: 1=paid');
      if (cmd === 'ai_complete') {
        return Promise.resolve(
          JSON.stringify({
            sql: 'SELECT count(*) FROM public.orders WHERE status = 1;',
            explanation: 'Counts paid orders.',
            warnings: []
          })
        );
      }
      return Promise.resolve(null);
    });

    const applied: string[] = [];
    await act(async () => {
      root.render(
        <AiSqlAssistDialog
          connectionId="dev-1"
          dialect="PostgreSQL"
          environment="development"
          relations={[
            {
              schema: 'public',
              name: 'orders',
              kind: 'table',
              columns: [{ name: 'id', dataType: 'bigint' }, { name: 'status', dataType: 'int' }]
            }
          ]}
          mode={{ kind: 'nl-to-sql' }}
          onApplySql={(sql) => applied.push(sql)}
          onClose={() => {}}
        />
      );
    });

    const textarea = container.querySelector('textarea') as HTMLTextAreaElement;
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
      setter?.call(textarea, 'Count paid orders');
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
    });

    const genBtn = [...container.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Generate SQL')
    ) as HTMLButtonElement;
    await act(async () => {
      genBtn.click();
    });

    expect(container.textContent).toContain('SELECT count(*) FROM public.orders WHERE status = 1;');
    expect(container.textContent).toContain('status: 1=paid');

    const insertBtn = [...container.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Insert into editor')
    ) as HTMLButtonElement;
    await act(async () => {
      insertBtn.click();
    });
    expect(applied).toEqual(['SELECT count(*) FROM public.orders WHERE status = 1;']);
  });
});
