/// test/helpers.js — Mock factories and test utilities for refactoring.
/// Provides dependency-injection-friendly mocks for api, state, render, and DOM.
/// These replace the global-singleton pattern so each test gets isolated instances.
///
/// Usage:
///   require('./setup');  // load browser mocks + modules
///   const { createMockApi, createMockState, createMockDom, createFetchMock } = require('./helpers');
///
///   const mockApi = createMockApi();
///   const mockState = createMockState();
///   // Pass to functions under test instead of relying on globals.

'use strict';

// ── Fetch mock ──
// Replaces globalThis.fetch with a controllable mock.
// Returns an object with methods to set up responses and inspect calls.
function createFetchMock() {
    const calls = [];

    // Default: return empty success
    let _nextResponse = {
        ok: true,
        status: 200,
        statusText: 'OK',
        headers: new Map([['content-type', 'application/json']]),
        json: async () => ({}),
        text: async () => '',
        blob: async () => new Blob([], { type: 'application/octet-stream' }),
        clone() { return this; },
    };

    function mockFetch(url, opts) {
        calls.push({ url, method: opts?.method || 'GET', headers: opts?.headers, body: opts?.body });
        const resp = { ..._nextResponse };
        resp.clone = function() { return { ...resp, clone: resp.clone }; };
        return Promise.resolve(resp);
    }

    mockFetch._calls = calls;
    mockFetch._setResponse = function(resp) { _nextResponse = resp; };
    mockFetch._setJsonResponse = function(data, status = 200) {
        _nextResponse = {
            ok: status >= 200 && status < 300,
            status,
            statusText: status === 200 ? 'OK' : String(status),
            headers: new Map([['content-type', 'application/json']]),
            json: async () => data,
            text: async () => JSON.stringify(data),
            clone() { return _nextResponse; },
        };
    };
    mockFetch._setErrorResponse = function(status, errorData) {
        _nextResponse = {
            ok: false,
            status,
            statusText: String(status),
            headers: new Map([['content-type', 'application/json']]),
            json: async () => errorData || { error: 'test error' },
            clone() { return _nextResponse; },
        };
    };
    mockFetch._setTextResponse = function(text, status = 200) {
        _nextResponse = {
            ok: status >= 200 && status < 300,
            status,
            statusText: String(status),
            headers: new Map([['content-type', 'text/plain']]),
            json: async () => { throw new Error('Not JSON'); },
            text: async () => text,
            clone() { return _nextResponse; },
        };
    };
    mockFetch._setBlobResponse = function(blob, status = 200) {
        _nextResponse = {
            ok: status >= 200 && status < 300,
            status,
            statusText: String(status),
            headers: new Map([['content-type', blob.type || 'application/octet-stream']]),
            json: async () => { throw new Error('Not JSON'); },
            text: async () => '',
            blob: async () => blob,
            clone() { return _nextResponse; },
        };
    };
    mockFetch._reset = function() {
        calls.length = 0;
        _nextResponse = {
            ok: true, status: 200, statusText: 'OK',
            headers: new Map([['content-type', 'application/json']]),
            json: async () => ({}), text: async () => '',
            blob: async () => new Blob([]),
            clone() { return _nextResponse; },
        };
    };

    return mockFetch;
}

