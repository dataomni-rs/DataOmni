import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { useLanguageStore } from '../stores/languageStore';
import {
  BREW_UPGRADE_COMMAND,
  loadUpdateCheck,
  noticeVersion,
  saveUpdateCheck,
  shouldCheckNow,
  type InstallChannel,
} from '../utils/updateCheck';

/**
 * 标签栏下面那一条「有新版本」。启动时跑一次：到了该问的时候去问 GitHub，
 * 问到的版本比自己新、用户又没关掉过这一版，就提示。没问成只进日志，不打扰。
 * Homebrew Cask 安装的副本额外提供一键复制 `brew upgrade --cask dataomni`，避免手动覆盖包管理器状态
 */
export function UpdateNotice() {
  const t = useLanguageStore((state) => state.t);
  const [version, setVersion] = useState<string | null>(null);
  const [channel, setChannel] = useState<InstallChannel>('standalone');
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      let state = loadUpdateCheck();
      const current = await getVersion();
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
        if (!cancelled) {
          setChannel(detected);
          setVersion(nextNotice);
        }
      }
    })().catch((error: unknown) => console.warn('update check failed', error));
    return () => {
      cancelled = true;
    };
  }, []);

  if (!version) {
    return null;
  }

  const dismiss = () => {
    saveUpdateCheck({ ...loadUpdateCheck(), dismissedVersion: version });
    setVersion(null);
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

  return (
    <div className="flex items-center gap-3 border-b border-line bg-accent-soft px-3 py-1.5 text-xs text-fg">
      <span className="min-w-0 flex-1">{t('update.available', { version })}</span>
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
        onClick={() => {
          invoke('open_release_page', { version }).catch((error: unknown) =>
            console.warn('open release page failed', error),
          );
        }}
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
  );
}
