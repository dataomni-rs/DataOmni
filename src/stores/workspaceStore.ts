import { create } from 'zustand';
import {
  createTableWorkspaceTab,
  markWorkspaceTabProfileDeleted,
  renameProfileInTabs,
  workspaceTabId,
  type ClosedWorkspaceTab,
  type WorkspaceTab
} from '../contracts/workspace';
import { tabTitleFromSqlPath, type SqlFileLink } from '../utils/sqlFile';

/** 保留多少个最近关闭的标签。超出的最旧的一个被挤掉 */
export const CLOSED_TAB_LIMIT = 10;

export interface PendingTableJump {
  /** 单调递增令牌，确保连续跳到同一张表（含自引用外键）也能触发筛选更新 */
  token: number;
  filters: Array<{ column: string; value: string }>;
}

let tableJumpTokenSeq = 0;

interface WorkspaceState {
  sidebarProfileId: string | null;
  tabs: WorkspaceTab[];
  activeTabId: string | null;
  /** 最近关闭且要求保留的标签，最新的在前 */
  closedTabs: ClosedWorkspaceTab[];
  /** 仅存内存、不落盘的表数据页待执行跳转筛选（按 table-data tabId 索引） */
  pendingTableJumps: Record<string, PendingTableJump>;
}

interface WorkspaceActions {
  selectSidebarProfile: (profileId: string | null) => void;
  restoreTabs: (
    tabs: WorkspaceTab[],
    activeTabId: string | null,
    closedTabs?: ClosedWorkspaceTab[]
  ) => void;
  toggleTabPinned: (tabId: string) => void;
  /** 关闭标签但保留它，之后可以重新打开 */
  retainClosedTab: (tab: WorkspaceTab, draft: string) => void;
  /** 取回最近关闭的标签；没有可取回的返回 null */
  reopenLastClosedTab: () => ClosedWorkspaceTab | null;
  registerTab: (tab: WorkspaceTab) => void;
  /** 打开（或切换到）目标表的数据页并预填等值筛选条件 */
  jumpToTableData: (
    profileId: string,
    object: { schema: string | null; table: string },
    filters: Array<{ column: string; value: string }>
  ) => void;
  /** 取出并消费某个表数据页的待执行跳转筛选 */
  consumePendingTableJump: (tabId: string) => PendingTableJump | null;
  activateTab: (tabId: string) => void;
  closeTab: (tabId: string) => void;
  /** SQL 标签存到了某个文件：记下它，标题换成文件名 */
  linkSqlTabFile: (tabId: string, file: SqlFileLink) => void;
  handleProfileDeleted: (profileId: string, tabIdsWithDrafts?: ReadonlySet<string>) => void;
  /** 连接改名：标题里带连接名的标签（含最近关闭的）跟着换 */
  handleProfileRenamed: (profileId: string, name: string) => void;
}

export type WorkspaceStore = WorkspaceState & WorkspaceActions;

