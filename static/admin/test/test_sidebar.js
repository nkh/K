/// test/test_sidebar.js — Tests for sidebar functions
require('./setup');

console.log('\n=== sidebar.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.loadCommands = function() { return Promise.resolve(); };

// ── toggleSidebar ──
console.log('toggleSidebar tests');
assert(typeof toggleSidebar === 'function', 'toggleSidebar is a function');
const sidebar = document.getElementById('sidebar');
sidebar.classList.remove('collapsed');
toggleSidebar();
assert(sidebar.classList.contains('collapsed'), 'toggleSidebar adds collapsed class');
toggleSidebar();
assert(!sidebar.classList.contains('collapsed'), 'toggleSidebar removes collapsed class');

// ── switchSidebarTab ──
console.log('switchSidebarTab tests');
assert(typeof switchSidebarTab === 'function', 'switchSidebarTab is a function');

// Create tab elements
const tabServers = document.createElement('div');
tabServers.id = 'tab-servers';
// Spawn is now a modal, not a tab — no tab-spawn in sidebar tabs
const tabTemplates = document.createElement('div');
tabTemplates.id = 'tab-templates';
const tabEnvs = document.createElement('div');
tabEnvs.id = 'tab-envs';
const tabCerts = document.createElement('div');
tabCerts.id = 'tab-certs';
const tabGroups = document.createElement('div');
tabGroups.id = 'tab-groups';

// Create sidebar-tab elements for querySelectorAll
const sidebarTab1 = document.createElement('div');
sidebarTab1.classList.add('sidebar-tab');

// Mock renderTemplates/renderGroups to avoid cascading errors
globalThis.renderTemplates = function() {};
globalThis.renderGroups = function() {};
state.connections = [];

assert(() => { switchSidebarTab('servers', sidebarTab1); }, 'switchSidebarTab servers does not throw');
// spawn is no longer a sidebar tab — removed from switchSidebarTab
assert(() => { switchSidebarTab('templates', sidebarTab1); }, 'switchSidebarTab templates does not throw');
assert(() => { switchSidebarTab('certs', sidebarTab1); }, 'switchSidebarTab certs does not throw');
assert(() => { switchSidebarTab('groups', sidebarTab1); }, 'switchSidebarTab groups does not throw');

// ── updateSidebarTabsVisibility ──
console.log('updateSidebarTabsVisibility tests');
if (typeof updateSidebarTabsVisibility === 'function') {
    assert(() => { updateSidebarTabsVisibility(); }, 'updateSidebarTabsVisibility does not throw');
}

// ── toggleResources ──
console.log('toggleResources tests');
if (typeof toggleResources === 'function') {
    assert(() => { toggleResources(); }, 'toggleResources does not throw');
}

// ── toggleBottombar ──
console.log('toggleBottombar tests');
if (typeof toggleBottombar === 'function') {
    const bottomBar = document.createElement('div');
    bottomBar.id = 'bottomBar';
    assert(() => { toggleBottombar(); }, 'toggleBottombar does not throw');
}

// ── initBottombar ──
console.log('initBottombar tests');
if (typeof initBottombar === 'function') {
    assert(() => { initBottombar(); }, 'initBottombar does not throw');
}

// ── toggleLogsView ──
console.log('toggleLogsView tests');
if (typeof toggleLogsView === 'function') {
    const viewLog = document.createElement('div');
    viewLog.id = 'view-log';
    const viewVtty = document.createElement('div');
    viewVtty.id = 'view-vtty';
    assert(() => { toggleLogsView(); }, 'toggleLogsView does not throw');
}

// ── getPinnedNames ──
console.log('getPinnedNames tests');
if (typeof getPinnedNames === 'function') {
    localStorage.removeItem('vrw_pinned_commands');
    const pinned = getPinnedNames();
    assert(Array.isArray(pinned), 'getPinnedNames returns array');
    assertEq(pinned.length, 0, 'empty by default');
}

// ── togglePinCmd ──
console.log('togglePinCmd tests');
if (typeof togglePinCmd === 'function') {
    assert(() => { togglePinCmd('http://localhost:9090', 'htop'); }, 'togglePinCmd does not throw');
}

// ── updateDisconnectedUI ──
console.log('updateDisconnectedUI tests');
if (typeof updateDisconnectedUI === 'function') {
    assert(() => { updateDisconnectedUI(); }, 'updateDisconnectedUI does not throw');
}

// ── updateSidebarBanner ──
console.log('updateSidebarBanner tests');
if (typeof updateSidebarBanner === 'function') {
    assert(() => { updateSidebarBanner(); }, 'updateSidebarBanner does not throw');
}

// ── updateTerminalDisconnectedOverlay ──
console.log('updateTerminalDisconnectedOverlay tests');
if (typeof updateTerminalDisconnectedOverlay === 'function') {
    assert(() => { updateTerminalDisconnectedOverlay(); }, 'updateTerminalDisconnectedOverlay does not throw');
}

// ── _showSpawnModal ──
console.log('_showSpawnModal tests');
if (typeof _showSpawnModal === 'function') {
    const spawnEl = document.getElementById('tab-spawn');
    assert(spawnEl !== null, 'tab-spawn element exists');
    spawnEl.classList.add('hidden');
    _showSpawnModal();
    assert(!spawnEl.classList.contains('hidden'), '_showSpawnModal reveals spawn modal');
    assertEq(window._userSpawnInstUrl, undefined, '_showSpawnModal clears spawn server');
}

// ── _spawnOnServer ──
console.log('_spawnOnServer tests');
if (typeof _spawnOnServer === 'function') {
    const spawnEl = document.getElementById('tab-spawn');
    spawnEl.classList.add('hidden');
    _spawnOnServer('http://localhost:9090');
    assert(!spawnEl.classList.contains('hidden'), '_spawnOnServer reveals spawn modal');
    assertEq(window._userSpawnInstUrl, 'http://localhost:9090', '_spawnOnServer sets spawn server');
}

// ── _closeSpawnModal ──
console.log('_closeSpawnModal tests');
if (typeof _closeSpawnModal === 'function') {
    const spawnEl = document.getElementById('tab-spawn');
    spawnEl.classList.remove('hidden');
    _closeSpawnModal();
    assert(spawnEl.classList.contains('hidden'), '_closeSpawnModal hides spawn modal');
}

// ── renderCmdList shows server badge for ALL servers (including main) ──
console.log('renderCmdList server badge consistency tests');
if (typeof _buildSidebar === 'function') {
    const container = document.getElementById('commandList');
    state.connections = [
        { url: 'http://localhost:9090', label: 'localhost:9090', reachable: true, _commands: [{ id: 'c1', name: 'bash', alive: true }] },
        { url: 'http://remote:8080', label: 'Production', reachable: true, _commands: [{ id: 'c2', name: 'node', alive: true }] }
    ];
    state._sidebarSort = 'name';
    _buildSidebar();
    const html = container.innerHTML;
    assert(html.includes('resource-badge'), 'server badges present in All tab');
    // Main server badge should use short label (port only for localhost), not full label
    assert(html.includes('>9090<'), 'main server badge uses short label (port)');
    assert(html.includes('>Production<'), 'remote server badge shows label');
    // All commands should have kill button with data-action
    const killBtnCount = (html.match(/data-action="KillCommand"/g) || []).length;
    assertEq(killBtnCount, 2, 'All tab: every command has kill button with KillCommand action');
    // All alive commands should have freeze button
    const freezeBtnCount = (html.match(/data-action="TogglePauseRunByIdx"/g) || []).length;
    assertEq(freezeBtnCount, 2, 'All tab: every alive command has freeze button');
    // No command should reference SwitchSidebarTab or data-tab=spawn
    assert(!html.includes('SwitchSidebarTab'), 'No SwitchSidebarTab in command list');
    assert(!html.includes('data-tab="spawn"'), 'No data-tab=spawn in command list');
    state.connections = [];
}

console.log('\n[sidebar.js] Tests complete');
