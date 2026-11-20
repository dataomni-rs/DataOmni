import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Loader2 } from 'lucide-react';
import { useLanguageStore } from '../stores/languageStore';
import { useSettingsStore } from '../stores/settingsStore';
import {
  DEFAULT_BASE_URLS,
  aiAvailable,
  cancelAiRequest,
  completeWithAi,
  createAiRequestId,
  isAiCancelled,
  isAiKeyMissing,
  isLocalAiEndpoint,
  type AiEndpointConfig,
  type AiEnvironmentPolicy,
  type AiProtocol
} from '../utils/aiSettings';
import { describeError } from '../utils/describeError';
import { PLAIN_TEXT_INPUT } from './FormControls';

const INPUT_CLASS =
  'w-64 shrink-0 rounded-control border border-line-strong bg-surface px-2 py-1 text-sm text-fg outline-none focus:border-accent';
const BUTTON_CLASS =
  'rounded-control border border-line-strong px-2.5 py-1 text-xs text-fg hover:bg-surface-hover disabled:opacity-50';

type TestStatus =
  | { state: 'idle' }
  | { state: 'running'; target: 'primary' | 'local' }
  | { state: 'ok'; target: 'primary' | 'local'; elapsedMs: number }
  | { state: 'error'; target: 'primary' | 'local'; message: string };

/**
 * 设置里的 AI 一节。构建里没编进 AI（`--no-default-features`）时整节不出现——
 * 能看见一个开关却打不开，比没有更让人困惑。
 *
 * Key 只进不出：填了就存进系统钥匙串，界面上只显示「已保存」，不回显。
 * 本机回环端点（Ollama / LM Studio）支持免 Key 直接探测与调用。
 */
