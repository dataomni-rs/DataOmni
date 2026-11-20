import { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle, Check, Copy, Loader2, ShieldAlert, Sparkles, X } from 'lucide-react';
import type { ConnectionEnvironment } from '../contracts';
import type { CompletionRelation } from '../contracts/databaseMetadata';
import type { QueryExecutionError } from '../contracts/queryExecution';
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
  MAX_AI_CONTEXT_TABLES,
  buildErrorFixPrompt,
  buildNlToSqlPrompt,
  defaultSelectedRelationKeys,
  parseErrorFixReply,
  parseNlToSqlReply,
  relationDisplayName,
  relationKey,
  type AiPromptMessages,
  type ErrorFixDiagnosis,
  type NlToSqlDraft
} from '../utils/aiSqlAssist';
import { describeError } from '../utils/describeError';
import { isImeKeyEvent } from '../utils/shortcuts';
import { PLAIN_TEXT_INPUT } from './FormControls';

export type AiSqlAssistMode =
  | { kind: 'nl-to-sql'; existingSql?: string }
  | { kind: 'error-fix'; failingSql: string; errorMessage: string; details?: QueryExecutionError };

interface AiSqlAssistDialogProps {
  connectionId: string;
  dialect: string;
  environment: ConnectionEnvironment;
  relations: readonly CompletionRelation[];
  mode: AiSqlAssistMode;
  onApplySql: (sql: string) => void;
  onClose: () => void;
}

/**
 * SQL 编辑器克制型 AI 助手弹窗（AI1）：
 * - 模式 1：自然语言生成 / 改写 SQL（显式选表上限 15 张，PROD 默认 0 张）
 * - 模式 2：报错一键诊断与修复（只发报错结构 + 出错语句 + 涉及表列）
 * 零表数据行外发、发包原文全程可展开查看、支持中途取消、绝不自动执行 SQL。
 */
