import { useEffect, useMemo, useRef, useState } from 'react';
import clsx from 'clsx';
import { HighlightedCode } from './HighlightedCode';
import { Check, ChevronDown, ChevronRight, Download, Loader2 } from 'lucide-react';
import { save } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import type { SqlDialect } from '../contracts/queryExecution';
import type { SerializedResultValue } from '../contracts/resultSet';
import {
  DEFAULT_EXPORT_OPTIONS,
  projectExportColumns,
  serializeExport,
  suggestExportFileName,
  suggestExportSourceName,
  toCsv,
  type CsvDelimiter,
  type ExportFormat,
  type ExportOptions
} from '../utils/exportResult';
import { describeError } from '../utils/describeError';
import { formatBytes } from '../utils/formatBytes';
import { Checkbox, Field, PLAIN_TEXT_INPUT, SegmentedControl } from './FormControls';
import { useLanguageStore } from '../stores/languageStore';
import { taskTitleFields, useTaskStore } from '../stores/taskStore';
import type { TranslationKey } from '../i18n/translate';
import { isImeKeyEvent } from '../utils/shortcuts';

/**
 * 一个可选的导出范围。
 *
 * 带 `sql` 的走后端流式导出：行从数据库直接落盘，不进内存也不过 IPC，
 * 所以整表这种行数未知的范围才有可能导出。不带 `sql` 的导出下面那份已经在
 * 内存里的行——那正是用户此刻看到的、排过序、隐过列的那一份。
 */
export interface ExportScope {
  id: string;
  label: string;
  /** 范围说明：是哪一页、少了哪些列、会不会重新执行一遍查询 */
  note?: string;
  /** 固定 SQL 的流式导出（如被截断的任意查询「完整结果」）；不支持再裁剪列 */
  sql?: string;
  /**
   * 按当前勾选的列与去重选项动态生成流式导出 SQL（表数据视图的「整张表」）。
   * 返回 null 时说明当前组合无法生成合法 SQL，导出按钮随之下灰。
   */
  buildSql?: (selectedColumns: readonly string[], options: { distinct: boolean }) => string | null;
  /**
   * 这个范围自己的那份数据。不给就用对话框的 `columns` / `rows`。
   *
   * 「选中的部分」需要它：选区是个矩形，行和列都可能比屏幕上那份少，
   * 而预览、行列数和最后写出去的文件都得是同一份。
   */
  columns?: readonly string[];
  rows?: ReadonlyArray<readonly SerializedResultValue[]>;
}

interface ExportResultDialogProps {
  columns: readonly string[];
  rows: ReadonlyArray<readonly SerializedResultValue[]>;
  /** 用来起默认文件名：表名，或者标签页名 */
  sourceName: string;
  /** 初始勾选的列下标（默认全部勾选；右键「导出这一列…」进入时只勾那一列） */
  initialColumnIndexes?: readonly number[];
  /** 结果被行数/字节上限截断过——导出的是截断后的那份，必须说清楚 */
  truncated?: boolean;
  /** 服务端分页时，这里只有当前页的数据 */
  scopeNote?: string;
  /** 可选范围。留空等于只有「上面这些行」一个范围，不显示选择器。 */
  scopes?: readonly ExportScope[];
  /** 流式导出要在哪个连接上跑 */
  connectionId?: string;
  /** 给了才有 `INSERT` 语句这一种格式：字面量与标识符得按这家的规矩写 */
  sqlDialect?: SqlDialect;
  /** `INSERT INTO` 的默认表名。查询结果多半说不出来自哪张表，留空让用户填 */
  sqlTable?: string;
  /** 这张表的计算列：`INSERT` 不写它们，见 `ExportOptions.sqlComputedColumns` */
  sqlComputedColumns?: readonly string[];
  /** 这张表的自增列：值照写，按方言写明覆盖，见 `ExportOptions.sqlIdentityColumns` */
  sqlIdentityColumns?: readonly string[];
  /** 这张表靠序列取号的列，见 `ExportOptions.sqlSequenceColumns` */
  sqlSequenceColumns?: readonly string[];
  onClose: () => void;
}

interface ExportSummary {
  rowsWritten: number;
  bytesWritten: number;
  path: string;
}

/** 存文案键而不是文案：模块级常量用不了 hook，而标签要跟着语言走 */
const DELIMITERS: Array<{ value: CsvDelimiter; labelKey: TranslationKey }> = [
  { value: ',', labelKey: 'export.delimiter.comma' },
  { value: ';', labelKey: 'export.delimiter.semicolon' },
  { value: '\t', labelKey: 'export.delimiter.tab' }
];

