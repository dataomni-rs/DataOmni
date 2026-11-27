import { useEffect, useState } from 'react';
import { Check } from 'lucide-react';
import { useLanguageStore } from '../stores/languageStore';
import type { SelectionSummary } from '../utils/selectionStats';

interface GridSelectionStatsBarProps {
  summary: SelectionSummary;
  onCopyValue: (value: string) => void;
}

interface StatChip {
  key: string;
  label: string;
  copyValue: string;
}

/**
 * 类 Excel 底部状态栏选区聚合：框选多格或整列时即时展示计数 / 非空 / 去重，
 * 含数值时补齐求和 / 均值 / 最小 / 最大；点击任意指标即可复制数值。
 */
export function GridSelectionStatsBar({ summary, onCopyValue }: GridSelectionStatsBarProps) {
  const t = useLanguageStore((state) => state.t);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  useEffect(() => {
    if (!copiedKey) {
      return;
    }
    const timer = window.setTimeout(() => setCopiedKey(null), 1200);
    return () => window.clearTimeout(timer);
  }, [copiedKey]);

  const chips: StatChip[] = [
    {
      key: 'cells',
      label: t('grid.stats.cells', { count: summary.cellCount }),
      copyValue: String(summary.cellCount)
    },
    {
      key: 'nonNull',
      label: t('grid.stats.nonNull', { value: summary.nonNullCount }),
      copyValue: String(summary.nonNullCount)
    },
    {
      key: 'distinct',
      label: t('grid.stats.distinct', { value: summary.distinctCount }),
      copyValue: String(summary.distinctCount)
    }
  ];

  if (summary.numericCount > 0 && summary.sum !== null && summary.avg !== null && summary.min !== null && summary.max !== null) {
    chips.push(
      { key: 'sum', label: t('grid.stats.sum', { value: summary.sum }), copyValue: summary.sum },
      { key: 'avg', label: t('grid.stats.avg', { value: summary.avg }), copyValue: summary.avg },
      { key: 'min', label: t('grid.stats.min', { value: summary.min }), copyValue: summary.min },
      { key: 'max', label: t('grid.stats.max', { value: summary.max }), copyValue: summary.max }
    );
  }

  return (
    <div
      className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 border-t border-accent-line/50 bg-accent-soft/40 px-3 py-1 text-xs text-fg"
      data-testid="grid-selection-stats"
    >
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        {chips.slice(0, 1).map((chip) => (
          <button
            key={chip.key}
            type="button"
            onClick={() => {
              onCopyValue(chip.copyValue);
              setCopiedKey(chip.key);
            }}
            title={t('grid.stats.clickToCopy', { value: chip.copyValue })}
            className="inline-flex items-center gap-1 rounded-control bg-accent-soft px-1.5 py-0.5 font-mono text-[11px] font-medium text-accent transition-colors hover:bg-surface-hover"
          >
            <span>{chip.label}</span>
            {copiedKey === chip.key && <Check size={11} className="text-success" />}
          </button>
        ))}
      </div>
      <div className="flex flex-wrap items-center justify-end gap-x-2 gap-y-1">
        {chips.slice(1).map((chip) => (
          <button
            key={chip.key}
            type="button"
            onClick={() => {
              onCopyValue(chip.copyValue);
              setCopiedKey(chip.key);
            }}
            title={t('grid.stats.clickToCopy', { value: chip.copyValue })}
            className="inline-flex items-center gap-1 rounded-control px-1.5 py-0.5 font-mono text-[11px] text-fg-muted transition-colors hover:bg-surface-hover hover:text-fg"
          >
            <span>{chip.label}</span>
            {copiedKey === chip.key && <Check size={11} className="text-success" />}
          </button>
        ))}
      </div>
    </div>
  );
}
