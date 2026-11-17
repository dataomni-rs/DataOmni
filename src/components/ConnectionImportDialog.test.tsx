/**
 * @vitest-environment happy-dom
 */
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DatabaseType, type ConnectionProfile } from '../contracts/connection';
import { createDefaultConfig } from '../stores/connectionStore';
import { useLanguageStore } from '../stores/languageStore';
import { parseConnectionImportFile } from '../utils/externalConnections';
import { ConnectionImportDialog } from './ConnectionImportDialog';

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
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

const SAMPLE_NCX = `<?xml version="1.0" encoding="UTF-8"?>
<Connections Ver="2.0">
  <Connection ConnectionName="Already Here" ConnType="PGSQL" Host="127.0.0.1" Port="5432" Database="shop" UserName="app" />
  <Connection ConnectionName="New Analytics" ConnType="MYSQL" Host="db.internal" Port="3306" Database="bi" UserName="reader" SSH="true" SSH_Host="jump.internal" SSH_Port="22" SSH_UserName="ops" />
  <Connection ConnectionName="New Redis" ConnType="REDIS" Host="redis.internal" Port="6379" UserName="default" />
  <Connection ConnectionName="Legacy DB2" ConnType="DB2" Host="db2.corp" />
</Connections>`;

describe('ConnectionImportDialog', () => {
  it('pre-selects new profiles, marks existing duplicates disabled, lists unsupported items, and submits chosen profiles', async () => {
    const parsed = parseConnectionImportFile(SAMPLE_NCX);
    const existing: ConnectionProfile[] = [
      {
        ...createDefaultConfig(DatabaseType.PostgreSQL),
        id: 'local-1',
        name: 'Already Here',
        host: '127.0.0.1',
        port: 5432,
        database: 'shop',
        username: 'app',
        created_at: '',
        updated_at: '',
      },
    ];
    const onConfirm = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    await act(async () => {
      root.render(
        <ConnectionImportDialog
          parsed={parsed}
          existingConnections={existing}
          onConfirm={onConfirm}
          onClose={onClose}
        />,
      );
    });

    expect(container.textContent).toContain('Navicat (.ncx)');
    expect(container.textContent).toContain('Legacy DB2');
    expect(container.textContent).toContain('SSH');

    const checkboxes = [...container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')];
    expect(checkboxes).toHaveLength(3);
    expect(checkboxes[0].disabled).toBe(true);
    expect(checkboxes[0].checked).toBe(false);
    expect(checkboxes[1].checked).toBe(true);
    expect(checkboxes[2].checked).toBe(true);

    // Uncheck the third profile (New Redis) so only New Analytics is imported
    await act(async () => {
      checkboxes[2].click();
    });
    expect(checkboxes[2].checked).toBe(false);

    const submitBtn = [...container.querySelectorAll('button')].find((btn) =>
      (btn.textContent ?? '').includes('Import (1)'),
    );
    expect(submitBtn).toBeDefined();

    await act(async () => {
      submitBtn?.click();
    });

    expect(onConfirm).toHaveBeenCalledTimes(1);
    const submitted = onConfirm.mock.calls[0][0] as ConnectionProfile[];
    expect(submitted).toHaveLength(1);
    expect(submitted[0].name).toBe('New Analytics');
  });
});
