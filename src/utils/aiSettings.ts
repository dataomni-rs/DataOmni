import { invoke } from '@tauri-apps/api/core';
import type { ConnectionEnvironment } from '../contracts';

/** 两种协议覆盖所有模型服务：国内各家与本地 Ollama 都给 OpenAI 兼容接口 */
export type AiProtocol = 'anthropic' | 'openai';

/**
 * 环境路由策略（rfcs/ai-sql-assist.md AI0）：
 * - `local-only-sensitive`：预发/生产（`staging` / `production`）仅允许发往本机回环模型端点
 * - `block-production`：彻底禁止对生产连接触发 AI
 * - `allow-all`：全部环境允许使用当前主端点
 */
export type AiEnvironmentPolicy = 'local-only-sensitive' | 'block-production' | 'allow-all';

export interface AiLocalPreset {
  protocol: AiProtocol;
  baseUrl: string;
  model: string;
}

export interface AiEndpointConfig {
  protocol: AiProtocol;
  baseUrl: string;
  model: string;
  /** 省略或 `'default'` 读 `#ai`，`'local'` 读 `#ai:local` */
  profileId?: string;
}

export interface AiSettings {
  /** 默认关：AI 是附加功能，开了才有入口，也才会有请求发出去 */
  enabled: boolean;
  protocol: AiProtocol;
  /** Anthropic 写到域名；OpenAI 兼容写到 `/chat/completions` 之前那一段 */
  baseUrl: string;
  model: string;
  /** 预发/生产环境路由守卫策略 */
  environmentPolicy: AiEnvironmentPolicy;
  /** 可选本机私有模型预设（如 Ollama / LM Studio），供敏感环境自动回落或单独使用 */
  localPreset: AiLocalPreset;
}

export const DEFAULT_BASE_URLS: Record<AiProtocol, string> = {
  anthropic: 'https://api.anthropic.com',
  openai: 'https://api.openai.com/v1'
};

export const DEFAULT_LOCAL_AI_PRESET: AiLocalPreset = {
  protocol: 'openai',
  baseUrl: 'http://localhost:11434/v1',
  model: ''
};

export const DEFAULT_AI_SETTINGS: AiSettings = {
  enabled: false,
  protocol: 'openai',
  baseUrl: DEFAULT_BASE_URLS.openai,
  model: '',
  environmentPolicy: 'local-only-sensitive',
  localPreset: { ...DEFAULT_LOCAL_AI_PRESET }
};

const STORAGE_KEY = 'dataomni.ai-settings';

function isEnvironmentPolicy(value: unknown): value is AiEnvironmentPolicy {
  return value === 'local-only-sensitive' || value === 'block-production' || value === 'allow-all';
}

function normalizeLocalPreset(raw: unknown): AiLocalPreset {
  if (!raw || typeof raw !== 'object') {
    return { ...DEFAULT_LOCAL_AI_PRESET };
  }
  const candidate = raw as Partial<AiLocalPreset>;
  const protocol: AiProtocol =
    candidate.protocol === 'anthropic' || candidate.protocol === 'openai'
      ? candidate.protocol
      : DEFAULT_LOCAL_AI_PRESET.protocol;
  return {
    protocol,
    baseUrl: typeof candidate.baseUrl === 'string' ? candidate.baseUrl : DEFAULT_LOCAL_AI_PRESET.baseUrl,
    model: typeof candidate.model === 'string' ? candidate.model : ''
  };
}

/** 存的是 JSON，读回来逐项核对；无损兼容旧版 `dataomni.ai-settings`（缺新字段时补默认） */
export function loadAiSettings(): AiSettings {
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null') as Partial<AiSettings> | null;
    if (!raw || typeof raw !== 'object') {
      return { ...DEFAULT_AI_SETTINGS, localPreset: { ...DEFAULT_LOCAL_AI_PRESET } };
    }
    const protocol = raw.protocol === 'anthropic' || raw.protocol === 'openai' ? raw.protocol : DEFAULT_AI_SETTINGS.protocol;
    return {
      enabled: raw.enabled === true,
      protocol,
      baseUrl: typeof raw.baseUrl === 'string' ? raw.baseUrl : DEFAULT_BASE_URLS[protocol],
      model: typeof raw.model === 'string' ? raw.model : '',
      environmentPolicy: isEnvironmentPolicy(raw.environmentPolicy)
        ? raw.environmentPolicy
        : DEFAULT_AI_SETTINGS.environmentPolicy,
      localPreset: normalizeLocalPreset(raw.localPreset)
    };
  } catch {
    return { ...DEFAULT_AI_SETTINGS, localPreset: { ...DEFAULT_LOCAL_AI_PRESET } };
  }
}

export function saveAiSettings(settings: AiSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // 存储不可用时这一轮仍然生效，只是重启后回到默认
  }
}

/**
 * 判断端点 URL 是否严格指向本机回环地址（`localhost` / `127.0.0.1` / `::1`）。
 * 与后端 `services::ai::is_local_ai_endpoint` 同口径，拒绝 `localhost.evil.com`。
 */
export function isLocalAiEndpoint(baseUrl: string): boolean {
  try {
    const parsed = new URL(baseUrl.trim());
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
      return false;
    }
    const host = parsed.hostname.toLowerCase();
    return host === 'localhost' || host === '127.0.0.1' || host === '::1' || host === '[::1]';
  } catch {
    return false;
  }
}

