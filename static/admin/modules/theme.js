// ─── Theme ───
(function() {
    'use strict';
// ─── Theme ───
// Global theme cycles: '' (Auto/OS) → 'grey' → 'dark' → ''.
// Theme is a client-side preference stored in localStorage — it takes effect
// immediately without requiring a running server.
function initTheme() {
    const saved = localStorage.getItem('vrw_theme');
    if (saved) {
        document.documentElement.setAttribute('data-theme', saved);
    }
    updateThemeButton();
}

function toggleGlobalTheme() {
    const current = document.documentElement.getAttribute('data-theme') || '';
    // Cycle: '' → 'grey' → 'dark' → ''
    const next = current === '' ? 'grey' : current === 'grey' ? 'dark' : '';
    if (next) {
        document.documentElement.setAttribute('data-theme', next);
        localStorage.setItem('vrw_theme', next);
    } else {
        document.documentElement.removeAttribute('data-theme');
        localStorage.removeItem('vrw_theme');
    }
    updateThemeButton();
}

function updateThemeButton() {
    const btn = document.getElementById('themeToggle');
    if (!btn) return;
    const theme = document.documentElement.getAttribute('data-theme') || '';
    if (!theme) {
        const prefersLight = window.matchMedia('(prefers-color-scheme: light)').matches;
        btn.textContent = prefersLight ? '☾' : '☀';
        btn.title = 'Theme: Auto (click to toggle)';
    } else if (theme === 'grey') {
        btn.textContent = '◼';
        btn.title = 'Theme: Grey (click to toggle)';
    } else if (theme === 'dark') {
        btn.textContent = '☀';
        btn.title = 'Theme: Dark (click to toggle)';
    }
}

function togglePanelTheme(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const next = panelObj.theme === '' ? 'light' : panelObj.theme === 'light' ? 'dark' : '';
    panelObj.theme = next;
    localStorage.setItem('vrw_panel_theme_' + panelId, next);
    applyPanelTheme(panelId, next);
    // Update shared toolbar button if this is the active panel
    if (panelId === getActivePanelId()) {
        const btn = document.getElementById('stPanelThemeBtn');
        if (btn) {
            btn.textContent = next === 'light' ? '\u263E' : next === 'dark' ? '\u2600' : '\u25D0';
            btn.title = next === 'light' ? 'Panel theme: light (click to toggle)' : next === 'dark' ? 'Panel theme: dark (click to toggle)' : 'Panel theme: inherit (click to toggle)';
        }
    }
}

function applyPanelTheme(panelId, theme) {
    const vttyEl = document.getElementById('vtty-' + panelId);
    if (!vttyEl) return;
    if (theme) {
        vttyEl.setAttribute('data-panel-theme', theme);
    } else {
        vttyEl.removeAttribute('data-panel-theme');
    }
    // Update the button label
    const btn = document.getElementById('panelThemeBtn-' + panelId);
    if (btn) {
        btn.textContent = theme === 'light' ? '\u263E' : theme === 'dark' ? '\u2600' : '\u25D0';
        btn.title = theme === 'light' ? 'Panel theme: light (click to toggle)' : theme === 'dark' ? 'Panel theme: dark (click to toggle)' : 'Panel theme: inherit (click to toggle)';
    }
}


    window.initTheme = initTheme;
    window.toggleGlobalTheme = toggleGlobalTheme;
    window.updateThemeButton = updateThemeButton;
    window.togglePanelTheme = togglePanelTheme;
    window.applyPanelTheme = applyPanelTheme;
})();
