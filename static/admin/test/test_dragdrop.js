/// test/test_dragdrop.js — Tests for drag-and-drop functionality
require('./setup');

console.log('\n=== dragdrop.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};

// ── onCmdDragStart ──
console.log('onCmdDragStart tests');
if (typeof onCmdDragStart === 'function') {
    const evt = {
        dataTransfer: { setData(key, val) { this[key] = val; } },
        target: { style: { opacity: '' } },
        stopPropagation() {},
        preventDefault() {},
    };
    assert(() => { onCmdDragStart(evt, 'http://localhost:9090', 'cmd-1', 'htop'); }, 'onCmdDragStart does not throw');
    assertEq(evt.dataTransfer['text/plain'], 'cmd-1', 'dataTransfer set with cmd id');
}

// ── initPanelDropTargets ──
console.log('initPanelDropTargets tests');
if (typeof initPanelDropTargets === 'function') {
    assert(() => { initPanelDropTargets(); }, 'initPanelDropTargets does not throw');
}

// ── getCmdOrder / setCmdOrder ──
console.log('cmd order tests');
if (typeof getCmdOrder === 'function') {
    localStorage.removeItem('vrw_cmd_order');
    const order = getCmdOrder();
    assert(typeof order === 'object', 'getCmdOrder returns object');
}

if (typeof setCmdOrder === 'function') {
    setCmdOrder({ 'http://localhost:9090': ['htop', 'vim', 'bash'] });
    const order = getCmdOrder();
    assert(Array.isArray(order['http://localhost:9090']), 'order is array');
    assertEq(order['http://localhost:9090'].length, 3, 'order length correct');
}

// ── getOrderedCmds ──
console.log('getOrderedCmds tests');
if (typeof getOrderedCmds === 'function') {
    const items = [
        { name: 'vim', id: 'c2' },
        { name: 'htop', id: 'c1' },
        { name: 'bash', id: 'c3' },
    ];
    setCmdOrder({ 'http://localhost:9090': ['htop', 'vim', 'bash'] });
    const ordered = getOrderedCmds('http://localhost:9090', items);
    assertEq(ordered[0].name, 'htop', 'first ordered item is htop');
    assertEq(ordered[1].name, 'vim', 'second ordered item is vim');
}

// ── _openCommandInNewPane ──
console.log('_openCommandInNewPane tests');
if (typeof _openCommandInNewPane === 'function') {
    globalThis.connectPanelWs = function() {};
    globalThis.startUpdateMode = function() {};
    globalThis.focusPanel = function() {};
    globalThis._restoreCachedDom = function() {};
    globalThis.updatePanelCommandInfo = function() {};
    globalThis.updateTerminalDisconnectedOverlay = function() {};
    globalThis.updateSidebarSelection = function() {};
    globalThis.loadVttyHttpForPanel = function() {};
    globalThis.startPanelUpdateMode = function() {};
    globalThis.disconnectPanelWs = function() {};
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', _commands: [
        { id: 'cmd-1', name: 'htop' }
    ]}];
    assert(() => { _openCommandInNewPane('http://localhost:9090', 'cmd-1', 'htop'); }, '_openCommandInNewPane does not throw');
}

// ── onPanelDragStart ──
console.log('onPanelDragStart tests');
if (typeof onPanelDragStart === 'function') {
    const evt = {
        dataTransfer: { setData() {} },
        target: { closest() { return { dataset: { panelId: 'p1' } }; } },
        stopPropagation() {},
    };
    assert(() => { onPanelDragStart(evt); }, 'onPanelDragStart does not throw');
}

// ── onPanelDrop ──
console.log('onPanelDrop tests');
if (typeof onPanelDrop === 'function') {
    const evt = {
        dataTransfer: { getData() { return 'p1'; } },
        preventDefault() {},
        target: { closest() { return null; } },
    };
    assert(() => { onPanelDrop(evt); }, 'onPanelDrop does not throw');
}

console.log('\n[dragdrop.js] Tests complete');
