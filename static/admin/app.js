// ─── State ───
// Fingerprint of last command list state to avoid redundant DOM updates
let _lastCommandState = '';
// Flat list of visible commands for prev/next navigation.
// Each entry: { instUrl, cmdId, name }
let _navCommands = [];
// Whether the welcome panel is currently displayed
let _showingWelcome = true;
// Sidebar sort: 'name' sorts all commands alphabetically, 'instance' groups by instance.
// Clicking an instance header sets it to that instance URL.
let _sidebarSort = 'name';

const state = {
    panels: [],
    // Store instUrl and cmdId separately to avoid ':' conflicts in URLs.
    selectedInstUrl: null,
    selectedCmdId: null,
    authToken: localStorage.getItem('vrw_auth_token') || '',
    refreshInterval: null,
    fontSize: parseInt(localStorage.getItem('vrw_font_size') || '10'),
    instanceUrls: [],
    currentView: 'vtty',
    // WebSocket for real-time VTTY streaming
    vttyWs: null,
    vttyWsUrl: null,
    vttyWsCmdId: null,
    // Buffer view: 'current', 'main', 'alt'
    bufferView: 'current',
    // Debounce timer for throttled HTTP VTTY fetches.
    _vttyHttpTimer: null,
    // Last-known buffer generation per command ID. Used to skip redundant
    // DOM updates when the server reports no change (Level 2 optimization).
    _lastGeneration: {},
    // Whether the user is currently viewing the live buffer (not scrolled
    // into scrollback history). Used for auto-scroll decisions.
    _userAtBottom: true,
    // Whether the user is actively scrolling the terminal container.
    // DOM updates are paused while this is true, then flushed on scroll-end.
    _userScrolling: false,
    _userScrollTimer: null,
    // Buffered VTTY update received while terminal was not visible or while
    // the user was scrolling.  Stored so it can be applied once conditions
    // allow (terminal visible, user stopped scrolling).
    _pendingVttyData: null,
    _pendingVttyDirty: false,
    // Level 3: Cell grid for incremental DOM patching.
    // Maps cmdId → { grid: [[span, ...], ...], rows: number, cols: number }
    // Built after each full HTML replacement; used by applyVttyDiff.
    _cellGrids: {},
    // Cached DOM: maps cmdId → DocumentFragment holding the detached <pre> children.
    // On switch-away, the <pre> subtree is moved into this cache so it can be
    // re-attached instantly on switch-back without a full HTML fetch.
    _cachedDomPre: {},
    // Cached scroll position per command, restored on switch-back.
    _cachedScrollPos: {},
    // Level 3: Flag indicating the client supports incremental diff.
    // Sent to server on WS connect so server knows to use vtty_diff.
    _level3Enabled: true,
    // VTTY update mode: 'push' (server sends dirty signals via WS)
    // or 'poll' (client polls /api/commands/:id/vtty/changed)
    updateMode: localStorage.getItem('vrw_update_mode') || 'push',
    pollInterval: parseInt(localStorage.getItem('vrw_poll_interval') || '500'),
    _pollTimer: null,
    // Client-side refresh throttle (ms).  In push mode, this throttles how
    // often VTTY updates are applied to the DOM even if the server sends them
    // faster.  0 = no throttle (apply immediately).  Range: 0–2000 in 100ms
    // steps.
    refreshMs: parseInt(localStorage.getItem('vrw_refresh_ms') || '0'),
    _refreshThrottleTimer: null,
    // Server-configured defaults (fetched from /api/info)
    serverUpdateMode: null,
    serverPollMs: null,
    serverDirtyMs: null,
    // Server-configured screenshot defaults
    serverScreenshotFontSize: 12,
    serverScreenshotFontName: 'monospace',
    // WebSocket for real-time log streaming
    logWs: null,
    logWsReconnectTimer: null,
    _logWsReconnectAttempts: 0,
    _logSearchReconnectTimer: null,
    // WebSocket connection quality tracking
    _wsLatency: 0,
    _wsPingInterval: null,
    _wsReconnectCount: 0,
    _wsPingSendTime: 0,
    // Resource usage cache: { cmdId: { cpu, memory_mb } }
    _resourceCache: {},
    _resourceInterval: null,
    // Whether to show CPU/memory resource badges in panel headers
    showResources: localStorage.getItem('vrw_show_resources') === 'true',
    // Sound notifications
    soundEnabled: localStorage.getItem('vrw_sound') !== 'false',
    // Whether the primary instance is reachable (fetched from /api/info)
    serverReachable: false,
};

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

// ─── Focus Management ───
const _focusState = {
    previousElement: null,
    releaseFn: null,
};

/**
 * Find all focusable elements inside a container.
 * @param {HTMLElement} container
 * @returns {HTMLElement[]}
 */
function _getFocusable(container) {
    const selector = 'button, input, select, textarea, [tabindex]:not([tabindex="-1"])';
    return Array.from(container.querySelectorAll(selector))
        .filter(el => {
            // Exclude hidden/disabled elements
            if (el.offsetParent === null && el.style.position !== 'fixed') return false;
            if (el.disabled) return false;
            return true;
        });
}

/**
 * Trap Tab/Shift+Tab focus within a container element.
 * Returns a cleanup function that removes the handler and restores focus.
 * @param {HTMLElement} container
 * @returns {Function} releaseFocus()
 */
function trapFocus(container) {
    // Save the currently focused element so we can restore it later
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
            // Shift+Tab: go backwards, wrap from first to last
            if (idx <= 0) {
                last.focus();
            } else {
                focusable[idx - 1].focus();
            }
        } else {
            // Tab: go forwards, wrap from last to first
            if (idx === -1 || idx >= focusable.length - 1) {
                first.focus();
            } else {
                focusable[idx + 1].focus();
            }
        }
    };

    document.addEventListener('keydown', handler, true);

    const releaseFn = () => {
        document.removeEventListener('keydown', handler, true);
        // Restore focus to previously focused element if it's still in the DOM
        if (_focusState.previousElement && _focusState.previousElement.isConnected) {
            _focusState.previousElement.focus();
        }
        _focusState.previousElement = null;
        _focusState.releaseFn = null;
    };

    _focusState.releaseFn = releaseFn;
    return releaseFn;
}

/**
 * Release the current focus trap if one is active.
 */
function releaseCurrentFocusTrap() {
    if (_focusState.releaseFn) {
        _focusState.releaseFn();
    }
}

// ─── Initialization ───
(function init() {
    initTheme();
    document.getElementById('authToken').value = state.authToken;
    applyFontSize();
    initBottombar();
    initSoundToggle();
    _syncRefreshMsUI();

    // Event delegation for command list — handles kill buttons without inline onclick
    document.getElementById('commandList').addEventListener('click', (e) => {
        const killBtn = e.target.closest('.cmd-kill-btn');
        if (killBtn) {
            e.stopPropagation();
            if (killBtn.disabled) return; // don't kill commands on unreachable instances
            killCommand(killBtn.dataset.instUrl, killBtn.dataset.cmdId);
        }
    });

    // Parse URL arguments for multi-instance
    const params = new URLSearchParams(window.location.search);
    const instances = params.getAll('instance');
    if (instances.length > 0) {
        // First instance is the primary (current origin), rest are additional panels
        state.instanceUrls = instances.map((u, i) => ({
            url: u,
            label: params.getAll('label')[i] || `Instance ${i + 1}`,
            token: params.getAll('token')[i] || '',
            reachable: undefined, // not yet checked
        }));
    } else {
        // Default: current origin
        state.instanceUrls = [{
            url: window.location.origin,
            label: 'Local',
            token: '',
            reachable: undefined, // not yet checked
        }];
    }

    // Create initial panels
    state.instanceUrls.forEach(inst => addPanelDirect(inst.url, inst.label, inst.token));

    // ── Scroll detection: pause VTTY DOM updates while user is scrolling ──
    // Listens on scroll events bubbling from .vtty-container elements.
    // When the user scrolls, _userScrolling is set to true and a 200ms
    // inactivity timer starts.  Once the timer fires, scrolling is
    // considered finished and any buffered updates are flushed.
    document.addEventListener('scroll', (e) => {
        const vttyEl = e.target.closest ? e.target.closest('.vtty-container') : null;
        if (!vttyEl) return;
        state._userScrolling = true;
        if (state._userScrollTimer) clearTimeout(state._userScrollTimer);
        state._userScrollTimer = setTimeout(() => {
            state._userScrolling = false;
            state._userScrollTimer = null;
            if (state._pendingVttyDirty && _isTerminalVisible()) {
                _flushPendingVttyUpdate();
            }
        }, 200);
    }, true);

    // Start refresh
    startRefresh();
    loadCertificates();
    fetchServerTemplates();
    // Fetch server config and apply update mode defaults
    fetchServerConfig();
    applyUpdateModeUI();
    updateSidebarTabsVisibility();

    // Fetch registered peers from the server and add them to instanceUrls.
    // Peers registered via WS push will be handled in the WS onmessage handler.
    fetchPeers();

    // Auto-collapse sidebar on small screens
    if (window.innerWidth <= 768) {
        const sidebar = document.getElementById('sidebar');
        sidebar.classList.add('collapsed');
        sidebar.style.width = '';
    }

    // Auto-fit terminal on window resize (debounced)
    let _resizeTimer = null;
    window.addEventListener('resize', () => {
        if (_resizeTimer) clearTimeout(_resizeTimer);
        _resizeTimer = setTimeout(() => {
            // Auto-collapse/expand sidebar on resize
            const sidebar = document.getElementById('sidebar');
            if (window.innerWidth <= 768) {
                sidebar.classList.add('collapsed');
                sidebar.style.width = '';
            }
            // Auto-fit terminal to panel size
            autoFitActiveTerminal();
        }, 300);
    });

    // ── Command-name URL routing ──
    // If the path is /command-name (e.g. /htop, /btop), auto-select
    // that command.  Supports basename matching so /usr/bin/htop works too.
    // If multiple commands share the same name, show a picker.
    const pathname = window.location.pathname.replace(/^\/+|\/+$/g, '');
    if (pathname && pathname !== 'admin' && !pathname.startsWith('api/')) {
        lookupAndSelectCommand(pathname);
    }

    // ── Sidebar resize ──
    const sidebarHandle = document.getElementById('sidebarResizeHandle');
    if (sidebarHandle) {
        let startX, startWidth;
        const sidebar = document.getElementById('sidebar');
        sidebarHandle.addEventListener('mousedown', (e) => {
            e.preventDefault();
            startX = e.clientX;
            startWidth = sidebar.offsetWidth;
            sidebarHandle.classList.add('active');
            document.body.style.cursor = 'col-resize';
            document.body.style.userSelect = 'none';
            const onMove = (e) => {
                const newWidth = Math.max(150, Math.min(600, startWidth + e.clientX - startX));
                sidebar.style.width = newWidth + 'px';
            };
            const onUp = () => {
                sidebarHandle.classList.remove('active');
                document.body.style.cursor = '';
                document.body.style.userSelect = '';
                document.removeEventListener('mousemove', onMove);
                document.removeEventListener('mouseup', onUp);
            };
            document.addEventListener('mousemove', onMove);
            document.addEventListener('mouseup', onUp);
        });
    }
})();

// ── Command-name URL lookup ──
async function lookupAndSelectCommand(name) {
    try {
        const base = getBaseUrl();
        const res = await fetch(apiUrl('/api/commands/lookup/' + encodeURIComponent(name)), {
            headers: authHeaders()
        });
        const json = await res.json();
        if (json.status !== 'ok') return;
        const matches = json.data;
        if (matches.length === 0) return; // no match, show admin page

        if (matches.length === 1) {
            // Single match — auto-select after loadCommands has run
            state._pendingSelectId = matches[0].id;
            loadCommands();
        } else {
            // Multiple matches — show picker overlay
            showCommandPicker(matches);
        }
    } catch (e) { /* ignore */ }
}

function showCommandPicker(matches) {
    // Remove existing picker if any
    const old = document.getElementById('cmdPicker');
    if (old) old.remove();

    function formatRuntime(secs) {
        if (secs < 60) return Math.floor(secs) + 's';
        if (secs < 3600) return Math.floor(secs / 60) + 'm ' + Math.floor(secs % 60) + 's';
        const h = Math.floor(secs / 3600);
        const m = Math.floor((secs % 3600) / 60);
        return h + 'h ' + m + 'm';
    }

    let items = matches.map(m => {
        const argsStr = (m.args || []).join(' ');
        const detail = argsStr ? `${argsStr} (pid ${m.pid})` : `pid ${m.pid}`;
        const aliveBadge = m.alive
            ? '<span style="color:var(--green);font-size:0.65rem;">● running ' + formatRuntime(m.runtime_secs) + '</span>'
            : '<span style="color:var(--red);font-size:0.65rem;">● exited</span>';
        return `<div class="cmd-item" data-cmd-id="${escHtml(m.id)}" data-cmd-name="${escHtml(m.name)}" style="cursor:pointer;">
            <div class="cmd-item-row">
                <div style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-family:var(--font-mono);font-size:0.75rem;color:var(--text-primary);">${escHtml(m.name)}</div>
                ${aliveBadge}
                <span class="pid" style="color:var(--text-muted);font-size:0.7rem;">pid ${m.pid}</span>
            </div>
            <div class="cmd-detail" style="font-family:var(--font-mono);font-size:0.65rem;color:var(--text-muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;padding-left:1.1rem;">${escHtml(detail)}</div>
        </div>`;
    }).join('');

    const overlay = document.createElement('div');
    overlay.id = 'cmdPicker';
    overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.6);z-index:100;display:flex;align-items:center;justify-content:center;';
    overlay.onclick = (e) => { if (e.target === overlay) { releaseCurrentFocusTrap(); overlay.remove(); } };
    overlay.innerHTML = `<div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;padding:1.25rem;min-width:420px;max-width:90vw;">
        <h2 style="font-size:1rem;color:var(--accent);margin-bottom:0.75rem;">Multiple commands matching "${escHtml(window.location.pathname.replace(/^\/+|\/+$/g, ''))}"</h2>
        <p style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem;">Click a command to view its terminal:</p>
        <div style="max-height:50vh;overflow-y:auto;">${items}</div>
        <div style="margin-top:0.75rem;text-align:right;">
            <button class="btn" onclick="releaseCurrentFocusTrap();document.getElementById('cmdPicker').remove()">Cancel</button>
        </div>
    </div>`;
    document.body.appendChild(overlay);
    // Event delegation for command picker items (no inline onclick to avoid XSS)
    overlay.addEventListener('click', (e) => {
        const item = e.target.closest('.cmd-item[data-cmd-id]');
        if (item) {
            pickCommand(item.dataset.cmdId, item.dataset.cmdName);
        }
    });
    // Trap focus inside the picker and focus the first command item
    const panel = overlay.querySelector('div[style*="background:var(--bg-secondary)"]');
    if (panel) trapFocus(panel);
    const firstItem = overlay.querySelector('.cmd-item');
    if (firstItem) firstItem.focus();
}

function pickCommand(id, name) {
    releaseCurrentFocusTrap();
    const picker = document.getElementById('cmdPicker');
    if (picker) picker.remove();
    state._pendingSelectId = id;
    loadCommands();
}

function getBaseUrl() {
    return state.instanceUrls.length > 0 ? state.instanceUrls[0].url : window.location.origin;
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

// ─── Token Management ───
function saveToken() {
    state.authToken = document.getElementById('authToken').value.trim();
    if (state.authToken) {
        localStorage.setItem('vrw_auth_token', state.authToken);
    } else {
        localStorage.removeItem('vrw_auth_token');
    }
}

// ─── Font Size ───
function changeFontSize(delta) {
    state.fontSize = Math.max(8, Math.min(28, state.fontSize + delta));
    applyFontSize();
}

function applyFontSize() {
    document.documentElement.style.setProperty('--font-size', state.fontSize + 'px');
    const label = document.getElementById('fontSizeLabel');
    if (label) label.textContent = state.fontSize + 'px';
    localStorage.setItem('vrw_font_size', state.fontSize.toString());
}

// Per-panel font size: changes only the specified panel's font size.
function changePanelFontSize(panelId, delta) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    panelObj.fontSize = Math.max(8, Math.min(28, panelObj.fontSize + delta));
    localStorage.setItem('vrw_panel_font_' + panelId, panelObj.fontSize.toString());
    // Apply inline style on the VTTY container
    const vttyEl = document.getElementById('vtty-' + panelId);
    if (vttyEl) vttyEl.style.fontSize = panelObj.fontSize + 'px';
    // Update the label in the panel header
    const label = document.querySelector(`#${panelId} .panel-font-size`);
    if (label) label.textContent = panelObj.fontSize + 'px';
}

// ─── Refresh throttle ───
// Controls how often VTTY updates are applied to the DOM.
// 0 = no throttle (updates applied immediately on every server push).
// 100–2000 = throttle interval in milliseconds (updates batched and applied
// at most once per interval).
function changeRefreshMs(delta) {
    state.refreshMs = Math.max(0, Math.min(2000, state.refreshMs + delta));
    // Snap to 100ms steps (0 stays 0)
    if (state.refreshMs > 0 && state.refreshMs % 100 !== 0) {
        state.refreshMs = Math.round(state.refreshMs / 100) * 100;
    }
    localStorage.setItem('vrw_refresh_ms', state.refreshMs.toString());
    // Update all panel widgets
    _syncRefreshMsUI();
}

/// Apply the refresh throttle from the input field (called on change).
function applyRefreshMs() {
    const val = parseInt(document.getElementById('refreshMs').value) || 0;
    state.refreshMs = Math.max(0, Math.min(2000, val));
    // Snap to 100ms steps (0 stays 0)
    if (state.refreshMs > 0 && state.refreshMs % 100 !== 0) {
        state.refreshMs = Math.round(state.refreshMs / 100) * 100;
    }
    localStorage.setItem('vrw_refresh_ms', state.refreshMs.toString());
    document.getElementById('refreshMs').value = state.refreshMs;
    _syncRefreshMsUI();
}

/// Sync all refresh throttle UI elements with state.refreshMs.
function _syncRefreshMsUI() {
    const input = document.getElementById('refreshMs');
    if (input) input.value = state.refreshMs;
    document.querySelectorAll('.refresh-val').forEach(el => {
        el.textContent = state.refreshMs || 'off';
    });
}

/// Throttled wrapper: if a refresh throttle is active, buffer the update and
/// apply it after the throttle window.  Returns true if the update was
/// throttled (caller should not apply it now), false if it should be applied
/// immediately.
function _throttleRefresh() {
    if (state.refreshMs <= 0) return false; // no throttle
    if (state._refreshThrottleTimer) return true; // already pending
    state._refreshThrottleTimer = setTimeout(() => {
        state._refreshThrottleTimer = null;
        _flushThrottledRefresh();
    }, state.refreshMs);
    return true;
}

/// Called when the throttle timer fires: fetch the latest VTTY state.
function _flushThrottledRefresh() {
    if (state.selectedInstUrl && state.selectedCmdId) {
        scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 0);
    }
}

