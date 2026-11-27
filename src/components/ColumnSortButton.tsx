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
}

/**
 * 表头上的排序指示。
 *
 * 未排序时也画一个淡箭头，否则「这列能不能点」只能靠试。悬停才显形会让
 * 触控板用户完全发现不了。按住 Shift 点击可追加/循环次级排序列。
 */
export function ColumnSortButton({ direction, priority, onToggle, columnLabel }: ColumnSortButtonProps) {
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
      onClick={(event) => onToggle(event.shiftKey)}
      title={label}
      aria-label={label}
      className={clsx(
        'inline-flex shrink-0 items-center gap-0.5 rounded-control p-0.5 transition-colors',
        direction ? 'text-accent' : 'text-fg-subtle opacity-40 hover:opacity-100'
      )}
    >
      {direction === 'asc'
        ? <ArrowUp size={12} />
        : direction === 'desc'
          ? <ArrowDown size={12} />
          : <ChevronsUpDown size={12} />}
      {direction && priority !== undefined && (
        <span className="text-[9px] font-semibold leading-none">{priority}</span>
      )}
    </button>
  );
}
