/// test/setup.js — Browser environment mocks for Node.js test runner.
/// Sets up globalThis with window, document, localStorage, etc.,
/// then loads all vrw modules in dependency order.
'use strict';

// ── Counter for assertions ──
globalThis._testPassed = 0;
globalThis._testFailed = 0;

globalThis.assert = function(cond, msg) {
    if (!cond) { _testFailed++; console.error('  FAIL: ' + msg); } else { _testPassed++; }
};
globalThis.assertEq = function(actual, expected, msg) {
    if (actual !== expected) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — expected ' + JSON.stringify(expected) + ', got ' + JSON.stringify(actual));
    } else { _testPassed++; }
};
globalThis.assertOk = function(val, msg) {
    if (!val) { _testFailed++; console.error('  FAIL: ' + msg + ' — falsy value'); } else { _testPassed++; }
};
globalThis.assertThrows = function(fn, msg) {
    try { fn(); _testFailed++; console.error('  FAIL: ' + msg + ' — expected throw'); }
    catch(e) { _testPassed++; }
};

// ── localStorage mock ──
function createStorage() {
    const store = new Map();
    return {
        getItem(key) { return store.has(key) ? String(store.get(key)) : null; },
        setItem(key, val) { store.set(key, String(val)); },
        removeItem(key) { store.delete(key); },
        clear() { store.clear(); },
        get length() { return store.size; },
        key(i) { return [...store.keys()][i] || null; },
        _store: store,
    };
}
globalThis.localStorage = createStorage();
globalThis.sessionStorage = createStorage();

// ── Event system ──
const _listeners = new Map();
function addEventListener(type, fn, opts) {
    if (!_listeners.has(type)) _listeners.set(type, []);
    _listeners.get(type).push(fn);
}
function removeEventListener(type, fn) {
    if (!_listeners.has(type)) return;
    const arr = _listeners.get(type).filter(f => f !== fn);
    _listeners.set(type, arr);
}
function emitEvent(type, detail) {
    const evt = { type, detail, target: globalThis, preventDefault() {}, stopPropagation() {}, stopImmediatePropagation() {} };
    if (_listeners.has(type)) {
        for (const fn of _listeners.get(type)) fn(evt);
    }
    return evt;
}

