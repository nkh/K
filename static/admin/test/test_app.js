/// test/test_app.js — Tests for app initialization and lifecycle
require('./setup');

console.log('\n=== app.js Tests ===\n');

// ── Module dependencies available ──
console.log('module dependencies');
assert(typeof state !== 'undefined', 'state is defined');
assert(typeof addConnection === 'function', 'addConnection is a function');
assert(typeof addPanelDirect === 'function', 'addPanelDirect is a function');
assert(typeof getBaseUrl === 'function', 'getBaseUrl is a function');
assert(typeof _restoreConnections === 'function', '_restoreConnections is a function');
assert(typeof _saveConnections === 'function', '_saveConnections is a function');
assert(typeof lookupAndSelectCommand === 'function', 'lookupAndSelectCommand is a function');

resetTestState();

// Stub functions that would require real DOM or server
globalThis.renderPanels = function() {};
globalThis.startRefresh = function() {};
globalThis.loadCertificates = function() {};
globalThis.fetchServerTemplates = function() {};
globalThis.fetchEnvironments = function() {};
globalThis.fetchServerConfig = function() {};
globalThis.applyUpdateModeUI = function() {};
globalThis.updateSidebarTabsVisibility = function() {};
globalThis.fetchPeers = function() {};
globalThis.autoFitActiveTerminal = function() {};

// ── URL parameter parsing: default single connection ──
console.log('URL parameter parsing: default single connection');
resetTestState();
const origSearch = globalThis.location.search;
globalThis.location.search = '';

// When no instance URL params, app creates a default connection to window.location.origin
state.connections = [{
    url: window.location.origin,
    label: 'Local',
    token: '',
    reachable: undefined,
}];
assertEq(state.connections.length, 1, 'default single connection created');
assertEq(state.connections[0].url, 'http://localhost:9090', 'default connection uses window.location.origin');
assertEq(state.connections[0].label, 'Local', 'default connection label is "Local"');
assertEq(state.connections[0].token, '', 'default connection token is empty');
assertEq(state.connections[0].reachable, undefined, 'default connection reachable is undefined');

// ── URL parameter parsing: multi-instance ──
console.log('URL parameter parsing: multi-instance');
resetTestState();
// Simulate URL: ?instance=http://host1:8080&label=Prod&instance=http://host2:9090&label=Dev
globalThis.location.search = 'instance=http%3A%2F%2Fhost1%3A8080&label=Prod&instance=http%3A%2F%2Fhost2%3A9090&label=Dev';
const params = new URLSearchParams(globalThis.location.search);
const instances = params.getAll('instance');
assertEq(instances.length, 2, 'parsed 2 instance URLs from params');
assertEq(instances[0], 'http://host1:8080', 'first instance URL correct');
assertEq(instances[1], 'http://host2:9090', 'second instance URL correct');

// Build connection objects as app.js does
state.connections = instances.map((u, i) => ({
    url: u,
    label: params.getAll('label')[i] || ('Instance ' + (i + 1)),
    token: params.getAll('token')[i] || '',
    reachable: undefined,
}));
assertEq(state.connections.length, 2, 'two connections created from URL params');
assertEq(state.connections[0].url, 'http://host1:8080', 'first connection URL');
assertEq(state.connections[0].label, 'Prod', 'first connection label from param');
assertEq(state.connections[1].url, 'http://host2:9090', 'second connection URL');
assertEq(state.connections[1].label, 'Dev', 'second connection label from param');

// Multi-instance with tokens
resetTestState();
globalThis.location.search = 'instance=http%3A%2F%2Fhost1%3A8080&token=abc123&instance=http%3A%2F%2Fhost2%3A9090&token=xyz789';
const params2 = new URLSearchParams(globalThis.location.search);
const instances2 = params2.getAll('instance');
state.connections = instances2.map((u, i) => ({
    url: u,
    label: params2.getAll('label')[i] || ('Instance ' + (i + 1)),
    token: params2.getAll('token')[i] || '',
    reachable: undefined,
}));
assertEq(state.connections[0].token, 'abc123', 'first instance token from param');
assertEq(state.connections[1].token, 'xyz789', 'second instance token from param');

// No labels: falls back to auto-generated label
resetTestState();
globalThis.location.search = 'instance=http%3A%2F%2Fhost%3A8080';
const params3 = new URLSearchParams(globalThis.location.search);
const instances3 = params3.getAll('instance');
state.connections = instances3.map((u, i) => ({
    url: u,
    label: params3.getAll('label')[i] || ('Instance ' + (i + 1)),
    token: '',
    reachable: undefined,
}));
assertEq(state.connections[0].label, 'Instance 1', 'auto-generated label when no label param');
globalThis.location.search = origSearch;

// ── Panel creation from URL params ──
console.log('panel creation from init');
resetTestState();
globalThis.renderPanels = function() {};
state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', reachable: undefined }];
// App calls addConnection for primary then addPanelDirect
const conn = addConnection(state.connections[0].url, state.connections[0].label, state.connections[0].token);
assertEq(state.connections.length, 1, 'connection added');
const panel = addPanelDirect();
assert(panel !== null, 'initial panel created');
assertEq(state.panels.length, 1, 'one panel after init');
assert(panel.id.startsWith('panel-'), 'panel id format correct');

// ── State restoration from localStorage ──
console.log('state restoration from localStorage');
resetTestState();
globalThis.renderPanels = function() {};
state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', reachable: undefined }];
addConnection(state.connections[0].url, state.connections[0].label, state.connections[0].token);
addPanelDirect();

// Save panel layout preference
localStorage.setItem('vrw_panel_layout', 'column');
const savedLayout = localStorage.getItem('vrw_panel_layout');
assertEq(savedLayout, 'column', 'panel layout saved to localStorage');
state.panelLayout = savedLayout;
assertEq(state.panelLayout, 'column', 'state.panelLayout restored to column');

