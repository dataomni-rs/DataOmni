import { useEffect, useMemo, useRef, useState } from 'react';
import { clsx } from 'clsx';
import { Check, Copy, Loader2, ShieldAlert, Sparkles, Square, X } from 'lucide-react';
import type { ConnectionEnvironment } from '../contracts';
import type { ProfileResult } from '../contracts/analytics';
import { useLanguageStore } from '../stores/languageStore';
import { useSettingsStore } from '../stores/settingsStore';
import {
  cancelAiRequest,
  completeWithAi,
  createAiRequestId,
  isAiCancelled,
  isAiKeyMissing,
  resolveAiEndpointForEnvironment
} from '../utils/aiSettings';
import {
  buildProfileInsightsPrompt,
  parseProfileInsightsReply,
  type AiProfileInsights
} from '../utils/aiProfileInsights';
import type { AiPromptMessages } from '../utils/aiSqlAssist';
import { describeError } from '../utils/describeError';

interface AiProfileInsightsPanelProps {
  title?: string;
  dialect?: string;
  scopeLabel?: string;
  environment?: ConnectionEnvironment;
  profile: ProfileResult;
  onClose: () => void;
}

/**
 * 基于脱敏统计摘要的 AI 体检解读内联面板（AN10）：
 * - 严格复用既有 `completeWithAi` / `cancelAiRequest` 与敏感环境守卫；
 * - 绝不自动发包，点击按钮后才发脱敏统计摘要；支持展开查看完整 `[system]`/`[user]` 原文；
 * - 建议 SQL 仅提供一键复制，绝不自动执行。
 */