// Per-panel theme toggle: cycles through '' (inherit global) → 'light' → 'dark' → ''.
// Only affects the VTTY terminal area, not the surrounding UI chrome.
function togglePanelTheme(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const next = panelObj.theme === '' ? 'light' : panelObj.theme === 'light' ? 'dark' : '';
    panelObj.theme = next;
    localStorage.setItem('vrw_panel_theme_' + panelId, next);
    applyPanelTheme(panelId, next);
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

// ─── Selection Mode ───
// When active, mouse events are NOT forwarded to PTY, enabling native text selection.
function toggleSelectionMode(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    panelObj.selectionMode = !panelObj.selectionMode;
    localStorage.setItem('vrw_panel_sel_' + panelId, panelObj.selectionMode.toString());
    const vttyEl = document.getElementById('vtty-' + panelId);
    if (vttyEl) vttyEl.classList.toggle('selection-mode', panelObj.selectionMode);
    const btn = document.getElementById('selectBtn-' + panelId);
    if (btn) {
        btn.classList.toggle('btn-primary', panelObj.selectionMode);
        btn.textContent = panelObj.selectionMode ? '✓ Select' : 'Select';
    }
}

// ─── Sidebar ───
function toggleSidebar() {
    const sidebar = document.getElementById('sidebar');
    sidebar.classList.toggle('collapsed');
    // Clear inline width set by the drag handle so the CSS class takes effect.
    // Without this, an inline style.width from dragging overrides .collapsed { width: 0 }.
    if (sidebar.classList.contains('collapsed')) {
        sidebar.style.width = '';
    }
}

// ─── Resource toggle ───
function toggleResources() {
    state.showResources = !state.showResources;
    localStorage.setItem('vrw_show_resources', state.showResources.toString());
    const display = state.showResources ? '' : 'none';
    document.querySelectorAll('.resource-badge, .instance-url').forEach(el => {
        el.style.display = display;
    });
}

// ─── Bottom bar toggle ───
function toggleBottombar() {
    const bar = document.getElementById('bottomBar');
    const btn = document.getElementById('statusBtn');
    bar.classList.toggle('hidden');
    const isHidden = bar.classList.contains('hidden');
    if (btn) {
        btn.style.background = isHidden ? '' : 'var(--accent)';
        btn.style.color = isHidden ? '' : '#fff';
    }
    localStorage.setItem('vrw_bottombar_hidden', isHidden ? 'true' : 'false');
}

function initBottombar() {
    const shouldHide = localStorage.getItem('vrw_bottombar_hidden') !== 'false'; // hidden by default
    const bar = document.getElementById('bottomBar');
    const btn = document.getElementById('statusBtn');
    if (shouldHide) {
        bar.classList.add('hidden');
    } else {
        bar.classList.remove('hidden');
        if (btn) { btn.style.background = 'var(--accent)'; btn.style.color = '#fff'; }
    }
}

// ─── Logs view toggle ───
function toggleLogsView() {
    const btn = document.getElementById('logsBtn');
    const vtty = document.getElementById('view-vtty');
    const log = document.getElementById('view-log');
    const prevView = state.currentView;
    if (state.currentView === 'log') {
        // Switch back to terminal
        state.currentView = 'vtty';
        vtty.style.display = 'flex';
        log.style.display = 'none';
        if (btn) { btn.style.background = ''; btn.style.color = ''; }
        disconnectLogWs();
        // Flush any VTTY updates that arrived while logs were shown
        _flushPendingVttyUpdate();
    } else {
        // Switch to logs
        state.currentView = 'log';
        vtty.style.display = 'none';
        log.style.display = 'flex';
        if (btn) { btn.style.background = 'var(--accent)'; btn.style.color = '#fff'; }
        loadLog();
        if (!document.getElementById('logSearch').value) {
            connectLogWs();
        }
    }
}

function switchSidebarTab(tab, el) {
    document.querySelectorAll('.sidebar-tab').forEach(t => t.classList.remove('active'));
    el.classList.add('active');
    document.getElementById('tab-commands').style.display = tab === 'commands' ? '' : 'none';
    document.getElementById('tab-spawn').style.display = tab === 'spawn' ? '' : 'none';
    document.getElementById('tab-templates').style.display = tab === 'templates' ? '' : 'none';
    document.getElementById('tab-certs').style.display = tab === 'certs' ? '' : 'none';
    if (tab === 'templates') renderTemplates();
}

// Update sidebar tab visibility based on server reachability.
// When no vrw instance is reachable, hide the Spawn tab.
function updateSidebarTabsVisibility() {
    const spawnTab = document.querySelector('.sidebar-tab:nth-child(2)');
    const spawnContent = document.getElementById('tab-spawn');
    const anyReachable = state.instanceUrls.some(i => i.reachable === true);
    if (anyReachable) {
        if (spawnTab) spawnTab.style.display = '';
        // Only show spawn content if the spawn tab is currently active;
        // otherwise let switchSidebarTab() manage content visibility.
        if (spawnContent && spawnTab && spawnTab.classList.contains('active')) {
            spawnContent.style.display = '';
        }
    } else {
        if (spawnTab) spawnTab.style.display = 'none';
        if (spawnContent) spawnContent.style.display = 'none';
        // If spawn tab was active, switch to commands
        const activeTab = document.querySelector('.sidebar-tab.active');
        if (activeTab && activeTab === spawnTab) {
            const cmdsTab = document.querySelector('.sidebar-tab:first-child');
            if (cmdsTab) switchSidebarTab('commands', cmdsTab);
        }
    }
}

/// Show/hide the command toolbar (filter + kill all) based on whether
/// there is a reachable server with commands.  Hidden when no server is
/// reachable or when there are zero commands across all instances.
function updateCmdToolbarVisibility() {
    const toolbar = document.getElementById('cmdToolbar');
    if (!toolbar) return;
    const anyReachable = state.instanceUrls.some(i => i.reachable === true);
    const anyCommands = state.instanceUrls.some(
        i => i._commands && i._commands.length > 0
    );
    toolbar.style.display = (anyReachable && anyCommands) ? 'flex' : 'none';
}

// ─── Disconnected state ───

/// Central function that updates all UI elements when instance reachability changes.
/// Called by loadCommands() when any instance's reachable flag changes.
function updateDisconnectedUI() {
    updateSidebarBanner();
    updateSidebarTabsVisibility();
    updateTerminalDisconnectedOverlay();
    updateCmdToolbarVisibility();
}

/// Show/hide a disconnected banner in the sidebar header area.
function updateSidebarBanner() {
    let banner = document.getElementById('disconnectedBanner');
    const unreachable = state.instanceUrls.filter(i => i.reachable === false);
    if (unreachable.length > 0) {
        if (!banner) {
            banner = document.createElement('div');
            banner.id = 'disconnectedBanner';
            banner.className = 'disconnected-banner';
            const content = document.getElementById('sidebarContent');
            content.insertBefore(banner, content.firstChild);
        }
        const labels = unreachable.map(i => i.label).join(', ');
        banner.innerHTML = '<span class="disconnected-icon">&#9888;</span> Server disconnected: ' +
            escHtml(labels) + ' &mdash; output may be stale';
    } else {
        if (banner) banner.remove();
    }
}

/// Show/hide a "Server unreachable" overlay on the terminal panel when
/// the currently selected command belongs to a disconnected instance.
function updateTerminalDisconnectedOverlay() {
    const panel = document.querySelector('.panel');
    if (!panel) return;
    let overlay = panel.querySelector('.disconnected-overlay');
    const inst = state.instanceUrls.find(i => i.url === state.selectedInstUrl);
    if (inst && inst.reachable === false) {
        if (!overlay) {
            overlay = document.createElement('div');
            overlay.className = 'disconnected-overlay';
            overlay.innerHTML = '<span>&#9888; Server unreachable &mdash; output is stale</span>';
            const vttyEl = panel.querySelector('.vtty-container');
            if (vttyEl) vttyEl.appendChild(overlay);
        }
    } else {
        if (overlay) overlay.remove();
    }
}

// ─── Peer instances (registration & failover) ───

/// Fetch the list of registered peers from the primary server.
/// Peers discovered this way are added to instanceUrls so the UI
/// shows commands from all registered instances.
async function fetchPeers() {
    try {
        const res = await fetch(apiUrl('/api/peers'), { headers: authHeaders() });
        if (!res.ok) return;
        const json = await res.json();
        if (json.status !== 'ok' || !Array.isArray(json.data)) return;

        for (const peer of json.data) {
            // Skip if already known
            if (state.instanceUrls.some(i => i.url === peer.url)) continue;
            addDiscoveredPeer(peer.url, peer.label || peer.url, peer.token || '');
        }

        // Save peers to localStorage for the reload edge case
        savePeersToStorage();

        if (json.data.length > 0) {
            loadCommands(); // Re-render sidebar with peer commands
        }
    } catch (e) {
        // Not critical — peers can also be discovered via WS push
    }
}

/// Add a peer instance to instanceUrls and create a panel for it.
function addDiscoveredPeer(url, label, token) {
    state.instanceUrls.push({
        url,
        label,
        token,
        reachable: undefined, // not yet checked
    });
    // Add a panel so the user can see this instance's commands
    addPanelDirect(url, label, token);
    console.log('[vrw] Peer discovered:', label, '(' + url + ')');
}

/// Handle a peer_registered or peer_unregistered WS message.
function handlePeerEvent(msg) {
    if (msg.type === 'peer_registered' && msg.data) {
        const { url, label, token } = msg.data;
        addDiscoveredPeer(url, label, token);
        savePeersToStorage();
    } else if (msg.type === 'peer_unregistered' && msg.data) {
        const { url } = msg.data;
        const idx = state.instanceUrls.findIndex(i => i.url === url);
        if (idx !== -1) {
            state.instanceUrls.splice(idx, 1);
            removePanelByInstUrl(url);
            loadCommands();
        }
        savePeersToStorage();
    }
}

/// Save known peer URLs to localStorage so that if the primary dies
/// and the page is reloaded pointing to a peer, the peer list survives.
function savePeersToStorage() {
    const peers = state.instanceUrls.filter(i => i.url !== window.location.origin);
    if (peers.length > 0) {
        try {
            localStorage.setItem('vrw_peers', JSON.stringify(
                peers.map(p => ({ url: p.url, label: p.label, token: p.token }))
            ));
        } catch (e) { /* quota exceeded — not critical */ }
    } else {
        localStorage.removeItem('vrw_peers');
    }
}

// ─── View Tabs ───
function switchViewTab(view, el) {
    document.querySelectorAll('.view-tab').forEach(t => t.classList.remove('active'));
    el.classList.add('active');
    const prevView = state.currentView;
    state.currentView = view;
    document.getElementById('view-vtty').style.display = view === 'vtty' ? 'flex' : 'none';
    document.getElementById('view-log').style.display = view === 'log' ? 'flex' : 'none';
    document.getElementById('view-docs').style.display = view === 'docs' ? 'block' : 'none';
    // Disconnect log WS when leaving log view
    if (prevView === 'log' && view !== 'log') {
        disconnectLogWs();
    }
    if (view === 'log') {
        loadLog();
        // After HTTP load, start WebSocket streaming (unless search is active)
        if (!document.getElementById('logSearch').value) {
            connectLogWs();
        }
    }
    // Flush any VTTY updates that arrived while viewing logs/docs
    if (view === 'vtty' && prevView !== 'vtty') {
        _flushPendingVttyUpdate();
    }
}

// ─── Commands ───

/// Fast initial load: fetch commands, VTTY HTML, and resources in a SINGLE
/// request from the primary instance.  This replaces the old flow of
/// loadCommands → _prefetchVttyHtml → pollResources (3+ serial round trips)
/// with just 1 round trip.
///
/// After the snapshot is processed, peer instances are fetched in parallel.
/// Subsequent refreshes use the lighter loadCommands() which only fetches
/// the commands list (no VTTY HTML, no resources).
let _snapshotLoaded = false;

// ── Command Navigation (prev/next) ──
// Navigate through the flat command list. These functions are called by
// the prev/next buttons in the topbar, useful when the sidebar is hidden.
function navigateCommand(direction) {
    if (_navCommands.length === 0) return;
    const currentIdx = _navCommands.findIndex(
        c => c.instUrl === state.selectedInstUrl && c.cmdId === state.selectedCmdId
    );
    let nextIdx;
    if (currentIdx === -1) {
        // No command selected — go to first
        nextIdx = direction > 0 ? 0 : _navCommands.length - 1;
    } else {
        nextIdx = (currentIdx + direction + _navCommands.length) % _navCommands.length;
    }
    const target = _navCommands[nextIdx];
    if (target) {
        selectCommand(target.instUrl, target.cmdId, target.name);
    }
}

function navigatePrevCommand() {
    navigateCommand(-1);
}

function navigateNextCommand() {
    navigateCommand(1);
}

async function loadSnapshot() {
    if (_snapshotLoaded) { loadCommands(); return; }
    _snapshotLoaded = true;

    const primaryInst = state.instanceUrls[0];
    if (!primaryInst) { loadCommands(); return; }

    try {
        const res = await fetch(apiUrl('/api/snapshot', primaryInst),
            { headers: authHeadersForInstance(primaryInst) });
        if (!res.ok) throw new Error('HTTP ' + res.status);
        const json = await res.json();
        if (json.status !== 'ok' || !json.data) throw new Error('bad snapshot');

        const { commands, vtty, resources } = json.data;

        // Store commands for the primary instance
        primaryInst._commands = commands || [];
        primaryInst.reachable = true;
        primaryInst._lastError = null;

        // Store resources in cache — sidebar will show them immediately
        if (resources) {
            for (const [cmdId, resData] of Object.entries(resources)) {
                state._resourceCache[cmdId] = resData;
            }
        }

        // Fetch peer instances in parallel (don't block the primary display)
        const peerPromises = state.instanceUrls.slice(1).map(async (inst) => {
            try {
                const r = await fetch(apiUrl('/api/commands', inst),
                    { headers: authHeadersForInstance(inst) });
                if (!r.ok) throw new Error('HTTP ' + r.status);
                const j = await r.json();
                inst._commands = j.status === 'ok' ? j.data : [];
                inst.reachable = true;
                inst._lastError = null;
            } catch (e) {
                inst._commands = inst._commands || [];
                inst.reachable = false;
                inst._lastError = 'connection lost (instance may have exited)';
            }
        });
        // Kick off peer fetches but don't await — render primary immediately
        const peersDone = Promise.all(peerPromises).then(() => {
            updateDisconnectedUI();
        });

        // ── Render terminal from embedded VTTY HTML ──
        const hasAnyCommands = commands && commands.length > 0;
        const firstCmd = hasAnyCommands
            ? (commands.find(c => c.alive) || commands[0])
            : null;
        const shouldShowWelcome = (state.panels.length === 1 && !hasAnyCommands && !state.selectedCmdId && !state.serverReachable);

        if (shouldShowWelcome !== _showingWelcome) {
            renderPanels();
        }

        if (vtty && vtty.html !== undefined && firstCmd) {
            state.selectedInstUrl = primaryInst.url;
            state.selectedCmdId = firstCmd.id;
            state._pendingVttyData = null;
            state._pendingVttyDirty = false;
            state.bufferView = 'current';

            // Store generation for subsequent incremental updates
            if (vtty.generation !== undefined) {
                state._lastGeneration[firstCmd.id] = vtty.generation;
            }

            // Write VTTY HTML directly into <pre> — NO second HTTP request
            const panel = getSelectedPanel();
            if (panel) {
                const vttyEl = panel.querySelector('.vtty-container');
                const pre = vttyEl ? vttyEl.querySelector('pre') : null;
                if (pre) {
                    pre.innerHTML = vtty.html;
                    // Build cell grid for Level 3 incremental diffing
                    if (state._level3Enabled && vtty.dimensions) {
                        buildCellGrid(firstCmd.id, pre, vtty.dimensions.rows, vtty.dimensions.cols);
                    }
                    // Update metadata (cursor, dimensions, alt screen, etc.)
                    updateVttyMetadataFromHttp(vtty, panel,
                        state.panels.find(p => p.id === panel.id), 0);
                }
            }

            updatePanelCommandInfo();
            updateTerminalDisconnectedOverlay();
            // Start push/poll for incremental updates
            startUpdateMode();
        } else {
            _showingWelcome = shouldShowWelcome;
            updateDisconnectedUI();
        }

        // Wait for peers to finish, then build the sidebar with full data
        await peersDone;
        // Build sidebar (includes resource data from cache)
        _buildSidebar();

    } catch (e) {
        primaryInst._commands = primaryInst._commands || [];
        primaryInst.reachable = false;
        primaryInst._lastError = 'connection lost';
        updateDisconnectedUI();
        // Fall back to regular loadCommands
        loadCommands();
    }
}

/// Extract sidebar-building logic into a reusable function so both
/// loadSnapshot() and loadCommands() can use it.
function _buildSidebar() {
    const filter = (document.getElementById('cmdFilter') || {}).value || '';
    const filterLower = filter.toLowerCase();
    let fingerprint = '';
    for (const inst of state.instanceUrls) {
        fingerprint += inst.url + ':reachable=' + inst.reachable + '|';
        for (const cmd of (inst._commands || [])) {
            const cmdName = cmd.name || cmd.id;
            if (filterLower && !cmdName.toLowerCase().includes(filterLower) &&
                !(cmd.args || []).join(' ').toLowerCase().includes(filterLower) &&
                !String(cmd.pid).includes(filterLower)) continue;
            const isAlive = cmd.alive !== false;
            fingerprint += inst.url + ':' + cmd.id + ':' + isAlive + ':' + (cmd.exit_code != null ? cmd.exit_code : '') + ':' + (cmd.runtime_secs || 0) + '|';
        }
    }
    if (fingerprint === _lastCommandState) {
        if (state._pendingSelectId) {
            const pendingId = state._pendingSelectId;
            state._pendingSelectId = null;
            for (const inst of state.instanceUrls) {
                if (inst._commands && inst._commands.find(c => c.id === pendingId)) {
                    const cmd = inst._commands.find(c => c.id === pendingId);
                    selectCommand(inst.url, cmd.id, cmd.name || cmd.id);
                    return;
                }
            }
        }
        if (state.selectedInstUrl && state.selectedCmdId) {
            updatePanelCommandInfo();
            if (state.updateMode === 'poll' || state.bufferView !== 'current') {
                scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 500);
            }
        }
        return;
    }
    _lastCommandState = fingerprint;

    const container = document.getElementById('commandList');
    let html = '';

    if (state.instanceUrls.length > 1) {
        html += '<div class="sidebar-sort-bar">';
        html += `<span class="sidebar-sort-item${_sidebarSort === 'name' ? ' active' : ''}" onclick="_sidebarSort='name';loadCommands()">All</span>`;
        for (const inst of state.instanceUrls) {
            const active = _sidebarSort === inst.url ? ' active' : '';
            html += `<span class="sidebar-sort-item${active}" onclick="_sidebarSort='${escHtml(inst.url)}';loadCommands()">${escHtml(inst.label)}</span>`;
        }
        html += '</div>';
    }

    function formatRuntime(secs) {
        if (!secs || secs < 0) return '';
        if (secs < 60) return Math.floor(secs) + 's';
        if (secs < 3600) return Math.floor(secs / 60) + 'm ' + Math.floor(secs % 60) + 's';
        const h = Math.floor(secs / 3600);
        const m = Math.floor((secs % 3600) / 60);
        return h + 'h ' + m + 'm';
    }

    let allCmds = [];
    for (const inst of state.instanceUrls) {
        for (const cmd of (inst._commands || [])) {
            const cmdName = cmd.name || cmd.id;
            if (filterLower && !cmdName.toLowerCase().includes(filterLower) &&
                !(cmd.args || []).join(' ').toLowerCase().includes(filterLower) &&
                !String(cmd.pid).includes(filterLower)) continue;
            allCmds.push({ inst, cmd, cmdName });
        }
    }

    // Sort allCmds globally for navigation (alphabetical by name)
    allCmds.sort((a, b) => a.cmdName.localeCompare(b.cmdName));

    // Build the flat navigation list from the sorted commands
    _navCommands = allCmds.map(({ inst, cmd, cmdName }) => ({
        instUrl: inst.url,
        cmdId: cmd.id,
        name: cmdName,
    }));

    if (_sidebarSort === 'name') {
        allCmds.sort((a, b) => a.cmdName.localeCompare(b.cmdName));
        html += renderCmdList(allCmds);
    } else {
        const targetUrl = _sidebarSort;
        const grouped = targetUrl === 'all' ? null : targetUrl;
        for (const inst of state.instanceUrls) {
            if (grouped && inst.url !== grouped) continue;
            const instCmds = allCmds.filter(c => c.inst.url === inst.url);
            if (instCmds.length === 0 && grouped) continue;
            if (inst._lastError && (!grouped || inst.url === grouped)) {
                html += `<div style="padding:0.5rem;color:var(--red);font-size:0.7rem;">${escHtml(inst.label)}: ${escHtml(inst._lastError)}</div>`;
                continue;
            }
            if (state.instanceUrls.length > 1) {
                html += `<div class="pinned-section-header">${escHtml(inst.label)}</div>`;
            }
            if (instCmds.length === 0) {
                html += `<div style="padding:0.3rem 0.4rem;color:var(--text-muted);font-size:0.7rem;">No commands</div>`;
                continue;
            }
            instCmds.sort((a, b) => a.cmdName.localeCompare(b.cmdName));
            html += renderCmdList(instCmds);
        }
    }

    function renderCmdList(cmds) {
        let out = '';
        for (const { inst, cmd, cmdName } of cmds) {
            const cert = cmd.certificate || '';
            const certBadge = cert
                ? `<span class="cert-badge" title="Bound to: ${escHtml(cert)}">${escHtml(cert)}</span>`
                : '';
            const selected = (state.selectedInstUrl === inst.url && state.selectedCmdId === cmd.id) ? ' selected' : '';
            const isAlive = cmd.alive !== false;
            const isFrozen = cmd.frozen === true;
            const runtimeStr = (isAlive || isFrozen) && cmd.runtime_secs > 0
                ? formatRuntime(cmd.runtime_secs)
                : '';
            const frozenBadge = isFrozen ? 'PAUSED ' : '';
            const exitBadge = (cmd.exit_code != null)
                ? `<span class="exit-badge ${cmd.exit_code === 0 ? 'success' : 'failure'}">exit ${cmd.exit_code}</span>`
                : '';
            const res = state._resourceCache[cmd.id];
            const resourceStr = (res && (res.cpu_percent != null || res.memory_mb != null))
                ? `${res.cpu_percent != null ? res.cpu_percent.toFixed(1) + '%' : ''}${res.cpu_percent != null && res.memory_mb != null ? ' ' : ''}${res.memory_mb != null ? res.memory_mb.toFixed(1) + 'MB' : ''}`
                : '';
            const pinnedNames = getPinnedNames();
            const isPinned = pinnedNames.includes(cmdName);
            const frozenClass = isFrozen ? ' frozen' : '';
            const exitedClass = (!isAlive && !isFrozen) ? ' exited' : '';
            const instUnreachable = inst.reachable === false;
            const dimStyle = instUnreachable ? 'opacity:0.4;' : ((isAlive || isFrozen) ? '' : 'opacity:0.6;');
            const killDisabled = instUnreachable ? ' disabled title="Server disconnected"' : ' title="Kill"';
            // Build detail parts as separate spans for the detail row
            const detailParts = [];
            if (runtimeStr) detailParts.push(escHtml(runtimeStr));
            if (frozenBadge) detailParts.push(escHtml(frozenBadge.trim()));
            if (res && res.cpu_percent != null) detailParts.push('CPU ' + res.cpu_percent.toFixed(1) + '%');
            if (res && res.memory_mb != null) detailParts.push('MEM ' + res.memory_mb.toFixed(1) + 'MB');
            if (cmd.pid) detailParts.push('pid ' + cmd.pid);
            const unreachableTitle = instUnreachable ? ` [disconnected]` : '';
            out += `
                <div class="cmd-item${selected}${frozenClass}${exitedClass}${instUnreachable ? ' unreachable' : ''}" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}" data-cmd-name="${escHtml(cmdName)}" data-cmd-alive="${isAlive}" data-cmd-frozen="${isFrozen}" tabindex="0" role="button" aria-label="Command ${escHtml(cmdName)}" onclick="selectCommand(this.dataset.instUrl,this.dataset.cmdId,this.dataset.cmdName)" oncontextmenu="showCmdContextMenu(event,this.dataset.instUrl,this.dataset.cmdId,this.dataset.cmdName,this.dataset.cmdAlive==='true')" title="${escHtml(inst.label)} / ${escHtml(cmdName)}${unreachableTitle}" style="${dimStyle}">
                    <div class="cmd-item-row">
                        <button class="btn btn-xs btn-danger cmd-kill-btn" data-inst-url="${escHtml(inst.url)}" data-cmd-id="${escHtml(cmd.id)}"${killDisabled}>&#x2715;</button>
                        <button class="pin-btn${isPinned ? ' active' : ''}" onclick="event.stopPropagation();togglePinCmd('${escHtml(cmdName)}')" title="${isPinned ? 'Unpin' : 'Pin'}">&#9734;</button>
                        <span class="name">${escHtml(cmdName)}</span>
                        ${certBadge}
                        ${exitBadge}
                    </div>
                    ${detailParts.length > 0 ? `<div class="cmd-detail-row">${detailParts.join('<span class="detail-sep">|</span>')}</div>` : ''}
                </div>`;
        }
        return out;
    }

    rearrangePinnedCommands(container);
    container.innerHTML = html || '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No running commands</div>';
    updateInstanceDropdown();
    updateCmdToolbarVisibility();

    if (state._pendingSelectId) {
        const pendingId = state._pendingSelectId;
        state._pendingSelectId = null;
        for (const inst of state.instanceUrls) {
            if (inst._commands && inst._commands.find(c => c.id === pendingId)) {
                const cmd = inst._commands.find(c => c.id === pendingId);
                selectCommand(inst.url, cmd.id, cmd.name || cmd.id);
                return;
            }
        }
    }

    if (!state.selectedCmdId) {
        for (const inst of state.instanceUrls) {
            if (inst._commands && inst._commands.length > 0) {
                const cmd = inst._commands[0];
                selectCommand(inst.url, cmd.id, cmd.name || cmd.id);
                return;
            }
        }
    }

    if (state.selectedInstUrl && state.selectedCmdId) {
        updatePanelCommandInfo();
        if (state.updateMode === 'poll' || state.bufferView !== 'current') {
            scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 500);
        }
    }
}

async function loadCommands() {
    // Load commands from all instances in PARALLEL and track reachability.
    let anyReachableChanged = false;
    await Promise.all(state.instanceUrls.map(async (inst) => {
        try {
            const res = await fetch(apiUrl('/api/commands', inst), { headers: authHeadersForInstance(inst) });
            if (!res.ok) throw new Error('HTTP ' + res.status);
            const json = await res.json();
            inst._commands = json.status === 'ok' ? json.data : [];
            const wasReachable = inst.reachable;
            inst.reachable = true;
            inst._lastError = null;
            if (wasReachable !== true) anyReachableChanged = true;
        } catch (e) {
            inst._commands = inst._commands || [];
            const wasReachable = inst.reachable;
            inst.reachable = false;
            inst._lastError = 'connection lost (instance may have exited)';
            if (wasReachable !== false) anyReachableChanged = true;
        }
    }));
    if (anyReachableChanged) {
        updateDisconnectedUI();
    }

    // Check if welcome-panel state changed and re-render panels if so
    let hasAnyCommands = false;
    for (const inst of state.instanceUrls) {
        if (inst._commands && inst._commands.length > 0) {
            hasAnyCommands = true;
            break;
        }
    }
    const shouldShowWelcome = (state.panels.length === 1 && !hasAnyCommands && !state.selectedCmdId && !state.serverReachable);
    if (shouldShowWelcome !== _showingWelcome) {
        renderPanels();
    }

    // Build sidebar (reuses extracted _buildSidebar for consistency)
    _buildSidebar();
}

/// Cache the terminal display DOM for the currently selected command.
/// Called before switching to a different command.  Moves the <pre> children
/// into a detached DocumentFragment so they can be re-attached instantly on
/// switch-back, avoiding a full HTML fetch when the command hasn't changed.
function _cacheTerminalForSwitch() {
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    const cmdId = state.selectedCmdId;
    if (!pre || !cmdId) return;

    // Detach all children into a DocumentFragment (preserves DOM nodes)
    const frag = document.createDocumentFragment();
    while (pre.firstChild) {
        frag.appendChild(pre.firstChild);
    }
    state._cachedDomPre[cmdId] = frag;
    // Save scroll position for this command
    if (vttyEl) {
        state._cachedScrollPos[cmdId] = vttyEl.scrollTop;
    }
    // Keep _cellGrids and _lastGeneration — they are still valid for the cached DOM.
}

/// Restore a previously cached DOM tree into the <pre> element for instant display.
/// Called from selectCommand() when switching to a command that was viewed before.
/// The cached DOM is moved (not cloned) back into the document, and scroll position
/// is restored.  After this, loadVttyHttp() checks generation — if unchanged, the
/// cached DOM stays; if changed, the full HTML fetch replaces it.
function _restoreCachedDom(cmdId) {
    const frag = state._cachedDomPre[cmdId];
    if (!frag) return;
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;
    // Move the cached DocumentFragment into the <pre> (O(1), no parsing)
    pre.appendChild(frag);
    delete state._cachedDomPre[cmdId];
    // Restore scroll position
    const savedScroll = state._cachedScrollPos[cmdId];
    if (savedScroll !== undefined) {
        vttyEl.scrollTop = savedScroll;
        delete state._cachedScrollPos[cmdId];
    }
}

/// Lightweight DOM-only update: toggle the .selected class on sidebar items
/// without re-fetching /api/commands. Used by selectCommand() to avoid
/// a redundant HTTP roundtrip that would delay the initial VTTY load.
function updateSidebarSelection() {
    document.querySelectorAll('#commandList .cmd-item').forEach(el => {
        const matchInst = el.dataset.instUrl === state.selectedInstUrl;
        const matchCmd = el.dataset.cmdId === state.selectedCmdId;
        el.classList.toggle('selected', matchInst && matchCmd);
    });
}

function selectCommand(instUrl, cmdId, name) {
    // Clear stored scrollback for the previously selected command
    if (state.selectedCmdId) {
        sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
    }

    // Cache the current command's terminal DOM before switching away.
    // This preserves the detached DOM subtree and cell grid so we can
    // restore them instantly on switch-back without a full HTML fetch.
    disconnectVttyWs();
    _cacheTerminalForSwitch();

    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    // Clear any buffered update — we fetch fresh data below
    state._pendingVttyData = null;
    state._pendingVttyDirty = false;
    // Restore cached DOM from previous visit if available (instant display).
    // Then loadVttyHttp will check generation — if unchanged, the cached
    // DOM is kept; if changed, a full HTML fetch replaces it.
    _restoreCachedDom(cmdId);
    state.bufferView = 'current';
    const globalBufferSel = document.getElementById('bufferSelect');
    if (globalBufferSel) globalBufferSel.value = 'current';
    // Reset panel-scoped buffer selects too
    state.panels.forEach(p => {
        const sel = document.getElementById('bufferSelect-' + p.id);
        if (sel) sel.value = 'current';
    });

    // Restore scrollback offset from sessionStorage for the new command
    const savedOffset = sessionStorage.getItem('vrw_scrollback_' + cmdId);
    const restoredOffset = savedOffset !== null ? parseInt(savedOffset, 10) : 0;
    state.panels.forEach(p => p.scrollbackOffset = restoredOffset);

    updatePanelCommandInfo();
    updateTerminalDisconnectedOverlay();
    updateSidebarSelection();
    // Fetch VTTY content — will skip DOM write if generation unchanged
    loadVttyHttp(instUrl, cmdId);
    // Start the active update mode (push or poll)
    startUpdateMode();
}

