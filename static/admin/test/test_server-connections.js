/// test/test_server-connections.js — Tests for server connection management
require('./setup');

console.log('\n=== server-connections.js Tests ===\n');

resetTestState();

// Mock network-dependent functions
globalThis.renderPanels = function() {};
globalThis.loadCommands = function() { return Promise.resolve(); };
globalThis.updateDisconnectedUI = function() {};
globalThis.loadCertificates = function() { return Promise.resolve(); };
globalThis.fetchServerTemplates = function() {};
globalThis.updatePanelCommandInfo = function() {};
globalThis.updateTerminalDisconnectedOverlay = function() {};
globalThis.updateSidebarSelection = function() {};
globalThis.startPanelUpdateMode = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.focusPanel = function(id) { state._focusedPanelId = id; };
globalThis.addPanelDirect = function() {
    const panel = {
        id: 'panel-test-' + Date.now(), scrollbackOffset: 0, mouseTracking: false,
        mouseSgr: false, focused: false, fontSize: 10, selectionMode: false,
        theme: '', customTitle: '', minimized: false, selectedCmdId: null,
        selectedInstUrl: null, ws: null, wsCmdId: null, wsInstUrl: null,
        wsReconnectCount: 0, wsReconnectTimer: null, wsPingInterval: null,
        wsPingSendTime: 0, wsLatency: 0, pollTimer: null, cmdHistory: [], cmdHistoryIdx: -1,
    };
    state.panels.push(panel);
    return panel;
};
globalThis.loadVttyHttpForPanel = function() {};
globalThis._cacheTerminalForSwitch = function() {};
globalThis._restoreCachedDom = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.selectCommand = function() {};
globalThis.trapFocus = function() {};
globalThis.releaseCurrentFocusTrap = function() {};

// ── togglePauseRun ──
console.log('togglePauseRun tests');
assert(typeof togglePauseRun === 'function', 'togglePauseRun is a function');

// No selectedCmdId → early return
state.selectedCmdId = null;
assert(() => { togglePauseRun(); }, 'togglePauseRun early return when no cmd selected');

// ── togglePauseRunPanel ──
console.log('togglePauseRunPanel tests');
assert(typeof togglePauseRunPanel === 'function', 'togglePauseRunPanel is a function');

// No panel found → early return
assert(() => { togglePauseRunPanel('nonexistent'); }, 'togglePauseRunPanel no-arg early return');

// Panel without inst/cmd → early return
state.panels = [];
const pp = addPanelDirect();
pp.selectedInstUrl = null;
pp.selectedCmdId = null;
assert(() => { togglePauseRunPanel(pp.id); }, 'togglePauseRunPanel early return when panel has no selection');

// Panel with inst but no commands → early return
pp.selectedInstUrl = 'http://localhost:9090';
state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', _commands: [] }];
assert(() => { togglePauseRunPanel(pp.id); }, 'togglePauseRunPanel early return when no commands loaded');

// ── applyUpdateModeUI ──
console.log('applyUpdateModeUI tests');
assert(typeof applyUpdateModeUI === 'function', 'applyUpdateModeUI is a function');

// Create needed DOM elements
const updateModeEl = document.createElement('select');
updateModeEl.id = 'updateMode';
const pollIntervalEl = document.createElement('input');
pollIntervalEl.id = 'pollInterval';
const pollIntervalWrap = document.createElement('div');
pollIntervalWrap.id = 'pollIntervalWrap';

state.updateMode = 'push';
state.pollInterval = 500;
assert(() => { applyUpdateModeUI(); }, 'applyUpdateModeUI does not throw');
assertEq(updateModeEl.value, 'push', 'updateMode select value set');
assertEq(pollIntervalEl.value, '500', 'pollInterval value set');

// Poll mode: pollIntervalWrap should be visible
state.updateMode = 'poll';
applyUpdateModeUI();
assert(!pollIntervalWrap.classList.contains('hidden'), 'pollIntervalWrap visible in poll mode');

// Push mode: pollIntervalWrap should be hidden
state.updateMode = 'push';
applyUpdateModeUI();
assert(pollIntervalWrap.classList.contains('hidden'), 'pollIntervalWrap hidden in push mode');

