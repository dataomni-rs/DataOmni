import { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle, Check, Copy, Loader2, ShieldAlert, Sparkles, X } from 'lucide-react';
import { clsx } from 'clsx';
import type { ConnectionEnvironment } from '../contracts';
import type { CompletionRelation } from '../contracts/databaseMetadata';
import { useLanguageStore } from '../stores/languageStore';
import { useQueryStore } from '../stores/queryStore';
import { useSettingsStore } from '../stores/settingsStore';
import {
  aiAvailable,
  cancelAiRequest,
  completeWithAi,
  createAiRequestId,
  isAiCancelled,
  isAiKeyMissing,
  resolveAiEndpointForEnvironment
} from '../utils/aiSettings';
import {
  buildExplainAnalysisPrompt,
  parseExplainAnalysisReply,
  type AiPromptMessages,
  type ExplainAnalysisResult
} from '../utils/aiSqlAssist';
import { describeError } from '../utils/describeError';
import { formatPlanMs, formatPlanRows, worstEstimate, type QueryPlan } from '../utils/planInsights';
import { PlanTree } from './PlanTree';
import { isImeKeyEvent } from '../utils/shortcuts';

interface QueryPlanDialogProps {
  sql: string;
  connectionId: string;
  /** 数据库类型，用来决定「真的执行一遍」给不给开 */
  dbType: string;
  databaseLabel: string;
  environment?: ConnectionEnvironment;
  relations?: readonly CompletionRelation[];
  onClose: () => void;
}

/**
 * PostgreSQL（`EXPLAIN ANALYZE`，CockroachDB 也走这个类型）与 SQL Server（`SET STATISTICS XML`）
 * 能真的跑一遍。
 *
 * 这一行和 Rust 的 `services::explain::supports_analyze` 是同一个判断。
 * 两边分叉时后端会**报错**而不是降级成普通 EXPLAIN——所以界面这边错了会立刻
 * 看见，不会变成一句「真的跑了一遍」的假话。
 */
function supportsAnalyze(dbType: string): boolean {
  return dbType === 'postgresql' || dbType === 'sqlserver';
}

