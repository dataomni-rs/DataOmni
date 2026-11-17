import { useEffect, useMemo, useState } from 'react';
import { X } from 'lucide-react';
import type { ConnectionProfile } from '../contracts/connection';
import type { TranslationKey } from '../i18n/translate';
import { useLanguageStore } from '../stores/languageStore';
import { isFileDatabase } from '../utils/databaseFiles';
import {
  isSameConnectionTarget,
  type ConnectionImportSource,
  type ParsedConnectionImport,
} from '../utils/externalConnections';
import { serverAddress } from '../utils/mongoConnection';
import { serverLabel } from '../utils/serverPresets';
import { isImeKeyEvent } from '../utils/shortcuts';

const SOURCE_LABEL_KEYS: Record<ConnectionImportSource, TranslationKey> = {
  dataomni: 'connectionTransfer.source.dataomni',
  navicat: 'connectionTransfer.source.navicat',
  dbeaver: 'connectionTransfer.source.dbeaver',
};

interface ConnectionImportDialogProps {
  parsed: ParsedConnectionImport;
  existingConnections: ConnectionProfile[];
  onConfirm: (selected: ConnectionProfile[]) => Promise<void> | void;
  onClose: () => void;
}

export function ConnectionImportDialog({
  parsed,
  existingConnections,
  onConfirm,
  onClose,
}: ConnectionImportDialogProps) {
  const t = useLanguageStore((state) => state.t);
  const [submitting, setSubmitting] = useState(false);

  const items = useMemo(
    () =>
      parsed.profiles.map((profile) => ({
        profile,
        duplicate: existingConnections.some((existing) => isSameConnectionTarget(existing, profile)),
      })),
    [existingConnections, parsed.profiles],
  );

  const [selectedIds, setSelectedIds] = useState<Set<string>>(
    () => new Set(items.filter((item) => !item.duplicate).map((item) => item.profile.id)),
  );

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImeKeyEvent(event) && !submitting) {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, submitting]);

  const selectableIds = useMemo(
    () => items.filter((item) => !item.duplicate).map((item) => item.profile.id),
    [items],
  );

  const toggleId = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectedProfiles = items
    .filter((item) => !item.duplicate && selectedIds.has(item.profile.id))
    .map((item) => item.profile);

  const handleSubmit = async () => {
    if (selectedProfiles.length === 0 || submitting) return;
    setSubmitting(true);
    try {
      await onConfirm(selectedProfiles);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim"
      role="dialog"
      aria-modal="true"
      aria-labelledby="connection-import-title"
      onClick={() => !submitting && onClose()}
    >
      <div
        className="flex max-h-[82vh] w-[580px] max-w-[calc(100vw-2rem)] flex-col rounded-panel bg-surface shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-line px-5 py-3">
          <div className="flex items-center gap-2.5">
            <h2 id="connection-import-title" className="text-base font-medium text-fg">
              {t('connectionTransfer.dialogTitle')}
            </h2>
            <span className="rounded-control border border-line bg-surface-muted px-2 py-0.5 font-mono text-xs text-fg-muted">
              {t(SOURCE_LABEL_KEYS[parsed.source])}
            </span>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={submitting}
            aria-label={t('common.close')}
            className="rounded-control p-1 text-fg-subtle hover:bg-surface-hover hover:text-fg disabled:opacity-50"
          >
            <X size={16} />
          </button>
        </div>

        <div className="border-b border-line bg-surface-muted px-5 py-2.5 text-xs leading-relaxed text-fg-muted">
          {t('connectionTransfer.previewNotice')}
        </div>

        {selectableIds.length > 0 && (
          <div className="flex items-center justify-end gap-3 border-b border-line px-5 py-1.5 text-xs">
            <button
              type="button"
              onClick={() => setSelectedIds(new Set(selectableIds))}
              className="text-accent hover:underline"
            >
              {t('connectionTransfer.selectAll')}
            </button>
            <button
              type="button"
              onClick={() => setSelectedIds(new Set())}
              className="text-fg-subtle hover:text-fg"
            >
              {t('connectionTransfer.clearSelection')}
            </button>
          </div>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-3">
          {items.length === 0 ? (
            <p className="py-4 text-center text-sm text-fg-subtle">{t('connectionTransfer.empty')}</p>
          ) : (
            <ul className="divide-y divide-line">
              {items.map(({ profile, duplicate }) => {
                const fileDb = isFileDatabase(profile.db_type);
                const endpoint = fileDb
                  ? profile.database || ':memory:'
                  : [serverAddress(profile), profile.database].filter(Boolean).join('/');
                const checked = !duplicate && selectedIds.has(profile.id);
                return (
                  <li key={profile.id} className="py-2">
                    <label
                      className={`flex items-start gap-3 rounded-control px-2 py-1.5 ${
                        duplicate ? 'opacity-55' : 'cursor-pointer hover:bg-surface-hover'
                      }`}
                    >
                      <input
                        type="checkbox"
                        checked={checked}
                        disabled={duplicate || submitting}
                        onChange={() => toggleId(profile.id)}
                        className="mt-1 shrink-0"
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-1.5">
                          <span className="truncate text-sm font-medium text-fg">{profile.name}</span>
                          <span className="rounded-control border border-line px-1.5 py-0.5 text-xs text-fg-muted">
                            {serverLabel(profile)}
                          </span>
                          {profile.ssh_tunnel && (
                            <span className="rounded-control border border-line-strong px-1.5 py-0.5 font-mono text-[11px] text-fg-muted">
                              SSH
                            </span>
                          )}
                          {profile.environment === 'production' && (
                            <span className="rounded-control bg-danger-soft px-1.5 py-0.5 font-mono text-[11px] font-medium text-danger">
                              PROD
                            </span>
                          )}
                          {duplicate && (
                            <span className="rounded-control bg-surface-muted px-1.5 py-0.5 text-xs text-fg-subtle">
                              {t('connectionTransfer.alreadyExists')}
                            </span>
                          )}
                        </div>
                        <p className="mt-0.5 truncate font-mono text-xs text-fg-subtle">
                          {profile.username && !fileDb ? `${profile.username}@${endpoint}` : endpoint}
                        </p>
                      </div>
                    </label>
                  </li>
                );
              })}
            </ul>
          )}

          {parsed.unsupported.length > 0 && (
            <div className="mt-3 rounded-control border border-line bg-surface-muted px-3 py-2 text-xs text-fg-muted">
              <p className="font-medium text-fg">
                {t('connectionTransfer.unsupportedHeading', { count: parsed.unsupported.length })}
              </p>
              <ul className="mt-1 space-y-0.5 font-mono text-fg-subtle">
                {parsed.unsupported.map((entry, idx) => (
                  <li key={`${entry.name}-${idx}`}>
                    {entry.name} · {entry.driver}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 border-t border-line px-5 py-3">
          <button
            type="button"
            onClick={onClose}
            disabled={submitting}
            className="rounded-control border border-line-strong px-3 py-1.5 text-sm text-fg-muted hover:bg-surface-hover disabled:opacity-50"
          >
            {t('common.cancel')}
          </button>
          <button
            type="button"
            disabled={selectedProfiles.length === 0 || submitting}
            onClick={() => void handleSubmit()}
            className="rounded-control bg-accent px-3 py-1.5 text-sm text-fg-on-accent hover:bg-accent-hover disabled:opacity-50"
          >
            {t('connectionTransfer.importSelected', { count: selectedProfiles.length })}
          </button>
        </div>
      </div>
    </div>
  );
}