// Update the panel header with the selected command's full name and args.
function updatePanelCommandInfo() {
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    // Find the command data from the loaded instance commands
    let cmd = null;
    for (const inst of state.instanceUrls) {
        if (inst.url === state.selectedInstUrl && inst._commands) {
            cmd = inst._commands.find(c => c.id === state.selectedCmdId);
            break;
        }
    }
    const panel = getSelectedPanel();
    if (!panel) return;
    const nameEl = panel.querySelector('.cmd-fullname');
    const argsEl = panel.querySelector('.cmd-args');
    if (nameEl && cmd) {
        const fullName = cmd.name || cmd.id;
        nameEl.textContent = fullName;
        nameEl.title = fullName;
        if (argsEl) {
            const argsStr = (cmd.args || []).join(' ');
            argsEl.textContent = argsStr;
            argsEl.title = argsStr || '';
        }
        // Update bottom bar command label
        updateBottomBarLabel(cmd);

        // Update per-panel pause button
        const pauseBtn = panel.querySelector(`[id^="pauseRunBtn-"]`);
        if (pauseBtn) {
            const isAlive = cmd.alive !== false;
            const isFrozen = cmd.frozen === true;
            if (isAlive) {
                pauseBtn.style.display = '';
                pauseBtn.textContent = isFrozen ? '\u25B6 Run' : '\u23F8 Pause';
                pauseBtn.className = 'btn btn-xs' + (isFrozen ? ' btn-primary' : '');
            } else {
                pauseBtn.style.display = 'none';
            }
        }

        // Show/hide restart button next to command name
        const restartBtn = panel.querySelector(`[id^="restartBtn-"]`);
        if (restartBtn) {
            restartBtn.style.display = '';
        }

        // Update resource badge in panel header
        const resourceBadgeEl = panel.querySelector(`[id^="resourceBadge-"]`);
        if (resourceBadgeEl) {
            const res = state._resourceCache[cmd.id];
            if (state.showResources && res && (res.cpu_percent != null || res.memory_mb != null)) {
                resourceBadgeEl.style.display = '';
                resourceBadgeEl.textContent = (res.cpu_percent != null ? 'CPU ' + res.cpu_percent.toFixed(1) + '%' : '') +
                    (res.cpu_percent != null && res.memory_mb != null ? ' | ' : '') +
                    (res.memory_mb != null ? res.memory_mb.toFixed(1) + 'MB' : '');
            } else {
                resourceBadgeEl.textContent = '';
                if (!state.showResources) resourceBadgeEl.style.display = 'none';
            }
        }

        // Update exited banner on VTTY container
        const exitedBanner = panel.querySelector(`[id^="exitedBanner-"]`);
        if (exitedBanner) {
            const isAlive = cmd.alive !== false;
            const isFrozen = cmd.frozen === true;
            if (!isAlive && !isFrozen) {
                const exitCode = cmd.exit_code != null ? cmd.exit_code : '?';
                const exitClass = cmd.exit_code === 0 ? 'success' : 'failure';
                exitedBanner.innerHTML = `<span class="exited-banner-icon">&#9632;</span> Command exited <span class="exit-badge ${exitClass}">exit ${exitCode}</span>`;
                exitedBanner.style.display = 'flex';
            } else {
                exitedBanner.style.display = 'none';
            }
        }
    } else if (nameEl) {
        nameEl.textContent = '';
        if (argsEl) argsEl.textContent = '';
        updateBottomBarLabel(null);
        // Hide pause button
        const pauseBtn = panel.querySelector(`[id^="pauseRunBtn-"]`);
        if (pauseBtn) pauseBtn.style.display = 'none';
        // Hide restart button
        const restartBtn = panel.querySelector(`[id^="restartBtn-"]`);
        if (restartBtn) restartBtn.style.display = 'none';
        // Hide exited banner
        const exitedBanner = panel.querySelector(`[id^="exitedBanner-"]`);
        if (exitedBanner) exitedBanner.style.display = 'none';
    }
}

// ─── Bottom bar: command label ───
function updateBottomBarLabel(cmd) {
    const el = document.getElementById('cmdLabel');
    if (!el) return;
    if (!cmd) {
        el.innerHTML = '';
        return;
    }
    const fullName = cmd.name || cmd.id;
    const argsStr = (cmd.args || []).join(' ');
    const pid = cmd.pid || '';
    let html = `<span class="cmd-label-name">${escHtml(fullName)}</span>`;
    if (argsStr) {
        html += `<span class="cmd-label-sep">|</span><span class="cmd-label-args">${escHtml(argsStr)}</span>`;
    }
    if (pid) {
        html += `<span class="cmd-label-sep">|</span><span class="cmd-label-pid">pid ${pid}</span>`;
    }
    el.innerHTML = html;
    el.title = argsStr ? `${fullName} ${argsStr} (pid ${pid})` : `${fullName} (pid ${pid})`;
}

// ─── Spawn: auto-fit terminal size ───
function autofitTerminalSize() {
    // Calculate optimal terminal size from the current panel container
    const panel = getSelectedPanel();
    if (!panel) {
        document.getElementById('autofitHint').textContent = 'No panel visible to measure';
        return;
    }
    const vttyEl = panel.querySelector('.vtty-container');
    if (!vttyEl) {
        document.getElementById('autofitHint').textContent = 'No terminal container found';
        return;
    }
    const rect = vttyEl.getBoundingClientRect();
    const charW = state.fontSize * 0.6;
    const charH = state.fontSize * 1.2;
    const cols = Math.max(20, Math.min(500, Math.floor(rect.width / charW)));
    const rows = Math.max(5, Math.min(200, Math.floor(rect.height / charH)));
    document.getElementById('spawnRows').value = rows;
    document.getElementById('spawnCols').value = cols;
    document.getElementById('autofitHint').textContent = `Panel is ${Math.floor(rect.width)}x${Math.floor(rect.height)}px → ${rows} rows × ${cols} cols`;
}

function getSelectedPanel() {
    // Find the panel STATE for the selected command's instance, then return
    // the actual DOM element (not the plain JS object).  Previously this
    // returned the object from state.panels, causing .querySelector() to
    // throw TypeError since plain objects lack that method — silently
    // preventing ALL VTTY content from ever being displayed.
    if (state.panels.length === 0) return null;
    let panelObj;
    if (!state.selectedInstUrl) {
        panelObj = state.panels[0];
    } else {
        panelObj = state.panels.find(p => p.instUrl === state.selectedInstUrl) || state.panels[0];
    }
    return document.getElementById(panelObj.id);
}

// ─── Pause/Run Toggle ───
async function togglePauseRun() {
    if (!state.selectedCmdId) return;
    const inst = state.instanceUrls.find(i => i.url === state.selectedInstUrl);
    const cmd = inst && inst._commands ? inst._commands.find(c => c.id === state.selectedCmdId) : null;
    const isFrozen = cmd && cmd.frozen;
    const endpoint = isFrozen ? 'thaw' : 'freeze';
    try {
        await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/${endpoint}`, { url: state.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: state.selectedInstUrl }),
            body: JSON.stringify({}),
        });
        loadCommands();
    } catch (e) { /* ignore */ }
}

async function togglePauseRunPanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const inst = state.instanceUrls.find(i => i.url === panelObj.instUrl);
    if (!inst || !inst._commands) return;
    // Find the command shown in this panel
    const isForSelected = (panelObj.instUrl === state.selectedInstUrl);
    const cmdId = isForSelected ? state.selectedCmdId : null;
    if (!cmdId) return;
    const cmd = inst._commands.find(c => c.id === cmdId);
    const isFrozen = cmd && cmd.frozen;
    const endpoint = isFrozen ? 'thaw' : 'freeze';
    try {
        await fetch(apiUrl(`/api/commands/${cmdId}/${endpoint}`, { url: panelObj.instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.instUrl, token: panelObj.token }),
            body: JSON.stringify({}),
        });
        loadCommands();
    } catch (e) { /* ignore */ }
}

// ─── VTTY Update Modes (Push / Poll) ───
// The web UI supports two modes for detecting VTTY buffer changes:
//
// PUSH MODE (default): The server monitors the buffer and sends lightweight
//   "vtty_dirty" signals over the WebSocket whenever the buffer changes.
//   On receiving a dirty signal, the client does a debounced HTTP fetch to
//   get the latest HTML.  This is the most efficient mode.
//
// POLL MODE: The client periodically calls GET /api/commands/:id/vtty/changed
//   to ask "has the buffer changed?".  If yes, it fetches the full HTML.
//   This mode is useful when WebSocket connections are unreliable.

/// Fetch server-side web config (update_mode, poll defaults) from /api/info.
/// Also tracks whether the server is reachable at all.
async function fetchServerConfig() {
    try {
        const res = await fetch(apiUrl('/api/info'), { headers: authHeaders() });
        const json = await res.json();
        const wasReachable = state.serverReachable;
        state.serverReachable = !!json.status;
        // Re-render panels if reachability changed (e.g. "not running" -> welcome)
        if (wasReachable !== state.serverReachable) {
            renderPanels();
            updateSidebarTabsVisibility();
        }
        if (json.status === 'ok' && json.data && json.data.web) {
            state.serverUpdateMode = json.data.web.update_mode;
            state.serverPollMs = json.data.web.default_poll_ms;
            state.serverDirtyMs = json.data.web.dirty_check_ms;
            // If no user preference is set, use the server default
            if (!localStorage.getItem('vrw_update_mode')) {
                state.updateMode = state.serverUpdateMode || 'push';
            }
            if (!localStorage.getItem('vrw_poll_interval')) {
                state.pollInterval = state.serverPollMs || 500;
            }
        }
        if (json.status === 'ok' && json.data && json.data.vtty) {
            state.serverScreenshotFontSize = json.data.vtty.screenshot_font_size || 12;
            state.serverScreenshotFontName = json.data.vtty.screenshot_font_name || 'monospace';
        }
    } catch (e) {
        const wasReachable = state.serverReachable;
        state.serverReachable = false;
        if (wasReachable !== state.serverReachable) {
            renderPanels();
            updateSidebarTabsVisibility();
        }
    }
}

/// Apply the current updateMode to the UI controls.
function applyUpdateModeUI() {
    document.getElementById('updateMode').value = state.updateMode;
    document.getElementById('pollInterval').value = state.pollInterval;
    document.getElementById('pollIntervalWrap').style.display = state.updateMode === 'poll' ? '' : 'none';
}

/// Switch update mode (called from the dropdown).
function switchUpdateMode(mode) {
    state.updateMode = mode;
    localStorage.setItem('vrw_update_mode', mode);
    applyUpdateModeUI();
    // Stop existing update mechanism and restart with new mode
    stopUpdateMode();
    if (state.selectedInstUrl && state.selectedCmdId) {
        startUpdateMode();
    }
}

/// Apply the poll interval from the input.
function applyPollInterval() {
    const val = parseInt(document.getElementById('pollInterval').value) || 500;
    state.pollInterval = Math.max(50, Math.min(5000, val));
    localStorage.setItem('vrw_poll_interval', state.pollInterval.toString());
    document.getElementById('pollInterval').value = state.pollInterval;
    // If currently polling, restart the timer with new interval
    if (state.updateMode === 'poll' && state._pollTimer) {
        stopPoll();
        startPoll();
    }
}

/// Whether the terminal content area is currently visible to the user.
/// Returns false when viewing logs/docs or when no command is selected.
function _isTerminalVisible() {
    if (state.currentView !== 'vtty') return false;
    if (!state.selectedCmdId) return false;
    return true;
}

/// Flush any buffered VTTY data that arrived while the terminal was not
/// visible or while the user was scrolling.  Called when the terminal
/// becomes visible again or when scrolling ends.
function _flushPendingVttyUpdate() {
    if (!state._pendingVttyDirty) return;
    state._pendingVttyDirty = false;
    if (state._pendingVttyData) {
        const data = state._pendingVttyData;
        state._pendingVttyData = null;
        if (data.cells && data.cells.length > 0) {
            applyVttyDiff(data);
        } else {
            updateVttyDisplay(data);
        }
    } else {
        // We know data changed but don't have a snapshot — fetch fresh
        if (state.selectedInstUrl && state.selectedCmdId) {
            loadVttyHttp(state.selectedInstUrl, state.selectedCmdId);
        }
    }
}

/// Start the active update mode (push or poll).
function startUpdateMode() {
    stopUpdateMode();
    if (state.bufferView !== 'current') return;
    if (state.updateMode === 'push') {
        connectVttyWs(state.selectedInstUrl, state.selectedCmdId);
    } else {
        startPoll();
    }
}

/// Stop the active update mode.
function stopUpdateMode() {
    disconnectVttyWs();
    stopPoll();
}

// ─── Push Mode: WebSocket ───
function connectVttyWs(instUrl, cmdId) {
    // Close existing connection if any
    disconnectVttyWs();

    const wsUrl = instUrl.replace(/^http/, 'ws');
    const token = state.authToken || (state.instanceUrls.find(i => i.url === instUrl) || {}).token || '';
    const sep = token ? '?' : '';
    const url = `${wsUrl}/api/commands/${cmdId}/ws${sep}${token ? 'token=' + encodeURIComponent(token) : ''}`;

    try {
        const ws = new WebSocket(url);
        state.vttyWs = ws;
        state.vttyWsUrl = instUrl;
        state.vttyWsCmdId = cmdId;

        ws.onopen = () => {
            document.getElementById('connStatus').textContent = 'WS Connected';
            // Start ping/pong latency measurement (every 10s)
            clearInterval(state._wsPingInterval);
            state._wsPingInterval = setInterval(() => {
                if (state.vttyWs && state.vttyWs.readyState === WebSocket.OPEN) {
                    state._wsPingSendTime = Date.now();
                    state.vttyWs.send(JSON.stringify({ type: 'ping' }));
                }
            }, 10000);
            updateWsQualityIndicator();
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                // Guard: discard messages for a command that is no longer selected.
                // This can happen if the WS was connected to command A and the user
                // switched to command B before the WS closed.
                if (msg.cmd_id && msg.cmd_id !== state.selectedCmdId) return;
                // Also guard on nested data.id — the server sends
                // {type:"vtty_full", data:{id:"...",...}} not top-level cmd_id.
                if (msg.data && msg.data.id && msg.data.id !== state.selectedCmdId) return;
                if (msg.type === 'vtty_full' && msg.data) {
                    // Initial full snapshot — buffer or apply
                    if (state.bufferView === 'current') {
                        if (_isTerminalVisible()) {
                            // Skip DOM update if refresh throttle is active —
                            // the throttle timer will fetch the latest state.
                            if (!_throttleRefresh()) {
                                updateVttyDisplay(msg.data);
                            }
                        } else {
                            state._pendingVttyData = msg.data;
                            state._pendingVttyDirty = true;
                        }
                    }
                    const selPanel = getSelectedPanel();
                    if (selPanel) {
                        const badge = document.getElementById('altScreenBadge-' + selPanel.id);
                        if (badge) badge.classList.toggle('visible', !!msg.data.alternate_screen);
                    }
                } else if (msg.type === 'vtty_diff' && msg.data) {
                    // Level 3: Incremental diff — buffer or apply
                    if (state.bufferView === 'current') {
                        if (_isTerminalVisible()) {
                            if (!_throttleRefresh()) {
                                applyVttyDiff(msg.data);
                            }
                        } else {
                            state._pendingVttyData = msg.data;
                            state._pendingVttyDirty = true;
                        }
                    }
                } else if (msg.type === 'vtty_dirty' && msg.data) {
                    // Legacy dirty signal (shouldn't arrive in Level 3 mode,
                    // but handled as fallback for older servers).
                    if (state.bufferView === 'current') {
                        if (_isTerminalVisible()) {
                            scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 50);
                        } else {
                            state._pendingVttyDirty = true;
                        }
                    }
                } else if (msg.type === 'command_ended') {
                    document.getElementById('connStatus').textContent = 'Command ended';
                    disconnectVttyWs();
                    // Browser notification on command exit
                    notifyCommandEnded(state.vttyWsCmdId);
                } else if (msg.type === 'pong') {
                    // Calculate RTT from ping/pong
                    if (state._wsPingSendTime > 0) {
                        state._wsLatency = Date.now() - state._wsPingSendTime;
                        state._wsPingSendTime = 0;
                        updateWsQualityIndicator();
                        // Also update connStatus to show latency
                        const connEl = document.getElementById('connStatus');
                        if (connEl) connEl.textContent = 'Connected (' + state._wsLatency + 'ms)';
                    }
                } else if (msg.type === 'connected') {
                    // Server confirms connection. A vtty_full follows immediately.
                } else if (msg.type === 'peer_registered' || msg.type === 'peer_unregistered') {
                    // Server-level peer notification — forward to handler
                    handlePeerEvent(msg);
                }
            } catch (e) {
                console.error('WS message parse error:', e);
            }
        };

        ws.onclose = () => {
            if (state.vttyWs === ws) {
                state.vttyWs = null;
                clearInterval(state._wsPingInterval);
                state._wsPingInterval = null;
                state._wsPingSendTime = 0;
                state._wsLatency = 0;
                document.getElementById('connStatus').textContent = 'WS Disconnected';
                updateWsQualityIndicator();
                // Mark instance as potentially unreachable when WS drops
                if (state.vttyWsUrl) {
                    const wsInst = state.instanceUrls.find(i => i.url === state.vttyWsUrl);
                    if (wsInst && wsInst.reachable) {
                        // Don't immediately mark unreachable — the server might just
                        // have closed this particular WS.  A failed /api/commands
                        // fetch in the next loadCommands() cycle will confirm it.
                        // But bump reconnect count so we stop retrying aggressively.
                    }
                }
                // When WebSocket disconnects, schedule an HTTP fetch to keep display alive
                if (state.selectedInstUrl && state.selectedCmdId) {
                    scheduleVttyHttp(state.selectedInstUrl, state.selectedCmdId, 0);
                }
                // Auto-reconnect after 2 seconds if the command is still selected and alive
                // Cap reconnect attempts to avoid hammering a dead server
                if (state.selectedInstUrl && state.selectedCmdId && !state._wsReconnectTimer) {
                    state._wsReconnectCount++;
                    if (state._wsReconnectCount <= 5) {
                        state._wsReconnectTimer = setTimeout(() => {
                            state._wsReconnectTimer = null;
                            if (state.selectedInstUrl && state.selectedCmdId && state.updateMode === 'push') {
                                // Only reconnect if the instance is still reachable
                                const inst = state.instanceUrls.find(i => i.url === state.selectedInstUrl);
                                if (inst && inst.reachable !== false) {
                                    connectVttyWs(state.selectedInstUrl, state.selectedCmdId);
                                }
                            }
                        }, 2000);
                    }
                }
            }
        };

        ws.onerror = (err) => {
            console.error('WebSocket error:', err);
            document.getElementById('connStatus').textContent = 'WS Error';
        };
    } catch (e) {
        console.error('WebSocket connect failed:', e);
    }
}

function disconnectVttyWs() {
    if (state._wsReconnectTimer) {
        clearTimeout(state._wsReconnectTimer);
        state._wsReconnectTimer = null;
    }
    clearInterval(state._wsPingInterval);
    state._wsPingInterval = null;
    state._wsPingSendTime = 0;
    state._wsLatency = 0;
    state._wsReconnectCount = 0;
    if (state.vttyWs) {
        state.vttyWs.onclose = null; // prevent re-entry
        state.vttyWs.close();
        state.vttyWs = null;
        state.vttyWsUrl = null;
        state.vttyWsCmdId = null;
    }
    updateWsQualityIndicator();
}

// ─── WebSocket Connection Quality Indicator ───
function updateWsQualityIndicator() {
    const el = document.getElementById('wsQuality');
    if (!el) return;

    const latency = state._wsLatency;
    const reconnects = state._wsReconnectCount;
    const isConnected = state.vttyWs && state.vttyWs.readyState === WebSocket.OPEN;

    if (!isConnected && latency === 0) {
        el.textContent = '--';
        el.style.color = 'var(--red)';
        el.title = 'Disconnected';
        return;
    }

    let color;
    if (latency === 0) {
        // Connected but no measurement yet
        color = 'var(--text-muted)';
    } else if (latency < 50) {
        color = 'var(--green)';
    } else if (latency < 200) {
        color = 'var(--yellow)';
    } else {
        color = 'var(--red)';
    }

    el.textContent = latency > 0 ? latency + 'ms' : '...';
    el.style.color = color;
    el.title = 'Latency: ' + (latency > 0 ? latency + 'ms' : 'measuring...') + ' | Reconnects: ' + reconnects;
}

// ─── Poll Mode ───
function startPoll() {
    stopPoll();
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    state._pollTimer = setInterval(() => pollOnce(), state.pollInterval);
    // Also poll immediately
    pollOnce();
}

function stopPoll() {
    if (state._pollTimer) {
        clearInterval(state._pollTimer);
        state._pollTimer = null;
    }
}

async function pollOnce() {
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    // Skip fetching when terminal is not visible
    if (!_isTerminalVisible()) return;
    const cmdId = state.selectedCmdId;
    const instUrl = state.selectedInstUrl;
    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/vtty/changed`, { url: instUrl }), { headers: authHeadersForInstance({ url: instUrl }) });
        const json = await res.json();
        if (json.status === 'ok' && json.data && json.data.changed) {
            loadVttyHttp(instUrl, cmdId);
        }
    } catch (e) {
        // Silently ignore — next poll will retry
    }
}

function updateVttyDisplay(data) {
    // Pause DOM updates while the user is actively scrolling
    if (state._userScrolling) {
        state._pendingVttyData = data;
        state._pendingVttyDirty = true;
        return;
    }
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    // Level 2: Skip redundant DOM updates if generation hasn't changed.
    const cmdId = state.selectedCmdId;
    if (cmdId && data.generation !== undefined) {
        if (state._lastGeneration[cmdId] === data.generation) {
            // Generation unchanged — only update metadata, skip DOM replacement
            updateVttyMetadata(data, panel, vttyEl);
            return;
        }
        state._lastGeneration[cmdId] = data.generation;
    }

    if (data.html !== undefined && data.html !== null) {
        // Level 1: Save scroll position before innerHTML replacement
        const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
        const oldScrollHeight = vttyEl.scrollHeight;

        pre.innerHTML = data.html;

        // Level 3: Rebuild cell grid after full HTML replacement
        if (state._level3Enabled && data.dimensions) {
            buildCellGrid(cmdId, pre, data.dimensions.rows, data.dimensions.cols);
        }

        // Level 1: Restore scroll position after DOM replacement.
        // If user was at bottom, snap to new bottom (auto-scroll).
        // Otherwise, adjust for content height change to maintain view position.
        if (wasAtBottom) {
            vttyEl.scrollTop = vttyEl.scrollHeight;
        } else {
            vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
        }
    }

    updateVttyMetadata(data, panel, vttyEl);
}

/// Update cursor, dimensions, mouse state, etc. without touching the DOM content.
/// Called both after innerHTML replacement and when generation is unchanged (skip path).
function updateVttyMetadata(data, panel, vttyEl) {
    // Cursor position
    const cursor = data.cursor || {};
    const dims = data.dimensions || {};
    document.getElementById('cursorPos').textContent = `Cursor: ${cursor.row + 1},${cursor.col + 1}`;
    document.getElementById('termDims').textContent = `${dims.rows}x${dims.cols}`;

    // Show cursor indicator (hide when in scrollback or app hid it via ?25l)
    const panelObj = state.panels.find(p => p.id === panel.id);
    const inScrollback = panelObj && panelObj.scrollbackOffset > 0;
    const cursorHidden = data.cursor_visible === false;
    const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
    if (cursorEl && cursor.row !== undefined && !inScrollback && !cursorHidden) {
        const charW = state.fontSize * 0.6;
        const charH = state.fontSize * 1.2;
        cursorEl.style.top = (cursor.row * charH) + 'px';
        cursorEl.style.left = (cursor.col * charW) + 'px';
        cursorEl.style.width = charW + 'px';
        cursorEl.style.height = charH + 'px';
        cursorEl.style.display = '';
    } else if (cursorEl) {
        cursorEl.style.display = 'none';
    }

    // Track mouse state from the server response
    if (panelObj) {
        panelObj.mouseTracking = !!data.mouse_tracking;
        panelObj.mouseSgr = !!data.mouse_sgr;
    }

    // Toggle selectable class on vtty container (enable text selection when mouse tracking is off)
    if (vttyEl) {
        const mt = panelObj ? panelObj.mouseTracking : false;
        vttyEl.classList.toggle('selectable', !mt);
        // Store dimensions on <pre> for screenshot filename generation
        const pre = vttyEl.querySelector('pre');
        if (pre && dims.rows && dims.cols) {
            pre._vttyRows = dims.rows;
            pre._vttyCols = dims.cols;
        }
    }

    state._termRows = dims.rows;
    state._termCols = dims.cols;
}