// ── Mock API ──
// Creates a mock api object matching the real api.js interface.
// Every method is a stub that records calls and returns a configurable value.
function createMockApi(opts = {}) {
    const callLog = [];

    function stub(name, defaultValue) {
        const s = function(...args) {
            callLog.push({ method: name, args });
            return defaultValue;
        };
        s._callCount = 0;
        s._lastArgs = null;
        const orig = s;
        const wrapped = function(...args) {
            s._callCount++;
            s._lastArgs = args;
            return orig.apply(this, args);
        };
        wrapped._isStub = true;
        wrapped._stubName = name;
        wrapped._callCount = 0;
        wrapped._lastArgs = null;
        return wrapped;
    }

    // Resolve/reject variants for promise-returning methods
    function stubResolve(name, resolvedValue) {
        const s = function(...args) {
            callLog.push({ method: name, args });
            return Promise.resolve(resolvedValue);
        };
        s._isStub = true;
        s._stubName = name;
        s._callCount = 0;
        s._lastArgs = null;
        const orig = s;
        const wrapped = function(...args) {
            s._callCount++;
            s._lastArgs = args;
            return orig.apply(this, args);
        };
        wrapped._isStub = true;
        wrapped._stubName = name;
        wrapped._callCount = 0;
        wrapped._lastArgs = null;
        return wrapped;
    }

    function stubReject(name, error) {
        const s = function(...args) {
            callLog.push({ method: name, args });
            return Promise.reject(error || new Error(name + ' failed'));
        };
        s._isStub = true;
        s._stubName = name;
        s._callCount = 0;
        s._lastArgs = null;
        return s;
    }

    const mock = {
        // Server info
        getInfo: stubResolve('getInfo', { name: 'test-server', version: '1.0' }),
        getCertificates: stubResolve('getCertificates', []),
        // Commands
        getCommands: stubResolve('getCommands', []),
        lookupCommand: stubResolve('lookupCommand', null),
        spawnCommand: stubResolve('spawnCommand', { id: 'new-cmd', name: 'test' }),
        getCommandResources: stubResolve('getCommandResources', { cpu: 0, mem: 0 }),
        // Command actions
        freeze: stubResolve('freeze', {}),
        thaw: stubResolve('thaw', {}),
        kill: stubResolve('kill', {}),
        killAll: stubResolve('killAll', {}),
        restart: stubResolve('restart', {}),
        keep: stubResolve('keep', {}),
        unkeep: stubResolve('unkeep', {}),
        purge: stubResolve('purge', {}),
        // Command I/O
        sendKeys: stubResolve('sendKeys', {}),
        sendMouse: stubResolve('sendMouse', {}),
        resize: stubResolve('resize', {}),
        // VTTY
        getVttyChanged: stubResolve('getVttyChanged', { changed: false }),
        getVttyHtml: stubResolve('getVttyHtml', '<pre>test</pre>'),
        getVttyPng: stubResolve('getVttyPng', new Blob()),
        // WebSocket: VTTY
        connectVtty: stub('connectVtty', { ws: { readyState: 1, close: function() {} }, close: function() {}, get readyState() { return 1; } }),
        // WebSocket: Logs
        connectLogWs: stub('connectLogWs', { ws: { readyState: 1, close: function() {} }, close: function() {} }),
        // Spawn completions
        getCompletions: stubResolve('getCompletions', []),
        // Templates
        getTemplates: stubResolve('getTemplates', []),
        // Logs
        getLog: stubResolve('getLog', { lines: [] }),
        // Environments
        getEnvironments: stubResolve('getEnvironments', []),
        activateEnvironment: stubResolve('activateEnvironment', {}),
        // Static docs
        getDocs: stubResolve('getDocs', '# Test Docs\n\nHello world'),
        // Snapshots
        getSnapshot: stubResolve('getSnapshot', {}),
        // Peers
        getPeers: stubResolve('getPeers', []),
        // Search
        getVttyText: stubResolve('getVttyText', ''),
        // Generic
        getJson: stubResolve('getJson', {}),
    };

    // Add call tracking proxy
    mock._callLog = callLog;
    mock._reset = function() {
        callLog.length = 0;
        for (const key of Object.keys(mock)) {
            if (key.startsWith('_')) continue;
            const s = mock[key];
            if (s && s._isStub) {
                s._callCount = 0;
                s._lastArgs = null;
            }
        }
    };

    return mock;
}

