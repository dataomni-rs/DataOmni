import { ArrowUpRight } from 'lucide-react';
import { useLanguageStore } from '../stores/languageStore';
import { useContextMenu } from '../hooks/useContextMenu';
import { SHORTCUTS, formatShortcut } from '../utils/shortcuts';
import type { ResolvedCellJump } from '../utils/foreignKeyJump';

export interface GridContextTarget {
  /** 数据行下标；为 null 表示右键点在列头上，只显示整列动作 */
  row: number | null;
  column: number;
  x: number;
  y: number;
}

interface GridContextMenuProps {
  target: GridContextTarget;
  /** 右键所在列的列名，用于「导出列 xxx…」清晰提示目标列 */
  columnName?: string;
  onSelectColumn?: (column: number) => void;
  onCopy: (withHeaders: boolean) => void;
  onCopyRow: (row: number) => void;
  onCopyColumnValues?: (column: number) => void;
  onCopyColumn: (column: number) => void;
  onCopyColumnSqlIn?: (column: number) => void;
  /** 这一列当前页全为 NULL 时置灰，避免复制出无意义的空列表 */
  copyColumnSqlInDisabled?: boolean;
  onExportColumn?: (column: number) => void;
  /** 只有这一行此刻能改时才给：表数据页的操作列在最右边，列一多就看不见 */
  onEditRow?: (row: number) => void;
  /** 右键所在单元格可跳转的目标表（物理外键 / SQL JOIN / 目录匹配） */
  jumpTargets?: readonly ResolvedCellJump[];
  onJump?: (jump: ResolvedCellJump) => void;
  onClose: () => void;
}

interface MenuItem {
  label: string;
  hint?: string;
  disabled?: boolean;
  run: () => void;
}

/**
 * 网格的右键菜单（数据格与表头共用）。
 *
 * 键盘上 ⌘C / ⇧Space / ⌥Space 已经够用，但没有任何地方写着它们存在——
 * 「复制这一列」「导出这一列」是每天都要做的动作，不该只有读过快捷键表或去藏列的人才会。
 */
export function GridContextMenu({
  target,
  columnName,
  onSelectColumn,
  onCopy,
  onCopyRow,
  onCopyColumnValues,
  onCopyColumn,
  onCopyColumnSqlIn,
  copyColumnSqlInDisabled = false,
  onExportColumn,
  onEditRow,
  jumpTargets,
  onJump,
  onClose
}: GridContextMenuProps) {
  const t = useLanguageStore((state) => state.t);
  const { ref: panelRef, style } = useContextMenu<HTMLDivElement>(target, onClose);

  const hint = formatShortcut;
  const exportColumnLabel = columnName
    ? t('grid.exportColumnNamed', { column: columnName })
    : t('grid.exportColumn');

  const groups: MenuItem[][] = [];

  if (target.row !== null) {
    const rowIndex = target.row;
    groups.push([
      { label: t('grid.copySelection'), hint: hint(SHORTCUTS.copySelection), run: () => onCopy(false) },
      { label: t('grid.copyWithHeaders'), hint: hint(SHORTCUTS.copyWithHeaders), run: () => onCopy(true) },
      { label: t('grid.copyRow'), hint: hint(SHORTCUTS.copyRow), run: () => onCopyRow(rowIndex) }
    ]);
  }

  const columnItems: MenuItem[] = [];
  if (target.row === null && onSelectColumn) {
    columnItems.push({ label: t('grid.selectColumn'), run: () => onSelectColumn(target.column) });
  }
  if (onCopyColumnValues) {
    columnItems.push({
      label: t('grid.copyColumnValues'),
      run: () => onCopyColumnValues(target.column)
    });
  }
  columnItems.push({
    label: t('grid.copyColumn'),
    hint: hint(SHORTCUTS.copyColumn),
    run: () => onCopyColumn(target.column)
  });
  if (onCopyColumnSqlIn) {
    columnItems.push({
      label: t('grid.copyColumnSqlIn'),
      disabled: copyColumnSqlInDisabled,
      run: () => onCopyColumnSqlIn(target.column)
    });
  }
  if (target.row !== null && onExportColumn) {
    columnItems.push({
      label: exportColumnLabel,
      run: () => onExportColumn(target.column)
    });
  }
  groups.push(columnItems);

  if (target.row === null && onExportColumn) {
    groups.push([{ label: exportColumnLabel, run: () => onExportColumn(target.column) }]);
  }

  if (target.row !== null && onEditRow) {
    const rowIndex = target.row;
    groups.push([{ label: t('grid.editRow'), run: () => onEditRow(rowIndex) }]);
  }

  const jumps = target.row !== null && onJump && jumpTargets && jumpTargets.length > 0 ? jumpTargets : [];

  return (
    <div
      ref={panelRef}
      style={style}
      className="fixed z-50 min-w-48 max-w-72 rounded-control border border-line-strong bg-surface py-1 shadow-lg"
    >
      {groups.map((items, groupIndex) => (
        <div key={groupIndex}>
          {groupIndex > 0 && <div className="my-1 border-t border-line" />}
          {items.map((item) => (
            <button
              key={item.label}
              type="button"
              disabled={item.disabled}
              onClick={() => {
                if (item.disabled) {
                  return;
                }
                item.run();
                onClose();
              }}
              className="flex w-full items-center justify-between gap-6 px-3 py-1.5 text-left text-xs text-fg hover:bg-surface-hover disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
            >
              <span className="truncate">{item.label}</span>
              {item.hint && <span className="shrink-0 text-fg-subtle">{item.hint}</span>}
            </button>
          ))}
        </div>
      ))}
      {jumps.length > 0 && onJump && (
        <>
          <div className="my-1 border-t border-line" />
          {jumps.map((jump) => (
            <button
              key={`${jump.schema ?? ''}.${jump.table}:${jump.targetLabel}`}
              type="button"
              onClick={() => {
                onJump(jump);
                onClose();
              }}
              className="flex w-full items-center justify-between gap-4 px-3 py-1.5 text-left text-xs text-accent hover:bg-surface-hover"
            >
              <span className="truncate">{t('grid.jumpToTable', { target: jump.targetLabel })}</span>
              <ArrowUpRight size={12} className="shrink-0" />
            </button>
          ))}
        </>
      )}
    </div>
  );
}