// ─── Level 3: Cell Grid for Incremental DOM Patching ───
// Builds a 2D array of span element references from the <pre> DOM tree,
// indexed as grid[row][col]. Each row is terminated by a \n text node in
// the HTML produced by VttyRenderer::to_html().
//
// This grid enables O(1) lookup for any (row, col) cell, allowing
// applyVttyDiff() to patch individual cells without destroying the entire
// DOM tree (no innerHTML replacement).

function buildCellGrid(cmdId, pre, rows, cols) {
    const grid = [];
    let currentRow = [];
    for (const child of pre.childNodes) {
        if (child.nodeType === Node.TEXT_NODE) {
            // Text nodes with only whitespace/newline mark row boundaries.
            // The server's to_html() emits a single '\n' between rows.
            if (child.textContent.includes('\n')) {
                // Split by newlines — each \n ends a row
                const parts = child.textContent.split('\n');
                for (let i = 0; i < parts.length - 1; i++) {
                    if (currentRow.length > 0 || i > 0) {
                        grid.push(currentRow);
                        currentRow = [];
                    }
                }
                // Trailing text (if any) is part of the next row — but there
                // shouldn't be any in the server's output format.
            }
        } else if (child.nodeType === Node.ELEMENT_NODE && child.tagName === 'SPAN') {
            // Server uses RLE: a single span may contain multiple characters.
            // Expand into per-cell entries for the cell grid.
            const text = child.textContent;
            // Use Array.from to iterate code points, not UTF-16 code units.
            // Supplementary-plane emoji (e.g. 😊) are 2 UTF-16 code units
            // but 1 terminal cell; indexing by code unit breaks the grid.
            const chars = Array.from(text);
            for (let i = 0; i < chars.length; i++) {
                currentRow.push({ span: child, idx: i, len: chars.length });
            }
        }
    }
    // Push the last row
    if (currentRow.length > 0) {
        grid.push(currentRow);
    }

    state._cellGrids[cmdId] = { grid, rows, cols };
}

// Generate the inline style string for a cell, matching the server's
// VttyRenderer::to_html() format exactly. This ensures visual consistency
// between full HTML replacement and incremental diff patching.
function _cellStyle(diff) {
    let fg = diff.fg;
    let bg = diff.bg;

    // Handle reverse video: swap fg and bg
    if (diff.reverse) {
        [fg, bg] = [bg, fg];
    }

    // Width in ch units: matches server-side run_len * cell_ch.
    // For single-cell updates (diff patching), run_len is always 1.
    const cellW = diff.width || 1;
    let style = 'width:' + (cellW > 0 ? cellW + 'ch' : '0') + ';color:#' + _hex(fg[0]) + _hex(fg[1]) + _hex(fg[2]) + ';background:#' + _hex(bg[0]) + _hex(bg[1]) + _hex(bg[2]);

    if (diff.bold) style += ';font-weight:bold';
    if (diff.italic) style += ';font-style:italic';
    if (diff.underline && diff.strikethrough) {
        style += ';text-decoration:underline line-through';
    } else if (diff.underline) {
        style += ';text-decoration:underline';
    } else if (diff.strikethrough) {
        style += ';text-decoration:line-through';
    }
    if (diff.blink) style += ';animation:blink 1s step-end infinite';

    return style;
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

/// Split a merged (RLE) span at the target cell position so that cell
/// gets its own individual <span> element.  Updates the cell grid entries
/// for all affected positions (before, target, and after the split).
///
/// Before: <span class="c w1" style="width:5ch;color:...">ABCDE</span>
///                         ^--- target at idx=2 (cell 'C')
/// After:  <span class="c w1" style="width:2ch;color:...">AB</span><span class="c w1" style="width:1ch;color:new">C'</span><span class="c w1" style="width:2ch;color:...">DE</span>
function _splitAndUpdateCell(cg, row, col, diff) {
    const entry = cg.grid[row][col];
    if (!entry || entry.len <= 1) return;

    const span = entry.span;
    const idx = entry.idx;
    const text = span.textContent;
    const origStyle = span.getAttribute('style') || '';
    const origClass = span.getAttribute('class') || 'c';
    // Determine cell width from class: w0→0, w1→1, w2→2
    const cellCh = origClass.includes('w2') ? 2 : origClass.includes('w0') ? 0 : 1;
    // Use Array.from for code point-aware splitting
    const chars = Array.from(text);

    // Characters before the target
    const before = chars.slice(0, idx).join('');
    const beforeLen = before.length; // code point count
    // Characters after the target
    const after = chars.slice(idx + 1).join('');
    const afterLen = after.length;

    // Helper: rebuild style with correct width for a given character count
    function _rebuildStyle(orig, charCount) {
        // Remove leading width:Nch or width:0 from origStyle
        const stripped = orig.replace(/^width:[^;]*;?/, '');
        const w = charCount * cellCh;
        return 'width:' + (w > 0 ? w + 'ch' : '0') + ';' + stripped;
    }

    // Create "after" span if there are trailing characters
    if (after.length > 0) {
        const afterSpan = document.createElement('span');
        afterSpan.className = origClass;
        afterSpan.setAttribute('style', _rebuildStyle(origStyle, afterLen));
        afterSpan.textContent = after;
        span.parentNode.insertBefore(afterSpan, span.nextSibling);
        // Update grid entries for characters after the target
        for (let k = col + 1; k < cg.grid[row].length; k++) {
            const e = cg.grid[row][k];
            if (e && e.span === span && e.idx > idx) {
                e.span = afterSpan;
                e.idx = e.idx - idx - 1;
                e.len = afterSpan.textContent.length;
            }
        }
    }

    // Create "before" span if there are leading characters
    if (before.length > 0) {
        const beforeSpan = document.createElement('span');
        beforeSpan.className = origClass;
        beforeSpan.setAttribute('style', _rebuildStyle(origStyle, beforeLen));
        beforeSpan.textContent = before;
        span.parentNode.insertBefore(beforeSpan, span);
        // Update grid entries for characters before the target
        for (let k = col - 1; k >= 0; k--) {
            const e = cg.grid[row][k];
            if (e && e.span === span && e.idx < idx) {
                e.span = beforeSpan;
                e.len = beforeSpan.textContent.length;
            }
        }
    }

    // Update the target cell in place
    const ch = diff.width === 0 ? '\u200b' : (diff.ch === '\u0000' ? ' ' : diff.ch);
    span.textContent = _htmlEscapeChar(ch);
    span.setAttribute('style', _cellStyle(diff));
    const wCls = diff.width === 0 ? 'c w0' : diff.width === 2 ? 'c w2' : 'c w1';
    span.className = wCls;

    // Update grid entry for the target cell
    entry.len = 1;
    entry.idx = 0;
}
function applyVttyDiff(data) {
    // Pause DOM updates while the user is actively scrolling
    if (state._userScrolling) {
        state._pendingVttyData = data;
        state._pendingVttyDirty = true;
        return;
    }
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    const cmdId = state.selectedCmdId;
    if (!cmdId) return;

    // Level 2: Skip if generation unchanged
    if (data.generation !== undefined && state._lastGeneration[cmdId] === data.generation) {
        updateVttyMetadata(data, panel, vttyEl);
        return;
    }
    if (data.generation !== undefined) {
        state._lastGeneration[cmdId] = data.generation;
    }

    // Check if we have a cell grid for this command
    const cg = state._cellGrids[cmdId];
    if (!cg || !data.cells || !data.cells.length) {
        // No grid or no cells — fall back to full HTML fetch
        scheduleVttyHttp(state.selectedInstUrl, cmdId, 0);
        return;
    }

    // Check for dimension mismatch — if dimensions changed, we need a full resync
    const dims = data.dimensions || {};
    if (dims.rows !== cg.rows || dims.cols !== cg.cols) {
        // Dimensions changed — fall back to full HTML fetch
        delete state._cellGrids[cmdId];
        scheduleVttyHttp(state.selectedInstUrl, cmdId, 0);
        return;
    }

    // Save scroll position (Level 1)
    const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
    const oldScrollHeight = vttyEl.scrollHeight;

    // Apply each cell diff
    for (let i = 0; i < data.cells.length; i++) {
        const c = data.cells[i];
        if (c.row < cg.grid.length && c.col < cg.grid[c.row].length) {
            const entry = cg.grid[c.row][c.col];
            if (entry) {
                // Cell grid entries are { span, idx, len } objects from RLE expansion.
                // If the span contains only this cell (len===1), update directly.
                // Otherwise, split the merged span so this cell gets its own element.
                if (entry.len === 1) {
                    // Fast path: single-char span — update directly
                    // width=0 → wide-char continuation (zero-width space).
                    // width=1 with space → normal empty cell (actual space).
                    const ch = c.width === 0 ? '\u200b' : (c.ch === '\u0000' ? ' ' : c.ch);
                    entry.span.textContent = _htmlEscapeChar(ch);
                    entry.span.setAttribute('style', _cellStyle(c));
                    // Update width class to match new cell width
                    const wCls = c.width === 0 ? 'c w0' : c.width === 2 ? 'c w2' : 'c w1';
                    entry.span.className = wCls;
                } else {
                    // Slow path: split the merged span at the target position.
                    _splitAndUpdateCell(cg, c.row, c.col, c);
                }
            }
        }
    }

    // Level 1: Restore scroll position
    if (wasAtBottom) {
        vttyEl.scrollTop = vttyEl.scrollHeight;
    } else {
        vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
    }

    // Update metadata (cursor, dimensions, etc.)
    updateVttyMetadata(data, panel, vttyEl);
}

// ─── Debounced VTTY HTTP Fetch ───
// Prevents request flooding when multiple code paths (dirty signals, onclose,
// periodic refresh, sendKeys) all want to refresh the VTTY display.
// Only the last call within the debounce window actually fires.
function scheduleVttyHttp(instUrl, cmdId, delayMs) {
    // Skip scheduling when terminal is not visible
    if (!_isTerminalVisible()) return;
    if (state._vttyHttpTimer) clearTimeout(state._vttyHttpTimer);
    state._vttyHttpTimer = setTimeout(() => {
        state._vttyHttpTimer = null;
        loadVttyHttp(instUrl, cmdId);
    }, delayMs);
}

/// Pre-fetch VTTY HTML for instant initial display.
/// Unlike loadVttyHttp, this does NOT check generation (first load, no cache)
/// and does NOT defer to pending state.  It writes directly into the <pre>.
async function _prefetchVttyHtml(instUrl, cmdId) {
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/vtty/html`, { url: instUrl }),
            { headers: authHeadersForInstance({ url: instUrl }) });
        if (!res.ok) return;
        const json = await res.json();
        if (json.status === 'ok' && json.data && json.data.html !== undefined) {
            pre.innerHTML = json.data.html;
            // Store generation for subsequent incremental updates
            if (json.data.generation !== undefined) {
                state._lastGeneration[cmdId] = json.data.generation;
            }
            // Build cell grid for Level 3 incremental diffing
            if (state._level3Enabled && json.data.dimensions) {
                buildCellGrid(cmdId, pre, json.data.dimensions.rows, json.data.dimensions.cols);
            }
            // Update metadata (cursor, dimensions, etc.)
            updateVttyMetadataFromHttp(json.data, panel,
                state.panels.find(p => p.id === panel.id), 0);
            // Start the push/poll update mode now that initial content is displayed
            startUpdateMode();
        }
    } catch (e) {
        console.error('Failed to pre-fetch VTTY HTML:', e);
    }
}

async function loadVttyHttp(instUrl, cmdId) {
    const panel = getSelectedPanel();
    if (!panel) return;

    // Get panel state for scrollback offset
    const panelObj = state.panels.find(p => p.id === panel.id);
    const sbOffset = panelObj ? panelObj.scrollbackOffset : 0;

    // If viewing a specific buffer, use the buffer endpoint
    let endpoint;
    if (state.bufferView !== 'current') {
        const screenParam = `?screen=${state.bufferView}`;
        endpoint = `/api/commands/${cmdId}/vtty/buffer${screenParam}`;
    } else if (sbOffset > 0) {
        endpoint = `/api/commands/${cmdId}/vtty/html?scrollback_offset=${sbOffset}`;
    } else {
        endpoint = `/api/commands/${cmdId}/vtty/html`;
    }

    try {
        const res = await fetch(apiUrl(endpoint, { url: instUrl }), { headers: authHeadersForInstance({ url: instUrl }) });
        if (!res.ok) {
            console.warn('VTTY HTTP fetch failed:', res.status, res.statusText);
            return;
        }
        const json = await res.json();
        if (json.status === 'ok' && json.data) {
            // Level 2: Skip redundant DOM updates if generation hasn't changed.
            if (json.data.generation !== undefined && state._lastGeneration[cmdId] === json.data.generation) {
                // Only update metadata (cursor position, dimensions, etc.)
                updateVttyMetadataFromHttp(json.data, panel, panelObj, sbOffset);
                return;
            }
            if (json.data.generation !== undefined) {
                state._lastGeneration[cmdId] = json.data.generation;
            }

            const vttyEl = panel.querySelector('.vtty-container');
            const pre = vttyEl ? vttyEl.querySelector('pre') : null;
            if (pre && json.data.html !== undefined) {
                // Pause DOM updates while the user is actively scrolling
                if (state._userScrolling) {
                    state._pendingVttyData = json.data;
                    state._pendingVttyDirty = true;
                    return;
                }
                // Level 1: Save scroll position before innerHTML replacement
                const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
                const oldScrollHeight = vttyEl.scrollHeight;

                pre.innerHTML = json.data.html;

                // Level 3: Rebuild cell grid after full HTML replacement
                if (state._level3Enabled && json.data.dimensions) {
                    buildCellGrid(cmdId, pre, json.data.dimensions.rows, json.data.dimensions.cols);
                } else {
                    // Clear stale grid if dimensions not available
                    delete state._cellGrids[cmdId];
                }

                // Level 1: Restore scroll position.
                // Only auto-scroll when user was viewing the bottom.
                if (wasAtBottom) {
                    vttyEl.scrollTop = vttyEl.scrollHeight;
                } else {
                    vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
                }
            }

            updateVttyMetadataFromHttp(json.data, panel, panelObj, sbOffset);
        }
    } catch (e) {
        console.error('Failed to load VTTY:', e);
    }
}

/// Update cursor, dimensions, mouse state, alt screen badge, and scrollback indicator
/// from an HTTP response, without touching the DOM content. Shared by both the
/// generation-skip path and the full-update path in loadVttyHttp.
function updateVttyMetadataFromHttp(data, panel, panelObj, sbOffset) {
    const vttyEl = panel.querySelector('.vtty-container');
    const cursor = data.cursor || {};
    const dims = data.dimensions || {};
    document.getElementById('cursorPos').textContent = `Cursor: ${(cursor.row + 1) || '-'},${(cursor.col + 1) || '-'}`;
    document.getElementById('termDims').textContent = `${dims.rows || '-'}x${dims.cols || '-'}`;

    // Update alt screen badge
    const badge = document.getElementById('altScreenBadge-' + panel.id);
    if (badge) {
        badge.classList.toggle('visible', !!data.alternate_screen);
    }

    // Update mouse state
    if (panelObj) {
        panelObj.mouseTracking = !!data.mouse_tracking;
        panelObj.mouseSgr = !!data.mouse_sgr;
    }

    // Toggle selectable class on vtty container (enable text selection when mouse tracking is off)
    if (vttyEl) {
        const mt = panelObj ? panelObj.mouseTracking : false;
        vttyEl.classList.toggle('selectable', !mt);
        // Store dimensions on <pre> for screenshot filename generation
        const pre = vttyEl.querySelector('pre');
        if (pre && dims.rows && dims.cols) {
            pre._vttyRows = dims.rows;
            pre._vttyCols = dims.cols;
        }
    }

    // Hide cursor when in scrollback view or app hid it via ?25l
    const cursorVisible = data.cursor_visible !== false;
    const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
    if (cursorEl) {
        if (sbOffset > 0 || !cursorVisible) {
            cursorEl.style.display = 'none';
        } else {
            cursorEl.style.display = '';
        }
    }

    // Show/hide scrollback indicator in bottom bar
    const sbIndicator = document.getElementById('scrollbackIndicator');
    if (sbIndicator) {
        sbIndicator.style.display = sbOffset > 0 ? '' : 'none';
    }
}

function switchBuffer(view) {
    state.bufferView = view;
    if (!state.selectedCmdId) return;

    // Reset scrollback when switching buffer views
    state.panels.forEach(p => p.scrollbackOffset = 0);
    // Clear stored scrollback since we reset
    sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);

    if (view === 'current') {
        // Re-enable the active update mode for live updates
        startUpdateMode();
    } else {
        // Disconnect WS / stop poll — we're viewing a static snapshot
        stopUpdateMode();
        loadVttyHttp(state.selectedInstUrl, state.selectedCmdId);
    }
}

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

async function spawnCommand() {
    const cmd = document.getElementById('spawnCmd').value.trim();
    if (!cmd) return;
    const argsStr = document.getElementById('spawnArgs').value.trim();
    // Parse arguments with support for quoted strings (double and single quotes)
    const args = parseSpawnArgs(argsStr);
    const cert = document.getElementById('spawnCert').value || null;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect.value;

    // Terminal size from spawn form (optional, use server defaults if empty)
    const body = { cmd, args, certificate: cert };
    const rows = parseInt(document.getElementById('spawnRows').value);
    const cols = parseInt(document.getElementById('spawnCols').value);
    if (rows > 0) body.rows = rows;
    if (cols > 0) body.cols = cols;

    // Working directory (optional)
    const dir = document.getElementById('spawnDir').value.trim();
    if (dir) body.dir = dir;

    // Retain on exit (optional)
    if (document.getElementById('spawnRetainOnExit').checked) {
        body.retain_on_exit = true;
    }

    try {
        const res = await fetch(apiUrl('/api/commands', { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl }),
            body: JSON.stringify(body),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            document.getElementById('spawnCmd').value = '';
            document.getElementById('spawnArgs').value = '';
            document.getElementById('spawnDir').value = '';
            document.getElementById('spawnRows').value = '';
            document.getElementById('spawnCols').value = '';
            document.getElementById('spawnRetainOnExit').checked = false;
            // Auto-select the newly spawned command so its terminal output appears
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                // Disconnect the old WS FIRST to prevent stale vtty_full
                // messages from the previous command overwriting the cleared terminal.
                disconnectVttyWs();
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    } catch (e) {
        alert('Spawn failed: ' + e.message);
    }
}

async function killCommand(instUrl, cmdId) {
    // Force full rebuild on state transition
    _lastCommandState = '';
    try {
        await fetch(apiUrl(`/api/commands/${cmdId}/kill`, { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl }),
            body: JSON.stringify({}),
        });
        if (state.selectedInstUrl === instUrl && state.selectedCmdId === cmdId) {
            state.selectedInstUrl = null;
            state.selectedCmdId = null;
        }
        loadCommands();
    } catch (e) { /* ignore */ }
}

async function purgeCommand(instUrl, cmdId, cmdName) {
    // Force full rebuild on state transition
    _lastCommandState = '';
    if (!confirm(`Purge "${cmdName || cmdId}"?\nThis permanently discards the VTTY buffer and all associated state.`)) return;
    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}`, { url: instUrl }), {
            method: 'DELETE',
            headers: authHeadersForInstance({ url: instUrl }),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            if (state.selectedInstUrl === instUrl && state.selectedCmdId === cmdId) {
                state.selectedInstUrl = null;
                state.selectedCmdId = null;
            }
            // Clear the VTTY display
            const panel = getSelectedPanel();
            if (panel) {
                const pre = panel.querySelector('.vtty-container pre');
                if (pre) pre.innerHTML = '';
                const nameEl = panel.querySelector('.cmd-fullname');
                if (nameEl) nameEl.textContent = '';
                const argsEl = panel.querySelector('.cmd-args');
                if (argsEl) argsEl.textContent = '';
            }
            loadCommands();
        } else {
            alert('Purge failed: ' + (json.error || 'Unknown error'));
        }
    } catch (e) {
        alert('Purge failed: ' + e.message);
    }
}

async function killAllCommands() {
    if (!confirm('Kill all running commands?')) return;
    // Force full rebuild on state transition
    _lastCommandState = '';
    const promises = [];
    for (const inst of state.instanceUrls) {
        for (const cmd of (inst._commands || [])) {
            if (cmd.alive) {
                promises.push(
                    fetch(apiUrl(`/api/commands/${cmd.id}/kill`, { url: inst.url }), {
                        method: 'POST',
                        headers: authHeadersForInstance({ url: inst.url }),
                        body: JSON.stringify({}),
                    }).catch(() => {})
                );
            }
        }
    }
    // Clear cached commands immediately so the sidebar empties
    for (const inst of state.instanceUrls) {
        inst._commands = [];
    }
    state.selectedInstUrl = null;
    state.selectedCmdId = null;
    // Render the empty sidebar immediately — do NOT call loadCommands() yet
    // because the server may not have processed all kills, causing stale data.
    const container = document.getElementById('commandList');
    if (container) {
        container.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No running commands</div>';
    }
    // Show empty terminal (not the welcome/spawn panel)
    _showingWelcome = false;
    renderPanels();
    // Wait for all kill requests to complete
    await Promise.all(promises);
    // Re-fetch from server after a delay to let the backend process kills
    setTimeout(() => { _lastCommandState = ''; loadCommands(); }, 1500);
}

async function sendKeys() {
    // Delegate to the per-panel sendKeysToPanel using the selected panel
    const panel = getSelectedPanel();
    if (!panel) return;
    await sendKeysToPanel(panel.id);
}

async function resizeTerminal() {
    if (!state.selectedCmdId) return;
    const panelId = state.panels.find(p => p.id && p.instUrl === state.selectedInstUrl)?.id;
    if (panelId) { resizeTerminalPanel(panelId); return; }
    // Fallback: use old global elements if present (backward compat)
    const rows = parseInt(document.getElementById('resizeRows')?.value) || 24;
    const cols = parseInt(document.getElementById('resizeCols')?.value) || 80;
    try {
        await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/resize`, { url: state.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: state.selectedInstUrl }),
            body: JSON.stringify({ rows, cols }),
        });
    } catch (e) { /* ignore */ }
}

async function resizeTerminalPanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    // Use the globally selected command when the panel matches the selected instance
    const cmdId = (panelObj.instUrl === state.selectedInstUrl) ? state.selectedCmdId : null;
    if (!cmdId) return;
    const rows = parseInt(document.getElementById('resizeRows-' + panelId)?.value) || 24;
    const cols = parseInt(document.getElementById('resizeCols-' + panelId)?.value) || 80;
    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/resize`, { url: panelObj.instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.instUrl, token: panelObj.token }),
            body: JSON.stringify({ rows, cols }),
        });
        if (res.ok) {
            const ri = document.getElementById('resizeRows-' + panelId);
            const ci = document.getElementById('resizeCols-' + panelId);
            if (ri) ri.value = rows;
            if (ci) ci.value = cols;
        }
    } catch (e) { /* ignore */ }
}

function switchBufferPanel(panelId, view) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    // Update the select element
    const sel = document.getElementById('bufferSelect-' + panelId);
    if (sel) sel.value = view;
    // If this is the currently selected panel, apply the buffer switch
    if (panelObj.instUrl === state.selectedInstUrl && state.selectedCmdId) {
        state.bufferView = view;
        state.panels.forEach(p => p.scrollbackOffset = 0);
        sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
        if (view === 'current') {
            startUpdateMode();
        } else {
            stopUpdateMode();
            loadVttyHttp(panelObj.instUrl, state.selectedCmdId);
        }
    }
}

// ─── Certificates ───
async function loadCertificates() {
    for (const inst of state.instanceUrls) {
        try {
            const res = await fetch(apiUrl('/api/certificates', inst), { headers: authHeadersForInstance(inst) });
            const json = await res.json();
            inst._certs = json.status === 'ok' ? json.data : [];
        } catch (e) {
            inst._certs = [];
        }
    }

    const container = document.getElementById('certList');
    let html = '';
    for (const inst of state.instanceUrls) {
        html += `<div style="font-size:0.7rem;color:var(--text-muted);padding:0.3rem 0;margin-top:0.3rem;">${escHtml(inst.label)}</div>`;
        const certs = inst._certs || [];
        if (certs.length === 0) {
            html += '<div style="padding:0.3rem;font-size:0.8rem;color:var(--text-muted);">No certificates</div>';
        }
        for (const cert of certs) {
            html += `<div style="padding:0.3rem 0.5rem;border-bottom:1px solid var(--border);font-size:0.8rem;">
                <span class="cert-badge">${escHtml(cert.name)}</span>
                <span style="color:var(--text-muted);font-size:0.7rem;margin-left:0.5rem;font-family:var(--font-mono);">${escHtml(cert.token_preview || '')}...</span>
            </div>`;
        }
    }
    container.innerHTML = html;

    // Update spawn cert dropdown
    updateCertDropdown();
}