// ── Element mock ──
class MockElement {
    constructor(tag) {
        this.tagName = tag ? tag.toUpperCase() : 'DIV';
        this.id = '';
        this.className = '';
        this._classList = new Set();
        this.style = new Proxy({}, { get: (t, p) => {
            if (p === 'setProperty') return () => {};
            if (p === 'getProperty') return () => '';
            if (p === 'removeProperty') return () => '';
            return t[p] || '';
        }, set: (t, p, v) => { t[p] = v; return true; } });
        this._innerHTML = '';
        this._textContent = undefined; // undefined means "derive from innerHTML"
        this.dataset = {};
        this.children = [];
        this.childNodes = [];
        this.parentElement = null;
        this.nextSibling = null;
        this.previousSibling = null;
        this.offsetLeft = 0;
        this.offsetTop = 0;
        this.offsetWidth = 800;
        this.offsetHeight = 600;
        this.scrollHeight = 0;
        this.scrollTop = 0;
        this.clientHeight = 600;
        this.scrollWidth = 0;
        this.scrollLeft = 0;
        this.clientWidth = 800;
        this.value = '';
        this.checked = false;
        this.disabled = false;
        this._type = '';
        this._href = '';
        this._src = '';
        this._listeners = new Map();
        this._attrs = {};
        this._display = '';
        this._tabIndex = 0;
        this._options = [];
        this.selected = false;
    }
    // innerHTML getter/setter
    get innerHTML() { return this._innerHTML; }
    set innerHTML(val) {
        this._innerHTML = val;
        this._textContent = undefined; // innerHTML was set explicitly
    }
    // textContent getter/setter — simulates browser behavior
    // Setting textContent auto-escapes when read back via innerHTML
    get textContent() {
        if (this._textContent !== undefined) return this._textContent;
        // Derive from innerHTML by stripping tags
        return this._innerHTML.replace(/<[^>]*>/g, '');
    }
    set textContent(val) {
        this._textContent = val;
        // When textContent is set, innerHTML returns the escaped version
        this._innerHTML = String(val)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }
    addEventListener(type, fn, opts) {
        if (!this._listeners.has(type)) this._listeners.set(type, []);
        this._listeners.get(type).push(fn);
    }
    removeEventListener(type, fn) {
        if (!this._listeners.has(type)) return;
        this._listeners.set(type, this._listeners.get(type).filter(f => f !== fn));
    }
    dispatchEvent(evt) {
        if (this._listeners.has(evt.type)) {
            for (const fn of this._listeners.get(evt.type)) fn(evt);
        }
        return true;
    }
    querySelector(sel) {
        // Simple id selector
        if (sel.startsWith('#')) {
            const el = _elementRegistry.get(sel.slice(1));
            return el || null;
        }
        // Class selector
        if (sel.startsWith('.')) {
            for (const el of _elementRegistry.values()) {
                if (el._classList && el._classList.has(sel.slice(1))) return el;
            }
        }
        // Tag selector
        for (const el of _elementRegistry.values()) {
            if (el.tagName === sel.toUpperCase()) return el;
        }
        return null;
    }
    querySelectorAll(sel) {
        const results = [];
        if (sel.startsWith('#')) {
            const el = _elementRegistry.get(sel.slice(1));
            if (el) results.push(el);
        } else {
            for (const el of _elementRegistry.values()) results.push(el);
        }
        return results;
    }
    closest(sel) { return null; }
    matches(sel) { return false; }
    getAttribute(name) { return this._attrs[name] || null; }
    setAttribute(name, val) { this._attrs[name] = String(val); }
    removeAttribute(name) { delete this._attrs[name]; }
    hasAttribute(name) { return name in this._attrs; }
    appendChild(child) {
        child.parentElement = this;
        this.children.push(child);
        this.childNodes.push(child);
        return child;
    }
    removeChild(child) {
        this.children = this.children.filter(c => c !== child);
        this.childNodes = this.childNodes.filter(c => c !== child);
        child.parentElement = null;
        return child;
    }
    insertBefore(newNode, refNode) {
        newNode.parentElement = this;
        if (refNode) {
            const idx = this.children.indexOf(refNode);
            this.children.splice(idx, 0, newNode);
        } else {
            this.children.push(newNode);
        }
        return newNode;
    }
    focus() {}
    blur() {}
    click() { this.dispatchEvent({ type: 'click', target: this, preventDefault() {}, stopPropagation() {} }); }
    get classList() {
        const self = this; // capture MockElement reference
        return {
            add(...cls) { cls.forEach(c => self._classList.add(c)); },
            remove(...cls) { cls.forEach(c => self._classList.delete(c)); },
            toggle(c) { if (self._classList.has(c)) { self._classList.delete(c); return false; } else { self._classList.add(c); return true; } },
            contains(c) { return self._classList.has(c); },
            get length() { return self._classList.size; },
        };
    }
    get outerHTML() { return '<' + this.tagName.toLowerCase() + '>' + this.innerHTML + '</' + this.tagName.toLowerCase() + '>'; }
    toString() { return '[' + this.tagName + '#' + this.id + '.' + [...this._classList].join('.') + ']'; }
}

// ── Element registry ──
const _elementRegistry = new Map();
globalThis._elementRegistry = _elementRegistry;
function registerElement(el) {
    if (el.id) _elementRegistry.set(el.id, el);
    return el;
}

// ── Document mock ──
const _doc = {
    body: null,
    documentElement: null,
    createElement(tag) {
        const el = new MockElement(tag);
        return registerElement(el);
    },
    createTextNode(text) {
        const el = new MockElement('#text');
        el.textContent = text;
        return el;
    },
    getElementById(id) {
        if (!_elementRegistry.has(id)) {
            // Auto-create stub element so code doesn't crash
            const el = new MockElement('div');
            el.id = id;
            registerElement(el);
        }
        return _elementRegistry.get(id);
    },
    querySelector(sel) {
        if (sel.startsWith('#')) return this.getElementById(sel.slice(1));
        return null;
    },
    querySelectorAll(sel) { return []; },
    addEventListener(type, fn) { addEventListener(type, fn); },
    removeEventListener(type, fn) { removeEventListener(type, fn); },
    createDocumentFragment() {
        return new MockElement('fragment');
    },
    get readyState() { return 'complete'; },
};
_doc.body = _doc.createElement('body');
_doc.documentElement = _doc.createElement('html');

