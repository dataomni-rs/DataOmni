/**
 * 新版本检查与渠道分流状态（见 `rfcs/auto-update.md`）：
 * - 启动时最多每 24 小时查询一次最新版本（可在「设置 → 关于与诊断」关闭，关闭时零联网请求）；
 * - Homebrew Cask 引导 `brew upgrade --cask dataomni`，Linux 系统包跳转发布页，
 *   独立安装包经用户三步显式确认与 Ed25519 验签后执行应用内更新（绝不后台静默下载或强制重启）。
 * - 查不到（离线、被墙、限流）也记下检查时刻，当天不再重复请求
 */

export type InstallChannel = 'standalone' | 'homebrew' | 'systemPackage';

export const BREW_UPGRADE_COMMAND = 'brew upgrade --cask dataomni';

export interface AvailableAppUpdate {
  version: string;
  currentVersion: string;
  notes: string | null;
  pubDate: string | null;
}

export interface UpdateDownloadProgress {
  downloadedBytes: number;
  totalBytes: number | null;
}

export interface VerifiedAppUpdate {
  version: string;
  bytesLen: number;
}

export interface UpdateCheckState {
  enabled: boolean;
  /** 上次去问的时刻（毫秒）。没问成也记 */
  lastCheckedAt: number | null;
  /** 上次问到的最新版本号，不带 `v` */
  latestVersion: string | null;
  /** 用户在提示条上点过关闭的那一版 */
  dismissedVersion: string | null;
}

export const DEFAULT_UPDATE_CHECK: UpdateCheckState = {
  enabled: true,
  lastCheckedAt: null,
  latestVersion: null,
  dismissedVersion: null
};

export const UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

const STORAGE_KEY = 'dataomni.update-check';

const SEMVER = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/;

function comparePrerelease(a: string | undefined, b: string | undefined): number {
  // 没有预发布标记的比有的新：1.0.0 > 1.0.0-rc.1
  if (a === undefined || b === undefined) {
    return a === b ? 0 : a === undefined ? 1 : -1;
  }
  const left = a.split('.');
  const right = b.split('.');
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const x = left[index];
    const y = right[index];
    if (x === undefined || y === undefined) {
      return x === undefined ? -1 : 1;
    }
    const numeric = /^\d+$/.test(x) && /^\d+$/.test(y);
    const order = numeric ? Number(x) - Number(y) : x.localeCompare(y);
    if (order !== 0) {
      return order;
    }
  }
  return 0;
}

/** `candidate` 比 `current` 新。任何一边认不出就是 false：宁可不提示，也不乱提示 */
export function isNewerVersion(candidate: string, current: string): boolean {
  const a = SEMVER.exec(candidate);
  const b = SEMVER.exec(current);
  if (!a || !b) {
    return false;
  }
  for (let index = 1; index <= 3; index += 1) {
    const order = Number(a[index]) - Number(b[index]);
    if (order !== 0) {
      return order > 0;
    }
  }
  return comparePrerelease(a[4], b[4]) > 0;
}

export function shouldCheckNow(state: UpdateCheckState, now: number): boolean {
  if (!state.enabled) {
    return false;
  }
  if (state.lastCheckedAt === null || state.lastCheckedAt > now) {
    return true;
  }
  return now - state.lastCheckedAt >= UPDATE_CHECK_INTERVAL_MS;
}

/** 该提示的版本号；不该提示时是 null */
export function noticeVersion(state: UpdateCheckState, currentVersion: string): string | null {
  const latest = state.latestVersion;
  if (!state.enabled || latest === null || latest === state.dismissedVersion) {
    return null;
  }
  return isNewerVersion(latest, currentVersion) ? latest : null;
}

export function loadUpdateCheck(): UpdateCheckState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const value = raw ? (JSON.parse(raw) as Record<string, unknown>) : null;
    if (typeof value !== 'object' || value === null) {
      return DEFAULT_UPDATE_CHECK;
    }
    const text = (field: unknown) => (typeof field === 'string' ? field : null);
    return {
      enabled: typeof value.enabled === 'boolean' ? value.enabled : DEFAULT_UPDATE_CHECK.enabled,
      lastCheckedAt: typeof value.lastCheckedAt === 'number' ? value.lastCheckedAt : null,
      latestVersion: text(value.latestVersion),
      dismissedVersion: text(value.dismissedVersion)
    };
  } catch {
    return DEFAULT_UPDATE_CHECK;
  }
}

export function saveUpdateCheck(state: UpdateCheckState): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // 存不下就是明天再多问一次，不影响别的
  }
}
