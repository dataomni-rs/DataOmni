import { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, Loader2, Save, ShieldAlert, Sparkles, X } from 'lucide-react';
import type { ConnectionEnvironment } from '../contracts';
import type { ErLink, ErTable } from '../utils/erLayout';
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
import type { AiPromptMessages } from '../utils/aiSqlAssist';
import {
  MAX_SEMANTICS_BYTES,
  buildSemanticsDraftPrompt,
  chunkTablesForSemantics,
  parseSemanticsDraftReply,
  renderSemanticsMarkdown,
  type SemanticsBatchDraft
} from '../utils/aiSemanticsDraft';
import { describeError } from '../utils/describeError';
import { isImeKeyEvent } from '../utils/shortcuts';
import { PLAIN_TEXT_INPUT } from './FormControls';

interface AiSemanticsDraftDialogProps {
  connectionId: string;
  connectionName: string;
  dialect: string;
  environment: ConnectionEnvironment;
  tables: readonly ErTable[];
  links: readonly ErLink[];
  onClose: () => void;
}

/**
 * 数据字典 AI 业务语义注释草拟弹窗（AI2，与 CM4 `semantics/<id>.md` 闭环）：
 * - 按每批最多 20 张表发送目录元数据（零数据行外发），标注 `[AI 推测]`；
 * - 用户在文本框中审阅修改后再显式点击保存至 `semantics/<connectionId>.md`。
 */