// ── Window mock ──
globalThis.window = globalThis;
globalThis.document = _doc;
try { globalThis.navigator = { userAgent: 'Node.js test' }; } catch(e) { /* navigator is read-only */ }
globalThis.addEventListener = addEventListener;
globalThis.removeEventListener = removeEventListener;
globalThis.dispatchEvent = emitEvent;
globalThis.matchMedia = function(query) { return { matches: false, media: query, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} }; };
globalThis.requestAnimationFrame = function(fn) { return setTimeout(fn, 16); };
globalThis.cancelAnimationFrame = clearTimeout;

// Timers are native in Node.js — no override needed
// But track intervals/timeouts for cleanup
const _intervals = new Set();
const _origSetInterval = globalThis.setInterval;
globalThis.setInterval = function(fn, ms) {
    const id = _origSetInterval(fn, ms);
    _intervals.add(id);
    return id;
};
function clearIntervalAll() {
    for (const id of _intervals) clearInterval(id);
    _intervals.clear();
}

// ── WebSocket mock ──
class MockWebSocket {
    constructor(url) {
        this.url = url;
        this.readyState = 0; // CONNECTING
        this.onopen = null;
        this.onmessage = null;
        this.onclose = null;
        this.onerror = null;
        this._calls = [];
        // Auto-open after construction
        setTimeout(() => {
            this.readyState = 1; // OPEN
            if (this.onopen) this.onopen({ type: 'open' });
        }, 0);
    }
    send(data) { this._calls.push({ method: 'send', data }); }
    close() { this.readyState = 3; if (this.onclose) this.onclose({ type: 'close' }); }
}
globalThis.WebSocket = MockWebSocket;

// ── Notification mock ──
globalThis.Notification = class {
    constructor(title, opts) { this.title = title; this.opts = opts; }
    static permission = 'granted';
    static requestPermission() { return Promise.resolve('granted'); }
};

// ── fetch mock ──
globalThis.fetch = async function(url, opts) {
    return {
        ok: true,
        status: 200,
        statusText: 'OK',
        headers: new Map([['content-type', 'application/json']]),
        json: async () => ({ status: 'ok', data: {} }),
        text: async () => '',
        clone() { return this; },
    };
};

// ── location mock ──
globalThis.location = {
    origin: 'http://localhost:9090',
    href: 'http://localhost:9090/admin',
    pathname: '/admin',
    search: '',
    hash: '',
    host: 'localhost:9090',
    hostname: 'localhost',
    port: '9090',
    protocol: 'http:',
    reload() {},
    replace(url) { this.href = url; },
};

// ── URL / URLSearchParams ──
globalThis.URL = require('url').URL;
globalThis.URLSearchParams = require('url').URLSearchParams;

// ── console passthrough ──
globalThis.console = console;
globalThis.alert = function(msg) { console.log('[alert] ' + msg); };
globalThis.confirm = function(msg) { return true; };
globalThis.prompt = function(msg) { return ''; };

// ── setTimeout mock (tracks for cleanup) ──
const _timeouts = new Set();
const _origSetTimeout = globalThis.setTimeout;
globalThis.setTimeout = function(fn, ms) {
    const id = _origSetTimeout(fn, ms);
    _timeouts.add(id);
    return id;
};
function clearTimeoutAll() {
    for (const id of _timeouts) clearTimeout(id);
    _timeouts.clear();
}

// ── Load all modules ──
const fs = require('fs');
const path = require('path');

const moduleDir = path.join(__dirname, '..', 'modules');

const moduleOrder = [
    'state.js', 'eventbus.js', 'utils.js', 'focus.js', 'theme.js',
    'sidebar.js', 'panels.js',
    'commands-core.js', 'command-selection.js', 'command-ui.js', 'server-connections.js',
    'websocket.js', 'vtty.js',
    'spawn.js', 'logs.js', 'keyboard.js', 'search.js', 'notifications.js',
    'onboarding.js', 'templates.js', 'dragdrop.js', 'workspaces.js',
    'misc.js'
];