/** 能不能发请求：开着、填了地址和模型（或已配置本地预设）。Key 在钥匙串里，本机端点免 Key */
export function aiConfigured(settings: AiSettings): boolean {
  if (!settings.enabled) {
    return false;
  }
  const primaryReady = settings.baseUrl.trim() !== '' && settings.model.trim() !== '';
  const localReady =
    settings.localPreset.baseUrl.trim() !== '' && settings.localPreset.model.trim() !== '';
  return primaryReady || localReady;
}

export type AiEnvironmentResolution =
  | { ok: true; endpoint: AiEndpointConfig; usedLocalFallback: boolean }
  | { ok: false; reason: 'not-configured' | 'blocked-production' | 'local-only-required' };

/**
 * 按连接环境与安全策略解析实际应使用的 AI 端点；被环境守卫拦截时明确返回原因，绝不静默外发。
 */
export function resolveAiEndpointForEnvironment(
  settings: AiSettings,
  environment: ConnectionEnvironment
): AiEnvironmentResolution {
  if (!settings.enabled) {
    return { ok: false, reason: 'not-configured' };
  }
  if (environment === 'production' && settings.environmentPolicy === 'block-production') {
    return { ok: false, reason: 'blocked-production' };
  }

  const primaryReady = settings.baseUrl.trim() !== '' && settings.model.trim() !== '';
  const localReady =
    settings.localPreset.baseUrl.trim() !== ''
    && settings.localPreset.model.trim() !== ''
    && isLocalAiEndpoint(settings.localPreset.baseUrl);

  const sensitive = environment === 'staging' || environment === 'production';
  if (sensitive && settings.environmentPolicy === 'local-only-sensitive') {
    if (primaryReady && isLocalAiEndpoint(settings.baseUrl)) {
      return {
        ok: true,
        endpoint: {
          protocol: settings.protocol,
          baseUrl: settings.baseUrl.trim(),
          model: settings.model.trim(),
          profileId: 'default'
        },
        usedLocalFallback: false
      };
    }
    if (localReady) {
      return {
        ok: true,
        endpoint: {
          protocol: settings.localPreset.protocol,
          baseUrl: settings.localPreset.baseUrl.trim(),
          model: settings.localPreset.model.trim(),
          profileId: 'local'
        },
        usedLocalFallback: true
      };
    }
    return { ok: false, reason: 'local-only-required' };
  }

  if (primaryReady) {
    return {
      ok: true,
      endpoint: {
        protocol: settings.protocol,
        baseUrl: settings.baseUrl.trim(),
        model: settings.model.trim(),
        profileId: 'default'
      },
      usedLocalFallback: false
    };
  }
  if (localReady) {
    return {
      ok: true,
      endpoint: {
        protocol: settings.localPreset.protocol,
        baseUrl: settings.localPreset.baseUrl.trim(),
        model: settings.localPreset.model.trim(),
        profileId: 'local'
      },
      usedLocalFallback: true
    };
  }
  return { ok: false, reason: 'not-configured' };
}

let availability: Promise<boolean> | null = null;

/**
 * 这个构建有没有编进 AI（Cargo feature `ai`）。进程内只问一次：构建期定下的事，运行中不会变。
 * 前端没有自己的构建开关，全听后端的，不会两边对不上
 */
export function aiAvailable(): Promise<boolean> {
  availability ??= invoke<boolean>('ai_available').catch(() => false);
  return availability;
}

export interface AiCompleteOptions {
  requestId?: string;
  maxTokens?: number;
  endpoint?: AiEndpointConfig;
}

/** 发一轮给模型，拿回文本。设计页与 SQL 助手共用这一个口子 */
export function completeWithAi(
  settings: AiSettings,
  messages: { system: string; user: string },
  options?: AiCompleteOptions
): Promise<string> {
  const target: AiEndpointConfig = options?.endpoint ?? {
    protocol: settings.protocol,
    baseUrl: settings.baseUrl,
    model: settings.model,
    profileId: 'default'
  };
  return invoke<string>('ai_complete', {
    request: {
      protocol: target.protocol,
      baseUrl: target.baseUrl,
      model: target.model,
      system: messages.system,
      user: messages.user,
      ...(options?.requestId ? { requestId: options.requestId } : {}),
      ...(target.profileId ? { profileId: target.profileId } : {}),
      ...(options?.maxTokens !== undefined ? { maxTokens: options.maxTokens } : {})
    }
  });
}

/** 取消正在进行的 AI 请求 */
export function cancelAiRequest(requestId: string): Promise<boolean> {
  return invoke<boolean>('ai_cancel', { requestId });
}

/** 生成用于取消的唯一请求 ID */
export function createAiRequestId(prefix = 'ai'): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

/**
 * 钥匙串里没有 AI Key 时，后端 `ai_commands.rs` 抛 `DATAOMNI_AI_KEY_MISSING`。
 * （曾误查 `DATAOMNI_CREDENTIAL_MISSING`，导致提示走错分支；AI0 修复并加回归测试）
 */
export function isAiKeyMissing(caught: unknown): boolean {
  const message = (caught as { message?: unknown } | null)?.message ?? caught;
  return typeof message === 'string' && message.startsWith('DATAOMNI_AI_KEY_MISSING');
}

export function isAiCancelled(caught: unknown): boolean {
  const message = (caught as { message?: unknown } | null)?.message ?? caught;
  return typeof message === 'string' && message.startsWith('DATAOMNI_AI_CANCELLED');
}
