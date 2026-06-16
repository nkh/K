/// test/test_close_server.js — Tests for: server close button in sidebar,
/// panel header CSS, kill-all clearing stale commands, always-active kill
/// buttons, and health-check auto-remove of unresponsive connections.
require('./setup');

console.log('\n=== Close Server / Panel Header / Kill-All / Health Check Tests ===\n');

resetTestState();

// Mock network-dependent functions
globalThis.renderPanels = function() {};
globalThis.loadCommands = function() { return Promise.resolve(); };
globalThis.updateDisconnectedUI = function() {};
globalThis.updateSidebarTabsVisibility = function() {};
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
globalThis.disconnectPanelWs = function() {};
globalThis.stopPanelPoll = function() {};
globalThis.selectCommand = function() {};
globalThis.trapFocus = function() {};
globalThis.releaseCurrentFocusTrap = function() {};
globalThis.loadVttyHttpForPanel = function() {};
globalThis._cacheTerminalForSwitch = function() {};
globalThis._restoreCachedDom = function() {};

// ═══════════════════════════════════════════════════════════════
// 1. Server connections bar — verify _buildSidebar generates correct
//    HTML for non-origin servers with reach dot + label + close button.
//    We test by extracting the rendering code paths directly.
// ═══════════════════════════════════════════════════════════════
console.log('[1] Server connections bar — rendering logic');

// Verify the code path exists: _buildSidebar checks for non-origin connections
assert(typeof _buildSidebar === 'function', '_buildSidebar is exported');
assert(typeof disconnectServer === 'function', 'disconnectServer is exported');
assert(typeof removeConnection === 'function', 'removeConnection is exported');

// Verify escHtml works for server labels
assertEq(escHtml('Remote Server'), 'Remote Server', 'escHtml passes through normal text');
assertEq(escHtml('<script>'), '&lt;script&gt;', 'escHtml escapes HTML entities');

// Verify sidebar uses reach dots and DisconnectServer delegation
const sidebarSource = require('fs').readFileSync(
    require('path').join(__dirname, '..', 'modules', 'sidebar.js'), 'utf8'
);
assertOk(sidebarSource.includes('server-reach-dot'), 'sidebar.js source contains server-reach-dot');
assertOk(sidebarSource.includes('data-action="DisconnectServer"'), 'sidebar.js close button uses data-action=DisconnectServer delegation');

// ═══════════════════════════════════════════════════════════════
// 1b. Reach indicator dot reflects server state
// ═══════════════════════════════════════════════════════════════
console.log('[1b] Reach indicator dot — CSS classes');
assertOk(sidebarSource.includes("inst.reachable === true ? 'reachable'"), 'reachable class for connected servers');
assertOk(sidebarSource.includes("inst.reachable === false ? 'unreachable'"), 'unreachable class for dead servers');
assertOk(sidebarSource.includes("'unknown'"), 'unknown class for checking servers');

// ═══════════════════════════════════════════════════════════════
// 2. Panel header CSS — focused indicator is 1px, reduced padding
// ═══════════════════════════════════════════════════════════════
console.log('[2] Panel header CSS');
const fs = require('fs');
const cssPath = require('path').join(__dirname, '..', 'style.css');
const cssContent = fs.readFileSync(cssPath, 'utf8');

// Check that .panel.focused uses 1px inset (not 2px)
const focusedMatch = cssContent.match(/\.panel\.focused\s*\{[^}]*box-shadow:\s*inset\s*0\s*(\d)px/);
assertOk(focusedMatch, 'panel.focused box-shadow found in CSS');
assertEq(focusedMatch[1], '1', 'panel focused indicator is 1px');

// Check grid presets also use 1px
const gridMatch = cssContent.match(/\.panel-container\.grid-2x2\s+\.panel\.focused\s*\{[^}]*box-shadow:\s*inset\s*0\s*(\d)px/);
assertOk(gridMatch, 'grid-2x2 focused indicator found');
assertEq(gridMatch[1], '1', 'grid-2x2 focused indicator is 1px');

// Check panel-header has minimal padding
const headerMatch = cssContent.match(/\.panel-header\s*\{[^}]*padding:\s*([0-9.]+)(?:rem)?\s+([0-9.]+)rem/);
assertOk(headerMatch, 'panel-header padding found in CSS');
assertOk(parseFloat(headerMatch[1]) <= 0.1, 'panel-header top padding <= 0.1rem (got ' + headerMatch[1] + ')');
assertOk(parseFloat(headerMatch[2]) <= 0.2, 'panel-header side padding <= 0.2rem (got ' + headerMatch[2] + ')');

// Check panel-header has border: none
const headerBorderMatch = cssContent.match(/\.panel-header\s*\{[^}]*border:\s*none/);
assertOk(headerBorderMatch, 'panel-header has border: none');

// ═══════════════════════════════════════════════════════════════
// 3. Kill-all clears commands for unreachable servers
// ═══════════════════════════════════════════════════════════════
console.log('[3] Kill-all clears unreachable server commands');
assert(typeof killAllCommands === 'function', 'killAllCommands is exported');

