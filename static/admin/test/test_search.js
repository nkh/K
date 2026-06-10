/// test/test_search.js — Tests for terminal search and global search
require('./setup');

console.log('\n=== search.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.scheduleVttyHttpForPanel = function() {};

// ── vttySearch ──
console.log('vttySearch tests');
if (typeof vttySearch === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    const vttyEl = document.createElement('div');
    vttyEl.id = 'vtty-' + p.id;
    _elementRegistry.set('vtty-' + p.id, vttyEl);
    const pre = document.createElement('pre');
    pre.innerHTML = 'hello world\nfoo bar\nbaz qux';
    pre._vttyRows = 3;
    pre._vttyCols = 15;
    vttyEl.appendChild(pre);

    const searchInput = document.createElement('input');
    searchInput.id = 'searchInput-' + p.id;
    searchInput.value = 'foo';
    _elementRegistry.set('searchInput-' + p.id, searchInput);

    assert(() => { vttySearch(p.id); }, 'vttySearch does not throw');
}

// ── vttyApplyHighlights ──
console.log('vttyApplyHighlights tests');
if (typeof vttyApplyHighlights === 'function') {
    const pre = document.createElement('pre');
    pre.innerHTML = 'hello world foo bar';
    const result = vttyApplyHighlights(pre, 'hello world', 'foo');
    assert(typeof result === 'undefined', 'vttyApplyHighlights returns undefined (void)');
}

// ── vttyRemoveHighlights ──
console.log('vttyRemoveHighlights tests');
if (typeof vttyRemoveHighlights === 'function') {
    const pre = document.createElement('pre');
    pre.innerHTML = 'hello <mark>world</mark> foo';
    assert(() => { vttyRemoveHighlights(pre); }, 'vttyRemoveHighlights does not throw');
}

// ── vttySearchNext / vttySearchPrev ──
console.log('vttySearchNext/vttySearchPrev tests');
if (typeof vttySearchNext === 'function') {
    assert(() => { vttySearchNext('panel-test'); }, 'vttySearchNext does not throw');
}
if (typeof vttySearchPrev === 'function') {
    assert(() => { vttySearchPrev('panel-test'); }, 'vttySearchPrev does not throw');
}

// ── vttySearchClose ──
console.log('vttySearchClose tests');
if (typeof vttySearchClose === 'function') {
    assert(() => { vttySearchClose('panel-test'); }, 'vttySearchClose does not throw');
}

// ── openGlobalSearch ──
console.log('openGlobalSearch tests');
if (typeof openGlobalSearch === 'function') {
    const modal = document.createElement('div');
    modal.id = 'globalSearchModal';
    const input = document.createElement('input');
    input.id = 'globalSearchInput';
    assert(() => { openGlobalSearch(); }, 'openGlobalSearch does not throw');
}

// ── closeGlobalSearch ──
console.log('closeGlobalSearch tests');
if (typeof closeGlobalSearch === 'function') {
    assert(() => { closeGlobalSearch(); }, 'closeGlobalSearch does not throw');
}

// ── executeGlobalSearch ──
console.log('executeGlobalSearch tests');
if (typeof executeGlobalSearch === 'function') {
    assert(() => { executeGlobalSearch(); }, 'executeGlobalSearch does not throw');
}

// ── openCmdManager ──
console.log('openCmdManager tests');
if (typeof openCmdManager === 'function') {
    const modal = document.createElement('div');
    modal.id = 'cmdManagerModal';
    assert(() => { openCmdManager(); }, 'openCmdManager does not throw');
}

// ── closeCmdManager ──
console.log('closeCmdManager tests');
if (typeof closeCmdManager === 'function') {
    assert(() => { closeCmdManager(); }, 'closeCmdManager does not throw');
}

// ── _freezeAllPanelsForSearch ──
console.log('_freezeAllPanelsForSearch tests');
if (typeof _freezeAllPanelsForSearch === 'function') {
    assert(() => { _freezeAllPanelsForSearch(); }, '_freezeAllPanelsForSearch does not throw');
}

// ── scrollTerminalBottom ──
console.log('scrollTerminalBottom tests');
if (typeof scrollTerminalBottom === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    const vttyEl = document.createElement('div');
    vttyEl.id = 'vtty-' + p.id;
    _elementRegistry.set('vtty-' + p.id, vttyEl);
    vttyEl.scrollHeight = 1000;
    vttyEl.clientHeight = 500;
    assert(() => { scrollTerminalBottom(p.id); }, 'scrollTerminalBottom does not throw');
}

console.log('\n[search.js] Tests complete');