export const useWorkspaceStore = create<WorkspaceStore>((set, get) => ({
  sidebarProfileId: null,
  tabs: [],
  activeTabId: null,
  closedTabs: [],
  pendingTableJumps: {},

  selectSidebarProfile: (profileId) => {
    set({ sidebarProfileId: profileId });
  },

  restoreTabs: (tabs, activeTabId, closedTabs = []) => {
    set({
      tabs,
      activeTabId: activeTabId && tabs.some((tab) => tab.id === activeTabId)
        ? activeTabId
        : tabs[0]?.id ?? null,
      closedTabs
    });
  },

  toggleTabPinned: (tabId) => {
    set((state) => ({
      tabs: state.tabs.map((tab) => (
        tab.id === tabId ? { ...tab, pinned: !tab.pinned } : tab
      ))
    }));
  },

  retainClosedTab: (tab, draft) => {
    set((state) => ({
      closedTabs: [
        { tab, draft, closedAt: new Date().toISOString() },
        // 同一个标签重复关闭时只留最新的一条
        ...state.closedTabs.filter((closed) => closed.tab.id !== tab.id)
      ].slice(0, CLOSED_TAB_LIMIT)
    }));
  },

  reopenLastClosedTab: () => {
    const [mostRecent] = get().closedTabs;
    if (!mostRecent) {
      return null;
    }

    set((state) => ({
      closedTabs: state.closedTabs.filter((closed) => closed.tab.id !== mostRecent.tab.id),
      tabs: state.tabs.some((tab) => tab.id === mostRecent.tab.id)
        ? state.tabs
        : [...state.tabs, mostRecent.tab],
      activeTabId: mostRecent.tab.id
    }));

    return mostRecent;
  },

  registerTab: (tab) => {
    set((state) => {
      const existingTab = state.tabs.find((candidate) => candidate.id === tab.id);

      return {
        tabs: existingTab ? state.tabs : [...state.tabs, tab],
        activeTabId: tab.id
      };
    });
  },

  jumpToTableData: (profileId, object, filters) => {
    const normalized = { schema: object.schema ?? null, table: object.table };
    const tabId = workspaceTabId(profileId, 'table-data', normalized);
    const qualified = normalized.schema ? `${normalized.schema}.${normalized.table}` : normalized.table;
    tableJumpTokenSeq += 1;
    const token = tableJumpTokenSeq;

    set((state) => {
      const existingTab = state.tabs.find((candidate) => candidate.id === tabId);
      const nextTabs = existingTab
        ? state.tabs
        : [
            ...state.tabs,
            createTableWorkspaceTab(profileId, normalized.table, {
              id: tabId,
              kind: 'table-data',
              schema: normalized.schema,
              title: qualified
            })
          ];
      return {
        tabs: nextTabs,
        activeTabId: tabId,
        pendingTableJumps: {
          ...state.pendingTableJumps,
          [tabId]: { token, filters }
        }
      };
    });
  },

  consumePendingTableJump: (tabId) => {
    const pending = get().pendingTableJumps[tabId] ?? null;
    if (!pending) {
      return null;
    }
    set((state) => {
      const next = { ...state.pendingTableJumps };
      delete next[tabId];
      return { pendingTableJumps: next };
    });
    return pending;
  },

  linkSqlTabFile: (tabId, file) => {
    set((state) => ({
      tabs: state.tabs.map((tab) => tab.id === tabId && tab.kind === 'sql'
        ? { ...tab, file, title: tabTitleFromSqlPath(file.path), titleKey: undefined, titleParams: undefined, dirty: false }
        : tab)
    }));
  },

  handleProfileRenamed: (profileId, name) => {
    set((state) => {
      const tabs = renameProfileInTabs(state.tabs, profileId, name);
      const renamedClosed = renameProfileInTabs(state.closedTabs.map((closed) => closed.tab), profileId, name);
      return {
        tabs,
        closedTabs: state.closedTabs.map((closed, index) => (
          renamedClosed[index] === closed.tab ? closed : { ...closed, tab: renamedClosed[index] }
        ))
      };
    });
  },

  activateTab: (tabId) => {
    set((state) => ({
      activeTabId: state.tabs.some((tab) => tab.id === tabId)
        ? tabId
        : state.activeTabId
    }));
  },

  closeTab: (tabId) => {
    set((state) => {
      const tabIndex = state.tabs.findIndex((tab) => tab.id === tabId);
      if (tabIndex === -1) {
        return state;
      }

      const tabs = state.tabs.filter((tab) => tab.id !== tabId);
      let activeTabId = state.activeTabId;

      if (activeTabId === tabId) {
        activeTabId = tabs[Math.min(tabIndex, tabs.length - 1)]?.id ?? null;
      }

      const nextPending = { ...state.pendingTableJumps };
      delete nextPending[tabId];
      return { tabs, activeTabId, pendingTableJumps: nextPending };
    });
  },

  /**
   * 连接被删除时清理它的标签，但保留还有内容的 SQL 草稿供离线查看。
   *
   * 「有没有内容」由调用方以 `tabIdsWithDrafts` 传入：草稿正文住在
   * queryStore 的文档里，`tab.dirty` 只在创建标签时按初始 SQL 设过一次、
   * 之后从不更新，单看它会把带草稿的标签一起删掉。
   */
  handleProfileDeleted: (profileId, tabIdsWithDrafts) => {
    set((state) => {
      const tabs = state.tabs.flatMap((tab) => {
        if (tab.binding.profileId !== profileId) {
          return [tab];
        }

        if (tab.kind === 'sql' && (tab.dirty || tabIdsWithDrafts?.has(tab.id))) {
          return [markWorkspaceTabProfileDeleted(tab)];
        }

        return [];
      });
      const activeTabId = state.activeTabId
        && tabs.some((tab) => tab.id === state.activeTabId)
        ? state.activeTabId
        : tabs[0]?.id ?? null;

      const remainingPending: Record<string, PendingTableJump> = {};
      const prefix = `${profileId}:`;
      for (const [key, value] of Object.entries(state.pendingTableJumps)) {
        if (!key.startsWith(prefix)) {
          remainingPending[key] = value;
        }
      }

      return {
        sidebarProfileId: state.sidebarProfileId === profileId
          ? null
          : state.sidebarProfileId,
        tabs,
        activeTabId,
        pendingTableJumps: remainingPending
      };
    });
  }
}));