function updateCertDropdown() {
    const select = document.getElementById('spawnCert');
    let html = '<option value="">None</option>';
    for (const inst of state.instanceUrls) {
        for (const cert of (inst._certs || [])) {
            html += `<option value="${escHtml(cert.name)}">${escHtml(inst.label)}: ${escHtml(cert.name)}</option>`;
        }
    }
    select.innerHTML = html;
}

function updateInstanceDropdown() {
    const select = document.getElementById('spawnInstance');
    const current = select.value;
    let html = '';
    for (const inst of state.instanceUrls) {
        html += `<option value="${escHtml(inst.url)}">${escHtml(inst.label)} (${escHtml(inst.url.replace(/^https?:\/\//, ''))})</option>`;
    }
    select.innerHTML = html;
    if (current) select.value = current;
}

// ─── Panels (Multi-view) ───
function addPanelDirect(instUrl, label, token) {
    const id = 'panel-' + Date.now() + '-' + Math.random().toString(36).slice(2, 6);
    const savedFontSize = parseInt(localStorage.getItem('vrw_panel_font_' + id));
    const fontSize = (savedFontSize >= 8 && savedFontSize <= 28) ? savedFontSize : state.fontSize;
    const savedSelMode = localStorage.getItem('vrw_panel_sel_' + id);
    const selectionMode = savedSelMode === 'true';
    const savedTheme = localStorage.getItem('vrw_panel_theme_' + id);
    // Per-panel theme: 'light', 'dark', or '' (inherit global). Default is inherit.
    const theme = (savedTheme === 'light' || savedTheme === 'dark') ? savedTheme : '';
    const panel = { id, instUrl, label, token, scrollbackOffset: 0, mouseTracking: false, mouseSgr: false, focused: false, fontSize, selectionMode, theme };
    state.panels.push(panel);
    renderPanels();
    return panel;
}

function addPanel() {
    const modal = document.getElementById('panelModal');
    modal.style.display = '';
    document.getElementById('panelUrl').value = 'http://localhost:9090';
    document.getElementById('panelLabel').value = '';
    document.getElementById('panelToken').value = '';
    // Trap focus inside the modal and auto-focus the URL input
    const modalInner = modal.querySelector('.modal');
    if (modalInner) trapFocus(modalInner);
    document.getElementById('panelUrl').focus();
}

function closePanelModal() {
    releaseCurrentFocusTrap();
    document.getElementById('panelModal').style.display = 'none';
}

function confirmAddPanel() {
    const url = document.getElementById('panelUrl').value.trim();
    const label = document.getElementById('panelLabel').value.trim() || new URL(url).host;
    const token = document.getElementById('panelToken').value.trim();

    if (!url) return;

    // Check if we already have this instance
    if (state.instanceUrls.some(i => i.url === url)) {
        alert('This instance is already connected.');
        closePanelModal();
        return;
    }

    const inst = { url, label, token };
    state.instanceUrls.push(inst);
    addPanelDirect(url, label, token);
    closePanelModal();
    loadCommands();
    loadCertificates();
    fetchServerTemplates();
}

function removePanel(id) {
    state.panels = state.panels.filter(p => p.id !== id);
    // Remove from instanceUrls if no more panels for it
    const remainingUrls = new Set(state.panels.map(p => p.instUrl));
    state.instanceUrls = state.instanceUrls.filter(i => remainingUrls.has(i.url));
    renderPanels();
}

function renderPanels() {
    const container = document.getElementById('view-vtty');
    let html = '';
    const hasMultiplePanels = state.panels.length > 1;

    // Check if there are any commands at all for the welcome state
    let hasAnyCommands = false;
    for (const inst of state.instanceUrls) {
        if (inst._commands && inst._commands.length > 0) {
            hasAnyCommands = true;
            break;
        }
    }

    if (state.panels.length === 1 && !hasAnyCommands && !state.selectedCmdId && !state.serverReachable) {
        _showingWelcome = true;
        // Server is unreachable — vrw is not running
        html += `
            <div class="welcome-panel">
                <div class="welcome-card">
                    <img src="/favicon.png" alt="vrw" style="height:2rem;width:auto;margin-bottom:0.75rem;">
                    <p class="welcome-not-running">vrw is not running</p>
                    <p style="margin-top:0.25rem;">No vrw instance could be reached at <span class="welcome-url">${escHtml(getBaseUrl())}</span></p>
                    <p>Start vrw and refresh this page to connect.</p>
                </div>
            </div>`;
    } else {
        _showingWelcome = false;
        for (const panel of state.panels) {
            const inst = state.instanceUrls.find(i => i.url === panel.instUrl);
            const resizeHandle = hasMultiplePanels ? `<div class="panel-resize-handle" data-panel="${panel.id}"></div>` : '';
            const dragHandle = hasMultiplePanels ? `<span class="drag-handle" draggable="true" ondragstart="onPanelDragStart(event,'${panel.id}')" title="Drag to reorder">&#x2840;</span>` : '';
            html += `
                <div class="panel" id="${panel.id}" draggable="${hasMultiplePanels}" ondragover="onPanelDragOver(event)" ondrop="onPanelDrop(event,'${panel.id}')" ondragend="onPanelDragEnd(event)" ondragleave="onPanelDragLeave(event)" style="flex: 1 1 0;">
                    <div class="panel-header" data-panel-id="${panel.id}" oncontextmenu="showPanelContextMenu(event,'${panel.id}')" tabindex="0" role="button" aria-label="Panel options for ${escHtml(inst ? inst.label : panel.instUrl)}">
                        ${dragHandle}
                        <div class="cmd-info" id="cmdInfo-${panel.id}">
                            <span class="cmd-fullname" id="cmdName-${panel.id}"></span>
                            <span class="cmd-args" id="cmdArgs-${panel.id}"></span>
                        </div>
                        <button class="btn btn-xs restart-btn" id="restartBtn-${panel.id}" onclick="restartCommand('${panel.id}')" title="Restart this command" style="display:none;">&#x21BB;</button>
                        <button class="btn btn-xs" onclick="toggleResources()" title="Toggle resource info">&#x2699;</button>
                        <span class="resource-badge" id="resourceBadge-${panel.id}" style="${state.showResources ? '' : 'display:none;'}"></span>
                        <span class="instance-url" style="${state.showResources ? '' : 'display:none;'}">${escHtml(panel.instUrl.replace(/^https?:\/\//, ''))}</span>
                        <span class="panel-font-size-ctrl">
                            <button class="btn btn-xs" onclick="changePanelFontSize('${panel.id}', -1)" title="Decrease font size">A-</button>
                            <span class="panel-font-size">${panel.fontSize}px</span>
                            <button class="btn btn-xs" onclick="changePanelFontSize('${panel.id}', 1)" title="Increase font size">A+</button>
                        </span>
                        <span class="panel-resize-controls" id="resizeControls-${panel.id}">
                            <label>Rows</label>
                            <input type="number" id="resizeRows-${panel.id}" value="24" min="1" max="10000" title="Terminal rows">
                            <label>Cols</label>
                            <input type="number" id="resizeCols-${panel.id}" value="80" min="1" max="1000" title="Terminal columns">
                            <button class="btn btn-xs" onclick="resizeTerminalPanel('${panel.id}')">Resize</button>
                            <button class="btn btn-xs" id="maxFitBtn-${panel.id}" onclick="toggleMaxFit('${panel.id}')" title="Resize terminal to fill the panel (toggle)">Max fit</button>
                            <button class="btn btn-xs" id="maxFontBtn-${panel.id}" onclick="toggleMaxFont('${panel.id}')" title="Maximize font size to fit terminal in pane (toggle)">Max font</button>
                        </span>
                        <select id="bufferSelect-${panel.id}" class="select-xs" onchange="switchBufferPanel('${panel.id}', this.value)" title="Which terminal buffer to view">
                            <option value="current">Current</option>
                            <option value="main">Main</option>
                            <option value="alt">Alt</option>
                        </select>
                        <span class="refresh-ctrl" title="Refresh throttle (ms). 0 = no throttle.">
                            <button class="btn btn-xs" onclick="changeRefreshMs(-100)" title="Decrease throttle">-</button>
                            <span class="refresh-val" id="refreshVal-${panel.id}">${state.refreshMs || 'off'}</span>
                            <button class="btn btn-xs" onclick="changeRefreshMs(100)" title="Increase throttle">+</button>
                        </span>
                        <div class="panel-send-row">
                            <input type="text" id="keyInput-${panel.id}" placeholder="Send keys..." onkeydown="if(event.key==='Enter'){event.preventDefault();sendKeysToPanel('${panel.id}')}">
                            <button class="btn btn-xs" onclick="sendKeysToPanel('${panel.id}')" title="Send keys to terminal">Send</button>
                            <button class="btn btn-xs" onclick="showSpecialKeysHelp()" title="Special keys reference">&#63;</button>
                        </div>
                        ${hasMultiplePanels ? `<button class="btn btn-xs btn-danger" onclick="removePanel('${panel.id}')" title="Remove panel">&#x2715;</button>` : ''}
                        <button class="btn btn-xs" onclick="copyTerminalSelection('${panel.id}')" title="Copy selected text to clipboard">Copy</button>
                        <button class="btn btn-xs" onclick="exportTerminal('${panel.id}')" title="Export terminal as text">&#x2913;</button>
                        <button class="btn btn-xs" onclick="screenshotPanel('${panel.id}')" title="Download screenshot as PNG">&#x1F4F7;</button>
                        <button class="btn btn-xs" id="panelThemeBtn-${panel.id}" onclick="togglePanelTheme('${panel.id}')" title="Panel theme: inherit (click to toggle)">${panel.theme === 'light' ? '\u263E' : panel.theme === 'dark' ? '\u2600' : '\u25D0'}</button>
                    </div>
                    <div class="vtty-container${panel.selectionMode ? ' selection-mode' : ''}" id="vtty-${panel.id}" ${panel.theme ? 'data-panel-theme="' + panel.theme + '"' : ''} style="font-size: ${panel.fontSize}px;">
                        <div class="exited-banner" id="exitedBanner-${panel.id}" style="display:none;"></div>
                        <div class="search-bar" id="searchBar-${panel.id}">
                            <input type="text" id="searchInput-${panel.id}" placeholder="Search terminal..." oninput="vttySearch('${panel.id}')">
                            <span class="search-count" id="searchCount-${panel.id}"></span>
                            <button onclick="vttySearchNext('${panel.id}')" title="Next match">&#x25BC;</button>
                            <button onclick="vttySearchPrev('${panel.id}')" title="Previous match">&#x25B2;</button>
                            <button onclick="vttySearchClose('${panel.id}')" title="Close search">&#x2715;</button>
                        </div>
                        <pre style="color:#484f58;">No command selected — spawn or select a command to view its output</pre>
                        <div class="cursor-indicator" style="display:none;"></div>
                        <div class="copy-feedback" id="copyFeedback-${panel.id}">Copied!</div>
                        <button class="scroll-bottom-btn" id="scrollBtn-${panel.id}" onclick="scrollTerminalBottom('${panel.id}')" title="Scroll to bottom">&#x25BC;</button>
                    </div>
                </div>
                ${resizeHandle}`;
        }
    }
    container.innerHTML = html;
    // Attach scroll listeners for scroll-to-bottom button visibility
    document.querySelectorAll('.vtty-container').forEach(vtty => {
        vtty.addEventListener('scroll', () => {
            const panelEl = vtty.closest('.panel');
            if (!panelEl) return;
            const btn = panelEl.querySelector('.scroll-bottom-btn');
            if (!btn) return;
            const isNearBottom = vtty.scrollHeight - vtty.scrollTop - vtty.clientHeight < 50;
            btn.classList.toggle('visible', !isNearBottom);
        });
    });
}

async function sendKeysToPanel(panelId) {
    const panel = state.panels.find(p => p.id === panelId);
    if (!panel) return;
    const input = document.getElementById('keyInput-' + panelId);
    if (!input || !input.value || !state.selectedCmdId) return;

    const keysValue = input.value;
    const cmdId = state.selectedCmdId;
    const instUrl = panel.instUrl;

    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/keys`, { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl, token: panel.token }),
            body: JSON.stringify({ keys: keysValue }),
        });
        let json;
        try {
            json = await res.json();
        } catch (parseErr) {
            console.error('send_keys: non-JSON response', res.status, res.statusText);
            input.value = '';
            loadVttyHttp(instUrl, cmdId);
            return;
        }
        if (json.status === 'ok') {
            input.value = '';
            loadVttyHttp(instUrl, cmdId);
        } else {
            console.error('send_keys server error:', res.status, json.error);
            input.value = '';
        }
    } catch (e) {
        console.error('send_keys network error:', e);
    }
}

// ─── Special Keys Help ───
function showSpecialKeysHelp() {
    // Remove existing modal if present
    const old = document.getElementById('specialKeysModal');
    if (old) { old.remove(); return; }

    const overlay = document.createElement('div');
    overlay.id = 'specialKeysModal';
    overlay.className = 'modal-overlay';
    overlay.style.display = 'flex';
    overlay.onclick = (e) => { if (e.target === overlay) { releaseCurrentFocusTrap(); overlay.remove(); } };

    overlay.innerHTML = `<div class="modal" style="max-width:560px;max-height:80vh;overflow-y:auto;">
        <h2 style="margin-bottom:0.5rem;">Special Keys Reference</h2>
        <p style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem;">
            Type special keys using <code style="background:var(--bg-tertiary);padding:0.1rem 0.3rem;border-radius:2px;">&lt;KeyName&gt;</code> syntax in the send-keys input.
            You can mix plain text with special keys, e.g. <code style="background:var(--bg-tertiary);padding:0.1rem 0.3rem;border-radius:2px;">hello&lt;Enter&gt;world</code>.
        </p>
        <table style="width:100%;font-size:0.75rem;border-collapse:collapse;">
            <thead>
                <tr style="border-bottom:1px solid var(--border);text-align:left;">
                    <th style="padding:0.3rem 0.5rem;color:var(--text-muted);font-weight:600;">Key</th>
                    <th style="padding:0.3rem 0.5rem;color:var(--text-muted);font-weight:600;">Syntax</th>
                    <th style="padding:0.3rem 0.5rem;color:var(--text-muted);font-weight:600;">Description</th>
                </tr>
            </thead>
            <tbody>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Return / Enter</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Enter&gt;</code> or <code>&lt;Return&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Send a newline (carriage return)</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Backspace</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Backspace&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Delete character before cursor</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Tab</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Tab&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Insert a tab character</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Escape</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Esc&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Send the Escape character (0x1B)</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Space</td><td style="padding:0.25rem 0.5rem;">(space character)</td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Type a literal space in the input</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Delete</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Delete&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Delete character at cursor (forward delete)</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Insert</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Insert&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Toggle insert/overwrite mode</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Home / End</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Home&gt;</code> <code>&lt;End&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Jump to beginning / end of line</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Page Up / Down</td><td style="padding:0.25rem 0.5rem;"><code>&lt;PageUp&gt;</code> <code>&lt;PageDown&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Scroll up / down one page</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Arrow Keys</td><td style="padding:0.25rem 0.5rem;"><code>&lt;Up&gt;</code> <code>&lt;Down&gt;</code> <code>&lt;Left&gt;</code> <code>&lt;Right&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Cursor movement</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">F1 &ndash; F12</td><td style="padding:0.25rem 0.5rem;"><code>&lt;F1&gt;</code> &hellip; <code>&lt;F12&gt;</code></td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Function keys</td></tr>
                <tr style="border-bottom:1px solid var(--border);"><td style="padding:0.25rem 0.5rem;">Ctrl + key</td><td style="padding:0.25rem 0.5rem;"><code>&lt;C-c&gt;</code> <code>&lt;C-a&gt;</code> <code>&lt;C-d&gt;</code> &hellip;</td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Control modifier (use lowercase letter). <code>&lt;C-c&gt;</code> = SIGINT (interrupt)</td></tr>
                <tr><td style="padding:0.25rem 0.5rem;">Alt + key</td><td style="padding:0.25rem 0.5rem;"><code>&lt;A-x&gt;</code> <code>&lt;A-enter&gt;</code> &hellip;</td><td style="padding:0.25rem 0.5rem;color:var(--text-secondary);">Alt/Meta modifier prefix (Escape + key)</td></tr>
            </tbody>
        </table>
        <div style="margin-top:0.75rem;text-align:right;">
            <button class="btn btn-xs" onclick="releaseCurrentFocusTrap();document.getElementById('specialKeysModal').remove()">Close</button>
        </div>
    </div>`;

    document.body.appendChild(overlay);
    const panel = overlay.querySelector('.modal');
    if (panel) trapFocus(panel);
    const closeBtn = overlay.querySelector('button');
    if (closeBtn) closeBtn.focus();
}

// ─── Logs ───

// Log WebSocket: connect, disconnect, and indicator helpers

function _updateLogTransportIndicator(mode) {
    const el = document.getElementById('logTransportIndicator');
    if (!el) return;
    el.textContent = mode.toUpperCase();
    el.dataset.mode = mode;
}

function connectLogWs() {
    // Don't connect if already connected or if there's an active search filter
    if (state.logWs && state.logWs.readyState === WebSocket.OPEN) return;

    disconnectLogWs();

    const wsUrl = getBaseUrl().replace(/^http/, 'ws');
    const token = state.authToken || (state.instanceUrls[0] || {}).token || '';
    const sep = token ? '?' : '';
    const url = `${wsUrl}/api/ws/logs${sep}${token ? 'token=' + encodeURIComponent(token) : ''}`;

    try {
        const ws = new WebSocket(url);
        state.logWs = ws;

        ws.onopen = () => {
            state._logWsReconnectAttempts = 0;
            _updateLogTransportIndicator('ws');
            // Append a connected indicator line
            const container = document.getElementById('logContent');
            if (container && container.querySelector('.log-line')) {
                const indicator = document.createElement('div');
                indicator.className = 'log-line log-ws-indicator';
                indicator.innerHTML = '<span class="timestamp">[' + new Date().toISOString().replace('T', ' ').replace(/\.\d+Z$/, '') + ']</span> <span class="details" style="color:var(--green);">Connected to log stream</span>';
                container.appendChild(indicator);
                _autoScrollLog(container);
            }
            // Start a ping interval to keep the connection alive
            clearInterval(state._logWsPingTimer);
            state._logWsPingTimer = setInterval(() => {
                if (state.logWs && state.logWs.readyState === WebSocket.OPEN) {
                    state.logWs.send(JSON.stringify({ type: 'ping' }));
                }
            }, 30000);
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                if (msg.type === 'log_entry' && msg.data) {
                    const container = document.getElementById('logContent');
                    if (!container) return;
                    // Remove the "no entries" placeholder if present
                    const placeholder = container.querySelector('[style*="text-align:center"]');
                    if (placeholder && !placeholder.classList.contains('log-line')) {
                        placeholder.remove();
                    }
                    const parsed = parseLogLine(msg.data);
                    const div = document.createElement('div');
                    div.className = 'log-line';
                    div.innerHTML = formatLogLine(parsed, msg.data);
                    container.appendChild(div);
                    _autoScrollLog(container);
                    // Update line count
                    const countEl = document.getElementById('logCount');
                    if (countEl) {
                        const current = container.querySelectorAll('.log-line').length;
                        countEl.textContent = `${current} lines (streaming)`;
                    }
                } else if (msg.type === 'connected') {
                    // Server confirmed connection — nothing extra to do
                } else if (msg.type === 'pong') {
                    // Heartbeat response — ignore
                }
            } catch (e) {
                console.error('Log WS message parse error:', e);
            }
        };

        ws.onclose = () => {
            _updateLogTransportIndicator('http');
            clearInterval(state._logWsPingTimer);
            state._logWsPingTimer = null;
            if (state.logWs === ws) {
                state.logWs = null;
            }
            _scheduleLogWsReconnect();
        };

        ws.onerror = () => {
            _updateLogTransportIndicator('http');
            clearInterval(state._logWsPingTimer);
            state._logWsPingTimer = null;
            if (state.logWs === ws) {
                state.logWs = null;
            }
            _scheduleLogWsReconnect();
        };
    } catch (e) {
        console.error('Log WebSocket connect failed:', e);
        _updateLogTransportIndicator('http');
        _scheduleLogWsReconnect();
    }
}

function _scheduleLogWsReconnect() {
    if (state.logWsReconnectTimer) return; // already scheduled
    if (state.currentView !== 'log') return; // don't reconnect if not viewing logs

    const delay = Math.min(1000 * Math.pow(2, state._logWsReconnectAttempts), 30000);
    state._logWsReconnectAttempts++;
    state.logWsReconnectTimer = setTimeout(() => {
        state.logWsReconnectTimer = null;
        if (state.currentView === 'log') {
            connectLogWs();
        }
    }, delay);
}

function disconnectLogWs() {
    if (state.logWsReconnectTimer) {
        clearTimeout(state.logWsReconnectTimer);
        state.logWsReconnectTimer = null;
    }
    clearInterval(state._logWsPingTimer);
    state._logWsPingTimer = null;
    if (state.logWs) {
        state.logWs.onclose = null;
        state.logWs.onerror = null;
        state.logWs.close();
        state.logWs = null;
    }
    _updateLogTransportIndicator('http');
}

function _autoScrollLog(container) {
    // Only auto-scroll if user is already near the bottom
    if (container.scrollHeight - container.scrollTop - container.clientHeight < 50) {
        container.scrollTop = container.scrollHeight;
    }
}

async function loadLog() {
    _updateLogTransportIndicator('http');
    try {
        const search = document.getElementById('logSearch').value;
        const params = new URLSearchParams();
        if (search) params.set('search', search);
        params.set('limit', '500');

        const res = await fetch(apiUrl('/api/log?' + params.toString()), { headers: authHeaders() });
        const json = await res.json();

        if (json.status === 'ok' && json.data) {
            const container = document.getElementById('logContent');
            const lines = json.data.lines || [];
            document.getElementById('logCount').textContent = `${json.data.filtered_lines}/${json.data.total_lines} lines`;

            if (lines.length === 0) {
                container.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No log entries found.' + (json.data.message ? ' ' + json.data.message : '') + '</div>';
            } else {
                container.innerHTML = lines.map(line => {
                    const parsed = parseLogLine(line);
                    if (search && line.toLowerCase().includes(search.toLowerCase())) {
                        return `<div class="log-line highlight">${formatLogLine(parsed, line)}</div>`;
                    }
                    return `<div class="log-line">${formatLogLine(parsed, line)}</div>`;
                }).join('');

                // Auto-scroll to bottom
                container.scrollTop = container.scrollHeight;
            }

            // Start WebSocket streaming after HTTP load if no search filter is active
            if (!search) {
                connectLogWs();
            }
        }
    } catch (e) {
        document.getElementById('logContent').innerHTML = `<div style="padding:1rem;color:var(--red);">Failed to load log: ${escHtml(e.message)}</div>`;
    }
}

function parseLogLine(line) {
    // Try to parse [timestamp] command: details
    const match = line.match(/^\[([^\]]+)\]\s+(\w+):\s+(.*)$/);
    if (match) {
        return { timestamp: match[1], command: match[2], details: match[3], raw: line };
    }
    return { timestamp: '', command: '', details: line, raw: line };
}

function formatLogLine(parsed, raw) {
    if (parsed.timestamp) {
        return `<span class="timestamp">[${escHtml(parsed.timestamp)}]</span> <span class="cmd-type">${escHtml(parsed.command)}</span> <span class="details">${escHtml(parsed.details)}</span>`;
    }
    return escHtml(raw);
}

function searchLogs() {
    // Disconnect WS during search — user is filtering, streaming would bypass the filter
    disconnectLogWs();
    state._logWsReconnectAttempts = 0; // reset for after search
    loadLog();
}

function clearLogSearch() {
    document.getElementById('logSearch').value = '';
    loadLog();
    // Reconnect WS after search is cleared (debounced)
    clearTimeout(state._logSearchReconnectTimer);
    state._logSearchReconnectTimer = setTimeout(() => {
        state._logSearchReconnectTimer = null;
        if (state.currentView === 'log') {
            connectLogWs();
        }
    }, 500);
}

// ─── Documentation ───
function showDocs() {
    const btn = document.getElementById('docsBtn');
    const vtty = document.getElementById('view-vtty');
    const log = document.getElementById('view-log');
    const docs = document.getElementById('view-docs');
    if (state.currentView === 'docs') {
        // Switch back to terminal
        state.currentView = 'vtty';
        vtty.style.display = 'flex';
        docs.style.display = 'none';
        if (btn) { btn.style.background = ''; btn.style.color = ''; }
    } else {
        // Disconnect log WS if active
        if (state.currentView === 'log') {
            disconnectLogWs();
            if (log) log.style.display = 'none';
        }
        state.currentView = 'docs';
        vtty.style.display = 'none';
        docs.style.display = 'block';
        if (btn) { btn.style.background = 'var(--accent)'; btn.style.color = '#fff'; }
        loadDocs();
    }
}