// ── Mock State ──
// Creates a fresh state object matching the real state.js shape.
// No DOM dependencies — pure data.
function createMockState(overrides = {}) {
    const s = {
        // Server connections
        connections: [
            { url: 'http://localhost:9090', token: 'test-token', label: 'Local', reachable: true, name: 'local' }
        ],
        authToken: 'test-token',

        // Commands
        commands: [],
        selectedCmdId: null,
        selectedInstUrl: null,

        // Panels
        panels: [],
        activePanelId: null,
        _focusedPanelId: null,

        // UI
        fontSize: 10,
        currentView: 'vtty',
        updateMode: 'push',
        pollInterval: 500,
        refreshMs: 0,
        panelLayout: 'row',
        showResources: false,
        soundEnabled: false,
        serverReachable: true,
        sidebarOpen: true,
        activeTab: 'servers',
        globalTheme: 'auto',

        // Internal
        _userScrolling: false,
        _mobileTabbedLayout: false,
        _pendingVttyDirty: false,
        _pendingVttyData: null,
        _refreshThrottleTimer: null,
        _lastRenderedPanelCount: -1,
        _lastRenderedPanelIds: '',
        _lastShowingWelcome: true,
        _showingWelcome: true,
        refreshInterval: null,
        _resourceInterval: null,
        _lastGeneration: {},
        _cellGrids: {},
        _cachedDomPre: {},
        _cachedScrollPos: {},
        _resourceCache: {},
        _wsLatency: 0,
        _wsPingInterval: null,
        _wsReconnectCount: 0,
        bufferView: 'current',
        _userAtBottom: true,
        logWs: null,
        logWsReconnectTimer: null,
    };

    // Apply overrides
    for (const [key, val] of Object.entries(overrides)) {
        s[key] = val;
    }

    return s;
}

// ── Mock Render ──
// Stubs for all render functions. Records calls for verification.
function createMockRender() {
    const callLog = [];
    const methods = [
        'sidebar', 'panels', 'toolbar', 'welcome',
        'commandList', 'panelHeader', 'panelTerminal', 'panelMetadata',
        'contextMenu', 'searchOverlay', 'addServerModal',
    ];

    const mock = {};
    for (const name of methods) {
        mock[name] = function(...args) {
            callLog.push({ method: name, args });
        };
        mock[name]._isStub = true;
        mock[name]._stubName = name;
        mock[name]._callCount = 0;
        mock[name]._lastArgs = null;
        // Wrap to track call count
        const orig = mock[name];
        const wrapped = function(...args) {
            mock[name]._callCount++;
            mock[name]._lastArgs = args;
            return orig.apply(this, args);
        };
        wrapped._isStub = true;
        wrapped._stubName = name;
        wrapped._callCount = 0;
        wrapped._lastArgs = null;
        mock[name] = wrapped;
    }

    mock._callLog = callLog;
    mock._reset = function() {
        callLog.length = 0;
        for (const name of methods) {
            if (mock[name]) {
                mock[name]._callCount = 0;
                mock[name]._lastArgs = null;
            }
        }
    };

    return mock;
}

// ── Mock DOM helpers ──
// Creates small DOM trees for testing render functions.
// Uses the MockElement from setup.js — no JSDOM needed.
function createTestDom(html) {
    // Parse a simple HTML string into a mock DOM tree.
    // Supports: <div>, <span>, <button>, <pre>, <input>, <a>, <select>
    // Attributes: id, class, data-*, style (limited), disabled, checked, value, href, src, type
    const container = document.createElement('div');
    if (html) container.innerHTML = html;
    return container;
}

// Find an element within a test DOM tree (convenience wrapper)
function findEl(root, selector) {
    return root.querySelector(selector);
}

// Find all elements within a test DOM tree
function findAllEl(root, selector) {
    return root.querySelectorAll(selector);
}

// Create a mock event for testing event handlers
function createMockEvent(opts = {}) {
    return {
        type: opts.type || 'click',
        target: opts.target || document.createElement('div'),
        currentTarget: opts.currentTarget || null,
        preventDefault: opts.preventDefault || function() {},
        stopPropagation: opts.stopPropagation || function() {},
        stopImmediatePropagation: opts.stopImmediatePropagation || function() {},
        ctrlKey: opts.ctrlKey || false,
        shiftKey: opts.shiftKey || false,
        altKey: opts.altKey || false,
        metaKey: opts.metaKey || false,
        key: opts.key || '',
        code: opts.code || '',
        clientX: opts.clientX || 0,
        clientY: opts.clientY || 0,
        dataTransfer: opts.dataTransfer || null,
        button: opts.button || 0,
        ...opts,
    };
}

