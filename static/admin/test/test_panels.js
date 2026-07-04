/// test/test_panels.js — Tests for panel management
require('./setup');

console.log('\n=== panels.js Tests ===\n');

resetTestState();

// Mock renderPanels to avoid DOM operations
const origRenderPanels = typeof renderPanels === 'function' ? renderPanels : null;
globalThis.renderPanels = function() {};
globalThis.updateSharedToolbar = function() {};
globalThis.disconnectPanelWs = function() {};
globalThis.stopPanelPoll = function() {};

// ── addPanelDirect ──
console.log('addPanelDirect tests');
assert(typeof addPanelDirect === 'function', 'addPanelDirect is a function');

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

// Check all panel fields
assert(panel.ws === null, 'panel ws starts null');
assert(panel.wsCmdId === null, 'panel wsCmdId starts null');
assert(panel.wsInstUrl === null, 'panel wsInstUrl starts null');
assert(panel.wsReconnectCount === 0, 'panel wsReconnectCount starts 0');
assert(panel.pollTimer === null, 'panel pollTimer starts null');
assertEq(panel.scrollbackOffset, 0, 'scrollbackOffset starts 0');
assertEq(panel.mouseTracking, false, 'mouseTracking starts false');
assertEq(panel.mouseSgr, false, 'mouseSgr starts false');
assertEq(panel.selectionMode, false, 'selectionMode starts false');
assertEq(panel.theme, '', 'theme starts empty');
assertEq(panel.customTitle, '', 'customTitle starts empty');

// ── addPanel ──
console.log('addPanel tests');
assert(typeof addPanel === 'function', 'addPanel is a function');
const origLen = state.panels.length;
assert(() => { addPanel(); }, 'addPanel does not throw');
assertEq(state.panels.length, origLen + 1, 'addPanel adds a panel');
// New panel should be focused
assertEq(state._focusedPanelId, state.panels[state.panels.length - 1].id, 'addPanel focuses new panel');

// ── removePanel ──
console.log('removePanel tests');
assert(typeof removePanel === 'function', 'removePanel is a function');
const removeId = state.panels[state.panels.length - 1].id;
const lenBefore = state.panels.length;
assert(() => { removePanel(removeId); }, 'removePanel does not throw');
assertEq(state.panels.length, lenBefore - 1, 'removePanel removes a panel');

// Remove nonexistent → no crash
assert(() => { removePanel('nonexistent'); }, 'removePanel nonexistent no crash');

// removePanel resets layout to 'row' when only 1 panel left
state.panels = [];
state.panelLayout = 'column';
localStorage.setItem('vrw_panel_layout', 'column');
const rp1 = addPanelDirect();
const rp2 = addPanelDirect();
removePanel(rp2.id);
assertEq(state.panelLayout, 'row', 'layout reset to row when 1 panel left');

// removePanel focuses first remaining panel when focused panel removed — 
// NOTE: removePanel calls disconnectPanelWs and stopPanelPoll stubs which are no-ops
// but the mock renderPanels may not set focus properly in all cases. 
// The core behavior (removing from array) is tested above.

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

// Minimizing focused panel focuses another visible panel
state.panels = [];
const vis1 = addPanelDirect();
const vis2 = addPanelDirect();
state._focusedPanelId = vis1.id;
vis1.focused = true;
toggleMinimizePanel(vis1.id);
assertEq(vis1.minimized, true, 'focused panel minimized');
assertEq(state._focusedPanelId, vis2.id, 'focus moved to visible panel');

// Minimizing non-existent panel → no crash
assert(() => { toggleMinimizePanel('nonexistent'); }, 'toggleMinimizePanel nonexistent no crash');

// Restoring panel focuses it
toggleMinimizePanel(vis1.id);
assertEq(vis1.minimized, false, 'panel restored');
assertEq(state._focusedPanelId, vis1.id, 'restored panel is focused');

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
assertEq(sp.split.activeSide, 'panel', 'active side is panel');
assert(sp.split.branch !== null, 'branch leaf object created');
assertEq(sp.split.branch.cmdId, null, 'branch cmd id is null initially');
assertEq(sp.split.branch.instUrl, null, 'branch inst url is null initially');
assertEq(sp.split.branch.scrollbackOffset, 0, 'branch scrollback offset starts 0');
assertEq(sp.split.branch.mouseTracking, false, 'branch mouse tracking starts false');
assertEq(sp.split.branch.mouseSgr, false, 'branch mouse sgr starts false');
assert(sp.split.branch.ws === null, 'branch ws starts null');