// Load state.js first and expose its variables globally
const stateCode = fs.readFileSync(path.join(moduleDir, 'state.js'), 'utf8');
(0, eval)(stateCode);
// state.js defines `const state` which doesn't leak to globalThis.
// Explicitly expose it so other modules can access it as a free variable.
if (typeof VRW !== 'undefined' && VRW.state) {
    globalThis.state = VRW.state;
}
// Expose module-level vars from state.js
if (typeof VRW !== 'undefined') {
    globalThis._lastCommandState = VRW._lastCommandState;
    globalThis._navCommands = VRW._navCommands;
    globalThis._showingWelcome = VRW._showingWelcome;
    globalThis._sidebarSort = VRW._sidebarSort;
    globalThis._searchFrozenPanelIds = VRW._searchFrozenPanelIds;
    globalThis._searchFrozenCmdIds = VRW._searchFrozenCmdIds;
    globalThis._lastRenderedPanelCount = VRW._lastRenderedPanelCount;
    globalThis._lastRenderedPanelIds = VRW._lastRenderedPanelIds;
    globalThis._lastSplitState = VRW._lastSplitState;
    globalThis._lastShowingWelcome = VRW._lastShowingWelcome;
}

// Some modules reference functions from other modules that are loaded later.
// Provide stubs for cross-module dependencies so loading doesn't crash.
// These will be overwritten by the actual module definitions.
const _crossDeps = [
    'updateDisconnectedUI', 'getSelectedPanel', 'getActivePanelId', 'loadSnapshot',
    'handlePeerEvent', 'notifyCommandEnded', 'connectLogWs',
    'disconnectLogWs', 'scheduleSecondaryVttyHttp', 'startRefresh',
    'loadVttyHttp', 'loadCommands', 'updatePanelCommandInfo',
    'updateTerminalDisconnectedOverlay', 'updateSidebarSelection',
    'updateSharedToolbar', 'updateCmdToolbarVisibility',
    'renderPanels', 'focusPanel', 'connectPanelWs', 'disconnectPanelWs',
    'startPanelPoll', 'stopPanelPoll', 'startUpdateMode', 'stopUpdateMode',
    'renderWorkspaceList', 'showSpecialKeysHelp', 'applyVttyDiff',
    'updateVttyDisplay', 'playExitSound', 'fetchServerTemplates',
    'loadCertificates', 'fetchEnvironments', 'fetchServerConfig',
    'applyUpdateModeUI', 'updateSidebarTabsVisibility', 'fetchPeers',
    'checkOnboarding', 'autoFitActiveTerminal', 'toggleMaxFit', 'toggleMaxFont',
    'scrollTerminalBottom', 'vttySearch', 'vttySearchNext', 'vttySearchPrev',
    'vttySearchClose', 'vttyRemoveHighlights', 'vttyApplyHighlights',
    'vttyScrollToMatch', '_updateSearchProgress',
    'updateSidebarBanner', 'initPanelDropTargets', 'addDiscoveredPeer',
    'navigatePrevCommand', 'navigateNextCommand', 'updateSidebarResourceText',
    'onCmdDragStart', 'openGlobalSearch', 'closeGlobalSearch', 'executeGlobalSearch',
    'onSearchResultClick', 'updateFrozenIndicator', 'cmdManagerKillAll',
    'openCmdManagerSpawn', 'renderCmdManagerList',
    'changePanelFontSize', '_isTerminalVisible', 'savePeersToStorage',
    'disconnectServer', 'closePanelModal', 'confirmAddServer',
    'renderGroups', 'renderTemplates', 'addConnection', 'removeConnection',
    '_flushPendingVttyUpdate', 'toggleSelectionMode', 'startPanelUpdateMode',
    'stopPanelUpdateMode', 'stopPanelPoll', 'startPanelPoll',
    'connectPanelWs', 'disconnectPanelWs', 'disconnectAllPanelWs',
    'updateVttyDisplayForPanel', 'updateVttyMetadataForPanel', 'applyVttyDiffForPanel',
    'scheduleVttyHttpForPanel', 'loadVttyHttpForPanel',
    'updateVttyMetadataFromHttp', 'switchBuffer', 'buildCellGrid',
    'notifyCommandEnded', 'pollResources', 'updateSidebarResourceText',
    'pollOncePanel', 'pollOnce', '_maxFontState', '_maxFitState',
    '_openCommandInNewPane', 'copyTerminalSelection', 'exportTerminal',
    'screenshotPanel', 'closeContextMenu', 'showCmdContextMenu',
    'showPanelContextMenu', 'startRenamePanel', 'finishRenamePanel',
    'copyCommandUrl', 'togglePauseCmd', 'autoFitActiveTerminal',
    'selectCommand', 'lookupAndSelectCommand', 'showCommandPicker',
    'pickCommand', 'navigateCommand', 'parseLogLine', 'sendDirectKey',
    'scheduleVttyHttp', 'sendMouseEvent',
    'togglePanelTheme', 'applyPanelTheme', 'toggleSoundNotifications',
    'changeFontSize', 'changeRefreshMs', 'showShortcuts', 'closeShortcuts',
    'togglePauseRun', 'togglePauseRunPanel',
    'restartCommand', 'restartCommandById',
    'updateCertDropdown', 'updateInstanceDropdown',
    'saveUserTemplates', 'getUserTemplates', 'saveCmdGroups', 'getCmdGroups',
    'getWorkspaces', 'deleteWorkspace', 'saveWorkspaces',
    'parseSpawnArgs', 'parseSpawnEnvVars',
    'saveToken', 'loadToken', 'renderMarkdown',
    'togglePanelLayout', 'toggleLayoutPresetMenu', 'applyLayoutPreset',
    '_resizePanelTo', '_onboardingSteps', '_hex',
];
for (const fn of _crossDeps) {
    if (typeof globalThis[fn] === 'undefined') {
        globalThis[fn] = function() {};
    }
}