// ── Async test runner ──
// Runs async test functions and returns a promise that resolves with pass/fail counts.
// Usage:
//   await runAsyncTests('My Test Group', [
//       async () => { assertOk(true, 'test 1'); },
//       async () => { await something(); assertOk(true, 'test 2'); },
//   ]);
async function runAsyncTests(groupName, tests) {
    console.log('\n=== ' + groupName + ' ===\n');
    let passed = 0;
    let failed = 0;

    for (let i = 0; i < tests.length; i++) {
        const beforePassed = globalThis._testPassed;
        const beforeFailed = globalThis._testFailed;
        try {
            await tests[i]();
        } catch (e) {
            globalThis._testFailed++;
            console.error('  FAIL: async test ' + (i + 1) + ' threw: ' + e.message);
        }
        const p = globalThis._testPassed - beforePassed;
        const f = globalThis._testFailed - beforeFailed;
        passed += p;
        failed += f;
    }

    console.log('\n  [' + groupName + '] ' + passed + ' passed, ' + failed + ' failed');
    return { passed, failed };
}

// ── Additional assertion helpers ──

globalThis.assertDeepEq = function(actual, expected, msg) {
    const actualStr = JSON.stringify(actual);
    const expectedStr = JSON.stringify(expected);
    if (actualStr !== expectedStr) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — expected ' + expectedStr + ', got ' + actualStr);
    } else {
        _testPassed++;
    }
};

globalThis.assertNotEq = function(actual, notExpected, msg) {
    if (actual === notExpected) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — should not equal ' + JSON.stringify(notExpected));
    } else {
        _testPassed++;
    }
};

globalThis.assertIncludes = function(str, substr, msg) {
    if (typeof str !== 'string' || !str.includes(substr)) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — "' + String(str).substring(0, 100) + '" does not include "' + substr + '"');
    } else {
        _testPassed++;
    }
};

globalThis.assertNotIncludes = function(str, substr, msg) {
    if (typeof str === 'string' && str.includes(substr)) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — should not include "' + substr + '"');
    } else {
        _testPassed++;
    }
};

globalThis.assertGt = function(actual, threshold, msg) {
    if (typeof actual !== 'number' || actual <= threshold) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — ' + actual + ' not > ' + threshold);
    } else {
        _testPassed++;
    }
};

globalThis.assertLt = function(actual, threshold, msg) {
    if (typeof actual !== 'number' || actual >= threshold) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — ' + actual + ' not < ' + threshold);
    } else {
        _testPassed++;
    }
};

globalThis.assertType = function(val, type, msg) {
    if (typeof val !== type) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — expected type ' + type + ', got ' + typeof val);
    } else {
        _testPassed++;
    }
};

globalThis.assertInstanceOf = function(val, constructor, msg) {
    if (!(val instanceof constructor)) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — not an instance of ' + (constructor.name || 'expected'));
    } else {
        _testPassed++;
    }
};

globalThis.assertNull = function(val, msg) {
    if (val !== null && val !== undefined) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — expected null/undefined, got ' + JSON.stringify(val));
    } else {
        _testPassed++;
    }
};

globalThis.assertNonNull = function(val, msg) {
    if (val === null || val === undefined) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — expected non-null, got ' + val);
    } else {
        _testPassed++;
    }
};

globalThis.assertLength = function(val, len, msg) {
    if (!val || typeof val.length !== 'number' || val.length !== len) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — expected length ' + len + ', got ' + (val ? val.length : 'not an array'));
    } else {
        _testPassed++;
    }
};

globalThis.assertProperty = function(obj, prop, msg) {
    if (!obj || typeof obj !== 'object' || !(prop in obj)) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — property "' + prop + '" not found');
    } else {
        _testPassed++;
    }
};

globalThis.assertThrowsAsync = async function(fn, msg) {
    try {
        await fn();
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — expected async throw');
    } catch (e) {
        _testPassed++;
    }
};

globalThis.assertResolves = async function(fn, msg) {
    try {
        await fn();
        _testPassed++;
    } catch (e) {
        _testFailed++;
        console.error('  FAIL: ' + msg + ' — threw: ' + e.message);
    }
};

// ── Exports ──
module.exports = {
    createMockApi,
    createMockState,
    createMockRender,
    createMockDom: createTestDom,
    createFetchMock,
    createMockEvent,
    findEl,
    findAllEl,
    runAsyncTests,
};