// ── switchUpdateMode ──
console.log('switchUpdateMode tests');
assert(typeof switchUpdateMode === 'function', 'switchUpdateMode is a function');
globalThis.stopPanelUpdateMode = function() {};
globalThis.startPanelUpdateMode = function() {};

state.updateMode = 'push';
state.selectedInstUrl = 'http://localhost:9090';
state.selectedCmdId = 'cmd-1';
switchUpdateMode('poll');
assertEq(state.updateMode, 'poll', 'switchUpdateMode changes state');
assertEq(localStorage.getItem('vrw_update_mode'), 'poll', 'switchUpdateMode saves to localStorage');

switchUpdateMode('push');
assertEq(state.updateMode, 'push', 'switchUpdateMode switches back to push');

// ── applyPollInterval ──
console.log('applyPollInterval tests');
assert(typeof applyPollInterval === 'function', 'applyPollInterval is a function');

pollIntervalEl.value = '1000';
applyPollInterval();
assertEq(state.pollInterval, 1000, 'applyPollInterval sets valid value');
assertEq(String(pollIntervalEl.value), '1000', 'applyPollInterval updates input');

// Test clamping: below minimum (50ms)
pollIntervalEl.value = '10';
applyPollInterval();
assertEq(state.pollInterval, 50, 'applyPollInterval clamps to minimum 50ms');

// Test clamping: above maximum (5000ms)
pollIntervalEl.value = '9999';
applyPollInterval();
assertEq(state.pollInterval, 5000, 'applyPollInterval clamps to maximum 5000ms');

// Test NaN handling
pollIntervalEl.value = 'abc';
applyPollInterval();
assertEq(state.pollInterval, 500, 'applyPollInterval defaults to 500 on NaN');

// ── addConnection ──
console.log('addConnection tests');
assert(typeof addConnection === 'function', 'addConnection is a function');

state.connections = [];
localStorage.removeItem('vrw_connections');

const conn1 = addConnection('http://localhost:9090', 'Local', '');
assert(conn1 !== null, 'addConnection returns connection object');
assertEq(conn1.url, 'http://localhost:9090', 'connection URL set');
assertEq(conn1.label, 'Local', 'connection label set');
assertEq(conn1.token, '', 'connection token set');
assertEq(conn1.reachable, undefined, 'reachable starts as undefined');
assertEq(conn1._commands, null, '_commands starts as null');
assertEq(conn1._certs, null, '_certs starts as null');
assertEq(conn1._serverName, null, '_serverName starts as null');
assert(Array.isArray(state.connections), 'connections is array');
assertEq(state.connections.length, 1, 'one connection added');

// Idempotent: same URL returns existing
const conn1b = addConnection('http://localhost:9090', 'Renamed', 'newtoken');
assert(conn1b === conn1, 'idempotent add returns existing connection');
assertEq(conn1.label, 'Local', 'existing connection label unchanged');
assertEq(conn1.token, '', 'existing connection token unchanged');

// Add second connection
const conn2 = addConnection('http://localhost:9091', 'Remote', 'tok123');
assertEq(conn2.url, 'http://localhost:9091', 'second connection URL set');
assertEq(conn2.label, 'Remote', 'second connection label set');
assertEq(conn2.token, 'tok123', 'second connection token set');
assertEq(state.connections.length, 2, 'two connections now');

// Default label = URL when not provided
const conn3 = addConnection('http://192.168.1.1:8080', '', '');
assertEq(conn3.label, '192.168.1.1:8080', 'label defaults to host:port from URL');

// ── _saveConnections ──
console.log('_saveConnections tests');
assert(typeof _saveConnections === 'function', '_saveConnections is a function');
state.connections = [{ url: 'http://a.com', label: 'A', token: 't1', _commands: null }];
_saveConnections();
const saved = JSON.parse(localStorage.getItem('vrw_connections') || '[]');
assertEq(saved.length, 1, 'saved one connection');
assertEq(saved[0].url, 'http://a.com', 'saved URL correct');
assertEq(saved[0].label, 'A', 'saved label correct');
assertEq(saved[0].token, 't1', 'saved token correct');
// Internal fields should NOT be persisted
assert(saved[0]._commands === undefined, '_commands not persisted');
assert(saved[0]._certs === undefined, '_certs not persisted');

