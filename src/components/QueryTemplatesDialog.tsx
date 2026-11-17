import { useEffect, useMemo, useRef, useState } from 'react';
import { clsx } from 'clsx';
import { Edit, Play, Plus, Search, Trash2, X } from 'lucide-react';
import type { DatabaseType } from '../contracts/connection';
import { useLanguageStore } from '../stores/languageStore';
import { useTemplateStore } from '../stores/templateStore';
import {
  filterQueryTemplates,
  SQL_TEMPLATE_DB_TYPES,
  templateParameterNames,
  type QueryTemplate,
  type QueryTemplateDraft,
  type QueryTemplateScope,
} from '../utils/queryTemplates';
import { serverLabel } from '../utils/serverPresets';
import { formatShortcut, isImeKeyEvent, SHORTCUTS } from '../utils/shortcuts';
import { PLAIN_TEXT_INPUT } from './FormControls';

export interface QueryTemplatesDialogProps {
  currentDbType?: DatabaseType;
  /** 当前编辑器选区或全文；非空时展示「保存当前 SQL」按钮 */
  currentSql?: string;
  onInsert?: (template: QueryTemplate) => void;
  onInsertAndRun?: (template: QueryTemplate) => void;
  onOpenInTab?: (template: QueryTemplate) => void;
  onClose: () => void;
}

