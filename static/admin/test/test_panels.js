/// test/test_panels.js — Tests for panel management
require('./setup');

console.log('\n=== panels.js Tests ===\n');

resetTestState();

// ── addPanelDirect ──
console.log('addPanelDirect tests');
assert(typeof addPanelDirect === 'function', 'addPanelDirect is a function');

// Mock renderPanels to avoid DOM operations
const origRenderPanels = typeof renderPanels === 'function' ? renderPanels : null;
globalThis.renderPanels = function() {};

const panel = addPanelDirect();
assert(panel !== null && panel !== undefined, 'addPanelDirect returns a panel object');
assert(typeof panel.id === 'string', 'panel has id');
assert(panel.id.startsWith('panel-'), 'panel id starts with panel-');
assertEq(panel.minimized, false, 'panel not minimized by default');
assertEq(panel.focused, false, 'panel not focused by default');
assertEq(panel.selectedCmdId, null, 'no command selected by default');
assertEq(panel.selectedInstUrl, null, 'no instance URL by default');
assert(typeof panel.fontSize === 'number', 'panel has fontSize');
assert(Array.isArray(panel.cmdHistory), 'panel has cmdHistory array');
assertEq(panel.cmdHistoryIdx, -1, 'cmdHistoryIdx starts at -1');

// ── addPanel ──
console.log('addPanel tests');
assert(typeof addPanel === 'function', 'addPanel is a function');
const origLen = state.panels.length;
assert(() => { addPanel(); }, 'addPanel does not throw');
assertEq(state.panels.length, origLen + 1, 'addPanel adds a panel');

// ── removePanel ──
console.log('removePanel tests');
assert(typeof removePanel === 'function', 'removePanel is a function');
const removeId = state.panels[state.panels.length - 1].id;
const lenBefore = state.panels.length;
assert(() => { removePanel(removeId); }, 'removePanel does not throw');
assertEq(state.panels.length, lenBefore - 1, 'removePanel removes a panel');

// ── toggleMinimizePanel ──
console.log('toggleMinimizePanel tests');
assert(typeof toggleMinimizePanel === 'function', 'toggleMinimizePanel is a function');
state.panels = [];
const mp = addPanelDirect();
assertEq(mp.minimized, false, 'panel starts unminimized');
toggleMinimizePanel(mp.id);
assertEq(mp.minimized, true, 'toggleMinimizePanel minimizes');
toggleMinimizePanel(mp.id);
assertEq(mp.minimized, false, 'toggleMinimizePanel restores');

// ── splitPanel / unsplitPanel ──
console.log('splitPanel tests');
assert(typeof splitPanel === 'function', 'splitPanel is a function');
assert(typeof unsplitPanel === 'function', 'unsplitPanel is a function');

state.panels = [];
const sp = addPanelDirect();
assertEq(sp.split, undefined, 'panel has no split initially');

splitPanel(sp.id, 'horizontal');
assert(sp.split !== null, 'split created');
assertEq(sp.split.direction, 'horizontal', 'split direction is horizontal');
assertEq(sp.split.splitRatio, 0.5, 'split ratio is 0.5');
assertEq(sp.split.activeSide, 'primary', 'active side is primary');
assertEq(sp.split.secondaryCmdId, null, 'secondary cmd id is null initially');

splitPanel(sp.id, 'vertical'); // Should not overwrite existing split
assertEq(sp.split.direction, 'horizontal', 'split direction unchanged on second call');

unsplitPanel(sp.id);
assertEq(sp.split, null, 'split removed after unsplit');

// ── focusPanel ──
console.log('focusPanel tests');
assert(typeof focusPanel === 'function', 'focusPanel is a function');
state.panels = [];
state._focusedPanelId = null;
const fp = addPanelDirect();
focusPanel(fp.id);
assertEq(state._focusedPanelId, fp.id, 'focusPanel sets _focusedPanelId');

// ── togglePanelLayout ──
console.log('togglePanelLayout tests');
if (typeof togglePanelLayout === 'function') {
    state.panelLayout = 'row';
    togglePanelLayout();
    assert(state.panelLayout !== 'row', 'togglePanelLayout changes layout');
}

// ── getActivePanelId ──
console.log('getActivePanelId tests');
if (typeof getActivePanelId === 'function') {
    state._focusedPanelId = 'panel-123';
    assertEq(getActivePanelId(), 'panel-123', 'getActivePanelId returns focused panel');
    state._focusedPanelId = null;
    // Should return first panel or null
    const result = getActivePanelId();
    assert(result === null || typeof result === 'string', 'getActivePanelId returns null or string when no focus');
}

// ── changePanelFontSize ──
console.log('changePanelFontSize tests');
if (typeof changePanelFontSize === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    const origSize = p.fontSize;
    changePanelFontSize(p.id, 2);
    assertEq(p.fontSize, origSize + 2, 'fontSize increased by delta');
    changePanelFontSize(p.id, -1);
    assertEq(p.fontSize, origSize + 1, 'fontSize decreased by delta');
}

// Restore original renderPanels if it existed
if (origRenderPanels) globalThis.renderPanels = origRenderPanels;

console.log('\n[panels.js] Tests complete');