// ── _restoreConnections ──
console.log('_restoreConnections tests');
assert(typeof _restoreConnections === 'function', '_restoreConnections is a function');

// No saved data
localStorage.removeItem('vrw_connections');
state.connections = [];
const result1 = _restoreConnections();
assert(result1 === null, '_restoreConnections returns null when no saved data');

// With saved data
localStorage.setItem('vrw_connections', JSON.stringify([
    { url: 'http://saved.com:9090', label: 'Saved', token: 'stok' }
]));
state.connections = [];
const restored = _restoreConnections();
assert(Array.isArray(restored), '_restoreConnections returns array');
assertEq(restored.length, 1, 'restored one connection');
assertEq(state.connections.length, 1, 'connection added to state');
assertEq(state.connections[0].url, 'http://saved.com:9090', 'restored URL correct');

// Origin URL is skipped
localStorage.setItem('vrw_connections', JSON.stringify([
    { url: 'http://localhost:9090', label: 'Origin', token: '' },
    { url: 'http://other.com:8080', label: 'Other', token: 'ot' },
]));
state.connections = [];
const restored2 = _restoreConnections();
assertEq(state.connections.length, 1, 'origin URL skipped, only non-origin restored');
assertEq(state.connections[0].url, 'http://other.com:8080', 'non-origin URL restored');

// Invalid JSON → null
localStorage.setItem('vrw_connections', 'not-json');
state.connections = [];
const result2 = _restoreConnections();
assert(result2 === null, '_restoreConnections returns null on invalid JSON');

// Empty array → null
localStorage.setItem('vrw_connections', '[]');
state.connections = [];
const result3 = _restoreConnections();
assert(result3 === null, '_restoreConnections returns null on empty array');

// ── removeConnection ──
console.log('removeConnection tests');
assert(typeof removeConnection === 'function', 'removeConnection is a function');

state.connections = [
    { url: 'http://a.com', label: 'A', token: '', _commands: null },
    { url: 'http://b.com', label: 'B', token: '', _commands: null },
];
removeConnection('http://a.com');
assertEq(state.connections.length, 1, 'connection removed');
assertEq(state.connections[0].url, 'http://b.com', 'remaining connection correct');

// Remove nonexistent → no error
assert(() => { removeConnection('http://nonexistent.com'); }, 'removeConnection nonexistent does not throw');

// ── updateInstanceDropdown ──
console.log('updateInstanceDropdown tests');
assert(typeof updateInstanceDropdown === 'function', 'updateInstanceDropdown is a function');

const selectEl = document.createElement('select');
selectEl.id = 'spawnInstance';
state.connections = [
    { url: 'http://localhost:9090', label: 'Local', token: '' },
    { url: 'http://remote:8080', label: 'Remote', token: 't' },
];
assert(() => { updateInstanceDropdown(); }, 'updateInstanceDropdown does not throw');
assert(selectEl.innerHTML.includes('Local'), 'dropdown includes Local label');
assert(selectEl.innerHTML.includes('Remote'), 'dropdown includes Remote label');
assert(selectEl.innerHTML.includes('localhost:9090'), 'dropdown includes Local URL');
assert(selectEl.innerHTML.includes('remote:8080'), 'dropdown includes Remote URL');

// Empty connections
state.connections = [];
updateInstanceDropdown();
assert(selectEl.innerHTML === '', 'dropdown empty when no connections');

// ── updateCertDropdown ──
console.log('updateCertDropdown tests');
assert(typeof updateCertDropdown === 'function', 'updateCertDropdown is a function');

const certSelect = document.createElement('select');
certSelect.id = 'spawnCert';
state.connections = [{ url: 'http://a.com', label: 'A', _certs: [] }];
assert(() => { updateCertDropdown(); }, 'updateCertDropdown does not throw');

// With certs
state.connections = [{ url: 'http://a.com', label: 'A', _certs: [{ name: 'cert1', token_preview: 'abc' }] }];
updateCertDropdown();
assert(certSelect.innerHTML.includes('cert1'), 'cert dropdown includes cert name');
assert(certSelect.innerHTML.includes('A'), 'cert dropdown includes instance label');

// ── showAddServerModal ──
console.log('showAddServerModal tests');
assert(typeof showAddServerModal === 'function', 'showAddServerModal is a function');

