import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { useLanguageStore } from '../stores/languageStore';
import { describeError } from '../utils/describeError';
import { formatBytes } from '../utils/formatBytes';
import { isImeKeyEvent } from '../utils/shortcuts';
import {
  BREW_UPGRADE_COMMAND,
  isNewerVersion,
  loadUpdateCheck,
  noticeVersion,
  saveUpdateCheck,
  shouldCheckNow,
  type AvailableAppUpdate,
  type InstallChannel,
  type UpdateDownloadProgress,
  type VerifiedAppUpdate,
} from '../utils/updateCheck';

type UpgradePhase = 'confirm' | 'downloading' | 'verified' | 'installing';

export interface UpdateNoticeProps {
  /**
   * 真正安装并重启前跑一遍退出前置守卫（询问未提交事务、确认运行中导入导出、
   * 冲刷工作区快照、关闭数据库连接池与 SSH 隧道）。返回 `false` 表示用户选择留下。
   */
  onBeforeRestart?: () => Promise<boolean>;
}

/**
 * 标签栏下方的轻量版本提示与三步显式升级流程（见 `rfcs/auto-update.md`）：
 * 1. 提示条告知新版本号：Homebrew Cask 给一键复制 `brew upgrade --cask dataomni`，
 *    独立安装包且远端有匹配当前平台签名包时给「立即升级」按钮；
 * 2. 点击「立即升级」打开确认对话框，由用户显式发起下载并在后端完成 Ed25519 签名校验；
 * 3. 验签通过后由用户显式点击「立即重启完成更新」（先走事务与任务退出守卫，绝不静默重启）。
 */
