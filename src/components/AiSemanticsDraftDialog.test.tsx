// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AiSemanticsDraftDialog } from './AiSemanticsDraftDialog';
import { useLanguageStore } from '../stores/languageStore';
import { useSettingsStore } from '../stores/settingsStore';
import { DEFAULT_AI_SETTINGS } from '../utils/aiSettings';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn()
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock
}));

describe('AiSemanticsDraftDialog', () => {
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

  it('drafts AI-inferred table semantics and saves reviewed Markdown into CM4 connection semantics', async () => {
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
      if (cmd === 'get_connection_semantics') return Promise.resolve('');
      if (cmd === 'ai_complete') {
        return Promise.resolve(
          JSON.stringify({
            overview: 'Commerce schema',
            tables: [
              {
                table: 'orders',
                summary: 'Customer purchase orders',
                columns: [{ name: 'status', meaning: 'Lifecycle status code' }]
              }
            ]
          })
        );
      }
      if (cmd === 'save_connection_semantics') return Promise.resolve(null);
      return Promise.resolve(null);
    });

    await act(async () => {
      root.render(
        <AiSemanticsDraftDialog
          connectionId="conn-dev-1"
          connectionName="dev-shop"
          dialect="PostgreSQL"
          environment="development"
          tables={[
            {
              schema: 'public',
              name: 'orders',
              columns: [{ name: 'status', dataType: 'smallint', isPrimaryKey: false, isNullable: false }]
            }
          ]}
          links={[]}
          onClose={() => {}}
        />
      );
    });

    const draftBtn = [...container.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Draft semantics')
    ) as HTMLButtonElement;
    await act(async () => {
      draftBtn.click();
    });

    const textarea = container.querySelector('textarea') as HTMLTextAreaElement;
    expect(textarea.value).toContain('<!-- AI-inferred: review before trusting -->');
    expect(textarea.value).toContain('### orders');
    expect(textarea.value).toContain('Lifecycle status code');

    const saveBtn = [...container.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Save to semantics')
    ) as HTMLButtonElement;
    await act(async () => {
      saveBtn.click();
    });

    expect(invokeMock).toHaveBeenCalledWith('save_connection_semantics', {
      id: 'conn-dev-1',
      content: textarea.value
    });
  });
});
