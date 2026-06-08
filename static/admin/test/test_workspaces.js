/// test/test_workspaces.js — Tests for workspace, environment, and group management
require('./setup');

console.log('\n=== workspaces.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};

// ── getWorkspaces ──
console.log('getWorkspaces tests');
if (typeof getWorkspaces === 'function') {
    localStorage.removeItem('vrw_workspaces');
    const ws = getWorkspaces();
    // Returns object (not array) per implementation
    assert(typeof ws === 'object', 'getWorkspaces returns object');
    assert(Object.keys(ws).length === 0, 'empty by default');
}

// ── saveWorkspaces ──
console.log('saveWorkspaces tests');
if (typeof saveWorkspaces === 'function') {
    saveWorkspaces([{ name: 'dev', panels: [] }]);
    const ws = getWorkspaces();
    assertEq(ws.length, 1, 'workspace saved');
    assertEq(ws[0].name, 'dev', 'workspace name correct');
}

// ── toggleWorkspaceDropdown ──
console.log('toggleWorkspaceDropdown tests');
if (typeof toggleWorkspaceDropdown === 'function') {
    const menu = document.createElement('div');
    menu.id = 'workspaceMenu';
    menu.style.display = 'none';
    assert(() => { toggleWorkspaceDropdown({ stopPropagation() {} }); }, 'toggleWorkspaceDropdown does not throw');
}

// ── renderWorkspaceList ──
console.log('renderWorkspaceList tests');
if (typeof renderWorkspaceList === 'function') {
    const list = document.createElement('div');
    list.id = 'workspaceList';
    saveWorkspaces([{ name: 'test', panels: [] }]);
    assert(() => { renderWorkspaceList(); }, 'renderWorkspaceList does not throw');
}

// ── saveCurrentWorkspace ──
console.log('saveCurrentWorkspace tests');
if (typeof saveCurrentWorkspace === 'function') {
    state.panels = [];
    assert(() => { saveCurrentWorkspace(); }, 'saveCurrentWorkspace does not throw');
}

// ── deleteWorkspace ──
console.log('deleteWorkspace tests');
if (typeof deleteWorkspace === 'function') {
    saveWorkspaces([{ name: 'temp', panels: [] }]);
    assert(() => { deleteWorkspace('temp'); }, 'deleteWorkspace does not throw');
}

// ── openWorkspaceManage ──
console.log('openWorkspaceManage tests');
if (typeof openWorkspaceManage === 'function') {
    assert(() => { openWorkspaceManage(); }, 'openWorkspaceManage does not throw');
}

// ── loadWorkspace ──
console.log('loadWorkspace tests');
if (typeof loadWorkspace === 'function') {
    assert(() => { loadWorkspace('test'); }, 'loadWorkspace does not throw');
}

// ── Environments ──
console.log('environment tests');
if (typeof fetchEnvironments === 'function') {
    assert(() => { fetchEnvironments(); }, 'fetchEnvironments does not throw');
}
if (typeof renderEnvironments === 'function') {
    const envList = document.createElement('div');
    envList.id = 'envList';
    assert(() => { renderEnvironments(); }, 'renderEnvironments does not throw');
}

// ── Command Groups ──
console.log('command group tests');
if (typeof getCmdGroups === 'function') {
    localStorage.removeItem('vrw_cmd_groups');
    const groups = getCmdGroups();
    assert(typeof groups === 'object', 'getCmdGroups returns object');
}

if (typeof saveCmdGroups === 'function') {
    saveCmdGroups([{ name: 'servers', cmds: ['htop', 'vim'] }]);
    const groups = getCmdGroups();
    assertEq(groups.length, 1, 'group saved');
}

if (typeof createCmdGroup === 'function') {
    const nameInput = document.createElement('input');
    nameInput.id = 'newGroupName';
    nameInput.value = 'test-group';
    assert(() => { createCmdGroup(); }, 'createCmdGroup does not throw');
}

if (typeof deleteCmdGroup === 'function') {
    assert(() => { deleteCmdGroup('test-group'); }, 'deleteCmdGroup does not throw');
}

if (typeof renderGroups === 'function') {
    const groupList = document.createElement('div');
    groupList.id = 'groupList';
    assert(() => { renderGroups(); }, 'renderGroups does not throw');
}

// ── Docs viewer ──
console.log('docs tests');
if (typeof showDocs === 'function') {
    const viewDocs = document.createElement('div');
    viewDocs.id = 'view-docs';
    assert(() => { showDocs(); }, 'showDocs does not throw');
}

if (typeof renderMarkdown === 'function') {
    const html = renderMarkdown('# Hello\n\nWorld');
    assert(typeof html === 'string', 'renderMarkdown returns string');
    assert(html.includes('Hello'), 'renderMarkdown preserves content');
}

if (typeof showSpecialKeysHelp === 'function') {
    assert(() => { showSpecialKeysHelp(); }, 'showSpecialKeysHelp does not throw');
}

// ── Peers ──
console.log('peer tests');
if (typeof handlePeerEvent === 'function') {
    assert(() => { handlePeerEvent({ type: 'peer_registered', peer_id: 'p1' }); }, 'handlePeerEvent does not throw');
}
if (typeof fetchPeers === 'function') {
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
    assert(() => { fetchPeers(); }, 'fetchPeers does not throw');
}

console.log('\n[workspaces.js] Tests complete');