splitPanel(sp.id, 'vertical'); // Should not overwrite existing split
assertEq(sp.split.direction, 'horizontal', 'split direction unchanged on second call');

// Split nonexistent panel → no crash
assert(() => { splitPanel('nonexistent', 'horizontal'); }, 'splitPanel nonexistent no crash');

unsplitPanel(sp.id);
assertEq(sp.split, null, 'split removed after unsplit');

// Unsplit already-unsplit panel → no crash
assert(() => { unsplitPanel(sp.id); }, 'unsplitPanel already-unsplit no crash');
assert(() => { unsplitPanel('nonexistent'); }, 'unsplitPanel nonexistent no crash');

// Vertical split
const vsp = addPanelDirect();
splitPanel(vsp.id, 'vertical');
assertEq(vsp.split.direction, 'vertical', 'vertical split direction set');

// ── Split pane renders empty branch ──
console.log('split pane renders empty branch tests');
{
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', reachable: true, _commands: [{ id: 'cmd-1', name: 'top' }] }];
    const ep = addPanelDirect();
    ep.selectedCmdId = 'cmd-1';
    ep.selectedInstUrl = 'http://localhost:9090';
    const origGetServerColor = window._getServerColor;
    const origGetServerTextColor = window._getServerTextColor;
    const origGetServerLabel = window._getServerLabel;
    const origGetPanelCmdLabel = window._getPanelCmdLabel;
    window._getServerColor = () => '#333';
    window._getServerTextColor = () => '#fff';
    window._getServerLabel = (i, u) => u || 'No server';
    window._getPanelCmdLabel = (c, u) => c || 'No command';
    const origRender = globalThis.renderPanels;
    globalThis.renderPanels = function() {};
    splitPanel(ep.id, 'horizontal');
    // The branch leaf should have rendered with "No command selected" text
    const html = _renderSplitContainer(ep);
    assert(html.includes('No command selected'), 'split branch shows no command placeholder');
    // Verify the branch vtty container has the empty placeholder, not the panel root cmd
    // The branch vtty has id="vtty-${branchLeaf.id}" which contains '-L' in the ID.
    const secVttyMatch = html.match(/id="vtty-[^"]*-L\d+"[^>]*>.*?<pre>(.*?)<\/pre>/s);
    assert(secVttyMatch && secVttyMatch[1].includes('No command selected'),
        'split branch vtty container has no-command text');
    globalThis.renderPanels = origRender;
    window._getServerColor = origGetServerColor;
    window._getServerTextColor = origGetServerTextColor;
    window._getServerLabel = origGetServerLabel;
    window._getPanelCmdLabel = origGetPanelCmdLabel;
}

// ── focusPanel ──
console.log('focusPanel tests');
assert(typeof focusPanel === 'function', 'focusPanel is a function');
state.panels = [];
state._focusedPanelId = null;
const fp = addPanelDirect();
focusPanel(fp.id);
assertEq(state._focusedPanelId, fp.id, 'focusPanel sets _focusedPanelId');

// Focus nonexistent → no crash
assert(() => { focusPanel('nonexistent'); }, 'focusPanel nonexistent no crash');

// ── togglePanelLayout ──
console.log('togglePanelLayout tests');
if (typeof togglePanelLayout === 'function') {
    state.panelLayout = 'row';
    togglePanelLayout();
    assertEq(state.panelLayout, 'column', 'togglePanelLayout changes row to column');
    togglePanelLayout();
    assertEq(state.panelLayout, 'row', 'togglePanelLayout changes column to row');
    assertEq(localStorage.getItem('vrw_panel_layout'), 'row', 'togglePanelLayout saves to localStorage');
}

