import type { ReactNode } from 'react';
import { ArrowDown, ArrowUp, ChevronsUpDown } from 'lucide-react';
import { clsx } from 'clsx';
import type { SortDirection } from '../utils/resultSorting';
import { useLanguageStore } from '../stores/languageStore';

interface ColumnSortButtonProps {
  /** 该列当前的排序方向；未按此列排序时为 null */
  direction: SortDirection | null;
  /** 多列组合排序时的 1-based 优先级；单列或未排序时不显示 */
  priority?: number;
  onToggle: (append: boolean) => void;
  columnLabel: string;
  /** 传入列名/标签作为按钮内容，让点击整段列头都能触发排序与 Shift+点击多列排序 */
  children?: ReactNode;
}

/**
 * 表头上的排序按钮。
 *
 * 未排序时也画一个淡箭头，否则「这列能不能点」只能靠试。悬停才显形会让
 * 触控板用户完全发现不了。按住 Shift 点击可追加/循环次级排序列；
 * `mousedown` 拦截 Shift 下的原生文本框选，避免 Shift+点击列头把整行表头涂蓝。
 */
export function ColumnSortButton({ direction, priority, onToggle, columnLabel, children }: ColumnSortButtonProps) {
  const t = useLanguageStore((state) => state.t);
  const baseLabel = direction === 'asc'
    ? t('grid.sortAscHint', { column: columnLabel })
    : direction === 'desc'
      ? t('grid.sortDescHint', { column: columnLabel })
      : t('grid.sortNoneHint', { column: columnLabel });
  const label = t('grid.sortMultiHint', { base: baseLabel });

  return (
    <button
      type="button"
      onMouseDown={(event) => {
        if (event.shiftKey) {
          event.preventDefault();
        }
      }}
      onClick={(event) => onToggle(event.shiftKey)}
      title={label}
      aria-label={label}
      className={clsx(
        'group/sort inline-flex min-w-0 select-none items-center gap-1 rounded-control px-0.5 py-0.5 text-left transition-colors hover:bg-surface-hover/70',
        children ? 'max-w-full flex-1' : 'shrink-0'
      )}
    >
      <span
        className={clsx(
          'inline-flex shrink-0 items-center gap-0.5 transition-colors',
          direction ? 'text-accent' : 'text-fg-subtle opacity-45 group-hover/sort:opacity-100'
        )}
      >
        {direction === 'asc'
          ? <ArrowUp size={12} />
          : direction === 'desc'
            ? <ArrowDown size={12} />
            : <ChevronsUpDown size={12} />}
        {direction && priority !== undefined && (
          <span className="inline-flex h-3.5 min-w-[14px] items-center justify-center rounded-full bg-accent-soft px-1 text-[9px] font-semibold leading-none text-accent">
            {priority}
          </span>
        )}
      </span>
      {children}
    </button>
  );
}