export function AiProfileInsightsPanel({
  title,
  dialect,
  scopeLabel,
  environment = 'development',
  profile,
  onClose
}: AiProfileInsightsPanelProps) {
  const t = useLanguageStore((state) => state.t);
  const language = useLanguageStore((state) => state.resolved);
  const ai = useSettingsStore((state) => state.ai);

  const endpointResolution = useMemo(
    () => resolveAiEndpointForEnvironment(ai, environment),
    [ai, environment]
  );

  const previewPrompt = useMemo(
    () =>
      buildProfileInsightsPrompt({
        title,
        dialect,
        scopeLabel,
        profile,
        language
      }),
    [title, dialect, scopeLabel, profile, language]
  );

  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [insights, setInsights] = useState<AiProfileInsights | null>(null);
  const [rawReply, setRawReply] = useState<string | null>(null);
  const [lastSent, setLastSent] = useState<AiPromptMessages | null>(null);
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null);

  const activeRequestIdRef = useRef<string | null>(null);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (activeRequestIdRef.current) {
        void cancelAiRequest(activeRequestIdRef.current);
      }
    };
  }, []);

  const handleRun = async () => {
    if (!endpointResolution.ok || running) {
      return;
    }
    const requestId = createAiRequestId('profile-insights');
    activeRequestIdRef.current = requestId;
    setRunning(true);
    setError(null);
    setInsights(null);
    setRawReply(null);
    setLastSent(previewPrompt);
    try {
      const reply = await completeWithAi(ai, previewPrompt, {
        requestId,
        endpoint: endpointResolution.endpoint,
        maxTokens: 1200
      });
      if (!mountedRef.current || activeRequestIdRef.current !== requestId) {
        return;
      }
      const parsed = parseProfileInsightsReply(reply);
      if (parsed.ok) {
        setInsights(parsed.value);
      } else {
        setRawReply(parsed.raw);
      }
    } catch (err) {
      if (!mountedRef.current || activeRequestIdRef.current !== requestId) {
        return;
      }
      if (isAiCancelled(err)) {
        setError(t('error.backend.aiCancelled'));
      } else if (isAiKeyMissing(err)) {
        setError(t('error.backend.aiKeyMissing'));
      } else {
        setError(describeError(err));
      }
    } finally {
      if (activeRequestIdRef.current === requestId) {
        activeRequestIdRef.current = null;
      }
      if (mountedRef.current) {
        setRunning(false);
      }
    }
  };

  const handleCancel = () => {
    if (activeRequestIdRef.current) {
      void cancelAiRequest(activeRequestIdRef.current);
    }
  };

  const handleCopySql = (idx: number, sql: string) => {
    navigator.clipboard
      .writeText(sql)
      .then(() => {
        setCopiedIdx(idx);
        window.setTimeout(() => {
          setCopiedIdx((cur) => (cur === idx ? null : cur));
        }, 1400);
      })
      .catch(() => {});
  };

  return (
    <section className="shrink-0 space-y-2.5 border-b border-line bg-surface px-5 py-3 text-xs">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Sparkles size={14} className="text-accent" />
          <span className="font-semibold text-fg">{t('profile.aiInsights.title')}</span>
          <span className="rounded bg-surface-sunken px-1.5 py-0.5 text-[10px] text-fg-muted">
            {t('profile.aiInsights.redactedBadge')}
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          {running ? (
            <button
              type="button"
              onClick={handleCancel}
              className="flex items-center gap-1 rounded-control bg-danger-solid px-2.5 py-1 text-xs font-medium text-fg-on-solid hover:opacity-90"
            >
              <Square size={11} />
              <span>{t('analytics.cancel')}</span>
            </button>
          ) : (
            <button
              type="button"
              disabled={!endpointResolution.ok}
              onClick={() => void handleRun()}
              className="flex items-center gap-1 rounded-control bg-accent px-2.5 py-1 text-xs font-medium text-fg-on-accent hover:opacity-90 disabled:opacity-50"
            >
              <Sparkles size={11} />
              <span>
                {insights || rawReply
                  ? t('profile.aiInsights.rerun')
                  : t('profile.aiInsights.run')}
              </span>
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            aria-label={t('common.close')}
            className="rounded-control p-1 text-fg-muted hover:bg-surface-hover"
          >
            <X size={13} />
          </button>
        </div>
      </div>

      {!endpointResolution.ok && (
        <div className="flex items-center gap-1.5 rounded-control border border-warning-line bg-warning-soft px-3 py-1.5 text-[11px] text-warning">
          <ShieldAlert size={13} className="shrink-0" />
          <span>
            {endpointResolution.reason === 'blocked-production'
              ? t('aiSql.blockedProduction')
              : endpointResolution.reason === 'local-only-required'
                ? t('aiSql.localOnlyRequired')
                : t('aiDesign.notConfigured')}
          </span>
        </div>
      )}

      {running && (
        <div className="flex items-center gap-2 text-fg-muted">
          <Loader2 size={13} className="animate-spin text-accent" />
          <span>{t('profile.aiInsights.loading')}</span>
        </div>
      )}

      {error && (
        <div className="rounded-control border border-danger-line bg-danger-soft px-3 py-1.5 text-[11px] text-danger">
          {error}
        </div>
      )}

      {insights && (
        <div className="space-y-2">
          <div className="flex flex-wrap items-center justify-between gap-2 rounded bg-surface-sunken px-3 py-2">
            <p className="text-xs text-fg">{insights.summary}</p>
            <span className="shrink-0 rounded bg-warning-soft px-1.5 py-0.5 text-[10px] text-warning">
              {t('profile.aiInsights.inferredTag')}
            </span>
          </div>
          {insights.findings.length > 0 && (
            <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
              {insights.findings.map((item, idx) => (
                <div
                  key={`${item.column ?? 'global'}-${idx}`}
                  className="space-y-1 rounded-control border border-line bg-surface-raised p-2.5"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium text-fg">
                      {item.column ? (
                        <code className="mr-1.5 rounded bg-surface-sunken px-1 py-0.5 font-mono text-[10px]">
                          {item.column}
                        </code>
                      ) : null}
                      {item.title}
                    </span>
                    <span
                      className={clsx(
                        'shrink-0 rounded px-1.5 py-0.5 text-[10px] uppercase',
                        item.severity === 'high'
                          ? 'bg-danger-soft text-danger'
                          : item.severity === 'medium'
                            ? 'bg-warning-soft text-warning'
                            : 'bg-surface-sunken text-fg-muted'
                      )}
                    >
                      {item.severity}
                    </span>
                  </div>
                  {item.detail && <p className="text-[11px] text-fg-muted">{item.detail}</p>}
                  {item.suggestedSql && (
                    <div className="flex items-center justify-between gap-2 rounded bg-surface-sunken px-2 py-1 font-mono text-[10px] text-fg">
                      <span className="truncate" title={item.suggestedSql}>
                        {item.suggestedSql}
                      </span>
                      <button
                        type="button"
                        onClick={() => handleCopySql(idx, item.suggestedSql ?? '')}
                        className="flex shrink-0 items-center gap-1 rounded px-1.5 py-0.5 text-fg-muted hover:bg-surface-hover hover:text-fg"
                      >
                        {copiedIdx === idx ? (
                          <Check size={10} className="text-success" />
                        ) : (
                          <Copy size={10} />
                        )}
                        <span>
                          {copiedIdx === idx ? t('profile.copiedSql') : t('common.copy')}
                        </span>
                      </button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {rawReply && (
        <pre className="max-h-40 overflow-y-auto whitespace-pre-wrap rounded bg-surface-sunken p-2.5 font-mono text-[11px] text-fg">
          {rawReply}
        </pre>
      )}

      <details className="text-[11px] text-fg-subtle">
        <summary className="cursor-pointer select-none hover:text-fg">
          {t('profile.aiInsights.inspectPrompt')}
        </summary>
        <div className="mt-1.5 max-h-40 space-y-1.5 overflow-y-auto rounded bg-surface-sunken p-2 font-mono text-[10px] text-fg-muted">
          <div>
            <p className="font-semibold text-fg">[system]</p>
            <pre className="whitespace-pre-wrap">{(lastSent ?? previewPrompt).system}</pre>
          </div>
          <div>
            <p className="font-semibold text-fg">[user]</p>
            <pre className="whitespace-pre-wrap">{(lastSent ?? previewPrompt).user}</pre>
          </div>
        </div>
      </details>
    </section>
  );
}