async function loadDocs() {
    const container = document.getElementById('view-docs');
    container.innerHTML = '<div style="padding:2rem;text-align:center;color:var(--text-muted);">Loading documentation...</div>';

    // Try fetching docs from the server, fall back to embedded docs
    try {
        const res = await fetch('/admin/docs.md', { headers: authHeaders() });
        if (res.ok) {
            const text = await res.text();
            container.innerHTML = renderMarkdown(text);
            return;
        }
    } catch (e) { /* fall through */ }

    // Embedded documentation
    container.innerHTML = renderEmbeddedDocs();
}

function renderMarkdown(md) {
    // Simple markdown to HTML (no external lib)
    let html = md
        .replace(/^### (.+)$/gm, '<h3>$1</h3>')
        .replace(/^## (.+)$/gm, '<h2>$1</h2>')
        .replace(/^# (.+)$/gm, '<h1>$1</h1>')
        .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code>$2</code></pre>')
        .replace(/`([^`]+)`/g, '<code>$1</code>')
        .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
        .replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2" target="_blank" style="color:var(--accent);">$1</a>')
        .replace(/^\- (.+)$/gm, '<li>$1</li>')
        .replace(/^(\d+)\. (.+)$/gm, '<li>$2</li>')
        .replace(/\n\n/g, '</p><p>')
        .replace(/\n/g, '<br>');
    return '<p>' + html + '</p>';
}

function renderEmbeddedDocs() {
    return `
<h1>vrw Administration</h1>

<h2>Overview</h2>
<p>vrw is a virtual terminal runner with a web control plane. It manages terminal applications, exposing their output through a web interface and REST API. This admin panel provides real-time monitoring and control of all running commands.</p>

<h2>Getting Started</h2>
<p>The admin panel connects to one or more vrw instances. Each instance manages its own set of terminal commands. Use the <strong>+ Panel</strong> button in the top bar to add connections to additional vrw instances.</p>

<h3>Connecting to an Instance</h3>
<p>By default, the admin panel connects to the vrw instance serving it. To add more instances:</p>
<ol>
    <li>Click <strong>+ Panel</strong> in the top bar</li>
    <li>Enter the instance URL (e.g., <code>http://localhost:9090</code>)</li>
    <li>Optionally set a label and auth token</li>
    <li>Click <strong>Add Panel</strong></li>
</ol>
<p>You can also use URL arguments: <code>?instance=http://host:8080&label=Prod&instance=http://host:9090&label=Dev</code></p>

<h2>URL Arguments for Multi-Instance</h2>
<p>The admin page accepts query parameters to pre-configure multi-panel views:</p>
<table>
    <tr><th>Parameter</th><th>Description</th><th>Example</th></tr>
    <tr><td><code>instance</code></td><td>vrw instance URL (repeatable)</td><td><code>?instance=http://host:8080</code></td></tr>
    <tr><td><code>label</code></td><td>Panel label (matches instance order)</td><td><code>&label=Production</code></td></tr>
    <tr><td><code>token</code></td><td>Auth token for instance (matches order)</td><td><code>&token=abc123</code></td></tr>
</table>
<p><strong>Full example:</strong> <code>/admin?instance=http://prod:8080&label=Production&instance=http://dev:9090&label=Development</code></p>

<h2>Managing Commands</h2>

<h3>Viewing Terminal Output</h3>
<p>Click on a command in the sidebar to view its real-time ANSI-rendered terminal output. The terminal emulator supports:</p>
<ul>
    <li>Full ANSI color rendering (16, 256, and 24-bit truecolor)</li>
    <li>Cursor position indicator (blue highlight)</li>
    <li>Text attributes: bold, italic, underline, strikethrough</li>
    <li>Scrollback buffer navigation via scrollbar</li>
</ul>

<h3>Spawning Commands</h3>
<p>Switch to the <strong>Spawn</strong> tab in the sidebar to create new commands. Specify the command path, optional arguments, an optional certificate for access control, and the target vrw instance.</p>

<h3>Sending Keystrokes</h3>
<p>Use the key input field in the panel header to send keystrokes to the selected command. Press <strong>Enter</strong> or click <strong>Send</strong> to transmit. Supports special keys using angle bracket notation:</p>
<ul>
    <li><code>&lt;Enter&gt;</code>, <code>&lt;Esc&gt;</code>, <code>&lt;Tab&gt;</code>, <code>&lt;Backspace&gt;</code></li>
    <li><code>&lt;Up&gt;</code>, <code>&lt;Down&gt;</code>, <code>&lt;Left&gt;</code>, <code>&lt;Right&gt;</code></li>
    <li><code>&lt;C-c&gt;</code> (Ctrl+C), <code>&lt;C-d&gt;</code> (Ctrl+D)</li>
    <li><code>&lt;F1&gt;</code> through <code>&lt;F12&gt;</code></li>
</ul>

<h3>Resizing the Terminal</h3>
<p>Use the <strong>R</strong> (rows) and <strong>C</strong> (columns) inputs in the top bar to resize the virtual terminal. Click <strong>Resize</strong> to apply. Valid ranges: rows 1-200, columns 1-500.</p>

<h3>Killing Commands</h3>
<p>Click the <strong>&#x2715;</strong> button next to a command in the sidebar to send SIGINT (Ctrl+C) to the process.</p>

<h2>Certificates</h2>
<p>The <strong>Certs</strong> tab in the sidebar shows all certificates configured in the connected instances' certificate pools. Certificates provide per-command access control — only clients presenting a certificate's derived token can interact with commands bound to that certificate.</p>
<p>When spawning a command, you can select a certificate to bind it. The certificate badge next to each command in the sidebar shows its binding status.</p>

<h2>Log Viewer</h2>
<p>The <strong>Logs</strong> tab provides access to the vrw command log. Use the search bar to filter log entries by content. Each entry shows a timestamp, the command type (spawn, kill, send_keys, etc.), and relevant details.</p>

<h2>Font Size</h2>
<p>Use the <strong>A-</strong> and <strong>A+</strong> buttons in the top bar to adjust the terminal font size (8px-28px). Your preference is saved in localStorage.</p>

<h2>VTTY Update Modes</h2>
<p>The web UI supports two modes for detecting when a terminal buffer has changed. You can switch between them using the <strong>Update</strong> dropdown in the bottom status bar. Your choice is saved in localStorage and will be restored on the next visit.</p>

<h3>Push Mode (default)</h3>
<p>In push mode, the server monitors each command's VTTY buffer at a configurable interval (default 200ms). When changes are detected, the server sends a lightweight <code>vtty_dirty</code> signal over the existing WebSocket connection. The signal contains only the command ID — no cell data, no HTML. The web UI then fetches the full HTML via <code>GET /api/commands/:id/vtty/html</code> at its own pace (debounced at 50ms). This is the most efficient mode because no polling is required; the server only sends when something actually changed.</p>
<p>Push mode is the default and is recommended for most use cases. It provides the lowest latency and lowest bandwidth overhead.</p>

<h3>Poll Mode</h3>
<p>In poll mode, the web client periodically calls <code>GET /api/commands/:id/vtty/changed</code> to ask "has the buffer changed since the last check?". The response is a simple <code>{ "changed": true/false }</code> with no HTML or diff data. If changed, the client then fetches the full HTML via the standard endpoint. The poll interval is configurable via the input next to the mode dropdown (50ms–5000ms, default 500ms).</p>
<p>Poll mode is useful when WebSocket connections are unreliable — for example, when a reverse proxy buffers WebSocket frames, when network conditions cause frequent WS reconnections, or when the client wants full control over refresh timing. The bandwidth overhead is slightly higher than push mode because the changed-check runs continuously even when nothing is changing.</p>

<h3>Server Configuration</h3>
<p>The server-side update settings can be configured in the vrw config file under the <code>web</code> section:</p>
<pre><code>web:
  update_mode: push       # "push" (default) or "poll"
  dirty_check_ms: 200     # server dirty-check interval (push mode)
  default_poll_ms: 500    # suggested client poll interval (poll mode)
</code></pre>
<p>The <code>dirty_check_ms</code> controls how often the server compares the VTTY buffer against the last-known snapshot in push mode. Lower values provide faster updates but increase CPU usage slightly. The <code>default_poll_ms</code> is the suggested interval that the web UI will use when in poll mode, but the user can override it via the UI controls at any time.</p>

<h2>API Reference</h2>
<table>
    <tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
    <tr><td>GET</td><td><code>/api/commands</code></td><td>List all running commands</td></tr>
    <tr><td>POST</td><td><code>/api/commands</code></td><td>Spawn a new command</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/vtty</code></td><td>Get VTTY output as ANSI text</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/vtty/html</code></td><td>Get VTTY as rendered HTML + cursor</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/vtty/changed</code></td><td>Check if VTTY buffer changed (poll mode)</td></tr>
    <tr><td>POST</td><td><code>/api/commands/:id/keys</code></td><td>Send keystrokes to a command</td></tr>
    <tr><td>POST</td><td><code>/api/commands/:id/kill</code></td><td>Kill a running command</td></tr>
    <tr><td>POST</td><td><code>/api/commands/:id/resize</code></td><td>Resize virtual terminal</td></tr>
    <tr><td>GET</td><td><code>/api/commands/:id/handles</code></td><td>List handles for a command</td></tr>
    <tr><td>GET</td><td><code>/api/certificates</code></td><td>List certificate pool</td></tr>
    <tr><td>GET</td><td><code>/api/info</code></td><td>Instance info and stats</td></tr>
    <tr><td>GET</td><td><code>/api/log</code></td><td>Command log with search</td></tr>
    <tr><td>POST</td><td><code>/api/shutdown</code></td><td>Graceful shutdown</td></tr>
</table>

<h2>Keyboard Shortcuts</h2>
<table>
    <tr><th>Shortcut</th><th>Action</th></tr>
    <tr><td><code>Enter</code> in key input</td><td>Send keystrokes</td></tr>
</table>
`;
}

// ─── Utilities ───
function escHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

// ─── Refresh Loop ───
function startRefresh() {
    // First call uses the snapshot endpoint (1 request = commands + VTTY + resources)
    loadSnapshot();
    if (state.refreshInterval) clearInterval(state.refreshInterval);
    state.refreshInterval = setInterval(() => {
        loadCommands();
        checkForExitedCommands();
    }, 1000);

    // Start resource polling (every 2 seconds) — first poll fires immediately
    if (state._resourceInterval) clearInterval(state._resourceInterval);
    pollResources(); // immediate first poll
    state._resourceInterval = setInterval(pollResources, 2000);
}

// ─── Keyboard handling ───
document.addEventListener('keydown', (e) => {
    // Direct terminal keyboard input: when a panel is focused,
    // capture keystrokes and send them to the PTY directly.
    if (state.currentView === 'vtty') {
        const panel = getSelectedPanel();
        if (panel) {
            const panelObj = state.panels.find(p => p.id === panel.id);
            if (panelObj && panelObj.focused && state.selectedCmdId) {
                // Skip if user is in a search input
                const searchBar = document.getElementById('searchBar-' + panel.id);
                if (searchBar && searchBar.classList.contains('visible') &&
                    document.activeElement && document.activeElement.id === 'searchInput-' + panel.id) {
                    // Let search input handle the key
                } else if (e.key === 'Escape') {
                    // Close Add Panel modal if open
                    const panelModal = document.getElementById('panelModal');
                    if (panelModal && panelModal.style.display !== 'none') {
                        closePanelModal();
                        return;
                    }
                    // Close Command Picker if open
                    const cmdPicker = document.getElementById('cmdPicker');
                    if (cmdPicker) {
                        releaseCurrentFocusTrap();
                        cmdPicker.remove();
                        return;
                    }
                    vttySearchClose(panel.id);
                    closeContextMenu();
                    closeShortcuts();
                    return;
                } else if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
                    e.preventDefault();
                    const sb = document.getElementById('searchBar-' + panel.id);
                    if (sb) {
                        sb.classList.add('visible');
                        // Trap focus inside the search bar
                        const vttyContainer = panel.querySelector('.vtty-container');
                        if (vttyContainer) trapFocus(vttyContainer);
                        const si = document.getElementById('searchInput-' + panel.id);
                        if (si) { si.focus(); si.select(); }
                    }
                    return;
                } else {
                    e.preventDefault();
                    sendDirectKey(e, panelObj);
                    return;
                }
            }
        }
    }

    // Focus key input when not in an input field and a command is selected
    if (state.currentView === 'vtty' && state.selectedCmdId &&
        !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panel = getSelectedPanel();
        if (panel) {
            const input = document.getElementById('keyInput-' + panel.id);
            if (input) input.focus();
        }
    }
    // Ctrl+F — open terminal search bar
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        const vttyContainer = e.target.closest && e.target.closest('.vtty-container');
        if (vttyContainer || state.currentView === 'vtty') {
            e.preventDefault();
            const panel = getSelectedPanel();
            if (panel) {
                const searchBar = document.getElementById('searchBar-' + panel.id);
                if (searchBar) {
                    searchBar.classList.add('visible');
                    // Trap focus inside the search bar area
                    const vtty = panel.querySelector('.vtty-container');
                    if (vtty) trapFocus(vtty);
                    const searchInput = document.getElementById('searchInput-' + panel.id);
                    if (searchInput) { searchInput.focus(); searchInput.select(); }
                }
            }
        }
    }
    // Shift+F10 or ContextMenu key — open context menu on focused cmd-item or panel-header
    if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
        e.preventDefault();
        const target = document.activeElement;
        if (!target) return;
        // Panel header context menu
        if (target.classList.contains('panel-header') && target.dataset.panelId) {
            const rect = target.getBoundingClientRect();
            showPanelContextMenu({ preventDefault: () => {}, clientX: rect.left + rect.width / 2, clientY: rect.bottom }, target.dataset.panelId);
        }
        // Command item context menu
        if (target.classList.contains('cmd-item') && target.dataset.instUrl) {
            const rect = target.getBoundingClientRect();
            showCmdContextMenu({ preventDefault: () => {}, clientX: rect.left + rect.width / 2, clientY: rect.bottom }, target.dataset.instUrl, target.dataset.cmdId, target.dataset.cmdName, target.dataset.cmdAlive === 'true');
        }
    }
    // Escape — close terminal search bar, panel modal, command picker, shortcuts
    if (e.key === 'Escape') {
        // Close Add Panel modal if open
        const panelModal = document.getElementById('panelModal');
        if (panelModal && panelModal.style.display !== 'none') {
            closePanelModal();
            return;
        }
        // Close Command Picker if open
        const cmdPicker = document.getElementById('cmdPicker');
        if (cmdPicker) {
            releaseCurrentFocusTrap();
            cmdPicker.remove();
            return;
        }
        const panel = getSelectedPanel();
        if (panel) {
            vttySearchClose(panel.id);
        }
        closeContextMenu();
        closeShortcuts();
    }
    // Ctrl+Shift+C — copy terminal selection
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'C' || e.key === 'c')) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            copyTerminalSelection(panel.id);
            return;
        }
    }
    // Ctrl+Shift+S — toggle selection mode
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'S' || e.key === 's')) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            toggleSelectionMode(panel.id);
            return;
        }
    }
    // Alt+S — toggle selection mode (alternative shortcut)
    if (e.altKey && (e.key === 's' || e.key === 'S') && !e.ctrlKey && !e.metaKey) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            toggleSelectionMode(panel.id);
            return;
        }
    }
    // ? — show keyboard shortcuts
    if (e.key === '?' && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        showShortcuts();
    }
    // Alt+Left / Alt+Right — navigate prev/next command (only when not focused on terminal)
    if (e.altKey && !e.ctrlKey && !e.metaKey && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panel = getSelectedPanel();
        const panelObj = panel && state.panels.find(p => p.id === panel.id);
        if (e.key === 'ArrowLeft' && !(panelObj && panelObj.focused)) {
            e.preventDefault();
            navigatePrevCommand();
            return;
        }
        if (e.key === 'ArrowRight' && !(panelObj && panelObj.focused)) {
            e.preventDefault();
            navigateNextCommand();
            return;
        }
    }
});

// ─── Direct key sending (when terminal is focused) ───
// Encodes a KeyboardEvent into escape sequences and sends to the PTY.
async function sendDirectKey(e, panelObj) {
    if (!state.selectedCmdId || !panelObj.instUrl) return;

    // Map common special keys to escape sequences
    const keyMap = {
        'Enter': '\r',
        'Backspace': '\x7f',
        'Tab': '\t',
        'Escape': '\x1b',
        'Home': '\x1b[H',
        'End': '\x1b[F',
        'Delete': '\x1b[3~',
        'ArrowUp': '\x1b[A',
        'ArrowDown': '\x1b[B',
        'ArrowRight': '\x1b[C',
        'ArrowLeft': '\x1b[D',
        'PageUp': '\x1b[5~',
        'PageDown': '\x1b[6~',
        'Insert': '\x1b[2~',
        'F1': '\x1bOP',
        'F2': '\x1bOQ',
        'F3': '\x1bOR',
        'F4': '\x1bOS',
        'F5': '\x1b[15~',
        'F6': '\x1b[17~',
        'F7': '\x1b[18~',
        'F8': '\x1b[19~',
        'F9': '\x1b[20~',
        'F10': '\x1b[21~',
        'F11': '\x1b[23~',
        'F12': '\x1b[24~',
    };

    let seq = '';
    if (e.ctrlKey && !e.altKey && !e.metaKey) {
        // Ctrl+letter
        if (e.key.length === 1 && e.key >= 'a' && e.key <= 'z') {
            seq = String.fromCharCode(e.key.charCodeAt(0) - 96);
        } else if (e.key === '[') seq = '\x1b'; // Ctrl+[ = ESC
        else if (e.key === '\\') seq = '\x1c';
        else if (e.key === ']') seq = '\x1d';
        else if (e.key === '^') seq = '\x1e';
        else if (e.key === '_') seq = '\x1f';
    } else if (e.altKey && !e.ctrlKey && !e.metaKey) {
        // Alt+letter = ESC + letter
        if (e.key.length === 1) seq = '\x1b' + e.key;
    } else if (keyMap[e.key]) {
        seq = keyMap[e.key];
    } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
        // Regular printable character
        seq = e.key;
    }

    if (!seq) return;

    try {
        const res = await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/keys`, { url: panelObj.instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.instUrl }),
            body: JSON.stringify({ keys: seq }),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            // Trigger a refresh
            scheduleVttyHttp(panelObj.instUrl, state.selectedCmdId, 50);
        }
    } catch (err) {
        console.error('Direct key send error:', err);
    }
}

// ─── Click-to-focus terminal ───
// Clicking on the VTTY container focuses the terminal for direct keyboard input.
// A second click on an already-focused terminal blurs it.
document.addEventListener('click', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (vttyContainer && state.currentView === 'vtty') {
        const panelEl = vttyContainer.closest('.panel');
        if (panelEl) {
            const panelObj = state.panels.find(p => p.id === panelEl.id);
            if (panelObj) {
                // Check if click is on a button inside the vtty container (search bar, scroll btn)
                if (e.target.closest('button') || e.target.closest('input')) return;

                if (panelObj.focused) {
                    // Already focused — blur
                    panelObj.focused = false;
                    vttyContainer.style.outline = '';
                } else {
                    // Focus this panel's terminal
                    state.panels.forEach(p => p.focused = false);
                    document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
                    panelObj.focused = true;
                    vttyContainer.style.outline = '2px solid var(--accent)';
                    vttyContainer.setAttribute('tabindex', '0');
                    vttyContainer.focus();
                }
            }
        }
    } else if (!vttyContainer) {
        // Click outside any terminal — blur all
        state.panels.forEach(p => p.focused = false);
        document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
    }
});

// ─── Mouse wheel handling on terminal ───
// Level 1 optimization: Don't block native scroll when viewing the live buffer.
// Only intercept wheel events at the top edge (scroll into scrollback history)
// or when mouse tracking is enabled (forward to PTY).
//
// When in scrollback view (scrollbackOffset > 0), scroll wheel navigates
// scrollback history via server-side offset (debounced with rAF).
//
// Native scroll provides smooth inertia and momentum — the browser handles
// repaint timing, which is far more efficient than per-tick HTTP round-trips.
let _wheelScrollRafId = null;
let _wheelScrollPanel = null;   // panel object for the pending rAF callback
let _wheelScrollAccum = 0;      // accumulated signed vertical delta

document.addEventListener('wheel', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') return;

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // If selection mode is active, let browser handle wheel natively (no scrollback, no PTY)
    if (panelObj.selectionMode) return;

    // If the child has mouse tracking enabled, forward wheel events to the PTY
    if (panelObj.mouseTracking) {
        e.preventDefault();
        const wheelEvent = e.deltaY < 0 ? 'wheel_up' : 'wheel_down';
        sendMouseEvent(panelObj, wheelEvent, 0, e);
        return;
    }

    // ── Live buffer view (scrollbackOffset === 0) ──
    // Allow native scroll. Only intercept when user scrolls up past the top
    // edge, which means they want to enter scrollback history.
    if (panelObj.scrollbackOffset === 0) {
        const atTop = vttyContainer.scrollTop <= 0;
        if (e.deltaY < 0 && atTop) {
            // User scrolled up at the top edge — enter scrollback history
            e.preventDefault();
            panelObj.scrollbackOffset += 3;
            sessionStorage.setItem('vrw_scrollback_' + state.selectedCmdId, panelObj.scrollbackOffset.toString());
            loadVttyHttp(panelObj.instUrl, state.selectedCmdId);
            // Show scrollback indicator
            const sbIndicator = document.getElementById('scrollbackIndicator');
            if (sbIndicator) sbIndicator.style.display = '';
            const btn = panelEl.querySelector('.scroll-bottom-btn');
            if (btn) btn.classList.add('visible');
        }
        // else: let browser handle native scroll (no preventDefault)
        return;
    }

    // ── Scrollback history view (scrollbackOffset > 0) ──
    e.preventDefault();

    // Accumulate scroll delta — will be processed in the next animation frame.
    // This coalesces rapid wheel ticks into a single HTTP round-trip.
    _wheelScrollPanel = panelObj;
    _wheelScrollAccum += e.deltaY;

    if (_wheelScrollRafId) cancelAnimationFrame(_wheelScrollRafId);
    _wheelScrollRafId = requestAnimationFrame(() => {
        _wheelScrollRafId = null;
        const p = _wheelScrollPanel;
        if (!p) return;

        // Snapshot and reset the accumulator before processing.
        const accum = _wheelScrollAccum;
        _wheelScrollAccum = 0;

        // Convert accumulated pixel delta to scrollback lines.
        // ~100px of scroll ≈ 3 lines (same ratio as the previous per-tick behavior).
        const lines = Math.max(1, Math.round(Math.abs(accum) / 100) * 3);

        if (accum > 0) {
            // Wheel down: decrease scrollback offset (move toward live view)
            const newOffset = Math.max(0, p.scrollbackOffset - lines);
            if (newOffset === 0) {
                // Reached the live buffer — restore native scroll
                p.scrollbackOffset = 0;
                sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
                loadVttyHttp(p.instUrl, state.selectedCmdId);
                // Scroll to bottom after returning to live view
                const vtty = panelEl.querySelector('.vtty-container');
                if (vtty) vtty.scrollTop = vtty.scrollHeight;
            } else {
                p.scrollbackOffset = newOffset;
                sessionStorage.setItem('vrw_scrollback_' + state.selectedCmdId, p.scrollbackOffset.toString());
                loadVttyHttp(p.instUrl, state.selectedCmdId);
            }
        } else {
            // Wheel up: increase scrollback offset (move into history)
            p.scrollbackOffset += lines;
            sessionStorage.setItem('vrw_scrollback_' + state.selectedCmdId, p.scrollbackOffset.toString());
            loadVttyHttp(p.instUrl, state.selectedCmdId);
        }

        // Update scroll-to-bottom button visibility and scrollback indicator
        const btn = panelEl.querySelector('.scroll-bottom-btn');
        if (btn) btn.classList.toggle('visible', p.scrollbackOffset > 0);
        const sbIndicator = document.getElementById('scrollbackIndicator');
        if (sbIndicator) sbIndicator.style.display = p.scrollbackOffset > 0 ? '' : 'none';
    });
}, { passive: false });

// ─── Mouse event forwarding to PTY ───
// Forwards mousedown, mouseup, mousemove events to the PTY when the child
// has enabled mouse tracking mode. Events are sent as escape sequences via
// POST /api/commands/:id/mouse.

let _mouseDownButton = null; // Track which button is pressed

document.addEventListener('mousedown', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') {
        _mouseDownButton = null;
        return;
    }

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // Skip if clicking on buttons/inputs inside vtty container
    if (e.target.closest('button') || e.target.closest('input')) return;

    // If selection mode is active, skip PTY forwarding — let browser handle selection
    if (panelObj.selectionMode) return;

    // If mouse tracking is enabled, forward the event to PTY
    if (panelObj.mouseTracking) {
        e.preventDefault();
        _mouseDownButton = e.button; // 0=left, 1=middle, 2=right
        sendMouseEvent(panelObj, 'down', e.button, e);
    }
});

