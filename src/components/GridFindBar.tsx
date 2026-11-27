import { useEffect, useRef } from 'react';
import { ChevronDown, ChevronUp, Search, X } from 'lucide-react';
import clsx from 'clsx';
import { PLAIN_TEXT_INPUT } from './FormControls';
import { useLanguageStore } from '../stores/languageStore';
import { isImeKeyEvent } from '../utils/shortcuts';

interface GridFindBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  matchCount: number;
  activeMatchIndex: number;
  onPrev: () => void;
  onNext: () => void;
  onClose: () => void;
  scopeHint: string;
  /** 查询结果集提供「仅看匹配行」即时筛选开关 */
  filterRows?: boolean;
  onToggleFilterRows?: () => void;
  filteredRowCount?: number;
  totalRowCount?: number;
}

export function GridFindBar({
  query,
  onQueryChange,
  matchCount,
  activeMatchIndex,
  onPrev,
  onNext,
  onClose,
  scopeHint,
  filterRows,
  onToggleFilterRows,
  filteredRowCount,
  totalRowCount
}: GridFindBarProps) {
  const t = useLanguageStore((state) => state.t);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const hasQuery = query.trim().length > 0;
  const showRowRatio = Boolean(
    filterRows
    && hasQuery
    && filteredRowCount !== undefined
    && totalRowCount !== undefined
  );

  return (
    <div
      className="flex flex-wrap items-center gap-2 border-b border-line bg-surface-sunken px-3 py-1.5 text-xs"
      data-testid="grid-find-bar"
    >
      <div className="relative flex min-w-52 flex-1 max-w-sm items-center">
        <Search size={13} className="pointer-events-none absolute left-2 text-fg-subtle" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          onKeyDown={(event) => {
            if (isImeKeyEvent(event.nativeEvent)) {
              return;
            }
            if (event.key === 'Escape') {
              event.preventDefault();
              onClose();
            } else if (event.key === 'Enter') {
              event.preventDefault();
              if (event.shiftKey) {
                onPrev();
              } else {
                onNext();
              }
            }
          }}
          placeholder={t('grid.findPlaceholder')}
          aria-label={t('grid.findPlaceholder')}
          className="w-full rounded-control border border-line-strong bg-surface py-1 pl-7 pr-6 text-xs text-fg outline-none focus:border-accent"
          {...PLAIN_TEXT_INPUT}
        />
        {query.length > 0 && (
          <button
            type="button"
            onClick={() => onQueryChange('')}
            aria-label={t('grid.findClear')}
            className="absolute right-1.5 rounded p-0.5 text-fg-subtle hover:bg-surface-hover hover:text-fg"
          >
            <X size={12} />
          </button>
        )}
      </div>

      {onToggleFilterRows && (
        <button
          type="button"
          onClick={onToggleFilterRows}
          className={clsx(
            'rounded-control border px-2 py-1 text-xs transition-colors',
            filterRows
              ? 'border-accent-line bg-accent-soft text-accent'
              : 'border-line-strong text-fg-muted hover:bg-surface-hover hover:text-fg'
          )}
        >
          {t('grid.findFilterRows')}
        </button>
      )}

      <span className="text-fg-muted">
        {!hasQuery
          ? scopeHint
          : matchCount === 0
            ? t('grid.findNoMatches')
            : showRowRatio
              ? t('grid.findMatchedRows', {
                  matched: filteredRowCount ?? 0,
                  total: totalRowCount ?? 0
                })
              : t('grid.findMatchCounter', {
                  current: Math.min(activeMatchIndex + 1, matchCount),
                  total: matchCount
                })}
      </span>

      <div className="flex items-center gap-0.5">
        <button
          type="button"
          onClick={onPrev}
          disabled={matchCount === 0}
          aria-label={t('grid.findPrev')}
          title={t('grid.findPrev')}
          className="rounded-control p-1 text-fg-muted hover:bg-surface-hover hover:text-fg disabled:opacity-40"
        >
          <ChevronUp size={14} />
        </button>
        <button
          type="button"
          onClick={onNext}
          disabled={matchCount === 0}
          aria-label={t('grid.findNext')}
          title={t('grid.findNext')}
          className="rounded-control p-1 text-fg-muted hover:bg-surface-hover hover:text-fg disabled:opacity-40"
        >
          <ChevronDown size={14} />
        </button>
      </div>

      <span className="ml-auto hidden text-[11px] text-fg-subtle sm:inline">{scopeHint}</span>

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
  );
}