const modal = document.createElement('div');
modal.id = 'addServerModal';
modal.classList.add('hidden');
const urlInput = document.createElement('input');
urlInput.id = 'addServerUrl';
const labelInput = document.createElement('input');
labelInput.id = 'addServerLabel';
const tokenInput = document.createElement('input');
tokenInput.id = 'addServerToken';
const openPane = document.createElement('input');
openPane.id = 'addServerOpenPane';
openPane.type = 'checkbox';

assert(() => { showAddServerModal(); }, 'showAddServerModal does not throw');
assert(!modal.classList.contains('hidden'), 'modal displayed');
assertEq(urlInput.value, 'http://localhost:9090', 'default URL set');
assertEq(openPane.checked, true, 'open pane checkbox checked by default');

// ── closeAddServerModal ──
console.log('closeAddServerModal tests');
assert(typeof closeAddServerModal === 'function', 'closeAddServerModal is a function');

modal.classList.remove('hidden');
assert(() => { closeAddServerModal(); }, 'closeAddServerModal does not throw');
assert(modal.classList.contains('hidden'), 'modal hidden');

// ── restartCommand ──
console.log('restartCommand tests');
assert(typeof restartCommand === 'function', 'restartCommand is a function');

// No panel → early return
state.panels = [];
assert(() => { restartCommand('nonexistent'); }, 'restartCommand no-panel early return');

// Panel without command → early return
state.panels = [];
const rp = addPanelDirect();
rp.selectedCmdId = null;
assert(() => { restartCommand(rp.id); }, 'restartCommand no-cmd early return');

// ── restartCommandById ──
console.log('restartCommandById tests');
assert(typeof restartCommandById === 'function', 'restartCommandById is a function');
assert(restartCommandById.constructor.name === 'AsyncFunction', 'restartCommandById is async');

// ── spawnFromWelcome ──
console.log('spawnFromWelcome tests');
assert(typeof spawnFromWelcome === 'function', 'spawnFromWelcome is a function');
assert(spawnFromWelcome.constructor.name === 'AsyncFunction', 'spawnFromWelcome is async');

// No welcome input → early return
assert(() => { spawnFromWelcome(); }, 'spawnFromWelcome no-input early return');

// With input but empty → early return
const welcomeInput = document.createElement('input');
welcomeInput.id = 'welcomeCmd';
welcomeInput.value = '   ';
assert(() => { spawnFromWelcome(); }, 'spawnFromWelcome empty-input early return');

// ── _fetchServerName ──
console.log('_fetchServerName tests');
assert(typeof _fetchServerName === 'function', '_fetchServerName is a function');
assert(_fetchServerName.constructor.name === 'AsyncFunction', '_fetchServerName is async');

const fetchConn = { url: 'http://localhost:9090', label: 'Test', _serverName: null };
// With default fetch mock returning ok, should not throw
assert(() => { _fetchServerName(fetchConn); }, '_fetchServerName does not throw');

// ── disconnectServer ──
console.log('disconnectServer tests');
assert(typeof disconnectServer === 'function', 'disconnectServer is a function');

// Nonexistent server → early return
state.connections = [];
assert(() => { disconnectServer('http://nonexistent.com'); }, 'disconnectServer nonexistent early return');

// ── fetchServerConfig ──
console.log('fetchServerConfig tests');
assert(typeof fetchServerConfig === 'function', 'fetchServerConfig is a function');
assert(fetchServerConfig.constructor.name === 'AsyncFunction', 'fetchServerConfig is async');

// Default fetch mock returns { status: 'ok', data: {} } → no crash
assert(() => { fetchServerConfig(); }, 'fetchServerConfig does not throw');

// ── loadCertificates ──
console.log('loadCertificates tests');
// Restore the real function for the signature check (was mocked at top of file)
globalThis.loadCertificates = _realFunctions.loadCertificates;
assert(typeof loadCertificates === 'function', 'loadCertificates is a function');
assert(loadCertificates.constructor.name === 'AsyncFunction', 'loadCertificates is async');

const certListEl = document.createElement('div');
certListEl.id = 'certList';
state.connections = [{ url: 'http://a.com', label: 'A', _certs: null }];
assert(() => { loadCertificates(); }, 'loadCertificates does not throw');

console.log('\n[server-connections.js] Tests complete');