document.addEventListener('mouseup', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') {
        _mouseDownButton = null;
        return;
    }

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // If selection mode is active, skip PTY forwarding
    if (panelObj.selectionMode) {
        _mouseDownButton = null;
        return;
    }

    if (panelObj.mouseTracking && _mouseDownButton !== null) {
        e.preventDefault();
        sendMouseEvent(panelObj, 'up', _mouseDownButton, e);
        _mouseDownButton = null;
    }
});

document.addEventListener('mousemove', (e) => {
    if (_mouseDownButton === null) return; // Only track during drag

    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') return;

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId || !panelObj.mouseTracking) return;

    // If selection mode is active, skip PTY forwarding
    if (panelObj.selectionMode) return;

    // Throttle mouse move events to avoid flooding
    if (!panelObj._lastMoveTime || Date.now() - panelObj._lastMoveTime > 16) {
        panelObj._lastMoveTime = Date.now();
        sendMouseEvent(panelObj, 'move', _mouseDownButton, e);
    }
});

// Send a mouse event to the PTY via the API
async function sendMouseEvent(panelObj, eventType, button, e) {
    if (!state.selectedCmdId || !panelObj.instUrl) return;

    // Calculate terminal cell coordinates from pixel position
    const vttyEl = document.getElementById(panelObj.id)?.querySelector('.vtty-container');
    if (!vttyEl) return;

    const rect = vttyEl.getBoundingClientRect();
    const charW = state.fontSize * 0.6;
    const charH = state.fontSize * 1.2;

    const x = Math.max(1, Math.floor((e.clientX - rect.left) / charW) + 1);
    const y = Math.max(1, Math.floor((e.clientY - rect.top) / charH) + 1);

    try {
        await fetch(apiUrl(`/api/commands/${state.selectedCmdId}/mouse`, { url: panelObj.instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.instUrl }),
            body: JSON.stringify({
                event: eventType,
                button: button,
                x: x,
                y: y,
            }),
        });
        // Refresh display after mouse events (the child may have reacted)
        scheduleVttyHttp(panelObj.instUrl, state.selectedCmdId, 30);
    } catch (err) {
        // Silently ignore — mouse events are best-effort
    }
}

// ─── Terminal Search ───
const vttySearchState = { matchIndex: 0, matches: [], panelId: null };

function vttySearch(panelId) {
    const input = document.getElementById('searchInput-' + panelId);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (!input || !countEl) return;
    const query = input.value;
    vttySearchState.panelId = panelId;
    vttySearchState.matchIndex = 0;

    // Get the text content of the terminal
    const panel = document.getElementById(panelId);
    const pre = panel ? panel.querySelector('pre') : null;
    if (!pre) { countEl.textContent = '0/0'; return; }

    // Remove previous highlights
    vttyRemoveHighlights(pre);

    if (!query) {
        vttySearchState.matches = [];
        countEl.textContent = '';
        return;
    }

    // Find all text nodes and mark matches
    const text = pre.textContent || '';
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    vttySearchState.matches = [];

    let pos = 0;
    while ((pos = lowerText.indexOf(lowerQuery, pos)) !== -1) {
        vttySearchState.matches.push(pos);
        pos += lowerQuery.length;
    }

    if (vttySearchState.matches.length > 0) {
        vttyApplyHighlights(pre, text, query);
        vttyScrollToMatch(panelId, 0);
        countEl.textContent = '1/' + vttySearchState.matches.length;
    } else {
        countEl.textContent = '0/0';
    }
}

function vttyApplyHighlights(pre, text, query) {
    // Walk through text and highlight matches
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    const fragment = document.createDocumentFragment();
    let lastIdx = 0;
    let matchIdx = 0;
    let pos = 0;

    // We need to rebuild using the pre's innerHTML which has spans for ANSI
    // Instead, work at the text level using a tree walker on text nodes
    const walker = document.createTreeWalker(pre, NodeFilter.SHOW_TEXT, null);
    const textNodes = [];
    while (walker.nextNode()) textNodes.push(walker.currentNode);

    if (textNodes.length === 0) return;

    // Simple approach: highlight by rebuilding innerHTML with mark spans
    // Get the full innerHTML and do string replacement on text portions
    let html = pre.innerHTML;
    const escaped = escHtml(query);
    // Use a regex that matches the query text (case insensitive) but only
    // within text content, not inside HTML tags
    const regex = new RegExp('(?![^<]*>)(' + escaped.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ')', 'gi');
    const results = [];
    let match;
    while ((match = regex.exec(html)) !== null) {
        results.push(match.index);
    }

    // Apply highlights in reverse order to preserve indices
    for (let i = results.length - 1; i >= 0; i--) {
        const idx = results[i];
        const originalLen = query.length;
        // Find the end of the match in the HTML
        const endIdx = html.indexOf(match[1], idx) + match[1].length;
        if (endIdx <= idx) continue;
        const cls = i === 0 ? 'vtty-search-highlight current' : 'vtty-search-highlight';
        html = html.substring(0, idx) + '<mark class="' + cls + '" data-match-idx="' + i + '">' + html.substring(idx, endIdx) + '</mark>' + html.substring(endIdx);
    }

    pre.innerHTML = html;
}

function vttyRemoveHighlights(pre) {
    const marks = pre.querySelectorAll('mark.vtty-search-highlight');
    marks.forEach(mark => {
        const parent = mark.parentNode;
        parent.replaceChild(document.createTextNode(mark.textContent), mark);
        parent.normalize();
    });
}

function vttyScrollToMatch(panelId, idx) {
    const panel = document.getElementById(panelId);
    if (!panel) return;
    const mark = panel.querySelector('mark.vtty-search-highlight.current');
    if (mark) mark.classList.remove('current');
    const marks = panel.querySelectorAll('mark.vtty-search-highlight');
    if (marks[idx]) {
        marks[idx].classList.add('current');
        marks[idx].scrollIntoView({ block: 'center', behavior: 'smooth' });
    }
}

function vttySearchNext(panelId) {
    if (vttySearchState.matches.length === 0) return;
    vttySearchState.matchIndex = (vttySearchState.matchIndex + 1) % vttySearchState.matches.length;
    vttyScrollToMatch(panelId, vttySearchState.matchIndex);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = (vttySearchState.matchIndex + 1) + '/' + vttySearchState.matches.length;
}

function vttySearchPrev(panelId) {
    if (vttySearchState.matches.length === 0) return;
    vttySearchState.matchIndex = (vttySearchState.matchIndex - 1 + vttySearchState.matches.length) % vttySearchState.matches.length;
    vttyScrollToMatch(panelId, vttySearchState.matchIndex);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = (vttySearchState.matchIndex + 1) + '/' + vttySearchState.matches.length;
}

function vttySearchClose(panelId) {
    releaseCurrentFocusTrap();
    const searchBar = document.getElementById('searchBar-' + panelId);
    if (searchBar) searchBar.classList.remove('visible');
    const panel = document.getElementById(panelId);
    const pre = panel ? panel.querySelector('pre') : null;
    if (pre) vttyRemoveHighlights(pre);
    vttySearchState.matches = [];
    vttySearchState.matchIndex = 0;
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = '';
    // Return focus to the VTTY container
    if (panel) {
        const vtty = panel.querySelector('.vtty-container');
        if (vtty) vtty.focus();
    }
}

// ─── Scroll to Bottom ───
function scrollTerminalBottom(panelId) {
    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const vtty = panelEl.querySelector('.vtty-container');
    if (vtty) {
        vtty.scrollTop = vtty.scrollHeight;
    }
    // Reset scrollback offset and re-fetch
    const panelObj = state.panels.find(p => p.id === panelId);
    if (panelObj && panelObj.scrollbackOffset > 0) {
        panelObj.scrollbackOffset = 0;
        // Clear stored scrollback since we reset
        if (state.selectedCmdId) {
            sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
        }
        const sbIndicator = document.getElementById('scrollbackIndicator');
        if (sbIndicator) sbIndicator.style.display = 'none';
        if (state.selectedCmdId && panelObj.instUrl) {
            loadVttyHttp(panelObj.instUrl, state.selectedCmdId);
        }
    }
}

// ─── Browser Notification on Command Exit ───
const _notifiedExits = new Set();

function notifyCommandEnded(cmdId) {
    if (!cmdId || _notifiedExits.has(cmdId)) return;
    _notifiedExits.add(cmdId);

    // Find command name and exit code
    let cmdName = cmdId;
    let exitCode = null;
    for (const inst of state.instanceUrls) {
        if (inst._commands) {
            const cmd = inst._commands.find(c => c.id === cmdId);
            if (cmd) { cmdName = cmd.name || cmdId; exitCode = cmd.exit_code; break; }
        }
    }

    // Play sound notification
    if (state.soundEnabled) {
        playExitSound(exitCode === 0);
    }

    if ('Notification' in window) {
        if (Notification.permission === 'granted') {
            new Notification('vrw: Command exited', { body: cmdName, icon: '/favicon.ico' });
        } else if (Notification.permission !== 'denied') {
            Notification.requestPermission().then(perm => {
                if (perm === 'granted') {
                    new Notification('vrw: Command exited', { body: cmdName, icon: '/favicon.ico' });
                }
            });
        }
    }
}

// Also detect command exits via polling — notify when a previously-alive command exits
function checkForExitedCommands() {
    for (const inst of state.instanceUrls) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false && !_notifiedExits.has(cmd.id)) {
                notifyCommandEnded(cmd.id);
            }
        }
    }
}

// ─── Panel Resize via Drag ───
(function() {
    let resizing = false;
    let startX = 0;
    let startWidth = 0;
    let resizePanel = null;

    document.addEventListener('mousedown', (e) => {
        const handle = e.target.closest('.panel-resize-handle');
        if (!handle) return;
        e.preventDefault();
        resizePanel = handle.previousElementSibling;
        if (!resizePanel) return;
        startX = e.clientX;
        startWidth = resizePanel.getBoundingClientRect().width;
        handle.classList.add('active');
        resizing = true;
    });

    document.addEventListener('mousemove', (e) => {
        if (!resizing || !resizePanel) return;
        const delta = e.clientX - startX;
        const containerWidth = resizePanel.parentElement.getBoundingClientRect().width;
        const panelCount = resizePanel.parentElement.children.length;
        const minW = 100;
        const newWidth = Math.max(minW, Math.min(containerWidth - (panelCount - 1) * minW, startWidth + delta));
        const pct = (newWidth / containerWidth) * 100;
        resizePanel.style.flex = `0 0 ${pct}%`;
    });

    document.addEventListener('mouseup', () => {
        if (resizing) {
            document.querySelectorAll('.panel-resize-handle.active').forEach(h => h.classList.remove('active'));
            resizing = false;
            resizePanel = null;
        }
    });
})();

// ─── Export Terminal Output ───
/// Copy terminal text to the clipboard.
/// If the user has selected text in the VTTY, copy that selection.
/// Otherwise, fall back to the full VTTY content.
function copyTerminalSelection(panelId) {
    const panel = document.getElementById(panelId);
    if (!panel) return;

    // First try the browser text selection
    const selection = window.getSelection();
    let text = selection ? selection.toString().trim() : '';

    // Fallback: copy full VTTY content
    if (!text) {
        const pre = panel.querySelector('pre');
        if (pre) {
            text = pre.textContent || pre.innerText || '';
        }
    }

    if (!text) return;

    navigator.clipboard.writeText(text).then(() => {
        // Show "Copied!" feedback
        const feedback = document.getElementById('copyFeedback-' + panelId);
        if (feedback) {
            feedback.classList.add('visible');
            setTimeout(() => feedback.classList.remove('visible'), 1200);
        }
    }).catch(() => {
        // Clipboard API may fail (e.g. non-HTTPS); fall back to execCommand
        const ta = document.createElement('textarea');
        ta.value = text;
        ta.style.cssText = 'position:fixed;opacity:0;';
        document.body.appendChild(ta);
        ta.select();
        try { document.execCommand('copy'); } catch (_) { /* ignore */ }
        document.body.removeChild(ta);
        const feedback = document.getElementById('copyFeedback-' + panelId);
        if (feedback) {
            feedback.classList.add('visible');
            setTimeout(() => feedback.classList.remove('visible'), 1200);
        }
    });
}

function exportTerminal(panelId) {
    const panel = document.getElementById(panelId);
    if (!panel) return;
    const pre = panel.querySelector('pre');
    if (!pre) return;
    const text = pre.textContent || pre.innerText || '';
    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    // Use command name for the filename
    let cmdName = 'terminal';
    for (const inst of state.instanceUrls) {
        if (inst._commands) {
            const cmd = inst._commands.find(c => c.id === state.selectedCmdId);
            if (cmd) { cmdName = (cmd.name || cmd.id).replace(/\//g, '_'); break; }
        }
    }
    a.href = url;
    a.download = cmdName + '.txt';
    a.click();
    URL.revokeObjectURL(url);
}

/// Download a PNG screenshot of the currently selected command's VTTY buffer.
/// Uses server-configured default font size and font name.
async function screenshotPanel(panelId) {
    // Determine which command is shown in this panel
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const instUrl = panelObj.instUrl;
    const isSelectedPanel = (instUrl === state.selectedInstUrl);
    const cmdId = isSelectedPanel ? state.selectedCmdId : null;
    if (!cmdId) {
        alert('No command selected to screenshot.');
        return;
    }

    // Use server-configured defaults for font
    const fontSize = state.serverScreenshotFontSize || 12;
    const fontName = state.serverScreenshotFontName || 'monospace';

    // Build the PNG endpoint URL
    const params = new URLSearchParams({ font_size: fontSize });
    if (fontName && fontName !== 'monospace') {
        params.set('font_name', fontName);
    }
    const url = apiUrl(`/api/commands/${cmdId}/vtty/png?${params}`, { url: instUrl });

    try {
        const res = await fetch(url, { headers: authHeadersForInstance({ url: instUrl }) });
        if (!res.ok) {
            const json = await res.json().catch(() => null);
            const error = (json && json.error) || `HTTP ${res.status}`;
            alert('Screenshot failed: ' + error);
            return;
        }
        const blob = await res.blob();
        const blobUrl = URL.createObjectURL(blob);
        const a = document.createElement('a');

        // Build filename: vrw_YYYYMMDD_HHMMSS_rowsxcols_command_args.png
        let cmdInfo = 'vrw';
        for (const inst of state.instanceUrls) {
            if (inst._commands) {
                const cmd = inst._commands.find(c => c.id === cmdId);
                if (cmd) {
                    const parts = [cmd.name || 'unknown'];
                    if (cmd.args && cmd.args.length > 0) parts.push(...cmd.args);
                    cmdInfo = parts.join(' ').replace(/[^a-zA-Z0-9_\-\.]/g, '_');
                    break;
                }
            }
        }
        const now = new Date();
        const pad = (n) => String(n).padStart(2, '0');
        const ts = `${now.getFullYear()}${pad(now.getMonth()+1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;

        // Include terminal dimensions if known from VTTY metadata
        let dims = '';
        const pre = document.querySelector(`#vtty-${panelId} pre`);
        if (pre && pre._vttyRows && pre._vttyCols) {
            dims = pre._vttyRows + 'x' + pre._vttyCols;
        }

        const truncated = cmdInfo.length > 120 ? cmdInfo.substring(0, 117) + '...' : cmdInfo;
        const filename = dims
            ? `vrw_${ts}_${dims}_${truncated}.png`
            : `vrw_${ts}_${truncated}.png`;

        a.href = blobUrl;
        a.download = filename;
        a.click();
        URL.revokeObjectURL(blobUrl);
    } catch (e) {
        alert('Screenshot failed: ' + e.message);
    }
}

// ─── Right-click Context Menu ───
// Tracks the currently focused menu item index for keyboard navigation.
let _ctxMenuFocusedIndex = -1;

function closeContextMenu() {
    const el = document.getElementById('ctxMenu');
    if (el) el.remove();
    _ctxMenuFocusedIndex = -1;
}

// Helper: create a single context menu item div with safe textContent + addEventListener.
function _createCtxMenuItem(label, onClick, isDanger) {
    const div = document.createElement('div');
    div.className = 'ctx-menu-item' + (isDanger ? ' danger' : '');
    div.setAttribute('role', 'menuitem');
    div.setAttribute('tabindex', '-1');
    div.textContent = label;
    div.addEventListener('click', () => {
        onClick();
        closeContextMenu();
    });
    return div;
}

// Helper: position menu at (x, y), ensuring it stays within the viewport.
function _positionCtxMenu(menu, x, y) {
    menu.style.left = x + 'px';
    menu.style.top = y + 'px';
    document.body.appendChild(menu);
    const rect = menu.getBoundingClientRect();
    if (rect.right > window.innerWidth) menu.style.left = (window.innerWidth - rect.width - 4) + 'px';
    if (rect.bottom > window.innerHeight) menu.style.top = (window.innerHeight - rect.height - 4) + 'px';
}

// Helper: set up close-on-click-outside and keyboard navigation for a context menu.
function _setupCtxMenuListeners(menu) {
    // Close on click outside
    setTimeout(() => {
        document.addEventListener('click', closeContextMenu, { once: true });
    }, 0);

    // Keyboard navigation inside the context menu
    menu.addEventListener('keydown', (e) => {
        const items = menu.querySelectorAll('.ctx-menu-item');
        if (items.length === 0) return;

        if (e.key === 'ArrowDown') {
            e.preventDefault();
            _ctxMenuFocusedIndex = (_ctxMenuFocusedIndex + 1) % items.length;
            _focusCtxMenuItem(items);
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            _ctxMenuFocusedIndex = (_ctxMenuFocusedIndex - 1 + items.length) % items.length;
            _focusCtxMenuItem(items);
        } else if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            if (_ctxMenuFocusedIndex >= 0 && _ctxMenuFocusedIndex < items.length) {
                items[_ctxMenuFocusedIndex].click();
            }
        } else if (e.key === 'Escape') {
            e.preventDefault();
            closeContextMenu();
        } else if (e.key === 'Tab') {
            // Prevent tab from leaving the menu
            e.preventDefault();
            closeContextMenu();
        }
    });

    // Focus the first item for keyboard users
    _ctxMenuFocusedIndex = 0;
    const firstItem = menu.querySelector('.ctx-menu-item');
    if (firstItem) firstItem.focus();
}

function _focusCtxMenuItem(items) {
    items.forEach((item, i) => {
        item.classList.toggle('ctx-menu-focused', i === _ctxMenuFocusedIndex);
        if (i === _ctxMenuFocusedIndex) {
            item.focus();
        }
    });
}

function showCmdContextMenu(e, instUrl, cmdId, cmdName, isAlive) {
    e.preventDefault();
    closeContextMenu();
    const menu = document.createElement('div');
    menu.id = 'ctxMenu';
    menu.className = 'ctx-menu';
    menu.setAttribute('role', 'menu');

    // View Terminal
    menu.appendChild(_createCtxMenuItem('View Terminal', () => selectCommand(instUrl, cmdId, cmdName), false));
    // Copy URL
    menu.appendChild(_createCtxMenuItem('Copy URL', () => copyCommandUrl(instUrl, cmdId, cmdName), false));

    if (isAlive) {
        // Separator
        const sep1 = document.createElement('div');
        sep1.className = 'ctx-menu-sep';
        sep1.setAttribute('role', 'separator');
        menu.appendChild(sep1);
        // Pause/Resume
        menu.appendChild(_createCtxMenuItem('Pause/Resume', () => togglePauseCmd(instUrl, cmdId), false));
        // Restart
        menu.appendChild(_createCtxMenuItem('Restart', () => restartCommandById(instUrl, cmdId), false));
        // Kill
        menu.appendChild(_createCtxMenuItem('Kill', () => killCommand(instUrl, cmdId), true));
    } else {
        // Separator
        const sep1 = document.createElement('div');
        sep1.className = 'ctx-menu-sep';
        sep1.setAttribute('role', 'separator');
        menu.appendChild(sep1);
        // Purge
        menu.appendChild(_createCtxMenuItem('Purge', () => purgeCommand(instUrl, cmdId, cmdName), true));
    }

    _positionCtxMenu(menu, e.clientX, e.clientY);
    _setupCtxMenuListeners(menu);
}

function showPanelContextMenu(e, panelId) {
    e.preventDefault();
    closeContextMenu();
    const panel = state.panels.find(p => p.id === panelId);
    if (!panel) return;

    const instUrl = panel.instUrl;
    // Determine which command is shown in this panel
    const isSelectedPanel = (instUrl === state.selectedInstUrl);
    const cmdId = isSelectedPanel ? state.selectedCmdId : null;

    const menu = document.createElement('div');
    menu.id = 'ctxMenu';
    menu.className = 'ctx-menu';
    menu.setAttribute('role', 'menu');

    // Copy URL
    menu.appendChild(_createCtxMenuItem('Copy URL', () => {
        if (cmdId) {
            // Find the command name from instance data
            const inst = state.instanceUrls.find(i => i.url === instUrl);
            const cmd = inst && inst._commands ? inst._commands.find(c => c.id === cmdId) : null;
            const cmdName = cmd ? (cmd.name || cmd.id) : cmdId;
            copyCommandUrl(instUrl, cmdId, cmdName);
        } else {
            // Just copy the instance URL
            navigator.clipboard.writeText(instUrl).catch(() => {});
        }
    }, false));

    if (cmdId) {
        // Pause/Resume
        menu.appendChild(_createCtxMenuItem('Pause/Resume', () => togglePauseCmd(instUrl, cmdId), false));
        // Restart
        menu.appendChild(_createCtxMenuItem('Restart', () => restartCommandById(instUrl, cmdId), false));
        // Kill
        menu.appendChild(_createCtxMenuItem('Kill', () => killCommand(instUrl, cmdId), true));
    }

    // Separator
    const sep = document.createElement('div');
    sep.className = 'ctx-menu-sep';
    sep.setAttribute('role', 'separator');
    menu.appendChild(sep);

    // Remove Panel (only if more than one panel)
    if (state.panels.length > 1) {
        menu.appendChild(_createCtxMenuItem('Remove Panel', () => removePanel(panelId), true));
    }

    _positionCtxMenu(menu, e.clientX, e.clientY);
    _setupCtxMenuListeners(menu);
}

function copyCommandUrl(instUrl, cmdId, cmdName) {
    const base = cmdName.replace(/.*\//, ''); // basename
    const url = instUrl.replace(/^http/, 'http') + '/' + encodeURIComponent(base);
    navigator.clipboard.writeText(url).catch(() => {});
}

async function togglePauseCmd(instUrl, cmdId) {
    // Temporarily set the selected command so togglePauseRun targets the right one
    const prevInstUrl = state.selectedInstUrl;
    const prevCmdId = state.selectedCmdId;
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    await togglePauseRun();
    // Restore previous selection if the panel context menu was for a non-selected panel
    if (prevInstUrl !== instUrl || prevCmdId !== cmdId) {
        state.selectedInstUrl = prevInstUrl;
        state.selectedCmdId = prevCmdId;
    }
}

// ─── Auto-fit Terminal on Window Resize ───
function autoFitActiveTerminal() {
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    if (!vttyEl) return;
    const rect = vttyEl.getBoundingClientRect();
    if (rect.width < 10 || rect.height < 10) return; // too small or hidden
    const charW = state.fontSize * 0.6;
    const charH = state.fontSize * 1.2;
    const cols = Math.max(20, Math.min(500, Math.floor(rect.width / charW)));
    const rows = Math.max(5, Math.min(200, Math.floor(rect.height / charH)));
    // Only resize if dimensions actually changed
    if (rows !== state._termRows || cols !== state._termCols) {
        fetch(apiUrl(`/api/commands/${state.selectedCmdId}/resize`, { url: state.selectedInstUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: state.selectedInstUrl }),
            body: JSON.stringify({ rows, cols }),
        }).catch(() => {});
    }
}

// ─── Max Fit Toggle ───
// Per-panel state: stores the previous rows/cols before max-fit was applied,
// so toggling back restores them.
const _maxFitState = {};  // panelId → { prevRows, prevCols, active }

/// Toggle "max fit" mode: resize the terminal rows/cols to the maximum that
/// fits in the panel container at the current font size.  Toggle back to
/// restore the previous dimensions.
async function toggleMaxFit(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;

    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const vttyEl = panelEl.querySelector('.vtty-container');
    if (!vttyEl) return;

    const st = _maxFitState[panelId];
    const btn = document.getElementById('maxFitBtn-' + panelId);

    if (st && st.active) {
        // Toggle back: restore previous dimensions
        st.active = false;
        if (btn) {
            btn.textContent = 'Max fit';
            btn.style.background = '';
            btn.style.color = '';
        }
        await _resizePanelTo(panelId, st.prevRows, st.prevCols);
    } else {
        // Apply max fit: calculate max rows/cols from container + current font
        const rect = vttyEl.getBoundingClientRect();
        if (rect.width < 10 || rect.height < 10) return;

        const fontSize = panelObj.fontSize || state.fontSize;
        const charW = fontSize * 0.6;
        const charH = fontSize * 1.2;
        const maxCols = Math.max(20, Math.min(500, Math.floor(rect.width / charW)));
        const maxRows = Math.max(5, Math.min(200, Math.floor(rect.height / charH)));

        // Save current dimensions
        const curRows = parseInt(document.getElementById('resizeRows-' + panelId)?.value) || 24;
        const curCols = parseInt(document.getElementById('resizeCols-' + panelId)?.value) || 80;

        _maxFitState[panelId] = { prevRows: curRows, prevCols: curCols, active: true };
        if (btn) {
            btn.textContent = 'Restore';
            btn.style.background = 'var(--accent)';
            btn.style.color = '#fff';
        }
        await _resizePanelTo(panelId, maxRows, maxCols);
    }
}

// ─── Max Font Toggle ───
// Per-panel state: stores the previous font size before max-font was applied.
const _maxFontState = {};  // panelId → { prevFontSize, prevRows, prevCols, active }

/// Toggle "max font" mode: increase the panel font size to the largest value
/// that still allows the current terminal dimensions (rows x cols) to fit
/// within the panel container.  Toggle back to restore the previous font size.
async function toggleMaxFont(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;

    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const vttyEl = panelEl.querySelector('.vtty-container');
    if (!vttyEl) return;

    const rect = vttyEl.getBoundingClientRect();
    if (rect.width < 10 || rect.height < 10) return;

    const st = _maxFontState[panelId];
    const btn = document.getElementById('maxFontBtn-' + panelId);

    const curRows = parseInt(document.getElementById('resizeRows-' + panelId)?.value) || 24;
    const curCols = parseInt(document.getElementById('resizeCols-' + panelId)?.value) || 80;

    if (st && st.active) {
        // Toggle back: restore previous font size and terminal dimensions
        st.active = false;
        if (btn) {
            btn.textContent = 'Max font';
            btn.style.background = '';
            btn.style.color = '';
        }
        // Restore font size
        panelObj.fontSize = st.prevFontSize;
        localStorage.setItem('vrw_panel_font_' + panelId, panelObj.fontSize.toString());
        if (vttyEl) vttyEl.style.fontSize = panelObj.fontSize + 'px';
        const label = document.querySelector(`#${panelId} .panel-font-size`);
        if (label) label.textContent = panelObj.fontSize + 'px';
        // Restore terminal dimensions
        await _resizePanelTo(panelId, st.prevRows, st.prevCols);
    } else {
        // Calculate max font size: largest font where rows*charH <= paneH and cols*charW <= paneW
        // charW ≈ fontSize * 0.6, charH ≈ fontSize * 1.2
        // So: fontSize * 1.2 * rows <= rect.height → fontSize <= rect.height / (1.2 * rows)
        //     fontSize * 0.6 * cols <= rect.width  → fontSize <= rect.width / (0.6 * cols)
        const maxFontByHeight = rect.height / (1.2 * curRows);
        const maxFontByWidth = rect.width / (0.6 * curCols);
        const maxFont = Math.floor(Math.min(maxFontByHeight, maxFontByWidth));
        const newFontSize = Math.max(8, Math.min(28, maxFont));

        // Save current state
        _maxFontState[panelId] = {
            prevFontSize: panelObj.fontSize,
            prevRows: curRows,
            prevCols: curCols,
            active: true,
        };
        if (btn) {
            btn.textContent = 'Restore';
            btn.style.background = 'var(--accent)';
            btn.style.color = '#fff';
        }

        // Apply new font size
        panelObj.fontSize = newFontSize;
        localStorage.setItem('vrw_panel_font_' + panelId, panelObj.fontSize.toString());
        if (vttyEl) vttyEl.style.fontSize = panelObj.fontSize + 'px';
        const label = document.querySelector(`#${panelId} .panel-font-size`);
        if (label) label.textContent = panelObj.fontSize + 'px';
    }
}

/// Helper: resize a panel's terminal to specific rows/cols via the API.
async function _resizePanelTo(panelId, rows, cols) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const cmdId = (panelObj.instUrl === state.selectedInstUrl) ? state.selectedCmdId : null;
    if (!cmdId) return;

    // Update the input fields
    const ri = document.getElementById('resizeRows-' + panelId);
    const ci = document.getElementById('resizeCols-' + panelId);
    if (ri) ri.value = rows;
    if (ci) ci.value = cols;

    try {
        await fetch(apiUrl(`/api/commands/${cmdId}/resize`, { url: panelObj.instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: panelObj.instUrl, token: panelObj.token }),
            body: JSON.stringify({ rows, cols }),
        });
    } catch (e) { /* ignore */ }
}