// Now load the remaining modules (skip state.js, already loaded)
for (const file of moduleOrder.slice(1)) {
    const code = fs.readFileSync(path.join(moduleDir, file), 'utf8');
    try {
        (0, eval)(code);
    } catch (e) {
        console.error('ERROR loading ' + file + ': ' + e.message);
    }
}

// Also load app.js init (but not the IIFE since it needs real DOM)
// app.js init will be tested separately

// ── Reset helper ──
globalThis.resetTestState = function() {
    clearIntervalAll();
    clearTimeoutAll();
    _elementRegistry.clear();
    // Re-register body
    _doc.body = _doc.createElement('body');
    _elementRegistry.set('body', _doc.body);
    _listeners.clear();
    localStorage.clear();
    sessionStorage.clear();
    // Reset state if available
    if (typeof state !== 'undefined' && state) {
        state.panels = [];
        state.connections = [];
        state.selectedCmdId = null;
        state.selectedInstUrl = null;
        state.authToken = '';
        state.fontSize = 10;
        state.currentView = 'vtty';
        state.updateMode = 'push';
        state.pollInterval = 500;
        state.refreshMs = 0;
        state.panelLayout = 'row';
        state.showResources = false;
        state.soundEnabled = false;
        state.serverReachable = false;
        state._focusedPanelId = null;
        state._userScrolling = false;
        state._mobileTabbedLayout = false;
        state._pendingVttyDirty = false;
        state._pendingVttyData = null;
        state._refreshThrottleTimer = null;
        state._lastRenderedPanelCount = -1;
        state._lastRenderedPanelIds = '';
        state._lastShowingWelcome = true;
        state._showingWelcome = true;
        state.refreshInterval = null;
        state._resourceInterval = null;
        state._lastGeneration = {};
        state._cellGrids = {};
        state._cachedDomPre = {};
        state._cachedScrollPos = {};
        state._resourceCache = {};
        state._wsLatency = 0;
        state._wsPingInterval = null;
        state._wsReconnectCount = 0;
        state.bufferView = 'current';
        state._userAtBottom = true;
        state.vttyWs = null;
        state.logWs = null;
        state.logWsReconnectTimer = null;
        state._pollTimer = null;
    }
    // Sync VRW vars
    if (typeof VRW !== 'undefined') {
        VRW._lastCommandState = '';
        VRW._showingWelcome = true;
        VRW._lastRenderedPanelCount = -1;
        VRW._lastRenderedPanelIds = '';
        VRW._lastShowingWelcome = true;
        VRW._searchFrozenPanelIds = new Set();
        VRW._searchFrozenCmdIds = [];
        VRW._sidebarSort = 'name';
        _lastCommandState = '';
        _showingWelcome = true;
        _lastRenderedPanelCount = -1;
        _lastRenderedPanelIds = '';
        _lastShowingWelcome = true;
        _searchFrozenPanelIds = new Set();
        _searchFrozenCmdIds = [];
        _sidebarSort = 'name';
    }
};

console.log('[setup] Browser environment mocks loaded, all modules evaluated.');