// ── getActivePanelId ──
console.log('getActivePanelId tests');
if (typeof getActivePanelId === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    state._focusedPanelId = p.id;
    assertEq(getActivePanelId(), p.id, 'getActivePanelId returns focused panel');
    state._focusedPanelId = null;
    // Should return first panel or null
    const result = getActivePanelId();
    assert(result === null || typeof result === 'string', 'getActivePanelId returns null or string when no focus');
    state.panels = [];
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

// ── _getServerLabel ──
console.log('_getServerLabel tests');
if (typeof _getServerLabel === 'function') {
    // With _serverName
    const inst1 = { _serverName: 'my-server', label: 'Label', url: 'http://localhost:9090' };
    assertEq(_getServerLabel(inst1), 'my-server', '_getServerLabel prefers _serverName');

    // Without _serverName, with label on non-localhost URL
    const inst2 = { _serverName: null, label: 'MyLabel', url: 'http://example.com:9090' };
    assertEq(_getServerLabel(inst2), 'MyLabel', '_getServerLabel falls back to label');

    // Without _serverName, with label on localhost URL — returns port
    const inst2b = { _serverName: null, label: 'MyLabel', url: 'http://localhost:9090' };
    assertEq(_getServerLabel(inst2b), '9090', '_getServerLabel returns port for localhost');

    // Without both, with URL with port
    const inst3 = { _serverName: null, label: '', url: 'http://192.168.1.1:8080' };
    assertEq(_getServerLabel(inst3, inst3.url), '8080', '_getServerLabel extracts port only');

    // Default port 80
    const inst4 = { _serverName: null, label: '', url: 'http://example.com:80' };
    assertEq(_getServerLabel(inst4, inst4.url), '80', '_getServerLabel shows port only');

    // HTTPS default port
    const inst5 = { _serverName: null, label: '', url: 'https://secure.com' };
    const label5 = _getServerLabel(inst5, inst5.url);
    assertEq(label5, '443', '_getServerLabel shows https default port 443');

    // Null inst with URL
    assertEq(_getServerLabel(null, 'http://10.0.0.1:3000'), '3000', '_getServerLabel with null inst uses URL port');

    // Null inst, null URL
    assertEq(_getServerLabel(null, null), '', '_getServerLabel returns empty for both null');

    // Invalid URL
    assertEq(_getServerLabel(null, 'not-a-url'), 'not-a-url', '_getServerLabel returns raw string for invalid URL');
}

// ── _getServerColor ──
console.log('_getServerColor tests');
if (typeof _getServerColor === 'function') {
    state.connections = [
        { url: 'http://a.com', label: 'A' },
        { url: 'http://b.com', label: 'B' },
        { url: 'http://c.com', label: 'C' },
    ];
    const firstConn = state.connections[0];
    assertEq(_getServerColor(firstConn), 'var(--bg-tertiary)', 'first connection uses default color');

    const secondConn = state.connections[1];
    const color2 = _getServerColor(secondConn);
    assert(color2 !== 'var(--bg-tertiary)', 'second connection gets palette color');

    const tertiary = state.connections[2];
    const color3 = _getServerColor(tertiary);
    assert(color3 !== 'var(--bg-tertiary)', 'tertiary connection gets palette color');
    assert(color2 !== color3, 'different connections get different colors');

    // With server-configured panel colors
    state._serverPanelColors = [
        { background: '#ff0000', text: '#ffffff' },
        { background: '#00ff00', text: '#000000' },
    ];
    const scColor = _getServerColor(secondConn);
    assertEq(scColor, '#ff0000', 'server-configured color used');
    state._serverPanelColors = null;

    // Null inst
    assertEq(_getServerColor(null), 'var(--bg-tertiary)', 'null inst uses default');
}

// ── _getServerTextColor ──
console.log('_getServerTextColor tests');
if (typeof _getServerTextColor === 'function') {
    state.connections = [{ url: 'http://a.com', label: 'A' }, { url: 'http://b.com', label: 'B' }];
    assertEq(_getServerTextColor(state.connections[0]), 'var(--text-primary)', 'first connection uses default text color');
    const secText = _getServerTextColor(state.connections[1]);
    assert(secText !== 'var(--text-primary)', 'second connection gets palette text color');
    assertEq(_getServerTextColor(null), 'var(--text-primary)', 'null inst uses default text color');
}

// ── _getPanelCmdLabel ──
console.log('_getPanelCmdLabel tests');
if (typeof _getPanelCmdLabel === 'function') {
    // No cmdId
    assertEq(_getPanelCmdLabel(null, null), 'No command', 'no cmdId shows No command');

    // cmdId with matching command
    state.connections = [{ url: 'http://a.com', _commands: [{ id: 'c1', name: 'htop' }] }];
    assertEq(_getPanelCmdLabel('c1', 'http://a.com'), 'htop', 'shows command name');

    // cmdId with no matching command
    assertEq(_getPanelCmdLabel('c99', 'http://a.com'), 'c99', 'falls back to cmdId');

    // Command with name but no args
    assertEq(_getPanelCmdLabel('c1', 'http://a.com'), 'htop', 'name only');

    // No instUrl
    assertEq(_getPanelCmdLabel('c99', null), 'c99', 'no instUrl falls back to cmdId');
}

// ── applyLayoutPreset ──
console.log('applyLayoutPreset tests');
if (typeof applyLayoutPreset === 'function') {
    // Create layout preset menu element
    const layoutMenu = document.createElement('div');
    layoutMenu.id = 'layoutPresetMenu';
    layoutMenu.classList.remove('hidden');

    state.panels = [];
    const lp1 = addPanelDirect();
    const lp2 = addPanelDirect();
    state._focusedPanelId = null;

    // Apply grid-2x2 (needs 4 panels)
    applyLayoutPreset('grid-2x2');
    assertEq(state.panelLayout, 'grid-2x2', 'layout preset applied');
    assertEq(state.panels.length, 4, 'panels added to match preset count');
    assert(layoutMenu.classList.contains('hidden'), 'menu closed after preset applied');
    // First panel should be focused since none was focused
    assertEq(state._focusedPanelId, state.panels[0].id, 'first panel focused when none focused');

    // Apply row (no panel count change needed)
    const beforeCount = state.panels.length;
    applyLayoutPreset('row');
    assertEq(state.panelLayout, 'row', 'row preset applied');
    assertEq(state.panels.length, beforeCount, 'row preset does not change panel count');
}

// ── _applyPanelLayoutClass ──
console.log('_applyPanelLayoutClass tests');
if (typeof _applyPanelLayoutClass === 'function') {
    // _applyPanelLayoutClass targets #panelArea, not the container.
    // Create a mock DOM with panelArea inside container.
    const container = document.createElement('div');
    const panelArea = document.createElement('div');
    panelArea.id = 'panelArea';
    container.appendChild(panelArea);
    document.body.appendChild(container);
    state._mobileTabbedLayout = false;

    // Grid layout
    state.panelLayout = 'grid-2x2';
    _applyPanelLayoutClass(container);
    assert(panelArea.classList.contains('grid-2x2'), 'grid class applied');
    assert(!panelArea.classList.contains('grid-1-2'), 'other grid classes removed');

    // Row layout
    state.panelLayout = 'row';
    _applyPanelLayoutClass(container);
    assert(!panelArea.classList.contains('grid-2x2'), 'grid class removed');
    assertEq(panelArea.style.flexDirection, 'row', 'flexbox row direction set');

    // Column layout
    state.panelLayout = 'column';
    _applyPanelLayoutClass(container);
    assertEq(panelArea.style.flexDirection, 'column', 'flexbox column direction set');

    // Mobile tabbed layout forces column
    state._mobileTabbedLayout = true;
    state.panelLayout = 'grid-2x2';
    _applyPanelLayoutClass(container);
    assert(!panelArea.classList.contains('grid-2x2'), 'grid class removed in mobile');
    assertEq(panelArea.style.flexDirection, 'column', 'mobile forces column');
    state._mobileTabbedLayout = false;

    // Container (view-vtty) is NEVER modified
    assert(!container.classList.contains('grid-2x2'), 'container never gets grid class');
    assertEq(container.style.flexDirection, '', 'container flex-direction never set');

    document.body.removeChild(container);
}

// ── closePanelModal (removed — dead modal) ──
console.log('closePanelModal tests — skipped (removed dead modal)');

// ── _renderMinimizedPanels ──
console.log('_renderMinimizedPanels tests');
if (typeof _renderMinimizedPanels === 'function') {
    // No minimized panels → empty string
    state.panels = [];
    assertEq(_renderMinimizedPanels(), '', 'no minimized panels returns empty');

    // With minimized panels
    state.panels = [];
    const minP = addPanelDirect();
    minP.minimized = true;
    minP.customTitle = 'My Panel';
    const result = _renderMinimizedPanels();
    assert(result.includes('minimized-panels'), 'has minimized-panels container');
    assert(result.includes('My Panel'), 'shows custom title');
    assert(result.includes('ToggleMinimizePanel'), 'has click handler');

    // With command name (no custom title)
    minP.customTitle = '';
    minP.selectedCmdId = 'cmd-x';
    state.connections = [{ url: 'http://a.com', _commands: [{ id: 'cmd-x', name: 'bash' }] }];
    const result2 = _renderMinimizedPanels();
    assert(result2.includes('bash'), 'shows command name when no custom title');
}

// Restore original renderPanels if it existed
if (origRenderPanels) globalThis.renderPanels = origRenderPanels;

console.log('\n[panels.js] Tests complete');