export function QueryTemplatesDialog({
  currentDbType,
  currentSql,
  onInsert,
  onInsertAndRun,
  onOpenInTab,
  onClose,
}: QueryTemplatesDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const templates = useTemplateStore((state) => state.templates);
  const saveTemplate = useTemplateStore((state) => state.saveTemplate);
  const removeTemplate = useTemplateStore((state) => state.removeTemplate);

  const [query, setQuery] = useState('');
  const [scope, setScope] = useState<QueryTemplateScope>(
    currentDbType ? 'compatible' : 'all',
  );
  const [draft, setDraft] = useState<QueryTemplateDraft | null>(null);
  const titleInputRef = useRef<HTMLInputElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (draft) {
      titleInputRef.current?.focus();
    } else {
      searchInputRef.current?.focus();
    }
  }, [draft]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImeKeyEvent(event)) {
        event.preventDefault();
        if (draft) {
          setDraft(null);
        } else {
          onClose();
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [draft, onClose]);

  const filtered = useMemo(
    () =>
      filterQueryTemplates(templates, {
        query,
        currentDbType,
        scope,
      }),
    [currentDbType, query, scope, templates],
  );

  const trimmedCurrentSql = (currentSql ?? '').trim();

  const openSaveCurrent = () => {
    setDraft({
      title: '',
      sql: trimmedCurrentSql,
      dbType: currentDbType,
    });
  };

  const openNewBlank = () => {
    setDraft({
      title: '',
      sql: '',
      dbType: currentDbType,
    });
  };

  const openEdit = (template: QueryTemplate) => {
    setDraft({
      id: template.id,
      title: template.title,
      sql: template.sql,
      dbType: template.dbType,
    });
  };

  const submitDraft = () => {
    if (!draft || !draft.title.trim() || !draft.sql.trim()) {
      return;
    }
    saveTemplate(draft);
    setDraft(null);
  };

  const draftParams = useMemo(
    () => (draft ? templateParameterNames(draft.sql, draft.dbType ?? currentDbType) : []),
    [currentDbType, draft],
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="query-templates-title"
      onClick={onClose}
    >
      <div
        className="flex max-h-[82vh] w-[660px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-line px-5 py-3">
          <div className="min-w-0">
            <h2 id="query-templates-title" className="text-base font-medium text-fg">
              {t('templates.title')}
            </h2>
            <p className="mt-0.5 text-xs text-fg-subtle">{t('templates.hint')}</p>
          </div>
          <div className="ml-3 flex shrink-0 items-center gap-2">
            {!draft && trimmedCurrentSql !== '' && (
              <button
                type="button"
                onClick={openSaveCurrent}
                className="flex items-center gap-1 rounded-control bg-accent px-2.5 py-1.5 text-xs font-medium text-fg-on-accent hover:bg-accent-hover"
              >
                <Plus size={13} />
                <span>{t('templates.saveCurrent')}</span>
              </button>
            )}
            {!draft && (
              <button
                type="button"
                onClick={openNewBlank}
                className="flex items-center gap-1 rounded-control border border-line-strong px-2.5 py-1.5 text-xs text-fg-muted hover:bg-surface-hover hover:text-fg"
              >
                <Plus size={13} />
                <span>{t('templates.newTemplate')}</span>
              </button>
            )}
            <button
              type="button"
              onClick={onClose}
              aria-label={t('common.close')}
              className="rounded-control p-1 text-fg-subtle hover:bg-surface-hover hover:text-fg"
            >
              <X size={16} />
            </button>
          </div>
        </div>

        {draft ? (
          <div className="space-y-3 overflow-y-auto px-5 py-4">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-[1fr_180px]">
              <div>
                <label
                  htmlFor="template-title-input"
                  className="mb-1 block text-xs font-medium text-fg-muted"
                >
                  {t('templates.fieldTitle')}
                </label>
                <input
                  id="template-title-input"
                  ref={titleInputRef}
                  value={draft.title}
                  onChange={(event) => setDraft({ ...draft, title: event.target.value })}
                  placeholder={t('templates.fieldTitlePlaceholder')}
                  className="w-full rounded-control border border-line-strong bg-surface px-2.5 py-1.5 text-sm text-fg placeholder:text-fg-subtle"
                  {...PLAIN_TEXT_INPUT}
                />
              </div>
              <div>
                <label
                  htmlFor="template-db-select"
                  className="mb-1 block text-xs font-medium text-fg-muted"
                >
                  {t('templates.fieldDbType')}
                </label>
                <select
                  id="template-db-select"
                  value={draft.dbType ?? ''}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      dbType: (event.target.value || undefined) as DatabaseType | undefined,
                    })
                  }
                  className="w-full rounded-control border border-line-strong bg-surface px-2.5 py-1.5 text-sm text-fg"
                >
                  <option value="">{t('templates.generalSql')}</option>
                  {SQL_TEMPLATE_DB_TYPES.map((dbType) => (
                    <option key={dbType} value={dbType}>
                      {serverLabel({ db_type: dbType, options: {} })}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <div>
              <label
                htmlFor="template-sql-textarea"
                className="mb-1 block text-xs font-medium text-fg-muted"
              >
                {t('templates.fieldSql')}
              </label>
              <textarea
                id="template-sql-textarea"
                rows={6}
                value={draft.sql}
                onChange={(event) => setDraft({ ...draft, sql: event.target.value })}
                placeholder={t('templates.fieldSqlPlaceholder')}
                className="w-full rounded-control border border-line-strong bg-surface px-3 py-2 font-mono text-xs leading-relaxed text-fg placeholder:text-fg-subtle"
                {...PLAIN_TEXT_INPUT}
              />
              {draftParams.length > 0 && (
                <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
                  {draftParams.map((param) => (
                    <span
                      key={param}
                      className="rounded-control border border-accent-line bg-accent-soft px-1.5 py-0.5 font-mono text-[11px] text-accent"
                    >
                      :{param}
                    </span>
                  ))}
                </div>
              )}
            </div>

            <div className="flex items-center justify-end gap-2 pt-1">
              <button
                type="button"
                onClick={() => setDraft(null)}
                className="rounded-control border border-line-strong px-3 py-1.5 text-sm text-fg-muted hover:bg-surface-hover"
              >
                {t('common.cancel')}
              </button>
              <button
                type="button"
                disabled={!draft.title.trim() || !draft.sql.trim()}
                onClick={submitDraft}
                className="rounded-control bg-accent px-3 py-1.5 text-sm text-fg-on-accent hover:bg-accent-hover disabled:opacity-50"
              >
                {t('templates.save')}
              </button>
            </div>
          </div>
        ) : (
          <>
            <div className="flex items-center gap-2 border-b border-line bg-surface-sunken px-5 py-2">
              <div className="flex min-w-0 flex-1 items-center gap-2 rounded-control border border-line bg-surface px-2.5 py-1.5">
                <Search size={14} className="shrink-0 text-fg-subtle" />
                <input
                  ref={searchInputRef}
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={t('templates.searchPlaceholder')}
                  className="min-w-0 flex-1 bg-transparent text-xs text-fg outline-none placeholder:text-fg-subtle"
                  {...PLAIN_TEXT_INPUT}
                />
              </div>
              {currentDbType && (
                <div className="flex shrink-0 items-center gap-0.5 rounded-control border border-line bg-surface p-0.5 text-xs">
                  <button
                    type="button"
                    onClick={() => setScope('compatible')}
                    className={clsx(
                      'rounded-control px-2 py-1 transition-colors',
                      scope === 'compatible'
                        ? 'bg-accent-soft font-medium text-accent'
                        : 'text-fg-muted hover:text-fg',
                    )}
                  >
                    {t('templates.scopeCompatible')}
                  </button>
                  <button
                    type="button"
                    onClick={() => setScope('all')}
                    className={clsx(
                      'rounded-control px-2 py-1 transition-colors',
                      scope === 'all'
                        ? 'bg-accent-soft font-medium text-accent'
                        : 'text-fg-muted hover:text-fg',
                    )}
                  >
                    {t('templates.scopeAll')}
                  </button>
                </div>
              )}
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto px-5 py-3">
              {templates.length === 0 ? (
                <p className="py-8 text-center text-xs leading-relaxed text-fg-subtle">
                  {t('templates.empty', { shortcut: formatShortcut(SHORTCUTS.commandPalette) })}
                </p>
              ) : filtered.length === 0 ? (
                <p className="py-8 text-center text-xs text-fg-subtle">
                  {t('templates.noMatches')}
                </p>
              ) : (
                <ul className="divide-y divide-line">
                  {filtered.map((template) => {
                    const params = templateParameterNames(
                      template.sql,
                      template.dbType ?? currentDbType,
                    );
                    return (
                      <li key={template.id} className="py-2.5 first:pt-0.5 last:pb-0.5">
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center gap-1.5">
                              <span className="truncate text-sm font-medium text-fg">
                                {template.title}
                              </span>
                              <span className="rounded-control border border-line bg-surface-muted px-1.5 py-0.5 text-[11px] text-fg-muted">
                                {template.dbType
                                  ? serverLabel({ db_type: template.dbType, options: {} })
                                  : t('templates.generalSql')}
                              </span>
                              {params.map((param) => (
                                <span
                                  key={param}
                                  className="rounded-control border border-accent-line bg-accent-soft px-1.5 py-0.5 font-mono text-[11px] text-accent"
                                >
                                  :{param}
                                </span>
                              ))}
                            </div>
                            <div className="mt-1.5 rounded-control border border-line bg-surface-sunken px-2.5 py-1.5">
                              <pre className="line-clamp-2 overflow-hidden whitespace-pre-wrap font-mono text-xs leading-relaxed text-fg-muted">
                                {template.sql}
                              </pre>
                            </div>
                          </div>

                          <div className="flex shrink-0 items-center gap-1.5 pt-0.5">
                            {onInsert && (
                              <button
                                type="button"
                                onClick={() => {
                                  onInsert(template);
                                  onClose();
                                }}
                                className="rounded-control border border-line-strong px-2.5 py-1 text-xs text-fg hover:bg-surface-hover"
                              >
                                {t('templates.insert')}
                              </button>
                            )}
                            {onInsertAndRun && (
                              <button
                                type="button"
                                onClick={() => {
                                  onInsertAndRun(template);
                                  onClose();
                                }}
                                className="flex items-center gap-1 rounded-control bg-accent px-2.5 py-1 text-xs text-fg-on-accent hover:bg-accent-hover"
                              >
                                <Play size={11} />
                                <span>{t('templates.insertAndRun')}</span>
                              </button>
                            )}
                            {!onInsert && onOpenInTab && (
                              <button
                                type="button"
                                onClick={() => {
                                  onOpenInTab(template);
                                  onClose();
                                }}
                                className="rounded-control bg-accent px-2.5 py-1 text-xs text-fg-on-accent hover:bg-accent-hover"
                              >
                                {t('templates.openInTab')}
                              </button>
                            )}
                            <button
                              type="button"
                              onClick={() => openEdit(template)}
                              aria-label={t('templates.edit')}
                              title={t('templates.edit')}
                              className="rounded-control p-1 text-fg-subtle hover:bg-surface-hover hover:text-fg"
                            >
                              <Edit size={13} />
                            </button>
                            <button
                              type="button"
                              onClick={() => removeTemplate(template.id)}
                              aria-label={t('templates.delete')}
                              title={t('templates.delete')}
                              className="rounded-control p-1 text-fg-subtle hover:bg-danger-soft hover:text-danger"
                            >
                              <Trash2 size={13} />
                            </button>
                          </div>
                        </div>
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
