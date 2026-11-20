import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_AI_SETTINGS,
  DEFAULT_LOCAL_AI_PRESET,
  aiConfigured,
  isAiCancelled,
  isAiKeyMissing,
  isLocalAiEndpoint,
  loadAiSettings,
  resolveAiEndpointForEnvironment,
  saveAiSettings,
  type AiSettings
} from './aiSettings';

describe('aiSettings', () => {
  // 测试环境没有 localStorage，照 gridColumns.test.ts 的做法换一个
  beforeEach(() => {
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => {
        store.set(key, value);
      }
    });
  });

  it('没存过就是默认：关着', () => {
    expect(loadAiSettings()).toEqual(DEFAULT_AI_SETTINGS);
    expect(DEFAULT_AI_SETTINGS.enabled).toBe(false);
  });

  it('存进去读得回来，且无损兼容旧版 dataomni.ai-settings JSON', () => {
    const legacy = {
      enabled: true,
      protocol: 'anthropic' as const,
      baseUrl: 'https://api.anthropic.com',
      model: 'claude-sonnet-5'
    };
    localStorage.setItem('dataomni.ai-settings', JSON.stringify(legacy));
    expect(loadAiSettings()).toEqual({
      ...legacy,
      environmentPolicy: 'local-only-sensitive',
      localPreset: DEFAULT_LOCAL_AI_PRESET
    });

    const full: AiSettings = {
      ...legacy,
      environmentPolicy: 'block-production',
      localPreset: { protocol: 'openai', baseUrl: 'http://127.0.0.1:11434/v1', model: 'qwen2.5-coder' }
    };
    saveAiSettings(full);
    expect(loadAiSettings()).toEqual(full);
  });

  it('坏掉的一项回到默认，别的保留', () => {
    localStorage.setItem(
      'dataomni.ai-settings',
      JSON.stringify({ enabled: 'yes', protocol: 'gemini', model: 'm', environmentPolicy: 'invalid' })
    );
    expect(loadAiSettings()).toEqual({
      enabled: false,
      protocol: 'openai',
      baseUrl: 'https://api.openai.com/v1',
      model: 'm',
      environmentPolicy: 'local-only-sensitive',
      localPreset: DEFAULT_LOCAL_AI_PRESET
    });
    localStorage.setItem('dataomni.ai-settings', '{not json');
    expect(loadAiSettings()).toEqual(DEFAULT_AI_SETTINGS);
  });

  it('开着且填了地址与模型才算配好', () => {
    expect(aiConfigured({ ...DEFAULT_AI_SETTINGS, enabled: true, model: 'deepseek-flash' })).toBe(true);
    expect(aiConfigured({ ...DEFAULT_AI_SETTINGS, enabled: true, model: ' ' })).toBe(false);
    expect(aiConfigured({ ...DEFAULT_AI_SETTINGS, model: 'deepseek-flash' })).toBe(false);
  });

  it('AI0 缺 Key 错误码匹配回归门：认 DATAOMNI_AI_KEY_MISSING 而非连接口令错误码', () => {
    expect(isAiKeyMissing('DATAOMNI_AI_KEY_MISSING')).toBe(true);
    expect(isAiKeyMissing({ message: 'DATAOMNI_AI_KEY_MISSING: no entry' })).toBe(true);
    expect(isAiKeyMissing('DATAOMNI_CREDENTIAL_MISSING')).toBe(false);
    expect(isAiKeyMissing({ message: 'DATAOMNI_CREDENTIAL_MISSING' })).toBe(false);
    expect(isAiCancelled({ message: 'DATAOMNI_AI_CANCELLED' })).toBe(true);
    expect(isAiCancelled('DATAOMNI_AI_REQUEST_FAILED')).toBe(false);
  });

  it('严格判定本机回环端点，拒绝伪装域名与公网地址', () => {
    expect(isLocalAiEndpoint('http://localhost:11434/v1')).toBe(true);
    expect(isLocalAiEndpoint('http://127.0.0.1:1234/v1')).toBe(true);
    expect(isLocalAiEndpoint('http://[::1]:11434/v1')).toBe(true);
    expect(isLocalAiEndpoint('https://api.openai.com/v1')).toBe(false);
    expect(isLocalAiEndpoint('http://localhost.evil.com/v1')).toBe(false);
    expect(isLocalAiEndpoint('http://10.0.0.5:11434/v1')).toBe(false);
  });

  it('AI0 环境路由守卫：STAGING/PROD 开启仅本地模型时拦截公网端点并可回落本地预设', () => {
    const cloudOnly: AiSettings = {
      ...DEFAULT_AI_SETTINGS,
      enabled: true,
      baseUrl: 'https://api.openai.com/v1',
      model: 'gpt-4o-mini',
      environmentPolicy: 'local-only-sensitive',
      localPreset: { protocol: 'openai', baseUrl: 'http://localhost:11434/v1', model: '' }
    };

    expect(resolveAiEndpointForEnvironment(cloudOnly, 'development')).toMatchObject({
      ok: true,
      usedLocalFallback: false,
      endpoint: { baseUrl: 'https://api.openai.com/v1' }
    });
    expect(resolveAiEndpointForEnvironment(cloudOnly, 'staging')).toEqual({
      ok: false,
      reason: 'local-only-required'
    });
    expect(resolveAiEndpointForEnvironment(cloudOnly, 'production')).toEqual({
      ok: false,
      reason: 'local-only-required'
    });

    const withLocalFallback: AiSettings = {
      ...cloudOnly,
      localPreset: { protocol: 'openai', baseUrl: 'http://127.0.0.1:11434/v1', model: 'qwen2.5-coder:7b' }
    };
    expect(resolveAiEndpointForEnvironment(withLocalFallback, 'production')).toEqual({
      ok: true,
      usedLocalFallback: true,
      endpoint: {
        protocol: 'openai',
        baseUrl: 'http://127.0.0.1:11434/v1',
        model: 'qwen2.5-coder:7b',
        profileId: 'local'
      }
    });

    const blockedProd: AiSettings = {
      ...withLocalFallback,
      environmentPolicy: 'block-production'
    };
    expect(resolveAiEndpointForEnvironment(blockedProd, 'production')).toEqual({
      ok: false,
      reason: 'blocked-production'
    });
  });
});