// `NULL` 与 `\N` 是写进文件里的字面量，不是界面文案，不翻译
const NULL_TEXTS: Array<{ value: string; labelKey?: TranslationKey; label?: string }> = [
  { value: '', labelKey: 'export.nullAs.empty' },
  { value: 'NULL', label: 'NULL' },
  { value: '\\N', label: '\\N' }
];

const PREVIEW_ROWS = 3;

function normalizeInitialColumnIndexes(
  totalColumns: number,
  initial?: readonly number[]
): number[] {
  const valid = (initial ?? []).filter(
    (index) => Number.isInteger(index) && index >= 0 && index < totalColumns
  );
  if (valid.length > 0) {
    return [...new Set(valid)].sort((a, b) => a - b);
  }
  return Array.from({ length: totalColumns }, (_, index) => index);
}

export function ExportResultDialog({
  columns,
  rows,
  sourceName,
  initialColumnIndexes,
  truncated,
  scopeNote,
  scopes,
  connectionId,
  sqlDialect,
  sqlTable,
  sqlComputedColumns,
  sqlIdentityColumns,
  sqlSequenceColumns,
  onClose
}: ExportResultDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const [options, setOptions] = useState<ExportOptions>({
    ...DEFAULT_EXPORT_OPTIONS,
    sqlTable: sqlTable ?? '',
    sqlDialect,
    sqlComputedColumns,
    sqlIdentityColumns,
    sqlSequenceColumns
  });
  const [scopeId, setScopeId] = useState(scopes?.[0]?.id ?? '');
  const [selectedIndexes, setSelectedIndexes] = useState<number[]>(() =>
    normalizeInitialColumnIndexes(columns.length, initialColumnIndexes)
  );
  const [columnPickerOpen, setColumnPickerOpen] = useState(false);
  const [distinct, setDistinct] = useState(false);
  const [writing, setWriting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [written, setWritten] = useState<ExportSummary | null>(null);
  /**
   * 上一次尝试用的路径。失败后「重试」就是拿它再跑一遍——不再弹一次保存
   * 对话框，因为用户已经选过了，让他重选一次只会让人怀疑是不是选错了。
   */
  const [lastPath, setLastPath] = useState<string | null>(null);
  const startTask = useTaskStore((state) => state.start);
  const cancelRef = useRef<HTMLButtonElement>(null);

  const scope = scopes?.find((candidate) => candidate.id === scopeId) ?? scopes?.[0];
  const fixedSqlScope = Boolean(scope?.sql && !scope.buildSql);
  const baseColumns = scope?.columns ?? columns;
  const baseRows = scope?.rows ?? rows;
  const canPickColumns = !fixedSqlScope && !scope?.columns && baseColumns.length > 1;
  const canDistinct = !fixedSqlScope;
  const effectiveDistinct = canDistinct && distinct;

  const selectedSet = useMemo(() => new Set(selectedIndexes), [selectedIndexes]);
  const projected = useMemo(() => {
    const indexes = canPickColumns
      ? selectedIndexes
      : Array.from({ length: baseColumns.length }, (_, index) => index);
    return projectExportColumns(baseColumns, baseRows, indexes, { distinct: effectiveDistinct });
  }, [baseColumns, baseRows, canPickColumns, effectiveDistinct, selectedIndexes]);

  const activeColumns = projected.columns;
  const activeRows = projected.rows;
  const resolvedSql = useMemo(() => {
    if (scope?.buildSql) {
      return scope.buildSql(activeColumns, { distinct: effectiveDistinct });
    }
    return scope?.sql ?? null;
  }, [activeColumns, effectiveDistinct, scope]);
  const streaming = Boolean(scope?.sql || scope?.buildSql);
  const effectiveSourceName = suggestExportSourceName(sourceName, activeColumns, baseColumns.length);
  // 没有表名的 INSERT 是一句语法错误，别让它写进文件
  const missingTable = options.format === 'sql' && options.sqlTable.trim() === '';

  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImeKeyEvent(event) && !writing) {
        event.preventDefault();
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, writing]);

  // 前几行的真实输出。分隔符和 NULL 写法这两个选项，光看名字判断不了对不对，
  // 看一眼结果就知道。
  const preview = useMemo(() => {
    if (options.format === 'json') {
      return serializeExport(activeColumns, activeRows.slice(0, 1), { ...options, byteOrderMark: false });
    }
    if (options.format === 'sql') {
      return missingTable ? '' : serializeExport(activeColumns, activeRows.slice(0, PREVIEW_ROWS), options);
    }
    return toCsv(activeColumns, activeRows.slice(0, PREVIEW_ROWS), options);
  }, [activeColumns, activeRows, missingTable, options]);

  const update = (patch: Partial<ExportOptions>) => {
    setOptions(current => ({ ...current, ...patch }));
    setWritten(null);
    setError(null);
  };

  const toggleColumn = (index: number) => {
    setSelectedIndexes((current) => {
      if (current.includes(index)) {
        if (current.length <= 1) {
          return current;
        }
        return current.filter((item) => item !== index);
      }
      return [...current, index].sort((a, b) => a - b);
    });
    setWritten(null);
    setError(null);
  };

  const selectOnlyColumn = (index: number) => {
    setSelectedIndexes([index]);
    setWritten(null);
    setError(null);
  };

  const selectAllColumns = () => {
    setSelectedIndexes(Array.from({ length: baseColumns.length }, (_, index) => index));
    setWritten(null);
    setError(null);
  };

  const runExport = async (path: string) => {
    setError(null);
    setWritten(null);
    setWriting(true);
    setLastPath(path);

    try {
      if (streaming) {
        if (!resolvedSql) {
          return;
        }
        // 整个范围的导出可能要跑几分钟，交给后台任务：进度、取消与重试都在那边，
        // 这个框关掉也不会打断它
        startTask({
          kind: 'export',
          ...taskTitleFields('task.title.export', { name: effectiveSourceName }),
          payload: { connectionId: connectionId ?? '', sql: resolvedSql, path, options }
        });
        onClose();
        return;
      } else {
        // 这一份已经在内存里了，再让它绕一趟数据库只会导出**另一份**数据：
        // 客户端排序、隐藏列、以及这期间别人对表的改动都会对不上
        const contents = serializeExport(activeColumns, activeRows, options);
        const bytesWritten = await invoke<number>('write_text_file', { path, contents });
        setWritten({ rowsWritten: activeRows.length, bytesWritten, path });
      }
    } catch (err) {
      setError(describeError(err, t('export.failed')));
    } finally {
      setWriting(false);
    }
  };

  const handleExport = async () => {
    const path = await save({
      defaultPath: suggestExportFileName(effectiveSourceName, options.format),
      filters: [
        options.format === 'csv'
          ? { name: 'CSV', extensions: ['csv'] }
          : options.format === 'sql'
            ? { name: 'SQL', extensions: ['sql'] }
            : { name: 'JSON', extensions: ['json'] }
      ]
    }).catch((err) => {
      setError(describeError(err, t('export.failed')));
      return null;
    });

    // 用户取消保存对话框不是错误，也不该留下任何提示
    if (!path) {
      return;
    }
    await runExport(path);
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="export-dialog-title"
      onClick={() => !writing && onClose()}
    >
      <div
        className="flex max-h-[calc(100vh-4rem)] w-[560px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-panel bg-surface-raised shadow-xl"
        onClick={event => event.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-line px-5 py-3">
          <Download size={16} className="shrink-0 text-fg-muted" />
          <h2 id="export-dialog-title" className="text-sm font-medium text-fg">
            {t('export.title')}
          </h2>
          <span className="ml-auto text-xs text-fg-subtle">
            {/* 流式导出时行数是**整个范围**的，不是下面这几行预览的，说「3 行」是假话 */}
            {streaming
              ? t('export.shapeColumns', { count: activeColumns.length })
              : t('export.shape', { rows: activeRows.length, columns: activeColumns.length })}
          </span>
        </div>

        <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-5 py-4">
          {(truncated || scope?.note || scopeNote || (streaming && effectiveDistinct)) && (
            <p className="rounded-control border border-warning-line bg-warning-soft px-3 py-2 text-xs text-warning">
              {[
                truncated ? t('export.truncated') : null,
                scope?.note ?? scopeNote ?? null,
                streaming && effectiveDistinct ? t('export.distinctStreamNote') : null
              ]
                .filter((part): part is string => Boolean(part))
                .join(' ')}
            </p>
          )}

          {scopes && scopes.length > 1 && (
            <Field label={t('export.scope')}>
              <SegmentedControl<string>
                value={scope?.id ?? ''}
                options={scopes.map((candidate) => ({
                  value: candidate.id,
                  label: candidate.label
                }))}
                onChange={(id) => {
                  setScopeId(id);
                  setWritten(null);
                  setError(null);
                }}
              />
            </Field>
          )}

          {canPickColumns && (
            <Field label={t('export.columns')}>
              <div className="rounded-control border border-line-strong bg-surface">
                <div className="flex items-center justify-between gap-2 px-2.5 py-1.5">
                  <button
                    type="button"
                    onClick={() => setColumnPickerOpen((open) => !open)}
                    aria-expanded={columnPickerOpen}
                    aria-label={t('export.columnsToggle')}
                    className="flex min-w-0 flex-1 items-center gap-1.5 text-left text-xs text-fg"
                  >
                    {columnPickerOpen ? (
                      <ChevronDown size={13} className="shrink-0 text-fg-muted" />
                    ) : (
                      <ChevronRight size={13} className="shrink-0 text-fg-muted" />
                    )}
                    <span className="truncate">
                      {activeColumns.length === baseColumns.length
                        ? t('export.columnsAll', { count: baseColumns.length })
                        : activeColumns.length === 1
                          ? t('export.columnsSingle', {
                              column: activeColumns[0] ?? '',
                              total: baseColumns.length
                            })
                          : t('export.columnsSelected', {
                              selected: activeColumns.length,
                              total: baseColumns.length
                            })}
                    </span>
                  </button>
                  {activeColumns.length < baseColumns.length && (
                    <button
                      type="button"
                      onClick={selectAllColumns}
                      className="shrink-0 rounded-control px-1.5 py-0.5 text-xs text-accent hover:bg-accent-soft"
                    >
                      {t('export.columnsSelectAll')}
                    </button>
                  )}
                </div>

                {columnPickerOpen && (
                  <div className="max-h-40 overflow-y-auto border-t border-line py-1">
                    {baseColumns.map((name, index) => {
                      const checked = selectedSet.has(index);
                      const isLast = checked && activeColumns.length <= 1;
                      return (
                        <div
                          key={`${name}-${index}`}
                          className="flex items-center justify-between gap-2 px-2.5 py-1 hover:bg-surface-hover"
                        >
                          <button
                            type="button"
                            onClick={() => toggleColumn(index)}
                            disabled={isLast}
                            title={isLast ? t('columns.lastVisible') : name}
                            className="flex min-w-0 flex-1 items-center gap-2 text-left text-xs text-fg disabled:opacity-50"
                          >
                            <span
                              className={clsx(
                                'flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded-sm border',
                                checked
                                  ? 'border-accent bg-accent-soft text-accent'
                                  : 'border-line-strong'
                              )}
                            >
                              {checked && <Check size={10} />}
                            </span>
                            <span className="truncate font-mono">{name}</span>
                          </button>
                          {!(checked && activeColumns.length === 1) && (
                            <button
                              type="button"
                              onClick={() => selectOnlyColumn(index)}
                              className="shrink-0 rounded-control px-1.5 py-0.5 text-[11px] text-fg-muted hover:bg-accent-soft hover:text-accent"
                            >
                              {t('export.columnsOnly')}
                            </button>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            </Field>
          )}

          {canDistinct && (
            <Field label={t('export.distinct')}>
              <Checkbox
                checked={distinct}
                onChange={(next) => {
                  setDistinct(next);
                  setWritten(null);
                  setError(null);
                }}
                label={t('export.distinctLabel')}
              />
            </Field>
          )}

          <Field label={t('export.format')}>
            <SegmentedControl<ExportFormat>
              value={options.format}
              options={[
                { value: 'csv', label: 'CSV' },
                { value: 'json', label: 'JSON' },
                ...(sqlDialect ? [{ value: 'sql' as const, label: t('export.format.sql') }] : [])
              ]}
              onChange={format => update({ format })}
            />
          </Field>

          {options.format === 'sql' && (
            <Field label={t('export.sqlTable')}>
              <input
                value={options.sqlTable}
                onChange={(event) => update({ sqlTable: event.target.value })}
                placeholder={t('export.sqlTablePlaceholder')}
                aria-label={t('export.sqlTable')}
                className="w-full rounded-control border border-line-strong bg-surface px-2 py-1 font-mono text-xs text-fg outline-none focus:border-accent"
                {...PLAIN_TEXT_INPUT}
              />
              <p className="mt-1 text-xs text-fg-subtle">{t('export.sqlTableHint')}</p>
            </Field>
          )}

          {options.format === 'csv' && (
            <>
              <Field label={t('export.delimiter')}>
                <SegmentedControl<CsvDelimiter>
                  value={options.delimiter}
                  options={DELIMITERS.map(({ value, labelKey }) => ({
                    value,
                    label: t(labelKey)
                  }))}
                  onChange={delimiter => update({ delimiter })}
                />
              </Field>

              <Field label={t('export.nullAs')}>
                <SegmentedControl<string>
                  value={options.nullText}
                  options={NULL_TEXTS.map(({ value, labelKey, label }) => ({
                    value,
                    label: labelKey ? t(labelKey) : label ?? value
                  }))}
                  onChange={nullText => update({ nullText })}
                />
              </Field>

              <Field label={t('export.header')}>
                <Checkbox
                  checked={options.includeHeader}
                  onChange={includeHeader => update({ includeHeader })}
                  label={t('export.header.writeNames')}
                />
              </Field>

              <Field label={t('export.encoding')}>
                <Checkbox
                  checked={options.byteOrderMark}
                  onChange={byteOrderMark => update({ byteOrderMark })}
                  label={t('export.encoding.bom')}
                  hint={t('export.encoding.bomHint')}
                />
              </Field>
            </>
          )}

          <div>
            <p className="mb-1 text-xs text-fg-subtle">
              {streaming
                ? t('export.previewStreamed', {
                    count: options.format === 'json' ? 1 : Math.min(PREVIEW_ROWS, activeRows.length)
                  })
                : options.format === 'json'
                  ? t('export.previewJson', { total: activeRows.length })
                  : options.format === 'sql'
                  ? t('export.previewSql', {
                      shown: Math.min(PREVIEW_ROWS, activeRows.length),
                      total: activeRows.length
                    })
                  : t('export.previewCsv', {
                      count: Math.min(PREVIEW_ROWS, activeRows.length),
                      total: activeRows.length
                    })}
            </p>
            <pre className="max-h-36 overflow-auto rounded-control border border-line bg-surface-sunken px-3 py-2 font-mono text-xs text-fg select-text whitespace-pre">
              {missingTable
                ? t('export.sqlTableMissing')
                : !preview
                  ? t('export.previewEmpty')
                  : options.format === 'csv'
                    ? preview
                    : <HighlightedCode code={preview} language={options.format} />}
            </pre>
          </div>
        </div>

        <div className="flex items-center gap-3 border-t border-line bg-surface-sunken px-5 py-3">
          <div className="min-w-0 flex-1 text-xs">
            {writing && <span className="text-fg-muted">{t('export.writing')}</span>}
            {!writing && error && <span className="break-words text-danger">{error}</span>}
            {!writing && !error && written && (
              <span className="break-all text-success">
                {t('export.writtenRows', {
                  count: written.rowsWritten,
                  size: formatBytes(written.bytesWritten),
                  path: written.path
                })}
              </span>
            )}
          </div>

          {/* 导出中：关闭按钮换成取消。留着「关闭」会让人以为关掉就停了，而它不停 */}
          {!writing && (
            <button
              type="button"
              ref={cancelRef}
              onClick={onClose}
              className="shrink-0 rounded-control border border-line-strong px-3 py-1.5 text-sm text-fg hover:bg-surface-hover"
            >
              {written ? t('common.done') : lastPath ? t('common.close') : t('common.cancel')}
            </button>
          )}

          {/* 失败后重试用的是同一个路径与同一份选项，不再让用户重选一次 */}
          {!writing && error && lastPath && (
            <button
              type="button"
              onClick={() => void runExport(lastPath)}
              className="shrink-0 rounded-control border border-line-strong px-3 py-1.5 text-sm text-fg hover:bg-surface-hover"
            >
              {t('export.retry')}
            </button>
          )}

          <button
            type="button"
            onClick={handleExport}
            disabled={writing || missingTable || (streaming ? !resolvedSql : activeRows.length === 0)}
            className="flex shrink-0 items-center gap-1.5 rounded-control bg-accent px-3 py-1.5 text-sm text-fg-on-accent hover:opacity-90 disabled:opacity-50"
          >
            {writing && <Loader2 size={14} className="animate-spin" />}
            {writing ? t('export.writing') : t('export.choosePath')}
          </button>
        </div>
      </div>
    </div>
  );
}