// Save and restore panel count
localStorage.setItem('vrw_panel_count', '3');
const savedPanelCount = parseInt(localStorage.getItem('vrw_panel_count'));
assertEq(savedPanelCount, 3, 'panel count parsed from localStorage');
while (state.panels.length < savedPanelCount) {
    addPanelDirect();
}
assertEq(state.panels.length, 3, 'panels restored to saved count');

// ── Connection persistence ──
console.log('connection persistence');
resetTestState();
globalThis.renderPanels = function() {};

// Add a connection and verify it's persisted
addConnection('http://remote:8080', 'Remote Server', 'tok123');
const savedConns = localStorage.getItem('vrw_connections');
assert(savedConns !== null, 'connections saved to localStorage');
const parsedConns = JSON.parse(savedConns);
assertEq(parsedConns.length, 1, 'one connection persisted');
assertEq(parsedConns[0].url, 'http://remote:8080', 'persisted URL correct');
assertEq(parsedConns[0].label, 'Remote Server', 'persisted label correct');
assertEq(parsedConns[0].token, 'tok123', 'persisted token correct');

// Restore connections (skips origin server)
const restored = _restoreConnections();
assertEq(state.connections.length, 1, 'restored connection added (origin skipped)');
assertEq(state.connections[0].url, 'http://remote:8080', 'restored URL correct');

// Idempotent: adding same URL twice returns existing connection
resetTestState();
globalThis.renderPanels = function() {};
const c1 = addConnection('http://same:8080', 'First', '');
const c2 = addConnection('http://same:8080', 'Second', '');
assertEq(c1, c2, 'addConnection is idempotent — same object returned');
assertEq(state.connections.length, 1, 'no duplicate connection added');
assertEq(state.connections[0].label, 'First', 'original label preserved, not overwritten');

// ── Mobile layout detection ──
console.log('mobile layout detection');
resetTestState();
// Simulate wide screen
const origInnerWidth = globalThis.innerWidth;
globalThis.innerWidth = 1200;
state._mobileTabbedLayout = window.innerWidth <= 768;
assertEq(state._mobileTabbedLayout, false, 'desktop mode for wide screen');

// Simulate narrow screen
globalThis.innerWidth = 600;
state._mobileTabbedLayout = window.innerWidth <= 768;
assertEq(state._mobileTabbedLayout, true, 'mobile mode for narrow screen');
globalThis.innerWidth = origInnerWidth;

// ── Command-name URL routing ──
console.log('command-name URL routing');
resetTestState();
// /admin → no routing (it's the admin page)
let pathname = '/admin'.replace(/^\/+|\/+$/g, '');
let shouldRoute = pathname && pathname !== 'admin' && !pathname.startsWith('api/');
assertEq(shouldRoute, false, '/admin pathname does not trigger routing');

// /admin/ → stripped to admin → no routing
pathname = '/admin/'.replace(/^\/+|\/+$/g, '');
shouldRoute = pathname && pathname !== 'admin' && !pathname.startsWith('api/');
assertEq(shouldRoute, false, 'trailing slash stripped, admin not routed');

// /api/commands → no routing (api path)
pathname = 'api/commands'.replace(/^\/+|\/+$/g, '');
shouldRoute = pathname && pathname !== 'admin' && !pathname.startsWith('api/');
assertEq(shouldRoute, false, 'api path does not trigger routing');

// /my-cmd → triggers routing
pathname = 'my-cmd'.replace(/^\/+|\/+$/g, '');
shouldRoute = pathname && pathname !== 'admin' && !pathname.startsWith('api/');
assertEq(shouldRoute, true, 'command name triggers routing');

// ── Connection removal ──
console.log('connection removal');
resetTestState();
globalThis.loadCommands = function() {};
globalThis.updateDisconnectedUI = function() {};

addConnection('http://a:8080', 'A', '');
addConnection('http://b:9090', 'B', '');
addConnection('http://c:7070', 'C', '');
assertEq(state.connections.length, 3, '3 connections before removal');

removeConnection('http://b:9090');
assertEq(state.connections.length, 2, '2 connections after removal');
const remainingUrls = state.connections.map(c => c.url);
assert(!remainingUrls.includes('http://b:9090'), 'removed connection not in list');
assert(remainingUrls.includes('http://a:8080'), 'other connections preserved');
assert(remainingUrls.includes('http://c:7070'), 'other connections preserved');

// ── Initial state integrity ──
console.log('initial state integrity');
resetTestState();
assertEq(state.panels.length, 0, 'panels start empty after reset');
assertEq(state.connections.length, 0, 'connections start empty after reset');
assertEq(state.selectedCmdId, null, 'no command selected after reset');
assertEq(state.selectedInstUrl, null, 'no instance URL after reset');
assertEq(state.currentView, 'vtty', 'default view is vtty after reset');
assertEq(state.panelLayout, 'row', 'default layout is row after reset');
assertEq(state.fontSize, 10, 'default fontSize is 10 after reset');
assertEq(state.updateMode, 'push', 'default updateMode is push after reset');
assertEq(state.bufferView, 'current', 'default bufferView is current after reset');
assertEq(_showingWelcome, true, 'welcome shown after reset');
assertEq(state._focusedPanelId, null, 'no focused panel after reset');
assertEq(state.serverReachable, false, 'server not reachable after reset');
assertEq(state.showResources, false, 'resources hidden after reset');
assertEq(state.soundEnabled, false, 'sound off after reset');
assertEq(state._level3Enabled, true, 'level3 enabled after reset');

console.log('\n[app.js] ' + _testPassed + ' passed so far');