export function QueryPlanDialog({
  sql,
  connectionId,
  dbType,
  databaseLabel,
  environment = 'development',
  relations = [],
  onClose
}: QueryPlanDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const language = useLanguageStore((state) => state.resolved);
  const ai = useSettingsStore((state) => state.ai);
  // 只订阅 id：`refreshTransaction` 每次都会换掉整个 session 对象，
  // 把对象本身放进 effect 的依赖里会让取计划和刷事务状态互相触发，转不停
  const sessionId = useQueryStore((state) => state.session?.id ?? null);
  const autocommit = useQueryStore((state) => state.autocommit);
  const refreshTransaction = useQueryStore((state) => state.refreshTransaction);

  const [analyze, setAnalyze] = useState(false);
  const [view, setView] = useState<'tree' | 'raw'>('tree');
  const [plan, setPlan] = useState<QueryPlan | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [aiInBuild, setAiInBuild] = useState(false);
  const [aiRunning, setAiRunning] = useState(false);
  const [aiSent, setAiSent] = useState<AiPromptMessages | null>(null);
  const [aiError, setAiError] = useState<string | null>(null);
  const [aiRawReply, setAiRawReply] = useState<string | null>(null);
  const [aiAnalysis, setAiAnalysis] = useState<ExplainAnalysisResult | null>(null);
  const [copiedSuggestionIdx, setCopiedSuggestionIdx] = useState<number | null>(null);
  const aiRequestIdRef = useRef<string | null>(null);

  const endpointResolution = useMemo(
    () => resolveAiEndpointForEnvironment(ai, environment),
    [ai, environment]
  );

  useEffect(() => {
    let cancelled = false;
    void aiAvailable().then((value) => {
      if (!cancelled) setAiInBuild(value);
    });
    return () => {
      cancelled = true;
      if (aiRequestIdRef.current) {
        void cancelAiRequest(aiRequestIdRef.current);
      }
    };
  }, []);

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
    if (!sessionId) {
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<QueryPlan>('explain_query', {
      request: { connectionId, sessionId, sql, analyze, autocommit }
    })
      .then((result) => {
        if (!cancelled) {
          setPlan(result);
        }
      })
      .catch((caught) => {
        if (!cancelled) {
          setError(describeError(caught));
          setPlan(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
        // ANALYZE 真的跑了语句，可能刚开了一个事务
        void refreshTransaction();
      });
    return () => {
      cancelled = true;
    };
  }, [sql, analyze, connectionId, sessionId, autocommit, refreshTransaction]);

  // 重新取计划时不显示上一份的结论：勾上「真的执行一遍」那一刻，旧计划的
  // 提示会停在屏幕上，读的人会以为那是新结果
  const worst = plan && !loading && !error ? worstEstimate(plan) : null;

  const runAiExplainAnalysis = async () => {
    if (!plan || !endpointResolution.ok) {
      return;
    }
    const worstSummary = worst
      ? `${worst.target ? `${worst.operation} (${worst.target})` : worst.operation}: estimated ${formatPlanRows(worst.estimatedRows)}, actual ${formatPlanRows(worst.actualRows)}`
      : undefined;
    const messages = buildExplainAnalysisPrompt({
      dialect: databaseLabel,
      sql,
      planRaw: plan.raw,
      worstEstimateSummary: worstSummary,
      relations,
      language
    });
    const requestId = createAiRequestId('explain');
    aiRequestIdRef.current = requestId;
    setAiSent(messages);
    setAiError(null);
    setAiRawReply(null);
    setAiAnalysis(null);
    setAiRunning(true);
    try {
      const reply = await completeWithAi(ai, messages, {
        requestId,
        endpoint: endpointResolution.endpoint
      });
      const parsed = parseExplainAnalysisReply(reply);
      if (!parsed.ok) {
        setAiError(t('aiSql.parseFailed'));
        setAiRawReply(parsed.raw);
      } else {
        setAiAnalysis(parsed.value);
      }
    } catch (caught) {
      if (isAiCancelled(caught)) {
        setAiError(t('error.backend.aiCancelled'));
      } else if (isAiKeyMissing(caught)) {
        setAiError(t('error.backend.aiKeyMissing'));
      } else {
        setAiError(describeError(caught));
      }
    } finally {
      if (aiRequestIdRef.current === requestId) {
        aiRequestIdRef.current = null;
      }
      setAiRunning(false);
    }
  };

  const copySuggestionSql = async (idx: number, sqlText: string) => {
    try {
      await navigator.clipboard.writeText(sqlText);
      setCopiedSuggestionIdx(idx);
      window.setTimeout(() => {
        setCopiedSuggestionIdx((current) => (current === idx ? null : current));
      }, 1500);
    } catch {
      // 忽略剪贴板错误
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="query-plan-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[84vh] w-[860px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between border-b border-line px-5 py-3">
          <div className="min-w-0">
            <h2 id="query-plan-title" className="text-base font-medium text-fg">
              {t('plan.title')}
            </h2>
            <div className="mt-1 flex flex-wrap items-center gap-3 text-xs text-fg-muted">
              {!loading && plan?.planningMs !== null && plan?.planningMs !== undefined && (
                <span>{t('plan.planningTime', { time: formatPlanMs(plan.planningMs) })}</span>
              )}
              {!loading && plan?.executionMs !== null && plan?.executionMs !== undefined && (
                <span>{t('plan.executionTime', { time: formatPlanMs(plan.executionMs) })}</span>
              )}
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-control p-1 text-fg-muted hover:bg-surface-hover hover:text-fg"
            aria-label={t('common.close')}
          >
            <X size={16} />
          </button>
        </div>

        <div className="flex flex-wrap items-center gap-3 border-b border-line bg-surface-sunken px-5 py-2 text-xs">
          <div className="flex items-center gap-1">
            {(['tree', 'raw'] as const).map((value) => (
              <button
                key={value}
                type="button"
                onClick={() => setView(value)}
                className={clsx(
                  'rounded-control px-2 py-1',
                  view === value
                    ? 'bg-accent-soft text-accent'
                    : 'text-fg-muted hover:bg-surface-hover'
                )}
              >
                {t(value === 'tree' ? 'plan.tab.tree' : 'plan.tab.raw')}
              </button>
            ))}
          </div>

          <label
            className={clsx(
              'flex items-center gap-1',
              supportsAnalyze(dbType) ? 'cursor-pointer text-fg-muted' : 'text-fg-subtle'
            )}
            title={
              supportsAnalyze(dbType)
                ? t('plan.analyzeHint')
                : t('plan.analyzeUnsupported', { database: databaseLabel })
            }
          >
            <input
              type="checkbox"
              checked={analyze}
              disabled={!supportsAnalyze(dbType)}
              onChange={(event) => setAnalyze(event.target.checked)}
            />
            {t('plan.analyze')}
          </label>

          {analyze && (
            <span className="flex items-center gap-1 text-warning">
              <AlertTriangle size={12} />
              {/* 自动提交开着时，它写进去的东西**当场就提交了**，没有回滚的机会 */}
              {autocommit ? t('plan.analyzeWillCommit') : t('plan.analyzeWillRun')}
            </span>
          )}

          {aiInBuild && ai.enabled && !loading && !error && plan && (
            <div className="ml-auto flex items-center gap-1.5">
              <button
                type="button"
                onClick={() => void runAiExplainAnalysis()}
                disabled={aiRunning || !endpointResolution.ok}
                className="inline-flex items-center gap-1 rounded-control border border-line-strong bg-surface px-2.5 py-1 text-xs text-fg hover:bg-surface-hover disabled:opacity-50"
              >
                {aiRunning ? <Loader2 size={12} className="animate-spin" /> : <Sparkles size={12} className="text-accent" />}
                {t('aiSql.explainAnalyzeButton')}
              </button>
              {aiRunning && (
                <button
                  type="button"
                  onClick={() => {
                    if (aiRequestIdRef.current) void cancelAiRequest(aiRequestIdRef.current);
                  }}
                  className="rounded-control border border-line-strong bg-surface px-2 py-1 text-xs text-fg hover:bg-surface-hover"
                >
                  {t('common.cancel')}
                </button>
              )}
            </div>
          )}
        </div>

        {aiInBuild && ai.enabled && !endpointResolution.ok && (
          <div className="flex items-center gap-1.5 border-b border-warning-line bg-warning-soft px-5 py-1.5 text-xs text-warning">
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

        {worst && (
          <p className="border-b border-warning-line bg-warning-soft px-5 py-2 text-xs text-warning">
            {t('plan.worstHint', {
              operation: worst.target ? `${worst.operation} (${worst.target})` : worst.operation,
              estimated: formatPlanRows(worst.estimatedRows),
              actual: formatPlanRows(worst.actualRows)
            })}
          </p>
        )}

        <div className="flex-1 overflow-auto px-5 py-3">
          {loading && (
            <p className="flex items-center gap-2 text-sm text-fg-muted">
              <Loader2 size={14} className="animate-spin" />
              {t('plan.loading')}
            </p>
          )}
          {!loading && error && <p className="text-sm text-danger">{error}</p>}
          {!loading && !error && plan && view === 'tree' && (
            plan.roots.length === 0
              ? <p className="text-sm text-fg-muted">{t('plan.empty')}</p>
              : (
                <PlanTree roots={plan.roots} />
              )
          )}
          {!loading && !error && plan && view === 'raw' && (
            <>
              <pre className="select-text whitespace-pre-wrap break-words rounded-control border border-line bg-surface-sunken px-3 py-2 font-mono text-xs text-fg">
                {plan.raw}
              </pre>
              <p className="mt-2 text-xs text-fg-subtle">{t('plan.rawNote')}</p>
            </>
          )}

          {(aiSent || aiError || aiAnalysis) && (
            <div className="mt-4 space-y-2 border-t border-line pt-3 text-xs">
              <div className="flex items-center gap-1.5 font-medium text-fg">
                <Sparkles size={13} className="text-accent" />
                <span>{t('aiSql.explainResultTitle')}</span>
              </div>
              {aiSent && (
                <details className="text-fg-muted">
                  <summary className="cursor-pointer select-none">{t('aiDesign.sent')}</summary>
                  <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-control border border-line bg-surface-sunken p-2 font-mono text-[11px] text-fg select-text">
                    {`[system]\n${aiSent.system}\n\n[user]\n${aiSent.user}`}
                  </pre>
                </details>
              )}
              {aiError && (
                <div className="rounded-control border border-danger-line bg-danger-soft px-3 py-2 text-danger">
                  <p className="break-words">{aiError}</p>
                  {aiRawReply && (
                    <details className="mt-1">
                      <summary className="cursor-pointer select-none">{t('aiDesign.rawReply')}</summary>
                      <pre className="mt-1 max-h-36 overflow-auto whitespace-pre-wrap break-words font-mono select-text">
                        {aiRawReply}
                      </pre>
                    </details>
                  )}
                </div>
              )}
              {aiAnalysis && (
                <div className="space-y-2 rounded-control border border-line bg-surface-subtle p-3">
                  <p className="text-fg">{aiAnalysis.summary}</p>
                  {aiAnalysis.bottlenecks.length > 0 && (
                    <div>
                      <p className="font-medium text-fg-muted">{t('aiSql.explainBottlenecks')}</p>
                      <ul className="mt-1 list-disc space-y-0.5 pl-4 text-fg">
                        {aiAnalysis.bottlenecks.map((item, idx) => (
                          <li key={idx}>{item}</li>
                        ))}
                      </ul>
                    </div>
                  )}
                  {aiAnalysis.suggestions.length > 0 && (
                    <div className="space-y-2">
                      <p className="font-medium text-fg-muted">{t('aiSql.explainSuggestions')}</p>
                      {aiAnalysis.suggestions.map((suggestion, idx) => (
                        <div key={idx} className="rounded-control border border-line bg-surface p-2.5 space-y-1">
                          <div className="flex items-center justify-between gap-2">
                            <span className="font-medium text-fg">{suggestion.title}</span>
                            {suggestion.sql && (
                              <button
                                type="button"
                                onClick={() => void copySuggestionSql(idx, suggestion.sql ?? '')}
                                className="inline-flex items-center gap-1 rounded-control border border-line-strong px-2 py-0.5 text-[11px] text-fg hover:bg-surface-hover"
                              >
                                {copiedSuggestionIdx === idx ? <Check size={11} /> : <Copy size={11} />}
                                {copiedSuggestionIdx === idx ? t('common.copied') : t('common.copy')}
                              </button>
                            )}
                          </div>
                          {suggestion.rationale && <p className="text-fg-muted">{suggestion.rationale}</p>}
                          {suggestion.sql && (
                            <pre className="overflow-x-auto rounded border border-line bg-surface-sunken px-2 py-1.5 font-mono text-[11px] text-fg select-text">
                              {suggestion.sql}
                            </pre>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
