// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProfileResult } from '../contracts/analytics';
import { useLanguageStore } from '../stores/languageStore';
import { useSettingsStore } from '../stores/settingsStore';
import { AiProfileInsightsPanel } from './AiProfileInsightsPanel';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

const SAMPLE_PROFILE: ProfileResult = {
  analyticsId: 'sess-ai',
  rowCount: 1000,
  columnCount: 2,
  elapsedMs: 12,
  columns: [
    {
      name: 'email',
      inferredType: 'text',
      duckdbType: 'VARCHAR',
      precisionDegraded: false,
      totalCount: 1000,
      nullCount: 50,
      nullRatio: 0.05,
      distinctCount: 950,
      distinctRatio: 1,
      min: 'alice.secret@example.com',
      max: 'zoe.secret@example.com',
      minLength: 10,
      maxLength: 30,
      emptyStringCount: 3,
      bins: [],
      topValues: [{ value: 'alice.secret@example.com', count: 1, ratio: 0.001 }]
    },
    {
      name: 'amount',
      inferredType: 'numeric',
      duckdbType: 'DOUBLE',
      precisionDegraded: false,
      totalCount: 1000,
      nullCount: 0,
      nullRatio: 0,
      distinctCount: 500,
      distinctRatio: 0.5,
      min: '-25',
      max: '9000',
      p25: 20,
      p50: 80,
      p75: 200,
      p95: 400,
      mean: 350,
      stddev: 1200,
      bins: [],
      topValues: []
    }
  ]
};

async function flush() {
  for (let i = 0; i < 6; i += 1) {
    await act(async () => {
      await Promise.resolve();
    });
  }
}

describe('AiProfileInsightsPanel', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    invoke.mockReset();
    useLanguageStore.getState().setPreference('zh');
    useSettingsStore.setState((state) => ({
      ...state,
      ai: {
        ...state.ai,
        enabled: true,
        protocol: 'openai',
        baseUrl: 'https://api.openai.com/v1',
        model: 'gpt-4o-mini',
        localOnlyForSensitive: true,
        localPreset: { protocol: 'openai', baseUrl: '', model: '' }
      }
    }));
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

  it('sends only redacted aggregate metrics on explicit click and renders structured insights', async () => {
    invoke.mockResolvedValueOnce(
      JSON.stringify({
        summary: 'amount 存在负值且均值显著高于 P95',
        findings: [
          {
            column: 'amount',
            severity: 'high',
            title: '负值金额异常',
            detail: '建议排查退款冲正逻辑',
            suggestedSql: 'SELECT * FROM result_set WHERE "amount" < 0'
          }
        ]
      })
    );

    await act(async () => {
      root.render(
        <AiProfileInsightsPanel
          title="orders"
          dialect="sqlite"
          environment="development"
          profile={SAMPLE_PROFILE}
          onClose={vi.fn()}
        />
      );
    });

    // 不自动发请求
    expect(invoke).not.toHaveBeenCalled();

    const runBtn = Array.from(container.querySelectorAll('button')).find((b) =>
      b.textContent?.includes('生成体检解读')
    );
    expect(runBtn).toBeTruthy();

    await act(async () => {
      runBtn?.click();
    });
    await flush();

    expect(invoke).toHaveBeenCalledTimes(1);
    const callArgs = invoke.mock.calls[0][1] as { request: { user: string; system: string } };
    expect(callArgs.request.user).not.toContain('alice.secret@example.com');
    expect(callArgs.request.user).not.toContain('zoe.secret@example.com');
    expect(container.textContent).toContain('amount 存在负值且均值显著高于 P95');
    expect(container.textContent).toContain('负值金额异常');
    expect(container.textContent).toContain('AI 推测');
  });

  it('blocks cloud AI on PROD when sensitive local-only guard is enabled', async () => {
    await act(async () => {
      root.render(
        <AiProfileInsightsPanel
          title="orders"
          environment="production"
          profile={SAMPLE_PROFILE}
          onClose={vi.fn()}
        />
      );
    });

    const runBtn = Array.from(container.querySelectorAll('button')).find((b) =>
      b.textContent?.includes('生成体检解读')
    );
    expect(runBtn?.disabled).toBe(true);
    expect(container.textContent).toContain('本机回环模型');
  });
});
