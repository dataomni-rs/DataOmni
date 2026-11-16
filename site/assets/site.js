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

  // Copy buttons for CLI / first-launch commands
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
})();