export function AiSettingsSection() {
  const t = useLanguageStore((state) => state.t);
  const ai = useSettingsStore((state) => state.ai);
  const setAi = useSettingsStore((state) => state.setAi);
  const [available, setAvailable] = useState(false);
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [keyDraft, setKeyDraft] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [testStatus, setTestStatus] = useState<TestStatus>({ state: 'idle' });
  const activeRequestIdRef = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void aiAvailable().then((value) => {
      if (cancelled || !value) return;
      setAvailable(true);
      invoke<boolean>('ai_has_key')
        .then((present) => !cancelled && setHasKey(present))
        .catch((caught) => !cancelled && setError(describeError(caught)));
    });
    return () => {
      cancelled = true;
      if (activeRequestIdRef.current) {
        void cancelAiRequest(activeRequestIdRef.current);
      }
    };
  }, []);

  if (!available) {
    return null;
  }

  const primaryIsLocal = isLocalAiEndpoint(ai.baseUrl);
  const localPresetIsLocal = isLocalAiEndpoint(ai.localPreset.baseUrl);

  const changeProtocol = (protocol: AiProtocol) => {
    const untouched = ai.baseUrl.trim() === '' || Object.values(DEFAULT_BASE_URLS).includes(ai.baseUrl.trim());
    setAi({ protocol, ...(untouched ? { baseUrl: DEFAULT_BASE_URLS[protocol] } : {}) });
  };

  const saveKey = async () => {
    setError(null);
    try {
      await invoke('ai_save_key', { key: keyDraft });
      setHasKey(keyDraft.trim() !== '');
      setKeyDraft('');
    } catch (caught) {
      setError(describeError(caught));
    }
  };

  const runTest = async (target: 'primary' | 'local') => {
    const endpoint: AiEndpointConfig =
      target === 'primary'
        ? {
            protocol: ai.protocol,
            baseUrl: ai.baseUrl.trim(),
            model: ai.model.trim(),
            profileId: 'default'
          }
        : {
            protocol: ai.localPreset.protocol,
            baseUrl: ai.localPreset.baseUrl.trim(),
            model: ai.localPreset.model.trim(),
            profileId: 'local'
          };
    if (!endpoint.baseUrl || !endpoint.model) {
      return;
    }
    const requestId = createAiRequestId(`test-${target}`);
    activeRequestIdRef.current = requestId;
    setTestStatus({ state: 'running', target });
    const started = performance.now();
    try {
      await completeWithAi(
        ai,
        { system: 'Reply with OK.', user: 'ping' },
        { requestId, maxTokens: 16, endpoint }
      );
      setTestStatus({ state: 'ok', target, elapsedMs: Math.max(1, Math.round(performance.now() - started)) });
    } catch (caught) {
      if (isAiCancelled(caught)) {
        setTestStatus({ state: 'idle' });
      } else if (isAiKeyMissing(caught)) {
        setTestStatus({ state: 'error', target, message: t('error.backend.aiKeyMissing') });
      } else {
        setTestStatus({ state: 'error', target, message: describeError(caught) });
      }
    } finally {
      if (activeRequestIdRef.current === requestId) {
        activeRequestIdRef.current = null;
      }
    }
  };

  const cancelTest = () => {
    const id = activeRequestIdRef.current;
    if (id) {
      void cancelAiRequest(id);
    }
  };

  const row = 'flex items-center justify-between gap-3 rounded-control px-2 py-1.5 hover:bg-surface-hover';

  return (
    <div className="border-t border-line px-5 py-4">
      <h3 className="text-sm font-medium text-fg">{t('settings.ai.title')}</h3>
      <p className="mt-1 text-xs leading-relaxed text-fg-muted">{t('settings.ai.description')}</p>

      <div className="mt-3 space-y-1.5">
        <label className={row}>
          <span className="text-sm text-fg">{t('settings.ai.enabled')}</span>
          <input type="checkbox" checked={ai.enabled} onChange={(event) => setAi({ enabled: event.target.checked })} />
        </label>

        <label className={row}>
          <span className="text-sm text-fg">{t('settings.ai.protocol')}</span>
          <select
            value={ai.protocol}
            onChange={(event) => changeProtocol(event.target.value as AiProtocol)}
            className="shrink-0 rounded-control border border-line-strong bg-surface px-2 py-1 text-sm text-fg"
          >
            <option value="openai">{t('settings.ai.protocol.openai')}</option>
            <option value="anthropic">Anthropic</option>
          </select>
        </label>

        <label className={row}>
          <span className="text-sm text-fg">{t('settings.ai.baseUrl')}</span>
          <input value={ai.baseUrl} onChange={(event) => setAi({ baseUrl: event.target.value })} className={INPUT_CLASS} {...PLAIN_TEXT_INPUT} />
        </label>

        <label className={row}>
          <span className="text-sm text-fg">{t('settings.ai.model')}</span>
          <input
            value={ai.model}
            onChange={(event) => setAi({ model: event.target.value })}
            placeholder={ai.protocol === 'anthropic' ? 'claude-sonnet-5' : 'deepseek-chat'}
            className={INPUT_CLASS}
            {...PLAIN_TEXT_INPUT}
          />
        </label>

        <div className={row}>
          <span className="text-sm text-fg">
            {t('settings.ai.key')}
            <span className="ml-2 text-xs text-fg-subtle">
              {primaryIsLocal && !hasKey
                ? t('settings.ai.keyOptionalLocal')
                : hasKey === null
                  ? ''
                  : hasKey
                    ? t('settings.ai.keySaved')
                    : t('settings.ai.keyMissing')}
            </span>
          </span>
          <span className="flex shrink-0 items-center gap-2">
            <input
              type="password"
              value={keyDraft}
              onChange={(event) => setKeyDraft(event.target.value)}
              placeholder={hasKey ? t('settings.ai.keyReplace') : ''}
              aria-label={t('settings.ai.key')}
              className="w-44 rounded-control border border-line-strong bg-surface px-2 py-1 text-sm text-fg outline-none focus:border-accent"
              {...PLAIN_TEXT_INPUT}
            />
            <button
              type="button"
              onClick={() => void saveKey()}
              disabled={keyDraft.trim() === '' && !hasKey}
              className="rounded-control border border-line-strong px-2 py-1 text-sm text-fg hover:bg-surface-hover disabled:opacity-50"
            >
              {keyDraft.trim() === '' && hasKey ? t('settings.ai.keyDelete') : t('settings.ai.keySave')}
            </button>
          </span>
        </div>

        <div className="flex flex-wrap items-center gap-2 px-2 pt-1">
          <button
            type="button"
            onClick={() => void runTest('primary')}
            disabled={testStatus.state === 'running' || ai.baseUrl.trim() === '' || ai.model.trim() === ''}
            className={`inline-flex items-center gap-1.5 ${BUTTON_CLASS}`}
          >
            {testStatus.state === 'running' && testStatus.target === 'primary' && (
              <Loader2 size={12} className="animate-spin" />
            )}
            {t('settings.ai.testConnection')}
          </button>
          {testStatus.state === 'running' && testStatus.target === 'primary' && (
            <button type="button" onClick={cancelTest} className={BUTTON_CLASS}>
              {t('common.cancel')}
            </button>
          )}
          {testStatus.state === 'ok' && testStatus.target === 'primary' && (
            <span className="text-xs text-success">
              {t('settings.ai.testSuccess', { ms: testStatus.elapsedMs })}
            </span>
          )}
          {testStatus.state === 'error' && testStatus.target === 'primary' && (
            <span className="min-w-0 flex-1 break-words text-xs text-danger">{testStatus.message}</span>
          )}
        </div>

        <div className="mt-2 border-t border-line pt-2.5">
          <label className={row}>
            <span className="min-w-0 text-sm text-fg">{t('settings.ai.environmentPolicy')}</span>
            <select
              value={ai.environmentPolicy}
              onChange={(event) => setAi({ environmentPolicy: event.target.value as AiEnvironmentPolicy })}
              className="shrink-0 rounded-control border border-line-strong bg-surface px-2 py-1 text-xs text-fg"
            >
              <option value="local-only-sensitive">{t('settings.ai.policy.localOnlySensitive')}</option>
              <option value="block-production">{t('settings.ai.policy.blockProduction')}</option>
              <option value="allow-all">{t('settings.ai.policy.allowAll')}</option>
            </select>
          </label>
          <p className="px-2 text-[11px] text-fg-subtle">{t('settings.ai.environmentPolicyHint')}</p>

          <div className="mt-2 rounded-control border border-line bg-surface-subtle p-2.5 space-y-1.5">
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs font-medium text-fg">{t('settings.ai.localPresetTitle')}</span>
              <span className="text-[11px] text-fg-subtle">{t('settings.ai.keyOptionalLocal')}</span>
            </div>
            <label className="flex items-center justify-between gap-3 py-0.5">
              <span className="text-xs text-fg-muted">{t('settings.ai.baseUrl')}</span>
              <input
                value={ai.localPreset.baseUrl}
                onChange={(event) =>
                  setAi({ localPreset: { ...ai.localPreset, baseUrl: event.target.value } })
                }
                placeholder="http://localhost:11434/v1"
                className="w-60 rounded-control border border-line-strong bg-surface px-2 py-1 text-xs text-fg outline-none focus:border-accent"
                {...PLAIN_TEXT_INPUT}
              />
            </label>
            <label className="flex items-center justify-between gap-3 py-0.5">
              <span className="text-xs text-fg-muted">{t('settings.ai.model')}</span>
              <input
                value={ai.localPreset.model}
                onChange={(event) =>
                  setAi({ localPreset: { ...ai.localPreset, model: event.target.value } })
                }
                placeholder="qwen2.5-coder:7b"
                className="w-60 rounded-control border border-line-strong bg-surface px-2 py-1 text-xs text-fg outline-none focus:border-accent"
                {...PLAIN_TEXT_INPUT}
              />
            </label>
            {ai.localPreset.baseUrl.trim() !== '' && !localPresetIsLocal && (
              <p className="text-[11px] text-warning">{t('settings.ai.localPresetMustBeLoopback')}</p>
            )}
            <div className="flex flex-wrap items-center gap-2 pt-1">
              <button
                type="button"
                onClick={() => void runTest('local')}
                disabled={
                  testStatus.state === 'running'
                  || !localPresetIsLocal
                  || ai.localPreset.model.trim() === ''
                }
                className={`inline-flex items-center gap-1.5 ${BUTTON_CLASS}`}
              >
                {testStatus.state === 'running' && testStatus.target === 'local' && (
                  <Loader2 size={12} className="animate-spin" />
                )}
                {t('settings.ai.testLocalConnection')}
              </button>
              {testStatus.state === 'running' && testStatus.target === 'local' && (
                <button type="button" onClick={cancelTest} className={BUTTON_CLASS}>
                  {t('common.cancel')}
                </button>
              )}
              {testStatus.state === 'ok' && testStatus.target === 'local' && (
                <span className="text-xs text-success">
                  {t('settings.ai.testSuccess', { ms: testStatus.elapsedMs })}
                </span>
              )}
              {testStatus.state === 'error' && testStatus.target === 'local' && (
                <span className="min-w-0 flex-1 break-words text-xs text-danger">{testStatus.message}</span>
              )}
            </div>
          </div>
        </div>
      </div>

      {error && <p className="mt-2 px-2 text-xs text-danger break-words">{error}</p>}
      <p className="mt-2 px-2 text-xs text-fg-subtle">{t('settings.ai.privacy')}</p>
    </div>
  );
}
