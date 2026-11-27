import { useMemo, useState } from 'react';
import { Check, ChevronDown, ChevronUp, Copy, Search, X } from 'lucide-react';
import clsx from 'clsx';
import type { SerializedResultValue } from '../contracts/resultSet';
import { PLAIN_TEXT_INPUT } from './FormControls';
import { useLanguageStore } from '../stores/languageStore';
import {
  buildRowInspection,
  filterRowInspection,
  type RowInspectorColumn
} from '../utils/rowInspector';

interface GridRowInspectorProps {
  columns: readonly RowInspectorColumn[];
  rowA: readonly SerializedResultValue[];
  rowALabel: string;
  rowB?: readonly SerializedResultValue[];
  rowBLabel?: string;
  onCopyText: (text: string) => void;
  onPrevRow?: () => void;
  onNextRow?: () => void;
  canPrevRow?: boolean;
  canNextRow?: boolean;
  maxHeight?: number;
  onClose: () => void;
}

/**
 * 宽表「单行竖排详情 / 双行并排对比」侧栏（类似 Excel 转置查阅与差异比对）。
 */
export function GridRowInspector({
  columns,
  rowA,
  rowALabel,
  rowB,
  rowBLabel,
  onCopyText,
  onPrevRow,
  onNextRow,
  canPrevRow = false,
  canNextRow = false,
  maxHeight,
  onClose
}: GridRowInspectorProps) {
  const t = useLanguageStore((state) => state.t);
  const [query, setQuery] = useState('');
  const [onlyDifferences, setOnlyDifferences] = useState(false);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const isCompare = rowB !== undefined;

  const fields = useMemo(
    () => buildRowInspection(columns, rowA, rowB),
    [columns, rowA, rowB]
  );
  const diffCount = useMemo(
    () => fields.filter((field) => field.differs).length,
    [fields]
  );
  const visibleFields = useMemo(
    () => filterRowInspection(fields, { query, onlyDifferences: isCompare && onlyDifferences }),
    [fields, query, isCompare, onlyDifferences]
  );

  const handleCopy = (id: string, value: string) => {
    onCopyText(value);
    setCopiedId(id);
    window.setTimeout(() => {
      setCopiedId((current) => (current === id ? null : current));
    }, 1200);
  };

  return (
    <aside
      style={maxHeight !== undefined ? { maxHeight: `${maxHeight}px` } : undefined}
      className={clsx(
        'flex shrink-0 flex-col border-l border-line bg-surface',
        isCompare ? 'w-[26rem] max-w-[55%]' : 'w-80 max-w-[45%]'
      )}
      data-testid="grid-row-inspector"
    >
      <div className="flex items-center justify-between gap-2 border-b border-line bg-surface-sunken px-3 py-2">
        <div className="min-w-0">
          <div className="truncate text-xs font-semibold text-fg">
            {isCompare
              ? t('grid.inspector.compareTitle', { a: rowALabel, b: rowBLabel ?? '' })
              : t('grid.inspector.rowTitle', { row: rowALabel })}
          </div>
          {isCompare && (
            <div className="text-[11px] text-fg-muted">
              {t('grid.inspector.diffCount', { count: diffCount })}
            </div>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {!isCompare && onPrevRow && onNextRow && (
            <>
              <button
                type="button"
                onClick={onPrevRow}
                disabled={!canPrevRow}
                aria-label={t('grid.inspector.prevRow')}
                title={t('grid.inspector.prevRow')}
                className="rounded-control p-1 text-fg-muted hover:bg-surface-hover hover:text-fg disabled:opacity-40"
              >
                <ChevronUp size={14} />
              </button>
              <button
                type="button"
                onClick={onNextRow}
                disabled={!canNextRow}
                aria-label={t('grid.inspector.nextRow')}
                title={t('grid.inspector.nextRow')}
                className="rounded-control p-1 text-fg-muted hover:bg-surface-hover hover:text-fg disabled:opacity-40"
              >
                <ChevronDown size={14} />
              </button>
            </>
          )}
          <button
            type="button"
            onClick={onClose}
            aria-label={t('common.close')}
            title={t('common.close')}
            className="rounded-control p-1 text-fg-muted hover:bg-surface-hover hover:text-fg"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      <div className="flex flex-col gap-1.5 border-b border-line px-3 py-2">
        <div className="relative flex items-center">
          <Search size={12} className="pointer-events-none absolute left-2 text-fg-subtle" />
          <input
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('grid.inspector.filterFields')}
            aria-label={t('grid.inspector.filterFields')}
            className="w-full rounded-control border border-line-strong bg-surface py-1 pl-7 pr-6 text-xs text-fg outline-none focus:border-accent"
            {...PLAIN_TEXT_INPUT}
          />
          {query.length > 0 && (
            <button
              type="button"
              onClick={() => setQuery('')}
              aria-label={t('grid.findClear')}
              className="absolute right-1.5 rounded p-0.5 text-fg-subtle hover:bg-surface-hover hover:text-fg"
            >
              <X size={11} />
            </button>
          )}
        </div>

        {isCompare && (
          <label className="flex cursor-pointer items-center gap-1.5 text-xs text-fg-muted select-none">
            <input
              type="checkbox"
              checked={onlyDifferences}
              onChange={(event) => setOnlyDifferences(event.target.checked)}
              className="rounded border-line-strong"
            />
            <span>{t('grid.inspector.onlyDifferences')}</span>
          </label>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-auto divide-y divide-line">
        {visibleFields.length === 0 ? (
          <div className="px-3 py-6 text-center text-xs text-fg-subtle">
            {t('grid.inspector.empty')}
          </div>
        ) : (
          visibleFields.map((field) => (
            <div
              key={`${field.index}-${field.name}`}
              className={clsx(
                'px-3 py-2 text-xs',
                isCompare && field.differs && 'bg-warning-soft/50'
              )}
            >
              <div className="mb-1 flex items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-1.5">
                  <span className="truncate font-medium text-fg" title={field.name}>
                    {field.name}
                  </span>
                  {field.dataType && (
                    <span className="shrink-0 text-[10px] text-fg-subtle">{field.dataType}</span>
                  )}
                </div>
                {!isCompare && (
                  <button
                    type="button"
                    onClick={() => handleCopy(`a-${field.index}`, field.prettyJsonA ?? field.formattedA)}
                    title={t('common.copy')}
                    aria-label={t('common.copy')}
                    className="shrink-0 rounded p-0.5 text-fg-subtle hover:bg-surface-hover hover:text-fg"
                  >
                    {copiedId === `a-${field.index}` ? <Check size={12} className="text-success" /> : <Copy size={12} />}
                  </button>
                )}
              </div>

              {isCompare ? (
                <div className="grid grid-cols-2 gap-2">
                  <div className="min-w-0 rounded-control border border-line bg-surface-sunken p-1.5">
                    <div className="mb-0.5 flex items-center justify-between text-[10px] text-fg-subtle">
                      <span>{rowALabel}</span>
                      <button
                        type="button"
                        onClick={() => handleCopy(`a-${field.index}`, field.prettyJsonA ?? field.formattedA)}
                        aria-label={t('common.copy')}
                        className="rounded p-0.5 hover:bg-surface-hover hover:text-fg"
                      >
                        {copiedId === `a-${field.index}` ? <Check size={10} className="text-success" /> : <Copy size={10} />}
                      </button>
                    </div>
                    <pre className={clsx(
                      'max-h-40 overflow-auto whitespace-pre-wrap break-all font-mono text-[11px]',
                      field.valueA === null ? 'italic text-fg-subtle' : 'text-fg'
                    )}
                    >
                      {field.prettyJsonA ?? field.formattedA}
                    </pre>
                  </div>
                  <div className="min-w-0 rounded-control border border-line bg-surface-sunken p-1.5">
                    <div className="mb-0.5 flex items-center justify-between text-[10px] text-fg-subtle">
                      <span>{rowBLabel ?? ''}</span>
                      <button
                        type="button"
                        onClick={() => handleCopy(`b-${field.index}`, field.prettyJsonB ?? field.formattedB ?? '')}
                        aria-label={t('common.copy')}
                        className="rounded p-0.5 hover:bg-surface-hover hover:text-fg"
                      >
                        {copiedId === `b-${field.index}` ? <Check size={10} className="text-success" /> : <Copy size={10} />}
                      </button>
                    </div>
                    <pre className={clsx(
                      'max-h-40 overflow-auto whitespace-pre-wrap break-all font-mono text-[11px]',
                      field.valueB === null ? 'italic text-fg-subtle' : 'text-fg'
                    )}
                    >
                      {field.prettyJsonB ?? field.formattedB ?? ''}
                    </pre>
                  </div>
                </div>
              ) : (
                <pre className={clsx(
                  'max-h-48 overflow-auto whitespace-pre-wrap break-all rounded-control bg-surface-sunken px-2 py-1 font-mono text-[11px]',
                  field.valueA === null ? 'italic text-fg-subtle' : 'text-fg'
                )}
                >
                  {field.prettyJsonA ?? field.formattedA}
                </pre>
              )}
            </div>
          ))
        )}
      </div>
    </aside>
  );
}
