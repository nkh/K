// ─── Utilities ───
// Pure utility functions used across all modules.
(function() {
    'use strict';

/// Format a runtime duration in seconds to a human-readable string.
/// Handles null/undefined inputs gracefully.
function formatRuntime(secs) {
    if (secs == null || secs < 0) return ''; // == null catches both null and undefined intentionally
    if (secs < 60) return Math.floor(secs) + 's';
    if (secs < 3600) return Math.floor(secs / 60) + 'm ' + Math.floor(secs % 60) + 's';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return h + 'h ' + m + 'm';
}

function getBaseUrl() {
    return state.connections.length > 0 ? state.connections[0].url : window.location.origin;
}

function authHeaders(token) {
    const t = token || state.authToken;
    const headers = { 'Content-Type': 'application/json' };
    if (t) headers['Authorization'] = 'Bearer ' + t;
    return headers;
}

function authHeadersForInstance(inst) {
    return authHeaders(inst.token || state.authToken);
}

function apiUrl(path, inst) {
    const base = inst ? inst.url : getBaseUrl();
    return base + path;
}

// ─── Utilities ───
function escHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

/// Convert a byte (0-255) to a 2-digit lowercase hex string.
function _hex(b) {
    return (b < 16 ? '0' : '') + b.toString(16);
}

// HTML-escape a character, matching the server's html_escape() function.
function _htmlEscapeChar(ch) {
    switch (ch) {
        case '&': return '&amp;';
        case '<': return '&lt;';
        case '>': return '&gt;';
        case "'": return '&#39;';
        case '"': return '&quot;';
        default: return ch;
    }
}

// Apply an incremental diff from the server directly to the DOM.
// This updates only the changed cells, avoiding a full innerHTML replacement.
//
// The diff data has the format:
//   { generation, cursor, dimensions, changed_count, cells: [...] }
// Each cell: { row, col, ch, fg: [r,g,b], bg: [r,g,b], bold, italic, ... }

// ─── Spawn argument parser ───
// Splits a string into arguments respecting quoted strings.
// Supports double-quoted and single-quoted strings.
// Examples:
//   '-c "echo hello; echo world"' -> ['-c', 'echo hello; echo world']
//   "--flag 'arg with spaces'"      -> ['--flag', 'arg with spaces']
//   'plain args'                    -> ['plain', 'args']
function parseSpawnArgs(str) {
    if (!str) return [];
    const args = [];
    let current = '';
    let inQuote = null; // '"' or "'"
    let escaped = false;
    for (let i = 0; i < str.length; i++) {
        const ch = str[i];
        if (escaped) {
            current += ch;
            escaped = false;
            continue;
        }
        if (ch === '\\') {
            escaped = true;
            continue;
        }
        if (inQuote) {
            if (ch === inQuote) {
                inQuote = null;
            } else {
                current += ch;
            }
            continue;
        }
        if (ch === '"' || ch === "'") {
            inQuote = ch;
            continue;
        }
        if (ch === ' ' || ch === '\t') {
            if (current) {
                args.push(current);
                current = '';
            }
            continue;
        }
        current += ch;
    }
    if (current) args.push(current);
    return args;
}

/// Parse the environment variables textarea into a {key: value} object.
/// Each line should be KEY=VALUE. Lines not containing '=' are skipped.
/// Whitespace around key and value is trimmed. Empty lines are ignored.
function parseSpawnEnvVars(text) {
    const env = {};
    if (!text) return env;
    const lines = text.split('\n');
    for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith('#')) continue;  // skip empty/comment lines
        const eqIdx = trimmed.indexOf('=');
        if (eqIdx < 1) continue;  // skip lines without '=' or with '=' at start
        const key = trimmed.substring(0, eqIdx).trim();
        const value = trimmed.substring(eqIdx + 1).trim();
        if (key) env[key] = value;
    }
    return env;
}


    // Expose to global scope
    window.formatRuntime = formatRuntime;
    window.getBaseUrl = getBaseUrl;
    window.authHeaders = authHeaders;
    window.authHeadersForInstance = authHeadersForInstance;
    window.apiUrl = apiUrl;
    window.escHtml = escHtml;
    window.parseSpawnArgs = parseSpawnArgs;
    window.parseSpawnEnvVars = parseSpawnEnvVars;
    window._hex = _hex;
    window._htmlEscapeChar = _htmlEscapeChar;

// ─── Focus Management ───
const _focusState = {
    previousElement: null,
    releaseFn: null,
};