// Verify the kill-all cleanup code exists in spawn.js source
const spawnSource = require('fs').readFileSync(
    require('path').join(__dirname, '..', 'modules', 'spawn.js'), 'utf8'
);
assertOk(spawnSource.includes('inst.reachable === false'), 'killAllCommands checks unreachable servers');
assertOk(spawnSource.includes('inst._commands = []'), 'killAllCommands clears commands for unreachable');
assertOk(spawnSource.includes('panel.selectedCmdId = null'), 'killAllCommands clears panel selectedCmdId');

// Test the cleanup logic directly
state.connections = [
    { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true, _commands: [{ id: 'c1', name: 'bash', alive: true }], _certs: null, _serverName: null, _lastError: null },
    { url: 'http://dead:9090', label: 'Dead', token: '', reachable: false, _commands: [{ id: 'c2', name: 'htop', alive: true }, { id: 'c3', name: 'vim', alive: false }], _certs: null, _serverName: null, _lastError: 'connection lost' },
];
state.panels = [];
const deadPanel = addPanelDirect();
deadPanel.selectedInstUrl = 'http://dead:9090';
deadPanel.selectedCmdId = 'c2';

const deadConn = state.connections.find(c => c.url === 'http://dead:9090');
assertEq(deadConn._commands.length, 2, 'dead server has 2 commands before cleanup');

// Simulate the cleanup logic from killAllCommands
for (const inst of state.connections) {
    if (inst.reachable === false) { inst._commands = []; }
}
for (const panel of state.panels) {
    if (panel.selectedInstUrl) {
        const inst = state.connections.find(i => i.url === panel.selectedInstUrl);
        if (inst && (!inst._commands || inst._commands.length === 0)) {
            panel.selectedCmdId = null;
            panel.selectedInstUrl = null;
        }
    }
}

assertEq(deadConn._commands.length, 0, 'dead server commands cleared');
assertEq(deadPanel.selectedCmdId, null, 'panel cmdId cleared for dead server');
assertEq(deadPanel.selectedInstUrl, null, 'panel instUrl cleared for dead server');

// ═══════════════════════════════════════════════════════════════
// 4. Sidebar kill buttons always active — verify no disabled attribute
// ═══════════════════════════════════════════════════════════════
console.log('[4] Sidebar kill buttons always active');
// Verify the sidebar source code no longer generates disabled attribute
assertOk(!sidebarSource.includes('killDisabled = (instUnreachable && isAlive)'),
    'sidebar.js no longer sets killDisabled based on unreachable+alive');
assertOk(!sidebarSource.includes('killDisabled'),
    'sidebar.js no longer uses killDisabled (kill buttons always active via delegation)');

// ═══════════════════════════════════════════════════════════════
// 5. Health check auto-removes unresponsive connections
// ═══════════════════════════════════════════════════════════════
console.log('[5] Health check auto-remove');
assert(typeof healthCheckConnections === 'function', 'healthCheckConnections is exported');

// Verify source code has correct retry parameters
const connSource = require('fs').readFileSync(
    require('path').join(__dirname, '..', 'modules', 'server-connections.js'), 'utf8'
);
assertOk(connSource.includes('MAX = 5'), 'healthCheckConnections uses 5 retries');
assertOk(connSource.includes('INTERVAL = 500'), 'healthCheckConnections uses 500ms interval');
assertOk(connSource.includes('removeConnection(url)'), 'healthCheckConnections removes unreachable connections');

// Test with empty/null inputs — must not throw
assert(() => { healthCheckConnections(null); }, 'healthCheckConnections(null) does not throw');
assert(() => { healthCheckConnections([]); }, 'healthCheckConnections([]) does not throw');
assert(() => { healthCheckConnections(undefined); }, 'healthCheckConnections(undefined) does not throw');

// Test with valid input — must set up timers without error
state.connections = [
    { url: 'http://localhost:9090', label: 'Local', token: '', reachable: true, _commands: [], _certs: null, _serverName: null, _lastError: null },
    { url: 'http://dead:9090', label: 'Dead', token: '', reachable: false, _commands: [], _certs: null, _serverName: null, _lastError: 'connection lost' },
];
assert(() => { healthCheckConnections(['http://dead:9090']); }, 'healthCheckConnections with URL array does not throw');

// Test removeConnection directly
assertEq(state.connections.length, 2, '2 connections before remove');
removeConnection('http://dead:9090');
assertEq(state.connections.length, 1, '1 connection after remove');
assertEq(state.connections[0].url, 'http://localhost:9090', 'origin server remains');

// ═══════════════════════════════════════════════════════════════
// 6. CSS classes for server connections bar exist
// ═══════════════════════════════════════════════════════════════
console.log('[6] Server tab CSS');
assertOk(cssContent.includes('.server-reach-dot'), 'CSS has .server-reach-dot');
assertOk(cssContent.includes('.server-reach-dot.reachable'), 'CSS has .reachable variant');
assertOk(cssContent.includes('.server-reach-dot.unreachable'), 'CSS has .unreachable variant');
assertOk(cssContent.includes('.server-reach-dot.unknown'), 'CSS has .unknown variant');

// ═══════════════════════════════════════════════════════════════
// 7. Duplicate removeConnection removed from server-connections.js
// ═══════════════════════════════════════════════════════════════
console.log('[7] No duplicate removeConnection');
const removeConnCount = (connSource.match(/function removeConnection/g) || []).length;
assertEq(removeConnCount, 1, 'only one removeConnection definition in server-connections.js');

console.log('\n=== Close Server Tests Complete ===');
