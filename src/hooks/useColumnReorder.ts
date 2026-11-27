import { useCallback, useEffect, useRef, useState } from 'react';
import { isImeKeyEvent } from '../utils/shortcuts';

/** 拖动超过该像素距离才进入列重排态，避免轻微手抖把正常点击排序误判为拖列 */
const DRAG_THRESHOLD_PX = 6;

export interface ColumnReorderState {
  /** 正在拖动的列（原始下标）；未越过拖动阈值前为 null */
  draggingIndex: number | null;
  /** 当前悬停的目标列（原始下标）；与拖动列相同时为 null */
  overIndex: number | null;
  /** 插入指示线画在目标列哪一侧 */
  dropSide: 'left' | 'right' | null;
  /** 挂在列头 <th> 的 onMouseDown 上；会自动避开列宽拖动把手和 Shift+点击多列排序 */
  startColumnDrag: (
    originalIndex: number,
    displayIndex: number,
    event: React.MouseEvent<HTMLElement>
  ) => void;
}

/**
 * 表头拖动交换/重排列顺序。
 *
 * 用原生 mouse 事件而不是 HTML5 `draggable`：
 * 1. 避开 macOS WKWebView / Tauri 原生文件拖放与 `<button>` 内嵌拖动冲突；
 * 2. `< 6px` 移动不拦截点击，单击排序和 `Shift+点击` 多列排序完全不受影响；
 * 3. 真正发生拖动并在释放时完成重排后，拦截紧随其后的合成 `click`，防止拖完列顺手把该列排序了。
 */
export function useColumnReorder(
  onReorder: (fromOriginalIndex: number, toOriginalIndex: number) => void
): ColumnReorderState {
  const [draggingIndex, setDraggingIndex] = useState<number | null>(null);
  const [overIndex, setOverIndex] = useState<number | null>(null);
  const [dropSide, setDropSide] = useState<'left' | 'right' | null>(null);

  const onReorderRef = useRef(onReorder);
  onReorderRef.current = onReorder;
  const detachRef = useRef<(() => void) | null>(null);

  useEffect(() => () => detachRef.current?.(), []);

  const startColumnDrag = useCallback((
    originalIndex: number,
    displayIndex: number,
    event: React.MouseEvent<HTMLElement>
  ) => {
    // 非主键、按住 Shift（多列排序）或点在列宽分隔线上都不启动拖列
    if (event.button !== 0 || event.shiftKey) {
      return;
    }
    if (event.target instanceof HTMLElement && event.target.closest('[role="separator"]')) {
      return;
    }

    const headerRow = event.currentTarget.closest('tr');
    if (!headerRow) {
      return;
    }

    detachRef.current?.();

    const startX = event.clientX;
    const startY = event.clientY;
    let active = false;
    let currentOver: number | null = null;
    let previousCursor = '';

    const resolveTarget = (clientX: number): { orig: number; display: number } | null => {
      const headers = Array.from(headerRow.querySelectorAll<HTMLElement>('[data-col-orig-index]'));
      if (headers.length === 0) {
        return null;
      }
      let firstMeta: { orig: number; display: number; rect: DOMRect } | null = null;
      let lastMeta: { orig: number; display: number; rect: DOMRect } | null = null;

      for (const th of headers) {
        const orig = Number(th.dataset.colOrigIndex);
        const display = Number(th.dataset.colDisplayIndex);
        if (!Number.isInteger(orig) || !Number.isInteger(display)) {
          continue;
        }
        const rect = th.getBoundingClientRect();
        if (!firstMeta) {
          firstMeta = { orig, display, rect };
        }
        lastMeta = { orig, display, rect };
        if (clientX >= rect.left && clientX <= rect.right) {
          return { orig, display };
        }
      }
      if (firstMeta && clientX < firstMeta.rect.left) {
        return { orig: firstMeta.orig, display: firstMeta.display };
      }
      if (lastMeta && clientX > lastMeta.rect.right) {
        return { orig: lastMeta.orig, display: lastMeta.display };
      }
      return null;
    };

    const cleanup = () => {
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
      window.removeEventListener('blur', handleCancel);
      window.removeEventListener('keydown', handleKeyDown);
      if (active) {
        document.body.style.cursor = previousCursor;
      }
      detachRef.current = null;
      setDraggingIndex(null);
      setOverIndex(null);
      setDropSide(null);
    };

    const suppressNextClick = () => {
      const stopClick = (clickEvent: MouseEvent) => {
        clickEvent.preventDefault();
        clickEvent.stopPropagation();
      };
      window.addEventListener('click', stopClick, true);
      window.setTimeout(() => {
        window.removeEventListener('click', stopClick, true);
      }, 0);
    };

    const handleMove = (moveEvent: MouseEvent) => {
      if ((moveEvent.buttons & 1) !== 1 && moveEvent.buttons !== 0) {
        cleanup();
        return;
      }
      if (!active) {
        if (Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY) < DRAG_THRESHOLD_PX) {
          return;
        }
        active = true;
        previousCursor = document.body.style.cursor;
        document.body.style.cursor = 'grabbing';
        setDraggingIndex(originalIndex);
      }

      const target = resolveTarget(moveEvent.clientX);
      if (!target || target.orig === originalIndex) {
        currentOver = null;
        setOverIndex(null);
        setDropSide(null);
        return;
      }
      currentOver = target.orig;
      setOverIndex(target.orig);
      setDropSide(target.display > displayIndex ? 'right' : 'left');
    };

    const handleUp = () => {
      const shouldCommit = active;
      const targetOrig = currentOver;
      cleanup();
      if (shouldCommit) {
        suppressNextClick();
        if (targetOrig !== null && targetOrig !== originalIndex) {
          onReorderRef.current(originalIndex, targetOrig);
        }
      }
    };

    const handleCancel = () => {
      cleanup();
    };

    const handleKeyDown = (keyEvent: KeyboardEvent) => {
      if (keyEvent.key === 'Escape' && !isImeKeyEvent(keyEvent)) {
        cleanup();
      }
    };

    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
    window.addEventListener('blur', handleCancel);
    window.addEventListener('keydown', handleKeyDown);
    detachRef.current = cleanup;
  }, []);

  return {
    draggingIndex,
    overIndex,
    dropSide,
    startColumnDrag
  };
}