function _getFocusable(container) {
    const selector = 'button, input, select, textarea, [tabindex]:not([tabindex="-1"])';
    return Array.from(container.querySelectorAll(selector))
        .filter(el => {
            if (el.offsetParent === null && el.style.position !== 'fixed') return false;
            if (el.disabled) return false;
            return true;
        });
}

function trapFocus(container) {
    _focusState.previousElement = document.activeElement;
    const handler = (e) => {
        if (e.key !== 'Tab') return;
        e.preventDefault();
        const focusable = _getFocusable(container);
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        const active = document.activeElement;
        const idx = focusable.indexOf(active);
        if (e.shiftKey) {
            if (idx <= 0) last.focus();
            else focusable[idx - 1].focus();
        } else {
            if (idx === -1 || idx >= focusable.length - 1) first.focus();
            else focusable[idx + 1].focus();
        }
    };
    document.addEventListener('keydown', handler, true);
    const releaseFn = () => {
        document.removeEventListener('keydown', handler, true);
        if (_focusState.previousElement && _focusState.previousElement.isConnected) {
            _focusState.previousElement.focus();
        }
        _focusState.previousElement = null;
        _focusState.releaseFn = null;
    };
    _focusState.releaseFn = releaseFn;
    return releaseFn;
}

function releaseCurrentFocusTrap() {
    if (_focusState.releaseFn) _focusState.releaseFn();
}
window.trapFocus = trapFocus;
window.releaseCurrentFocusTrap = releaseCurrentFocusTrap;

// ─── Theme ───
function initTheme() {
    const saved = localStorage.getItem('vrw_theme');
    if (saved) document.documentElement.setAttribute('data-theme', saved);
    updateThemeButton();
}

function toggleGlobalTheme() {
    const current = document.documentElement.getAttribute('data-theme') || '';
    const next = current === '' ? 'light' : current === 'light' ? 'grey' : current === 'grey' ? 'dark' : '';
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
        btn.textContent = prefersLight ? '\u263E' : '\u2600';
        btn.title = 'Theme: Auto (click to toggle)';
    } else if (theme === 'light') {
        btn.textContent = '\u2600';
        btn.title = 'Theme: Light (click to toggle)';
    } else if (theme === 'grey') {
        btn.textContent = '\u25FC';
        btn.title = 'Theme: Grey (click to toggle)';
    } else if (theme === 'dark') {
        btn.textContent = '\u263E';
        btn.title = 'Theme: Dark (click to toggle)';
    }
}

function togglePanelTheme(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const next = panelObj.theme === '' ? 'light' : panelObj.theme === 'light' ? 'grey' : panelObj.theme === 'grey' ? 'dark' : '';
    panelObj.theme = next;
    localStorage.setItem('vrw_panel_theme_' + panelId, next);
    applyPanelTheme(panelId, next);
    if (panelId === getActivePanelId()) {
        const btn = document.getElementById('stPanelThemeBtn');
        if (btn) {
            btn.textContent = next === 'light' ? '\u2600' : next === 'grey' ? '\u25FC' : next === 'dark' ? '\u263E' : '\u25D0';
            btn.title = next === 'light' ? 'Panel theme: light (click to toggle)' : next === 'grey' ? 'Panel theme: grey (click to toggle)' : next === 'dark' ? 'Panel theme: dark (click to toggle)' : 'Panel theme: inherit (click to toggle)';
        }
    }
}

function applyPanelTheme(panelId, theme) {
    const vttyEl = document.getElementById('vtty-' + panelId);
    if (!vttyEl) return;
    if (theme) vttyEl.setAttribute('data-panel-theme', theme);
    else vttyEl.removeAttribute('data-panel-theme');
    const btn = document.getElementById('panelThemeBtn-' + panelId);
    if (btn) {
        btn.textContent = theme === 'light' ? '\u263E' : theme === 'grey' ? '\u25FC' : theme === 'dark' ? '\u2600' : '\u25D0';
        btn.title = theme === 'light' ? 'Panel theme: light (click to toggle)' : theme === 'grey' ? 'Panel theme: grey (click to toggle)' : theme === 'dark' ? 'Panel theme: dark (click to toggle)' : 'Panel theme: inherit (click to toggle)';
    }
}
window.initTheme = initTheme;
window.toggleGlobalTheme = toggleGlobalTheme;
window.updateThemeButton = updateThemeButton;
window.togglePanelTheme = togglePanelTheme;
window.applyPanelTheme = applyPanelTheme;
})();