export function UpdateNotice({ onBeforeRestart }: UpdateNoticeProps = {}) {
  const t = useLanguageStore((state) => state.t);
  const [currentVersion, setCurrentVersion] = useState('');
  const [version, setVersion] = useState<string | null>(null);
  const [channel, setChannel] = useState<InstallChannel>('standalone');
  const [inAppUpdate, setInAppUpdate] = useState<AvailableAppUpdate | null>(null);
  const [copied, setCopied] = useState(false);

  const [dialogOpen, setDialogOpen] = useState(false);
  const [phase, setPhase] = useState<UpgradePhase>('confirm');
  const [progress, setProgress] = useState<UpdateDownloadProgress | null>(null);
  const [upgradeError, setUpgradeError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      let state = loadUpdateCheck();
      const current = await getVersion();
      if (!cancelled) {
        setCurrentVersion(current);
      }
      if (shouldCheckNow(state, Date.now())) {
        let latestVersion = state.latestVersion;
        try {
          latestVersion = await invoke<string>('latest_release');
        } catch (error) {
          console.warn('update check failed', error);
        }
        state = { ...loadUpdateCheck(), lastCheckedAt: Date.now(), latestVersion };
        saveUpdateCheck(state);
      }
      const nextNotice = noticeVersion(state, current);
      if (nextNotice && !cancelled) {
        let detected: InstallChannel = 'standalone';
        try {
          const raw = await invoke<string>('install_channel');
          if (raw === 'homebrew' || raw === 'systemPackage' || raw === 'standalone') {
            detected = raw;
          }
        } catch {
          detected = 'standalone';
        }

        let signedUpdate: AvailableAppUpdate | null = null;
        if (detected === 'standalone') {
          try {
            const candidate = await invoke<AvailableAppUpdate | null>('check_app_update');
            if (
              candidate &&
              isNewerVersion(candidate.version, current) &&
              candidate.version !== state.dismissedVersion
            ) {
              signedUpdate = candidate;
            }
          } catch (error) {
            console.warn('in-app update manifest check failed', error);
          }
        }

        if (!cancelled) {
          setChannel(detected);
          setInAppUpdate(signedUpdate);
          setVersion(signedUpdate ? signedUpdate.version : nextNotice);
        }
      }
    })().catch((error: unknown) => console.warn('update check failed', error));
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!dialogOpen) {
      return;
    }
    let unlisten: (() => void) | undefined;
    let active = true;
    void listen<UpdateDownloadProgress>('app-update-progress', (event) => {
      if (active) {
        setProgress(event.payload);
      }
    })
      .then((fn) => {
        if (active) {
          unlisten = fn;
        } else {
          fn();
        }
      })
      .catch(() => {});
    return () => {
      active = false;
      unlisten?.();
    };
  }, [dialogOpen]);

  useEffect(() => {
    if (!dialogOpen || phase === 'downloading' || phase === 'installing') {
      return;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isImeKeyEvent(event)) {
        event.preventDefault();
        setDialogOpen(false);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [dialogOpen, phase]);

  if (!version) {
    return null;
  }

  const dismiss = () => {
    saveUpdateCheck({ ...loadUpdateCheck(), dismissedVersion: version });
    setDialogOpen(false);
    setVersion(null);
  };

  const openReleaseNotes = () => {
    invoke('open_release_page', { version }).catch((error: unknown) =>
      console.warn('open release page failed', error),
    );
  };

  const copyBrewCommand = async () => {
    try {
      await navigator.clipboard.writeText(BREW_UPGRADE_COMMAND);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch (error) {
      console.warn('copy brew upgrade command failed', error);
    }
  };

  const startDownload = async () => {
    setUpgradeError(null);
    setProgress(null);
    setPhase('downloading');
    try {
      await invoke<VerifiedAppUpdate>('download_app_update');
      setPhase('verified');
    } catch (caught) {
      setUpgradeError(describeError(caught));
      setPhase('confirm');
    }
  };

  const confirmInstallAndRestart = async () => {
    setUpgradeError(null);
    setPhase('installing');
    try {
      if (onBeforeRestart) {
        const allowed = await onBeforeRestart();
        if (!allowed) {
          setPhase('verified');
          return;
        }
      }
      await invoke('install_app_update');
    } catch (caught) {
      setUpgradeError(describeError(caught));
      setPhase('verified');
    }
  };

  const busy = phase === 'downloading' || phase === 'installing';
  const percent =
    progress && progress.totalBytes && progress.totalBytes > 0
      ? Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100))
      : null;

  return (
    <>
      <div className="flex items-center gap-3 border-b border-line bg-accent-soft px-3 py-1.5 text-xs text-fg">
        <span className="min-w-0 flex-1">{t('update.available', { version })}</span>
        {channel === 'standalone' && inAppUpdate && (
          <button
            type="button"
            onClick={() => {
              setUpgradeError(null);
              setDialogOpen(true);
            }}
            className="shrink-0 font-medium text-accent hover:underline"
          >
            {t('update.upgrade')}
          </button>
        )}
        {channel === 'homebrew' && (
          <button
            type="button"
            onClick={() => void copyBrewCommand()}
            title={BREW_UPGRADE_COMMAND}
            className="shrink-0 font-mono font-medium text-accent hover:underline"
          >
            {copied ? t('update.copiedBrewUpgrade') : t('update.copyBrewUpgrade')}
          </button>
        )}
        <button
          type="button"
          onClick={openReleaseNotes}
          className="shrink-0 font-medium text-accent hover:underline"
        >
          {t('update.view')}
        </button>
        <button
          type="button"
          onClick={dismiss}
          aria-label={t('common.close')}
          className="shrink-0 text-fg-subtle hover:text-fg"
        >
          ✕
        </button>
      </div>

      {dialogOpen && inAppUpdate && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-scrim p-4"
          onMouseDown={(event) => {
            if (!busy && event.target === event.currentTarget) {
              setDialogOpen(false);
            }
          }}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="app-update-dialog-title"
            className="w-[460px] max-w-[calc(100vw-2rem)] overflow-hidden rounded-panel border border-line bg-surface shadow-xl"
          >
            <div className="p-5">
              <h2 id="app-update-dialog-title" className="text-base font-medium text-fg">
                {t('update.dialogTitle', { version })}
              </h2>
              <p className="mt-1 font-mono text-xs text-fg-muted">
                {t('update.versionTransition', {
                  current: inAppUpdate.currentVersion || currentVersion,
                  version,
                })}
              </p>

              {inAppUpdate.notes && (
                <div className="mt-3 max-h-40 overflow-y-auto whitespace-pre-wrap rounded-control border border-line bg-surface-sunken px-3 py-2 text-xs leading-relaxed text-fg">
                  {inAppUpdate.notes}
                </div>
              )}

              <p className="mt-3 text-xs leading-relaxed text-fg-muted">{t('update.safetyNote')}</p>

              {phase === 'downloading' && (
                <div className="mt-3">
                  <div className="flex items-center justify-between text-xs text-fg-muted">
                    <span>
                      {progress
                        ? progress.totalBytes
                          ? t('update.downloadProgress', {
                              downloaded: formatBytes(progress.downloadedBytes),
                              total: formatBytes(progress.totalBytes),
                            })
                          : t('update.downloadProgressUnknown', {
                              downloaded: formatBytes(progress.downloadedBytes),
                            })
                        : t('update.downloading')}
                    </span>
                    {percent !== null && <span className="font-mono">{percent}%</span>}
                  </div>
                  {percent !== null && (
                    <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-surface-sunken">
                      <div
                        className="h-full bg-accent transition-all"
                        style={{ width: `${percent}%` }}
                      />
                    </div>
                  )}
                </div>
              )}

              {(phase === 'verified' || phase === 'installing') && (
                <p className="mt-3 rounded-control border border-line bg-accent-soft px-3 py-2 text-xs text-fg">
                  {t('update.verifiedReady')}
                </p>
              )}

              {upgradeError && (
                <p role="alert" className="mt-3 break-words text-xs text-danger">
                  {upgradeError}
                </p>
              )}
            </div>

            <div className="flex flex-wrap items-center justify-end gap-2 border-t border-line bg-surface-sunken px-5 py-3">
              <button
                type="button"
                onClick={openReleaseNotes}
                disabled={busy}
                className="mr-auto text-xs font-medium text-accent hover:underline disabled:opacity-50"
              >
                {t('update.view')}
              </button>

              {phase === 'confirm' || phase === 'downloading' ? (
                <>
                  <button
                    type="button"
                    onClick={() => setDialogOpen(false)}
                    disabled={busy}
                    className="rounded-control border border-line-strong bg-surface px-3 py-1.5 text-xs text-fg hover:bg-surface-hover disabled:opacity-50"
                  >
                    {t('common.cancel')}
                  </button>
                  <button
                    type="button"
                    onClick={() => void startDownload()}
                    disabled={busy}
                    className="rounded-control bg-accent px-3 py-1.5 text-xs font-medium text-fg-on-accent hover:bg-accent-hover disabled:opacity-50"
                  >
                    {phase === 'downloading'
                      ? t('update.downloading')
                      : upgradeError
                        ? t('update.retry')
                        : t('update.downloadAndVerify')}
                  </button>
                </>
              ) : (
                <>
                  <button
                    type="button"
                    onClick={() => setDialogOpen(false)}
                    disabled={busy}
                    className="rounded-control border border-line-strong bg-surface px-3 py-1.5 text-xs text-fg hover:bg-surface-hover disabled:opacity-50"
                  >
                    {t('update.later')}
                  </button>
                  <button
                    type="button"
                    onClick={() => void confirmInstallAndRestart()}
                    disabled={busy}
                    className="rounded-control bg-accent px-3 py-1.5 text-xs font-medium text-fg-on-accent hover:bg-accent-hover disabled:opacity-50"
                  >
                    {phase === 'installing'
                      ? t('update.installing')
                      : t('update.installAndRestart')}
                  </button>
                </>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
