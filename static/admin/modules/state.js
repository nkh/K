// ─── Event Bus ───
const Events = {
    _handlers: {},
    on(event, handler) {
        (this._handlers[event] = this._handlers[event] || []).push(handler);
    },
    off(event, handler) {
        const list = this._handlers[event];
        if (!list) return;
        this._handlers[event] = list.filter(h => h !== handler);
    },
    emit(event, data) {
        (this._handlers[event] || []).forEach(h => { try { h(data); } catch (e) { console.error('Event handler error:', e); } });
    },
};

// ─── State ───
const state = {
    // Flat list of visible commands for prev/next navigation.
    // Each entry: { instUrl, cmdId, name }
    _navCommands: [],
    // Whether the welcome panel is currently displayed
    _showingWelcome: true,
    // Sidebar sort: 'name' sorts all commands alphabetically, 'instance' groups by instance.
    // Clicking an instance header sets it to that instance URL.
    _sidebarSort: 'name',
    // Global search freeze state: tracks which panels had their updates stopped
    // while the search overlay was open, and which commands were frozen.
    _searchFrozenPanelIds: new Set(),
    _searchFrozenCmdIds: [], // { instUrl, cmdId, wasFrozen }

    panels: [],
    // Store instUrl and cmdId separately to avoid ':' conflicts in URLs.
    selectedInstUrl: null,
    selectedCmdId: null,
    authToken: localStorage.getItem('vrw_auth_token') || '',
    refreshInterval: null,
    fontSize: parseInt(localStorage.getItem('vrw_font_size') || '10'),
    instanceUrls: [],
    currentView: 'vtty',
    // Buffer view: 'current', 'main', 'alt' — GLOBAL for shared toolbar
    bufferView: 'current',
    // Last-known buffer generation per command ID. Used to skip redundant
    // DOM updates when the server reports no change (Level 2 optimization).
    _lastGeneration: {},
    // Whether the user is currently viewing the live buffer (not scrolled
    // into scrollback history). Used for auto-scroll decisions.
    _userAtBottom: true,
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
    // Per-command diff baseline UUIDs.  Each panel/viewer has its own
    // baseline so multiple tabs don't clobber each other.
    _diffBaselines: {},
    // VTTY update mode: 'push' (server sends dirty signals via WS)
    // or 'poll' (client polls /api/commands/:id/vtty/changed)
    updateMode: localStorage.getItem('vrw_update_mode') || 'push',
    pollInterval: parseInt(localStorage.getItem('vrw_poll_interval') || '500'),
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
    // Panel layout direction: 'row' (horizontal, side-by-side) or 'column' (vertical, stacked)
    panelLayout: localStorage.getItem('vrw_panel_layout') || 'row',
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
    // ID of the panel that last received user interaction (click, key, etc.).
    // VTTY updates (HTTP + WS diff) are routed to ALL panels, but
    // selectCommand targets this panel unless overridden.
    _focusedPanelId: null,
};

// Expose module-level vars to VRW namespace for cross-module access
window.VRW = window.VRW || {};
VRW.state = state;
VRW.Events = Events;
VRW._navCommands = state._navCommands;
VRW._showingWelcome = state._showingWelcome;
VRW._sidebarSort = state._sidebarSort;
VRW._searchFrozenPanelIds = state._searchFrozenPanelIds;
VRW._searchFrozenCmdIds = state._searchFrozenCmdIds;
window.VRWEvents = Events;