export function AiSqlAssistDialog({
  connectionId,
  dialect,
  environment,
  relations,
  mode,
  onApplySql,
  onClose
}: AiSqlAssistDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const language = useLanguageStore((state) => state.resolved);
  const ai = useSettingsStore((state) => state.ai);

  const endpointResolution = useMemo(
    () => resolveAiEndpointForEnvironment(ai, environment),
    [ai, environment]
  );

  const [requirement, setRequirement] = useState('');
  const [tableSearch, setTableSearch] = useState('');
  const [selectedKeys, setSelectedKeys] = useState<string[]>(() =>
    mode.kind === 'nl-to-sql'
      ? defaultSelectedRelationKeys(mode.existingSql ?? '', relations, environment)
      : []
  );
  const [semanticsNotes, setSemanticsNotes] = useState('');

  const [running, setRunning] = useState(false);
  const [sent, setSent] = useState<AiPromptMessages | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rawReply, setRawReply] = useState<string | null>(null);
  const [nlDraft, setNlDraft] = useState<NlToSqlDraft | null>(null);
  const [errorDiagnosis, setErrorDiagnosis] = useState<ErrorFixDiagnosis | null>(null);
  const [copied, setCopied] = useState(false);

  const activeRequestIdRef = useRef<string | null>(null);

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
    return () => {
      if (activeRequestIdRef.current) {
        void cancelAiRequest(activeRequestIdRef.current);
      }
    };
  }, []);

  // 按需读取连接级本地语义备注（CM4），反哺 NL → SQL
  useEffect(() => {
    if (mode.kind !== 'nl-to-sql' || !connectionId) {
      return;
    }
    let cancelled = false;
    void invoke<string>('get_connection_semantics', { id: connectionId })
      .then((content) => {
        if (!cancelled && typeof content === 'string') {
          setSemanticsNotes(content);
        }
      })
      .catch(() => {
        // 无备注或读取失败不影响生成
      });
    return () => {
      cancelled = true;
    };
  }, [connectionId, mode.kind]);

  const filteredRelations = useMemo(() => {
    const needle = tableSearch.trim().toLowerCase();
    if (!needle) return relations;
    return relations.filter((rel) => relationDisplayName(rel).toLowerCase().includes(needle));
  }, [relations, tableSearch]);

  const selectedRelationSet = useMemo(() => new Set(selectedKeys), [selectedKeys]);

  const toggleRelation = (key: string) => {
    setSelectedKeys((prev) => {
      if (prev.includes(key)) {
        return prev.filter((item) => item !== key);
      }
      if (prev.length >= MAX_AI_CONTEXT_TABLES) {
        return prev;
      }
      return [...prev, key];
    });
  };

  const runRequest = async () => {
    if (!endpointResolution.ok) {
      return;
    }
    if (mode.kind === 'nl-to-sql' && requirement.trim() === '') {
      return;
    }

    const messages =
      mode.kind === 'nl-to-sql'
        ? buildNlToSqlPrompt({
            dialect,
            requirement,
            existingSql: mode.existingSql,
            selectedRelations: relations.filter((rel) => selectedRelationSet.has(relationKey(rel))),
            semanticsNotes,
            language
          })
        : buildErrorFixPrompt({
            dialect,
            sql: mode.failingSql,
            errorMessage: mode.errorMessage,
            details: mode.details,
            relations,
            language
          });

    const requestId = createAiRequestId(mode.kind);
    activeRequestIdRef.current = requestId;
    setSent(messages);
    setError(null);
    setRawReply(null);
    setNlDraft(null);
    setErrorDiagnosis(null);
    setRunning(true);

    try {
      const reply = await completeWithAi(ai, messages, {
        requestId,
        endpoint: endpointResolution.endpoint
      });
      if (mode.kind === 'nl-to-sql') {
        const parsed = parseNlToSqlReply(reply);
        if (!parsed.ok) {
          setError(t('aiSql.parseFailed'));
          setRawReply(parsed.raw);
        } else {
          setNlDraft(parsed.value);
        }
      } else {
        const parsed = parseErrorFixReply(reply);
        if (!parsed.ok) {
          setError(t('aiSql.parseFailed'));
          setRawReply(parsed.raw);
        } else {
          setErrorDiagnosis(parsed.value);
        }
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
      if (activeRequestIdRef.current === requestId) {
        activeRequestIdRef.current = null;
      }
      setRunning(false);
    }
  };

  const cancelRunning = () => {
    const id = activeRequestIdRef.current;
    if (id) {
      void cancelAiRequest(id);
    }
  };

  const copySql = async (sqlText: string) => {
    try {
      await navigator.clipboard.writeText(sqlText);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // 忽略剪贴板错误
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
      aria-labelledby="ai-sql-assist-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[86vh] w-[720px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between border-b border-line px-5 py-3">
          <div className="flex items-center gap-2">
            <Sparkles size={15} className="text-accent" />
            <h2 id="ai-sql-assist-title" className="text-base font-medium text-fg">
              {mode.kind === 'nl-to-sql' ? t('aiSql.nlTitle') : t('aiSql.errorFixTitle')}
            </h2>
            {endpointResolution.ok && endpointResolution.usedLocalFallback && (
              <span className="rounded bg-accent-soft px-1.5 py-0.5 text-[11px] text-accent">
                {t('aiSql.usingLocalModel')}
              </span>
            )}
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
          <p className="text-xs text-fg-muted">{t('aiSql.zeroRowPrivacyNote')}</p>

          {guardWarning && (
            <div className="flex items-start gap-2 rounded-control border border-warning-line bg-warning-soft px-3 py-2 text-xs text-warning">
              <ShieldAlert size={14} className="mt-0.5 shrink-0" />
              <span>{guardWarning}</span>
            </div>
          )}

          {mode.kind === 'nl-to-sql' ? (
            <>
              <div className="space-y-1.5">
                <label htmlFor="ai-sql-requirement" className="block text-xs font-medium text-fg">
                  {t('aiSql.requirementLabel')}
                </label>
                <textarea
                  id="ai-sql-requirement"
                  rows={3}
                  value={requirement}
                  onChange={(event) => setRequirement(event.target.value)}
                  placeholder={t('aiSql.requirementPlaceholder')}
                  {...PLAIN_TEXT_INPUT}
                  className="w-full rounded-control border border-line-strong bg-surface px-3 py-2 text-sm text-fg outline-none focus:border-accent"
                />
              </div>

              {mode.existingSql && mode.existingSql.trim() !== '' && (
                <div className="space-y-1">
                  <span className="block text-xs font-medium text-fg-muted">
                    {t('aiSql.existingSqlLabel')}
                  </span>
                  <pre className="max-h-24 overflow-auto rounded-control border border-line bg-surface-sunken p-2 font-mono text-xs text-fg-muted">
                    {mode.existingSql.trim()}
                  </pre>
                </div>
              )}

              <div className="space-y-1.5">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-medium text-fg">
                    {t('aiSql.selectTablesLabel', {
                      selected: selectedKeys.length,
                      max: MAX_AI_CONTEXT_TABLES
                    })}
                  </span>
                  {environment === 'production' && selectedKeys.length === 0 && (
                    <span className="text-[11px] text-warning">{t('aiSql.prodZeroTablesHint')}</span>
                  )}
                </div>
                {relations.length > 0 && (
                  <>
                    <input
                      type="text"
                      value={tableSearch}
                      onChange={(event) => setTableSearch(event.target.value)}
                      placeholder={t('aiSql.filterTablesPlaceholder')}
                      {...PLAIN_TEXT_INPUT}
                      className="w-full rounded-control border border-line-strong bg-surface px-2.5 py-1 text-xs text-fg placeholder:text-fg-subtle"
                    />
                    <div className="flex max-h-28 flex-wrap gap-1.5 overflow-auto rounded-control border border-line bg-surface-sunken p-2">
                      {filteredRelations.slice(0, 60).map((rel) => {
                        const key = relationKey(rel);
                        const checked = selectedRelationSet.has(key);
                        const disabled = !checked && selectedKeys.length >= MAX_AI_CONTEXT_TABLES;
                        return (
                          <button
                            key={key}
                            type="button"
                            disabled={disabled}
                            onClick={() => toggleRelation(key)}
                            className={
                              checked
                                ? 'rounded border border-accent bg-accent-soft px-2 py-0.5 font-mono text-[11px] text-accent'
                                : 'rounded border border-line bg-surface px-2 py-0.5 font-mono text-[11px] text-fg-muted hover:bg-surface-hover disabled:opacity-40'
                            }
                          >
                            {relationDisplayName(rel)}
                          </button>
                        );
                      })}
                    </div>
                  </>
                )}
              </div>
            </>
          ) : (
            <div className="space-y-2 rounded-control border border-line bg-surface-sunken p-3 text-xs">
              <div>
                <span className="font-medium text-danger">{t('queryError.message')}: </span>
                <span className="break-words text-fg">{mode.errorMessage}</span>
              </div>
              <pre className="max-h-28 overflow-auto rounded-control border border-line bg-surface p-2 font-mono text-xs text-fg-muted">
                {mode.failingSql.trim()}
              </pre>
            </div>
          )}

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => void runRequest()}
              disabled={
                !endpointResolution.ok
                || running
                || (mode.kind === 'nl-to-sql' && requirement.trim() === '')
              }
              className="inline-flex items-center gap-1.5 rounded-control bg-accent px-3 py-1.5 text-xs font-medium text-fg-on-accent hover:opacity-90 disabled:opacity-50"
            >
              {running && <Loader2 size={13} className="animate-spin" />}
              {mode.kind === 'nl-to-sql' ? t('aiSql.generateButton') : t('aiSql.diagnoseButton')}
            </button>
            {running && (
              <button
                type="button"
                onClick={cancelRunning}
                className="rounded-control border border-line-strong px-3 py-1.5 text-xs text-fg hover:bg-surface-hover"
              >
                {t('common.cancel')}
              </button>
            )}
          </div>

          {sent && (
            <details className="text-xs text-fg-muted">
              <summary className="cursor-pointer select-none">{t('aiDesign.sent')}</summary>
              <pre className="mt-1 max-h-48 overflow-auto whitespace-pre-wrap break-words rounded-control border border-line bg-surface-sunken p-2 font-mono text-[11px] text-fg select-text">
                {`[system]\n${sent.system}\n\n[user]\n${sent.user}`}
              </pre>
            </details>
          )}

          {error && (
            <div className="rounded-control border border-danger-line bg-danger-soft px-3 py-2 text-xs text-danger">
              <p className="break-words">{error}</p>
              {rawReply && (
                <details className="mt-1">
                  <summary className="cursor-pointer select-none">{t('aiDesign.rawReply')}</summary>
                  <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words font-mono select-text">
                    {rawReply}
                  </pre>
                </details>
              )}
            </div>
          )}

          {nlDraft && (
            <div className="space-y-2 rounded-control border border-line bg-surface-sunken p-3">
              {nlDraft.explanation && <p className="text-xs text-fg">{nlDraft.explanation}</p>}
              <pre className="max-h-56 overflow-auto rounded-control border border-line bg-surface p-2.5 font-mono text-xs text-fg select-text">
                {nlDraft.sql}
              </pre>
              {nlDraft.warnings.length > 0 && (
                <ul className="space-y-1 text-xs text-warning">
                  {nlDraft.warnings.map((warning, idx) => (
                    <li key={idx} className="flex items-start gap-1.5">
                      <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                      <span>{warning}</span>
                    </li>
                  ))}
                </ul>
              )}
              <div className="flex items-center gap-2 pt-1">
                <button
                  type="button"
                  onClick={() => {
                    onApplySql(nlDraft.sql);
                    onClose();
                  }}
                  className="rounded-control bg-accent px-3 py-1 text-xs font-medium text-fg-on-accent hover:opacity-90"
                >
                  {mode.kind === 'nl-to-sql' && mode.existingSql
                    ? t('aiSql.replaceSql')
                    : t('aiSql.insertSql')}
                </button>
                <button
                  type="button"
                  onClick={() => void copySql(nlDraft.sql)}
                  className="inline-flex items-center gap-1 rounded-control border border-line-strong px-2.5 py-1 text-xs text-fg hover:bg-surface-hover"
                >
                  {copied ? <Check size={12} /> : <Copy size={12} />}
                  {copied ? t('common.copied') : t('common.copy')}
                </button>
              </div>
            </div>
          )}

          {errorDiagnosis && (
            <div className="space-y-2 rounded-control border border-line bg-surface-sunken p-3">
              <p className="text-xs font-medium text-fg">{errorDiagnosis.cause}</p>
              {errorDiagnosis.fixedSql ? (
                <>
                  <pre className="max-h-56 overflow-auto rounded-control border border-line bg-surface p-2.5 font-mono text-xs text-fg select-text">
                    {errorDiagnosis.fixedSql}
                  </pre>
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        onApplySql(errorDiagnosis.fixedSql ?? '');
                        onClose();
                      }}
                      className="rounded-control bg-accent px-3 py-1 text-xs font-medium text-fg-on-accent hover:opacity-90"
                    >
                      {t('aiSql.replaceSql')}
                    </button>
                    <button
                      type="button"
                      onClick={() => void copySql(errorDiagnosis.fixedSql ?? '')}
                      className="inline-flex items-center gap-1 rounded-control border border-line-strong px-2.5 py-1 text-xs text-fg hover:bg-surface-hover"
                    >
                      {copied ? <Check size={12} /> : <Copy size={12} />}
                      {copied ? t('common.copied') : t('common.copy')}
                    </button>
                  </div>
                </>
              ) : (
                <p className="text-xs text-fg-muted">{t('aiSql.noAutoFixAvailable')}</p>
              )}
              {errorDiagnosis.notes.length > 0 && (
                <ul className="list-disc space-y-0.5 pl-4 text-xs text-fg-muted">
                  {errorDiagnosis.notes.map((note, idx) => (
                    <li key={idx}>{note}</li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
