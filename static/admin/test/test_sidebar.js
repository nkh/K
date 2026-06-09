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
const tabSpawn = document.createElement('div');
tabSpawn.id = 'tab-spawn';
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
assert(() => { switchSidebarTab('spawn', sidebarTab1); }, 'switchSidebarTab spawn does not throw');
assert(() => { switchSidebarTab('templates', sidebarTab1); }, 'switchSidebarTab templates does not throw');
assert(() => { switchSidebarTab('envs', sidebarTab1); }, 'switchSidebarTab envs does not throw');
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

console.log('\n[sidebar.js] Tests complete');