export function AiSemanticsDraftDialog({
  connectionId,
  connectionName,
  dialect,
  environment,
  tables,
  links,
  onClose
}: AiSemanticsDraftDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const language = useLanguageStore((state) => state.resolved);
  const ai = useSettingsStore((state) => state.ai);

  const endpointResolution = useMemo(
    () => resolveAiEndpointForEnvironment(ai, environment),
    [ai, environment]
  );

  const batches = useMemo(() => chunkTablesForSemantics(tables), [tables]);

  const [draftText, setDraftText] = useState('');
  const [loadedExisting, setLoadedExisting] = useState(false);
  const [running, setRunning] = useState(false);
  const [batchProgress, setBatchProgress] = useState<{ current: number; total: number } | null>(null);
  const [lastSent, setLastSent] = useState<AiPromptMessages | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rawReply, setRawReply] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [savedNotice, setSavedNotice] = useState(false);

  const activeRequestIdRef = useRef<string | null>(null);
  const cancelledLoopRef = useRef(false);

  const byteCount = useMemo(() => new TextEncoder().encode(draftText).length, [draftText]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImeKeyEvent(event)) {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  useEffect(() => {
    let cancelled = false;
    void invoke<string>('get_connection_semantics', { id: connectionId })
      .then((content) => {
        if (!cancelled && typeof content === 'string') {
          setDraftText(content);
          setLoadedExisting(content.trim() !== '');
        }
      })
      .catch(() => {
        // 忽略读取已有备注错误
      });
    return () => {
      cancelled = true;
      cancelledLoopRef.current = true;
      if (activeRequestIdRef.current) {
        void cancelAiRequest(activeRequestIdRef.current);
      }
    };
  }, [connectionId]);

  const runDraft = async () => {
    if (!endpointResolution.ok || batches.length === 0) {
      return;
    }
    cancelledLoopRef.current = false;
    setRunning(true);
    setError(null);
    setRawReply(null);
    setSavedNotice(false);
    const collected: SemanticsBatchDraft[] = [];

    try {
      for (let i = 0; i < batches.length; i += 1) {
        if (cancelledLoopRef.current) {
          break;
        }
        setBatchProgress({ current: i + 1, total: batches.length });
        const messages = buildSemanticsDraftPrompt({
          dialect,
          tables: batches[i],
          links,
          language
        });
        setLastSent(messages);
        const requestId = createAiRequestId(`semantics-${i + 1}`);
        activeRequestIdRef.current = requestId;
        const reply = await completeWithAi(ai, messages, {
          requestId,
          endpoint: endpointResolution.endpoint
        });
        const parsed = parseSemanticsDraftReply(reply);
        if (!parsed.ok) {
          setError(t('aiSql.parseFailed'));
          setRawReply(parsed.raw);
          break;
        }
        collected.push(parsed.value);
      }

      if (collected.length > 0) {
        const rendered = renderSemanticsMarkdown(collected, language);
        setDraftText(rendered);
      }
    } catch (caught) {
      if (isAiCancelled(caught)) {
        setError(t('error.backend.aiCancelled'));
      } else if (isAiKeyMissing(caught)) {
        setError(t('error.backend.aiKeyMissing'));
      } else {
        setError(describeError(caught));
      }
    } finally {
      activeRequestIdRef.current = null;
      setBatchProgress(null);
      setRunning(false);
    }
  };

  const cancelDraft = () => {
    cancelledLoopRef.current = true;
    const id = activeRequestIdRef.current;
    if (id) {
      void cancelAiRequest(id);
    }
  };

  const saveDraft = async () => {
    if (byteCount > MAX_SEMANTICS_BYTES) {
      setError(t('form.semanticsTooLarge'));
      return;
    }
    setSaving(true);
    setError(null);
    setSavedNotice(false);
    try {
      await invoke('save_connection_semantics', { id: connectionId, content: draftText });
      setSavedNotice(true);
    } catch (caught) {
      setError(describeError(caught));
    } finally {
      setSaving(false);
    }
  };

  const guardWarning = !endpointResolution.ok
    ? endpointResolution.reason === 'blocked-production'
      ? t('aiSql.blockedProduction')
      : endpointResolution.reason === 'local-only-required'
        ? t('aiSql.localOnlyRequired')
        : t('aiDesign.notConfigured')
    : null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="ai-semantics-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[86vh] w-[740px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between border-b border-line px-5 py-3">
          <div className="flex items-center gap-2">
            <Sparkles size={15} className="text-accent" />
            <h2 id="ai-semantics-title" className="text-base font-medium text-fg">
              {t('aiSemantics.title', { connection: connectionName })}
            </h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t('common.close')}
            className="rounded-control p-1 text-fg-subtle hover:bg-surface-hover hover:text-fg"
          >
            <X size={16} />
          </button>
        </div>

        <div className="min-h-0 flex-1 space-y-3 overflow-auto px-5 py-4 text-sm">
          <p className="text-xs leading-relaxed text-fg-muted">
            {t('aiSemantics.description', { count: tables.length, batches: batches.length })}
          </p>

          {guardWarning && (
            <div className="flex items-start gap-2 rounded-control border border-warning-line bg-warning-soft px-3 py-2 text-xs text-warning">
              <ShieldAlert size={14} className="mt-0.5 shrink-0" />
              <span>{guardWarning}</span>
            </div>
          )}

          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => void runDraft()}
              disabled={!endpointResolution.ok || running || tables.length === 0}
              className="inline-flex items-center gap-1.5 rounded-control bg-accent px-3 py-1.5 text-xs font-medium text-fg-on-accent hover:opacity-90 disabled:opacity-50"
            >
              {running ? <Loader2 size={13} className="animate-spin" /> : <Sparkles size={13} />}
              {loadedExisting ? t('aiSemantics.redraftButton') : t('aiSemantics.draftButton')}
            </button>
            {running && (
              <button
                type="button"
                onClick={cancelDraft}
                className="rounded-control border border-line-strong px-3 py-1.5 text-xs text-fg hover:bg-surface-hover"
              >
                {t('common.cancel')}
              </button>
            )}
            {batchProgress && (
              <span className="text-xs text-fg-muted">
                {t('aiSemantics.batchProgress', {
                  current: batchProgress.current,
                  total: batchProgress.total
                })}
              </span>
            )}
          </div>

          {lastSent && (
            <details className="text-xs text-fg-muted">
              <summary className="cursor-pointer select-none">{t('aiDesign.sent')}</summary>
              <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-control border border-line bg-surface-sunken p-2 font-mono text-[11px] text-fg select-text">
                {`[system]\n${lastSent.system}\n\n[user]\n${lastSent.user}`}
              </pre>
            </details>
          )}

          {error && (
            <div className="rounded-control border border-danger-line bg-danger-soft px-3 py-2 text-xs text-danger">
              <p className="break-words">{error}</p>
              {rawReply && (
                <details className="mt-1">
                  <summary className="cursor-pointer select-none">{t('aiDesign.rawReply')}</summary>
                  <pre className="mt-1 max-h-36 overflow-auto whitespace-pre-wrap break-words font-mono select-text">
                    {rawReply}
                  </pre>
                </details>
              )}
            </div>
          )}

          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-xs">
              <label htmlFor="ai-semantics-editor" className="font-medium text-fg">
                {t('aiSemantics.reviewLabel')}
              </label>
              <span
                className={
                  byteCount > MAX_SEMANTICS_BYTES
                    ? 'font-mono text-[11px] text-danger'
                    : 'font-mono text-[11px] text-fg-subtle'
                }
              >
                {(byteCount / 1024).toFixed(1)} / 32 KB
              </span>
            </div>
            <textarea
              id="ai-semantics-editor"
              rows={12}
              value={draftText}
              onChange={(event) => {
                setDraftText(event.target.value);
                setSavedNotice(false);
              }}
              placeholder={t('form.semanticsPlaceholder')}
              {...PLAIN_TEXT_INPUT}
              className="w-full rounded-control border border-line-strong bg-surface px-3 py-2 font-mono text-xs text-fg outline-none focus:border-accent"
            />
          </div>

          <div className="flex items-center justify-between gap-2 pt-1">
            <div>
              {savedNotice && (
                <span className="inline-flex items-center gap-1 text-xs text-success">
                  <Check size={13} />
                  {t('aiSemantics.savedNotice')}
                </span>
              )}
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={onClose}
                className="rounded-control border border-line-strong px-3 py-1.5 text-xs text-fg hover:bg-surface-hover"
              >
                {t('common.close')}
              </button>
              <button
                type="button"
                onClick={() => void saveDraft()}
                disabled={saving || running || byteCount > MAX_SEMANTICS_BYTES}
                className="inline-flex items-center gap-1.5 rounded-control bg-accent px-3 py-1.5 text-xs font-medium text-fg-on-accent hover:opacity-90 disabled:opacity-50"
              >
                {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
                {t('aiSemantics.saveButton')}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
