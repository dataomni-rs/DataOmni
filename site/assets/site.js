(() => {
  const root = document.documentElement;
  const storageKey = 'dataomni-site-theme';

  const currentTheme = () =>
    root.getAttribute('data-theme') === 'dark' ? 'dark' : 'light';

  const syncThemeButton = () => {
    const btn = document.getElementById('theme-toggle');
    if (!btn) return;
    const isDark = currentTheme() === 'dark';
    const lightLabel = btn.getAttribute('data-label-light') || 'Light';
    const darkLabel = btn.getAttribute('data-label-dark') || 'Dark';
    btn.textContent = isDark ? lightLabel : darkLabel;
    btn.setAttribute('aria-pressed', String(isDark));
  };

  syncThemeButton();

  const themeBtn = document.getElementById('theme-toggle');
  if (themeBtn) {
    themeBtn.addEventListener('click', () => {
      const next = currentTheme() === 'dark' ? 'light' : 'dark';
      root.setAttribute('data-theme', next);
      try {
        localStorage.setItem(storageKey, next);
      } catch {
        // Ignore storage restrictions in private windows
      }
      syncThemeButton();
    });
  }

  // Workbench tabs (ARIA tablist with Left/Right keyboard support)
  const tabs = Array.from(document.querySelectorAll('[role="tab"][data-tab-target]'));
  const panels = Array.from(document.querySelectorAll('[role="tabpanel"][data-tab-panel]'));
  const captionTitle = document.getElementById('workbench-caption-title');
  const captionBody = document.getElementById('workbench-caption-body');

  const hydratePanelImage = (panel) => {
    if (!panel) return;
    const source = panel.querySelector('source[data-deferred-srcset]');
    if (source) {
      const deferredWebp = source.getAttribute('data-deferred-srcset');
      if (deferredWebp) {
        source.setAttribute('srcset', deferredWebp);
        source.removeAttribute('data-deferred-srcset');
      }
    }
    const img = panel.querySelector('img[data-deferred-src]');
    if (!img) return;
    const deferred = img.getAttribute('data-deferred-src');
    if (deferred) {
      img.setAttribute('src', deferred);
      img.removeAttribute('data-deferred-src');
    }
  };

  const activateTab = (selectedTab, moveFocus) => {
    const target = selectedTab.getAttribute('data-tab-target');
    tabs.forEach((tab) => {
      const active = tab === selectedTab;
      tab.setAttribute('aria-selected', String(active));
      tab.setAttribute('tabindex', active ? '0' : '-1');
    });
    panels.forEach((panel) => {
      const active = panel.getAttribute('data-tab-panel') === target;
      if (active) hydratePanelImage(panel);
      panel.hidden = !active;
    });
    if (captionTitle) {
      captionTitle.textContent = selectedTab.getAttribute('data-caption-title') || '';
    }
    if (captionBody) {
      captionBody.textContent = selectedTab.getAttribute('data-caption-body') || '';
    }
    if (moveFocus) {
      selectedTab.focus();
    }
  };

  tabs.forEach((tab, index) => {
    const preloadTarget = () => {
      const target = tab.getAttribute('data-tab-target');
      const panel = panels.find((item) => item.getAttribute('data-tab-panel') === target);
      hydratePanelImage(panel);
    };
    tab.addEventListener('pointerenter', preloadTarget, { once: true });
    tab.addEventListener('focus', preloadTarget, { once: true });
    tab.addEventListener('click', () => activateTab(tab, false));
    tab.addEventListener('keydown', (event) => {
      if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
      event.preventDefault();
      const delta = event.key === 'ArrowRight' ? 1 : -1;
      const nextIndex = (index + delta + tabs.length) % tabs.length;
      activateTab(tabs[nextIndex], true);
    });
  });

  // Compatibility table category filter
  const filterBtns = Array.from(document.querySelectorAll('[data-ledger-filter]'));
  const ledgerRows = Array.from(document.querySelectorAll('[data-ledger-kind]'));
  filterBtns.forEach((btn) => {
    btn.addEventListener('click', () => {
      const filter = btn.getAttribute('data-ledger-filter') || 'all';
      filterBtns.forEach((other) => {
        other.setAttribute('aria-pressed', String(other === btn));
      });
      ledgerRows.forEach((row) => {
        const kind = row.getAttribute('data-ledger-kind');
        row.hidden = filter !== 'all' && kind !== filter;
      });
    });
  });

  // Highlight visitor's platform download without hiding other platforms
  const osStorageKey = 'dataomni-site-os';
  const ua = (navigator.userAgent || '').toLowerCase();
  const detectedOs = ua.includes('mac')
    ? 'macos'
    : ua.includes('win')
      ? 'windows'
      : ua.includes('linux')
        ? 'linux'
        : '';
  if (detectedOs) {
    document.querySelectorAll(`[data-os="${detectedOs}"]`).forEach((el) => {
      el.classList.add('is-detected-os');
    });
    const primaryBtn = document.getElementById('primary-download-btn');
    const osLink = document.querySelector(`a[data-os-primary="${detectedOs}"]`);
    if (primaryBtn && osLink) {
      const href = osLink.getAttribute('href');
      const label = osLink.getAttribute('data-primary-cta');
      if (href) primaryBtn.setAttribute('href', href);
      if (label) primaryBtn.textContent = label;
    }
  }

  // Synced multi-platform code tabs & MCP preset switchers
  const codeTabGroups = Array.from(document.querySelectorAll('[data-code-tabs]'));
  const mcpSwitchers = Array.from(document.querySelectorAll('[data-mcp-switcher]'));

  const activateCodeTab = (group, tabIndex, moveFocus) => {
    const groupTabs = Array.from(group.querySelectorAll('[role="tab"][data-code-tab]'));
    const groupPanels = Array.from(group.querySelectorAll('[role="tabpanel"][data-code-panel]'));
    const copyBtn = group.querySelector('[data-code-copy-btn]');
    const targetIdx = String(tabIndex);

    groupTabs.forEach((tab) => {
      const active = tab.getAttribute('data-code-tab') === targetIdx;
      tab.setAttribute('aria-selected', String(active));
      tab.setAttribute('tabindex', active ? '0' : '-1');
      if (active) {
        if (moveFocus) tab.focus();
        tab.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      }
    });

    groupPanels.forEach((panel) => {
      const active = panel.getAttribute('data-code-panel') === targetIdx;
      if (active) {
        panel.removeAttribute('hidden');
        if (copyBtn) {
          const panelCopy = panel.getAttribute('data-panel-copy') || '';
          if (panelCopy) {
            copyBtn.hidden = false;
            copyBtn.setAttribute('data-copy-text', panelCopy);
            const defaultLabel = copyBtn.getAttribute('data-default-label');
            if (defaultLabel) copyBtn.textContent = defaultLabel;
          } else {
            copyBtn.hidden = true;
          }
        }
      } else {
        panel.setAttribute('hidden', 'until-found');
      }
    });
  };

  const renderMcpSwitcher = (switcher, nextOs, nextClient) => {
    const currentOs =
      nextOs ||
      switcher.getAttribute('data-active-os') ||
      'macos';
    const currentClient =
      nextClient ||
      switcher.getAttribute('data-active-client') ||
      'claude';

    switcher.setAttribute('data-active-os', currentOs);
    switcher.setAttribute('data-active-client', currentClient);

    switcher.querySelectorAll('[data-mcp-os-btn]').forEach((btn) => {
      btn.setAttribute('aria-pressed', String(btn.getAttribute('data-mcp-os-btn') === currentOs));
    });
    switcher.querySelectorAll('[data-mcp-preset]').forEach((btn) => {
      btn.setAttribute(
        'aria-pressed',
        String(btn.getAttribute('data-mcp-preset') === currentClient)
      );
    });

    let activeRawCmd = '';
    switcher.querySelectorAll('code[data-mcp-os][data-mcp-client]').forEach((codeEl) => {
      const match =
        codeEl.getAttribute('data-mcp-os') === currentOs &&
        codeEl.getAttribute('data-mcp-client') === currentClient;
      codeEl.hidden = !match;
      if (match) {
        activeRawCmd = codeEl.getAttribute('data-raw-cmd') || '';
      }
    });

    const copyBtn = switcher.querySelector('[data-mcp-copy-btn]');
    if (copyBtn && activeRawCmd) {
      copyBtn.setAttribute('data-copy-text', activeRawCmd);
      const defaultLabel = copyBtn.getAttribute('data-default-label');
      if (defaultLabel) copyBtn.textContent = defaultLabel;
    }

    switcher.querySelectorAll('[data-mcp-os-note]').forEach((noteEl) => {
      noteEl.hidden = noteEl.getAttribute('data-mcp-os-note') !== currentOs;
    });
  };

  const syncOsPreference = (os, persist) => {
    if (!['macos', 'windows', 'linux'].includes(os)) return;
    if (persist) {
      try {
        localStorage.setItem(osStorageKey, os);
      } catch {
        // Ignore storage restrictions in private windows
      }
    }
    codeTabGroups.forEach((group) => {
      const matchTab = Array.from(group.querySelectorAll('[role="tab"][data-code-tab]')).find(
        (tab) => (tab.getAttribute('data-os-targets') || '').split(/\s+/).includes(os)
      );
      if (matchTab) {
        activateCodeTab(group, matchTab.getAttribute('data-code-tab') || '0', false);
      }
    });
    mcpSwitchers.forEach((switcher) => {
      renderMcpSwitcher(switcher, os, null);
    });
  };

  codeTabGroups.forEach((group) => {
    const groupTabs = Array.from(group.querySelectorAll('[role="tab"][data-code-tab]'));
    const groupPanels = Array.from(group.querySelectorAll('[role="tabpanel"][data-code-panel]'));

    groupTabs.forEach((tab, index) => {
      const selectAndSync = (moveFocus) => {
        const idx = tab.getAttribute('data-code-tab') || '0';
        activateCodeTab(group, idx, moveFocus);
        const osKey = tab.getAttribute('data-os-sync') || '';
        if (osKey) syncOsPreference(osKey, true);
      };
      tab.addEventListener('click', () => selectAndSync(false));
      tab.addEventListener('keydown', (event) => {
        let nextIndex = -1;
        if (event.key === 'ArrowRight') nextIndex = (index + 1) % groupTabs.length;
        else if (event.key === 'ArrowLeft')
          nextIndex = (index - 1 + groupTabs.length) % groupTabs.length;
        else if (event.key === 'Home') nextIndex = 0;
        else if (event.key === 'End') nextIndex = groupTabs.length - 1;
        if (nextIndex < 0) return;
        event.preventDefault();
        const nextTab = groupTabs[nextIndex];
        activateCodeTab(group, nextTab.getAttribute('data-code-tab') || '0', true);
        const osKey = nextTab.getAttribute('data-os-sync') || '';
        if (osKey) syncOsPreference(osKey, true);
      });
    });

    // Sync tab state when browser find-in-page opens a hidden="until-found" panel
    groupPanels.forEach((panel) => {
      panel.addEventListener('beforematch', () => {
        activateCodeTab(group, panel.getAttribute('data-code-panel') || '0', false);
      });
    });
  });

  mcpSwitchers.forEach((switcher) => {
    switcher.querySelectorAll('[data-mcp-os-btn]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const os = btn.getAttribute('data-mcp-os-btn') || 'macos';
        syncOsPreference(os, true);
      });
    });
    switcher.querySelectorAll('[data-mcp-preset]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const client = btn.getAttribute('data-mcp-preset') || 'claude';
        renderMcpSwitcher(switcher, null, client);
      });
    });
  });

  let savedOs = '';
  try {
    savedOs = localStorage.getItem(osStorageKey) || '';
  } catch {
    savedOs = '';
  }
  const initialOs = ['macos', 'windows', 'linux'].includes(savedOs) ? savedOs : detectedOs;
  if (initialOs) {
    syncOsPreference(initialOs, false);
  }

  // Copy buttons for CLI / MCP / first-launch commands
  document.querySelectorAll('[data-copy-text]').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const text = btn.getAttribute('data-copy-text') || '';
      const copiedLabel = btn.getAttribute('data-copied-label') || 'Copied';
      const defaultLabel = btn.getAttribute('data-default-label') || btn.textContent || 'Copy';
      try {
        await navigator.clipboard.writeText(text);
        btn.textContent = copiedLabel;
        window.setTimeout(() => {
          btn.textContent = defaultLabel;
        }, 1600);
      } catch {
        // Clipboard API unavailable
      }
    });
  });

  // Documentation table-of-contents scrollspy (CSS scroll-target-group + IntersectionObserver fallback)
  const tocLinks = Array.from(document.querySelectorAll('.docs-toc-list a[href^="#"]'));
  const docSections = Array.from(document.querySelectorAll('.docs-section[id]'));
  if (tocLinks.length > 0 && docSections.length > 0) {
    if (typeof CSS !== 'undefined' && CSS.supports('scroll-target-group: auto')) {
      const syncAriaCurrent = () => {
        const current = document.querySelector('.docs-toc-list a:target-current');
        tocLinks.forEach((link) => {
          link.setAttribute('aria-current', link === current ? 'true' : 'false');
        });
      };
      syncAriaCurrent();
      document.addEventListener('scrollend', syncAriaCurrent);
    } else if ('IntersectionObserver' in window) {
      const observer = new IntersectionObserver(
        (entries) => {
          entries.forEach((entry) => {
            if (!entry.isIntersecting) return;
            const activeHref = `#${entry.target.id}`;
            tocLinks.forEach((link) => {
              const active = link.getAttribute('href') === activeHref;
              link.classList.toggle('target-current', active);
              link.setAttribute('aria-current', String(active));
            });
          });
        },
        { rootMargin: '-20% 0px -65% 0px', threshold: 0 }
      );
      docSections.forEach((section) => observer.observe(section));
    }
  }
})();