// ─── Keyboard Shortcuts Help ───
function showShortcuts() {
    closeShortcuts();
    const overlay = document.createElement('div');
    overlay.className = 'shortcuts-overlay';
    overlay.id = 'shortcutsOverlay';
    overlay.onclick = (e) => { if (e.target === overlay) closeShortcuts(); };
    overlay.innerHTML = `<div class="shortcuts-panel">
        <h2>Keyboard Shortcuts</h2>
        <table>
            <tr><td>?</td><td>Show this help</td></tr>
            <tr><td>Ctrl+F</td><td>Search in terminal</td></tr>
            <tr><td>Ctrl+Shift+C</td><td>Copy terminal selection</td></tr>
            <tr><td>Ctrl+Shift+S / Alt+S</td><td>Toggle selection mode</td></tr>
            <tr><td>Escape</td><td>Close search / menu</td></tr>
            <tr><td>Alt+Left / Alt+Right</td><td>Navigate prev/next command</td></tr>
            <tr><td>Any key</td><td>Focus key input (when not in a field)</td></tr>
            <tr><td>Enter</td><td>Send keystrokes to terminal</td></tr>
        </table>
        <p style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.5rem;">Click on the terminal to focus the key input field.</p>
        <div style="text-align:right;margin-top:0.75rem;">
            <button class="btn" onclick="closeShortcuts()">Close</button>
        </div>
    </div>`;
    document.body.appendChild(overlay);
    // Trap focus inside the shortcuts panel and focus the close button
    const shortcutsPanel = overlay.querySelector('.shortcuts-panel');
    if (shortcutsPanel) trapFocus(shortcutsPanel);
    const closeBtn = overlay.querySelector('button');
    if (closeBtn) closeBtn.focus();
}

function closeShortcuts() {
    releaseCurrentFocusTrap();
    const el = document.getElementById('shortcutsOverlay');
    if (el) el.remove();
}

// ─── Resource Polling ───
async function pollResources() {
    // Fetch all alive commands' resources in PARALLEL (not serial).
    const promises = [];
    for (const inst of state.instanceUrls) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false) continue;
            promises.push((async () => {
                try {
                    const res = await fetch(apiUrl(`/api/commands/${cmd.id}/resources`, { url: inst.url }), {
                        headers: authHeadersForInstance(inst),
                    });
                    const json = await res.json();
                    if (json.status === 'ok' && json.data) {
                        state._resourceCache[cmd.id] = json.data;
                    }
                } catch (e) {
                    // Silently ignore — resources are optional
                }
            })());
        }
    }
    await Promise.all(promises);
    // Update sidebar resource text without full DOM rebuild
    updateSidebarResourceText();
}

/// Update the .cmd-detail-inline text in sidebar command items to reflect
/// the latest resource data from state._resourceCache. This avoids a full
/// DOM rebuild (which the fingerprint optimization would skip anyway).
function updateSidebarResourceText() {
    for (const inst of state.instanceUrls) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false) continue;
            const res = state._resourceCache[cmd.id];
            const item = document.querySelector(`.cmd-item[data-cmd-id="${cmd.id}"]`);
            if (!item) continue;
            const isFrozen = cmd.frozen === true;
            const runtimeStr = cmd.runtime_secs > 0
                ? (cmd.runtime_secs < 60 ? Math.floor(cmd.runtime_secs) + 's'
                   : cmd.runtime_secs < 3600 ? Math.floor(cmd.runtime_secs / 60) + 'm ' + Math.floor(cmd.runtime_secs % 60) + 's'
                   : Math.floor(cmd.runtime_secs / 3600) + 'h ' + Math.floor((cmd.runtime_secs % 3600) / 60) + 'm')
                : '';
            const frozenBadge = isFrozen ? 'PAUSED' : '';
            const detailParts = [];
            if (runtimeStr) detailParts.push(runtimeStr);
            if (frozenBadge) detailParts.push(frozenBadge);
            if (res && res.cpu_percent != null) detailParts.push('CPU ' + res.cpu_percent.toFixed(1) + '%');
            if (res && res.memory_mb != null) detailParts.push('MEM ' + res.memory_mb.toFixed(1) + 'MB');
            if (cmd.pid) detailParts.push('pid ' + cmd.pid);

            // Find or create the detail row
            let detailRow = item.querySelector('.cmd-detail-row');
            if (detailParts.length === 0) {
                if (detailRow) detailRow.remove();
            } else {
                if (!detailRow) {
                    detailRow = document.createElement('div');
                    detailRow.className = 'cmd-detail-row';
                    item.appendChild(detailRow);
                }
                detailRow.innerHTML = detailParts.join('<span class="detail-sep">|</span>');
            }
        }
    }
}

// ─── Restart Command ───
async function restartCommand(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const inst = state.instanceUrls.find(i => i.url === panelObj.instUrl);
    if (!inst || !inst._commands) return;
    const isForSelected = (panelObj.instUrl === state.selectedInstUrl);
    const cmdId = isForSelected ? state.selectedCmdId : null;
    if (!cmdId) return;
    await restartCommandById(panelObj.instUrl, cmdId);
}

async function restartCommandById(instUrl, cmdId) {
    // Use the atomic restart endpoint: the server spawns the new command
    // FIRST, then kills the old one.  This prevents the server from
    // shutting down when the old command was the last one running.
    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/restart`, { url: instUrl }), {
            method: 'POST',
            headers: authHeadersForInstance({ url: instUrl }),
            body: JSON.stringify({}),
        });
        const json = await res.json();
        if (json.status === 'ok' && json.data && json.data.id) {
            const newId = json.data.id;
            state.selectedInstUrl = instUrl;
            state.selectedCmdId = newId;
            _lastCommandState = '';
            // Reload command list so the sidebar contains the new command.
            await loadCommands();
            // Find the new command's name from the refreshed list.
            const inst = state.instanceUrls.find(i => i.url === instUrl);
            let newName = newId;
            if (inst && inst._commands) {
                const newCmd = inst._commands.find(c => c.id === newId);
                if (newCmd) newName = newCmd.name || newCmd.id;
            }
            // Stop the old WS/poll (connected to the now-dead old command)
            // and start fresh with the new command.
            selectCommand(instUrl, newId, newName);
        }
    } catch (e) { /* ignore */ }
}

// ─── Welcome Panel Spawn ───
async function spawnFromWelcome() {
    const input = document.getElementById('welcomeCmd');
    if (!input || !input.value.trim()) return;
    const cmd = input.value.trim();
    const instUrl = getBaseUrl();
    try {
        const res = await fetch(apiUrl('/api/commands', { url: instUrl }), {
            method: 'POST',
            headers: authHeaders(),
            body: JSON.stringify({ cmd }),
        });
        const json = await res.json();
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    } catch (e) {
        alert('Spawn failed: ' + e.message);
    }
}

// ─── Command Pinning / Favorites ───
function getPinnedNames() {
    try {
        return JSON.parse(localStorage.getItem('vrw_pinned_cmds') || '[]');
    } catch { return []; }
}

function setPinnedNames(names) {
    localStorage.setItem('vrw_pinned_cmds', JSON.stringify(names));
}

function togglePinCmd(cmdName) {
    const pinned = getPinnedNames();
    const idx = pinned.indexOf(cmdName);
    if (idx >= 0) {
        pinned.splice(idx, 1);
    } else {
        pinned.push(cmdName);
    }
    setPinnedNames(pinned);
    loadCommands();
}

function rearrangePinnedCommands(container) {
    // This is called before innerHTML is set, so we work with the container
    // after it's rendered. The actual DOM rearrangement happens after container.innerHTML is set.
    // We use a MutationObserver-like approach: after innerHTML, rearrange.
    setTimeout(() => {
        if (!container) return;
        const items = container.querySelectorAll('.cmd-item[data-cmd-name]');
        const pinned = getPinnedNames();
        const pinnedItems = [];
        const unpinnedItems = [];
        items.forEach(item => {
            const name = item.dataset.cmdName;
            if (pinned.includes(name)) {
                pinnedItems.push(item);
            } else {
                unpinnedItems.push(item);
            }
        });
        if (pinnedItems.length > 0 && unpinnedItems.length > 0) {
            // Create pinned section header
            const header = document.createElement('div');
            header.className = 'pinned-section-header';
            header.textContent = '★ Pinned';
            // Insert pinned items first
            const parent = items[0] && items[0].parentNode;
            if (parent) {
                const first = parent.firstChild;
                // Remove all items, then re-add in pinned-first order
                items.forEach(item => item.remove());
                if (first) {
                    parent.insertBefore(header, first);
                    pinnedItems.forEach(item => parent.insertBefore(item, first));
                }
                unpinnedItems.forEach(item => parent.appendChild(item));
            }
        }
        // Update pin button icons
        container.querySelectorAll('.pin-btn').forEach(btn => {
            const item = btn.closest('.cmd-item');
            if (item && pinned.includes(item.dataset.cmdName)) {
                btn.classList.add('active');
                btn.textContent = '★';
                btn.title = 'Unpin';
            } else {
                btn.classList.remove('active');
                btn.textContent = '☆';
                btn.title = 'Pin';
            }
        });
    }, 0);
}

// ─── Command Templates ───
// Server-side templates are loaded from the vrw config file ([[templates]]).
// User templates are stored in localStorage and are editable in the web UI.
let _serverTemplates = []; // cached from /api/templates

function getServerTemplates() {
    return _serverTemplates;
}

async function fetchServerTemplates() {
    try {
        const res = await fetch(apiUrl('/api/templates'), { headers: authHeaders() });
        const json = await res.json();
        if (json.status === 'ok') {
            _serverTemplates = json.data || [];
        }
    } catch { /* ignore — use cached */ }
}

function getUserTemplates() {
    try {
        return JSON.parse(localStorage.getItem('vrw_templates') || '[]');
    } catch { return []; }
}

function saveUserTemplates(templates) {
    localStorage.setItem('vrw_templates', JSON.stringify(templates));
}

function renderTemplates() {
    const container = document.getElementById('templateList');
    if (!container) return;

    const server = getServerTemplates();
    const user = getUserTemplates();
    const hasAny = server.length > 0 || user.length > 0;

    if (!hasAny) {
        container.innerHTML = '<div style="padding:0.5rem;color:var(--text-muted);font-size:0.7rem;text-align:center;">No templates configured. Add templates in your config file under [[templates]].</div>';
        return;
    }

    let html = '';

    // Server templates section
    if (server.length > 0) {
        html += '<div style="font-size:0.6rem;color:var(--text-muted);padding:0.2rem 0.3rem;text-transform:uppercase;letter-spacing:0.05em;">From config</div>';
        html += server.map((t, i) => {
            const detail = [t.cmd, t.args].filter(Boolean).join(' ');
            const extras = [];
            if (t.workdir) extras.push('dir: ' + t.workdir);
            if (t.certificate) extras.push('cert: ' + t.certificate);
            if (t.rows || t.cols) extras.push((t.rows || '?') + 'x' + (t.cols || '?'));
            const extraStr = extras.length > 0 ? extras.join(' | ') : '';
            return `<div class="template-card" onclick="spawnServerTemplate(${i})" title="Click to spawn this command">
                <div style="display:flex;align-items:center;gap:0.3rem;">
                    <div class="template-name">${escHtml(t.name)}</div>
                    <span style="font-size:0.5rem;background:var(--accent);color:#fff;padding:0 0.25rem;border-radius:2px;">config</span>
                </div>
                <div class="template-cmd">${escHtml(detail)}</div>
                ${extraStr ? `<div style="font-size:0.6rem;color:var(--text-muted);padding-left:0.2rem;">${escHtml(extraStr)}</div>` : ''}
            </div>`;
        }).join('');
    }

    // User templates section
    if (user.length > 0) {
        html += '<div style="font-size:0.6rem;color:var(--text-muted);padding:0.3rem 0.3rem 0.1rem;text-transform:uppercase;letter-spacing:0.05em;">Custom</div>';
        html += user.map((t, i) => `
            <div class="template-card" onclick="spawnUserTemplate(${i})" title="Click to spawn this command">
                <div class="template-name">${escHtml(t.name)}</div>
                <div class="template-cmd">${escHtml(t.cmd)}${t.args ? ' ' + escHtml(t.args) : ''}</div>
                <div class="template-actions">
                    <button class="btn btn-xs btn-danger" onclick="event.stopPropagation();deleteUserTemplate(${i})" title="Delete">&#x2715;</button>
                </div>
            </div>
        `).join('');
    }

    container.innerHTML = html;
}

function spawnServerTemplate(index) {
    const t = getServerTemplates()[index];
    if (!t) return;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect ? instSelect.value : getBaseUrl();
    const args = t.args ? t.args.split(/\s+/) : [];
    const body = { cmd: t.cmd, args };
    if (t.env && t.env.length > 0) body.env = t.env;
    if (t.workdir) body.workdir = t.workdir;
    if (t.certificate) body.certificate = t.certificate;
    if (t.rows) body.rows = t.rows;
    if (t.cols) body.cols = t.cols;
    fetch(apiUrl('/api/commands', { url: instUrl }), {
        method: 'POST',
        headers: authHeadersForInstance({ url: instUrl }),
        body: JSON.stringify(body),
    }).then(res => res.json()).then(json => {
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
            const cmdTab = document.querySelector('.sidebar-tab');
            if (cmdTab) switchSidebarTab('commands', cmdTab);
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    }).catch(e => alert('Spawn failed: ' + e.message));
}

function spawnUserTemplate(index) {
    const user = getUserTemplates();
    const t = user[index];
    if (!t) return;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect ? instSelect.value : getBaseUrl();
    const args = t.args ? t.args.split(/\s+/) : [];
    const body = { cmd: t.cmd, args };
    fetch(apiUrl('/api/commands', { url: instUrl }), {
        method: 'POST',
        headers: authHeadersForInstance({ url: instUrl }),
        body: JSON.stringify(body),
    }).then(res => res.json()).then(json => {
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
            const cmdTab = document.querySelector('.sidebar-tab');
            if (cmdTab) switchSidebarTab('commands', cmdTab);
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    }).catch(e => alert('Spawn failed: ' + e.message));
}

function deleteUserTemplate(index) {
    const templates = getUserTemplates();
    templates.splice(index, 1);
    saveUserTemplates(templates);
    renderTemplates();
}

function showAddTemplateForm() {
    const form = document.getElementById('templateAddForm');
    if (form) form.style.display = '';
}

function hideAddTemplateForm() {
    const form = document.getElementById('templateAddForm');
    if (form) form.style.display = 'none';
    document.getElementById('templateName').value = '';
    document.getElementById('templateCmd').value = '';
    document.getElementById('templateArgs').value = '';
}

function saveTemplate() {
    const name = document.getElementById('templateName').value.trim();
    const cmd = document.getElementById('templateCmd').value.trim();
    const args = document.getElementById('templateArgs').value.trim();
    if (!name || !cmd) { alert('Name and command are required'); return; }
    const templates = getUserTemplates();
    templates.push({ name, cmd, args });
    saveUserTemplates(templates);
    hideAddTemplateForm();
    renderTemplates();
}

// ─── Drag-and-Drop Panel Reorder ───
let _draggedPanelId = null;

function onPanelDragStart(e, panelId) {
    _draggedPanelId = panelId;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', panelId);
    setTimeout(() => {
        const el = document.getElementById(panelId);
        if (el) el.classList.add('dragging');
    }, 0);
}

function onPanelDragOver(e) {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    const panel = e.target.closest('.panel');
    if (!panel || panel.id === _draggedPanelId) return;
    const rect = panel.getBoundingClientRect();
    const midX = rect.left + rect.width / 2;
    panel.classList.remove('drag-over-left', 'drag-over-right');
    if (e.clientX < midX) {
        panel.classList.add('drag-over-left');
    } else {
        panel.classList.add('drag-over-right');
    }
}

function onPanelDragLeave(e) {
    const panel = e.target.closest('.panel');
    if (panel) panel.classList.remove('drag-over-left', 'drag-over-right');
}

function onPanelDrop(e, targetPanelId) {
    e.preventDefault();
    if (!_draggedPanelId || _draggedPanelId === targetPanelId) {
        onPanelDragEnd(e);
        return;
    }
    const container = document.getElementById('view-vtty');
    const draggedEl = document.getElementById(_draggedPanelId);
    const targetEl = document.getElementById(targetPanelId);
    if (!draggedEl || !targetEl || !container) {
        onPanelDragEnd(e);
        return;
    }
    // Determine insert position
    const rect = targetEl.getBoundingClientRect();
    const midX = rect.left + rect.width / 2;
    if (e.clientX < midX) {
        container.insertBefore(draggedEl, targetEl);
    } else {
        container.insertBefore(draggedEl, targetEl.nextSibling);
    }
    // Also remove the resize handle and re-add it after the panel
    const handle = draggedEl.nextElementSibling;
    if (handle && handle.classList.contains('panel-resize-handle')) {
        container.removeChild(handle);
        const nextEl = draggedEl.nextElementSibling;
        container.insertBefore(handle, nextEl);
    }
    // Update state.panels order to match DOM
    const panelEls = container.querySelectorAll('.panel');
    const newOrder = [];
    panelEls.forEach(el => {
        const p = state.panels.find(pp => pp.id === el.id);
        if (p) newOrder.push(p);
    });
    state.panels = newOrder;
    localStorage.setItem('vrw_panel_order', JSON.stringify(newOrder.map(p => p.id)));
    onPanelDragEnd(e);
}

function onPanelDragEnd(e) {
    _draggedPanelId = null;
    document.querySelectorAll('.panel').forEach(p => {
        p.classList.remove('dragging', 'drag-over-left', 'drag-over-right');
    });
}

// ─── Global Search ───
function openGlobalSearch() {
    const modal = document.getElementById('globalSearchModal');
    modal.style.display = '';
    const input = document.getElementById('globalSearchInput');
    input.value = '';
    input.focus();
    document.getElementById('globalSearchResults').innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;font-size:0.75rem;">Type a query and press Enter to search across all command output</div>';
}

function closeGlobalSearch() {
    const modal = document.getElementById('globalSearchModal');
    modal.style.display = 'none';
}

async function executeGlobalSearch() {
    const query = document.getElementById('globalSearchInput').value.trim();
    if (!query) return;
    const resultsContainer = document.getElementById('globalSearchResults');
    resultsContainer.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;font-size:0.75rem;">Searching...</div>';
    let allResults = [];
    for (const inst of state.instanceUrls) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            try {
                const res = await fetch(apiUrl(`/api/commands/${cmd.id}/vtty/text`, { url: inst.url }), {
                    headers: authHeadersForInstance(inst),
                });
                if (!res.ok) continue;
                const json = await res.json();
                if (json.status !== 'ok' || !json.data || !json.data.text) continue;
                const lines = json.data.text.split('\n');
                const cmdName = cmd.name || cmd.id;
                const matchingLines = [];
                lines.forEach((line, idx) => {
                    if (line.toLowerCase().includes(query.toLowerCase())) {
                        matchingLines.push({ lineNum: idx + 1, text: line.trim() });
                    }
                });
                if (matchingLines.length > 0) {
                    allResults.push({ cmdName, cmdId: cmd.id, instUrl: inst.url, lines: matchingLines.slice(0, 50) });
                }
            } catch (e) { /* skip */ }
        }
    }
    if (allResults.length === 0) {
        resultsContainer.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;font-size:0.75rem;">No results found</div>';
        return;
    }
    resultsContainer.innerHTML = allResults.map(group => `
        <div class="search-result-group">
            <div class="search-result-header" onclick="selectCommand('${escHtml(group.instUrl)}','${escHtml(group.cmdId)}','${escHtml(group.cmdName)}');closeGlobalSearch()">
                ${escHtml(group.cmdName)} <span style="color:var(--text-muted);font-size:0.6rem;">(${group.lines.length} matches)</span>
            </div>
            ${group.lines.map(l => `<div class="search-result-line" title="${escHtml(l.text)}"><span style="color:var(--text-muted);">${l.lineNum}:</span> ${escHtml(l.text)}</div>`).join('')}
        </div>
    `).join('');
}

// ─── Sound Notifications ───
function initSoundToggle() {
    const btn = document.getElementById('soundBtn');
    if (!btn) return;
    if (state.soundEnabled) btn.classList.add('sound-btn-active');
}

function toggleSoundNotifications() {
    state.soundEnabled = !state.soundEnabled;
    localStorage.setItem('vrw_sound', state.soundEnabled.toString());
    const btn = document.getElementById('soundBtn');
    if (btn) btn.classList.toggle('sound-btn-active', state.soundEnabled);
}

function playExitSound(success) {
    try {
        const ctx = new (window.AudioContext || window.webkitAudioContext)();
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.connect(gain);
        gain.connect(ctx.destination);
        if (success) {
            osc.frequency.value = 880;
            osc.type = 'sine';
        } else {
            osc.frequency.value = 440;
            osc.type = 'square';
        }
        gain.gain.value = 0.1;
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
        osc.start(ctx.currentTime);
        osc.stop(ctx.currentTime + 0.5);
    } catch (e) { /* ignore — audio not supported */ }
}
