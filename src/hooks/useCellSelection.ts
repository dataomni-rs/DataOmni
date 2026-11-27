import { useCallback, useEffect, useRef, useState } from 'react';
import type { SerializedResultValue } from '../contracts/resultSet';
import {
  columnSelection,
  isCellMoveKey,
  isWithinSelection,
  moveSelection,
  retainSelection,
  rowSelection,
  selectAllCells,
  selectionToClipboardText,
  type CellSelection
} from '../utils/cellSelection';
import { describeError } from '../utils/describeError';
import { translateNow } from '../stores/languageStore';
import { SHORTCUTS, hasCommandModifier, matchesShortcut, isImeKeyEvent } from '../utils/shortcuts';

/** 按键来自一个能打字的元素（输入框、文本域、下拉框、可编辑区域） */
export function isTextEntry(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

export interface CellSelectionController {
  selection: CellSelection | null;
  isSelected: (row: number, column: number) => boolean;
  /** 焦点格：方向键从它出发，视觉上描一圈边框 */
  isFocused: (row: number, column: number) => boolean;
  selectCell: (row: number, column: number, extend?: boolean) => void;
  /** 左键按下起点：开启矩形拖拽选区，Shift+按下从原锚点延伸 */
  beginDragSelection: (row: number, column: number, extend?: boolean) => void;
  /** 拖拽经过单元格时只更新焦点端，保持矩形选区而不触发跨列文本高亮 */
  updateDragSelection: (row: number, column: number, primaryButtonDown: boolean) => void;
  /** 点击兜底：真实指针事件已在 mousedown 处理，合成 click 测试仍照常选格 */
  handleCellClick: (row: number, column: number, extend?: boolean) => void;
  /** 选中整列（例如表头右键「选中整列」） */
  selectColumn: (column: number) => void;
  /** 复制当前选区；`withHeaders` 在最前面加一行列名 */
  copy: (withHeaders?: boolean) => void;
  /** 把任意格式化好的文本写入剪贴板，共用同一套失败提示（如整列 SQL IN 列表） */
  copyText: (text: string) => void;
  /**
   * 选中并复制整行 / 整列。
   *
   * 不拆成「先选后复制」两步：选区是 state，刚 set 进去的值在同一次事件里
   * 读不到，复制出来的会是上一次的选区
   */
  copyRow: (row: number, withHeaders?: boolean) => void;
  copyColumn: (column: number, withHeaders?: boolean) => void;
  clearSelection: () => void;
  /** 挂在可滚动容器上：需要 tabIndex 才能收到键盘事件 */
  gridProps: {
    tabIndex: number;
    onKeyDown: (event: React.KeyboardEvent) => void;
  };
  /** 复制失败时的提示；成功时为 null */
  copyError: string | null;
}

/**
 * 网格的单元格选择与复制。
 *
 * `rows` 换了引用就清掉选区：留着的话焦点会指向一份已经不存在的数据，
 * 复制出来的是另一行的内容。
 *
 * 因此调用方必须传一个**稳定的**数组引用（useMemo 或 state），不能每次渲染
 * 现 slice 一个新数组——那样选区会在每次渲染时被清掉，根本选不中。
 */
/**
 * @param datasetKey 标识「这是哪一份数据」。刷新时保持不变，翻页 / 改排序 /
 *   改筛选 / 换表时必须变——选区的去留全看它，见 `retainSelection`
 */
export interface CellSelectionShortcuts {
  onFind?: () => void;
  onFillDown?: () => void;
}

export function useCellSelection(
  rows: readonly (readonly SerializedResultValue[])[],
  columns: readonly string[],
  datasetKey: string,
  extraShortcuts?: CellSelectionShortcuts
): CellSelectionController {
  const columnCount = columns.length;
  const [selection, setSelection] = useState<CellSelection | null>(null);
  const [copyError, setCopyError] = useState<string | null>(null);
  const extraShortcutsRef = useRef(extraShortcuts);
  extraShortcutsRef.current = extraShortcuts;
  const draggingRef = useRef(false);
  const dragMovedRef = useRef(false);
  const pointerDownCellRef = useRef<{ row: number; column: number } | null>(null);

  useEffect(() => {
    const endDrag = () => {
      if (!draggingRef.current) {
        return;
      }
      draggingRef.current = false;
      if (dragMovedRef.current) {
        pointerDownCellRef.current = null;
      }
    };
    window.addEventListener('mouseup', endDrag);
    window.addEventListener('blur', endDrag);
    return () => {
      window.removeEventListener('mouseup', endDrag);
      window.removeEventListener('blur', endDrag);
    };
  }, []);

  const rowCount = rows.length;

  const datasetKeyRef = useRef(datasetKey);
  useEffect(() => {
    const sameDataset = datasetKeyRef.current === datasetKey;
    datasetKeyRef.current = datasetKey;
    setSelection((current) => retainSelection(current, { rowCount, columnCount }, sameDataset));
  }, [datasetKey, rows, rowCount, columnCount]);

  const rowsRef = useRef(rows);
  rowsRef.current = rows;
  const columnsRef = useRef(columns);
  columnsRef.current = columns;

  const writeClipboard = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopyError(null);
    } catch (cause) {
      // 剪贴板可能被权限或非安全上下文拒绝；静默失败会让人以为复制成功了
      setCopyError(describeError(cause, translateNow('common.copyFailed')));
    }
  }, []);

  const copySelection = useCallback(async (current: CellSelection, withHeaders = false) => {
    const text = selectionToClipboardText(
      rowsRef.current,
      current,
      withHeaders ? { headers: columnsRef.current } : {}
    );
    await writeClipboard(text);
  }, [writeClipboard]);

  const handleKeyDown = useCallback((event: React.KeyboardEvent) => {
    // 格子里正开着编辑框时，按键归编辑框：方向键挪光标、⌘A 全选文字、⌘C 复制
    // 选中的文字。这个处理器挂在整张表上，编辑框里的按键会冒泡上来——
    // 不让开的话，方向键被 preventDefault 吃掉，⌘A 选中的是一整张网格
    if (isTextEntry(event.target)) {
      return;
    }
    const commandKey = hasCommandModifier(event);

    const withHeaders = matchesShortcut(event, SHORTCUTS.copyWithHeaders);
    if (withHeaders || matchesShortcut(event, SHORTCUTS.copySelection)) {
      if (selection) {
        event.preventDefault();
        void copySelection(selection, withHeaders);
      }
      return;
    }

    // Shift+Space 选整行，沿用表格软件的惯例。
    //
    // 整列没有用惯例里的 ⌃Space：macOS 上它默认是「切换输入法」，而这个应用
    // 的用户基本都装着不止一种输入源，那个键按下去根本到不了这里。⌥Space 不是
    // 任何人熟悉的约定，但它在右键菜单里写着，而 ⌃Space 写了也按不出来。
    //
    // 判 `code` 不判 `key`：macOS 上 ⌥Space 产生的是不换行空格（U+00A0），
    // `event.key === ' '` 会漏掉它
    if (event.code === 'Space' && (event.shiftKey || event.altKey)) {
      const focus = selection?.focus;
      if (focus) {
        event.preventDefault();
        setSelection(
          event.shiftKey
            ? rowSelection(focus.row, { rowCount, columnCount })
            : columnSelection(focus.column, { rowCount, columnCount })
        );
      }
      return;
    }

    if (matchesShortcut(event, SHORTCUTS.selectAll)) {
      event.preventDefault();
      setSelection(selectAllCells({ rowCount, columnCount }));
      return;
    }

    if (matchesShortcut(event, SHORTCUTS.findInGrid) && extraShortcutsRef.current?.onFind) {
      event.preventDefault();
      extraShortcutsRef.current.onFind();
      return;
    }

    if (matchesShortcut(event, SHORTCUTS.fillDown) && extraShortcutsRef.current?.onFillDown) {
      event.preventDefault();
      extraShortcutsRef.current.onFillDown();
      return;
    }

    if (event.key === 'Escape' && !isImeKeyEvent(event.nativeEvent)) {
      setSelection(null);
      return;
    }

    if (!isCellMoveKey(event.key)) {
      return;
    }

    // 方向键在可滚动容器里默认会滚动页面，交给选区之后再由浏览器滚到可视区
    event.preventDefault();
    setSelection((current) => moveSelection(current, event.key as never, { rowCount, columnCount }, {
      extend: event.shiftKey,
      toEdge: commandKey
    }));
  }, [selection, copySelection, rowCount, columnCount]);

  return {
    selection,
    isSelected: useCallback(
      (row, column) => selection !== null && isWithinSelection(selection, row, column),
      [selection]
    ),
    isFocused: useCallback(
      (row, column) => selection?.focus.row === row && selection?.focus.column === column,
      [selection]
    ),
    selectCell: useCallback((row, column, extend = false) => {
      setSelection((current) => ({
        anchor: extend && current ? current.anchor : { row, column },
        focus: { row, column }
      }));
    }, []),
    beginDragSelection: useCallback((row, column, extend = false) => {
      draggingRef.current = true;
      dragMovedRef.current = false;
      pointerDownCellRef.current = { row, column };
      setSelection((current) => ({
        anchor: extend && current ? current.anchor : { row, column },
        focus: { row, column }
      }));
    }, []),
    updateDragSelection: useCallback((row, column, primaryButtonDown) => {
      if (!draggingRef.current) {
        return;
      }
      if (!primaryButtonDown) {
        draggingRef.current = false;
        return;
      }
      setSelection((current) => {
        if (!current) {
          return { anchor: { row, column }, focus: { row, column } };
        }
        if (current.focus.row === row && current.focus.column === column) {
          return current;
        }
        dragMovedRef.current = true;
        return { anchor: current.anchor, focus: { row, column } };
      });
    }, []),
    handleCellClick: useCallback((row, column, extend = false) => {
      const handled = pointerDownCellRef.current;
      if (handled && handled.row === row && handled.column === column) {
        pointerDownCellRef.current = null;
        return;
      }
      if (dragMovedRef.current) {
        dragMovedRef.current = false;
        pointerDownCellRef.current = null;
        return;
      }
      setSelection((current) => ({
        anchor: extend && current ? current.anchor : { row, column },
        focus: { row, column }
      }));
    }, []),
    selectColumn: useCallback((column) => {
      const next = columnSelection(column, { rowCount, columnCount });
      if (next) {
        setSelection(next);
      }
    }, [rowCount, columnCount]),
    copy: useCallback((withHeaders = false) => {
      if (selection) {
        void copySelection(selection, withHeaders);
      }
    }, [selection, copySelection]),
    copyText: useCallback((text) => {
      void writeClipboard(text);
    }, [writeClipboard]),
    copyRow: useCallback((row, withHeaders = false) => {
      const next = rowSelection(row, { rowCount, columnCount });
      if (next) {
        setSelection(next);
        void copySelection(next, withHeaders);
      }
    }, [rowCount, columnCount, copySelection]),
    copyColumn: useCallback((column, withHeaders = false) => {
      const next = columnSelection(column, { rowCount, columnCount });
      if (next) {
        setSelection(next);
        void copySelection(next, withHeaders);
      }
    }, [rowCount, columnCount, copySelection]),
    clearSelection: useCallback(() => setSelection(null), []),
    gridProps: { tabIndex: 0, onKeyDown: handleKeyDown },
    copyError
  };
}
