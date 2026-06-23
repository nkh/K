/// test/test_bug_autofit_focused_leaf.js — Fix 4.3
///   "Adapt font button doesn't do it on the selected pane"
///
/// BUG: autofitTerminalSize() queries the first .vtty-container in the
/// panel DOM, which in a split pane may not be the focused leaf.
///
/// FIX: Target the focused leaf's vtty-container using _focusedLeafId.

require('./setup');

console.log('\n=== Fix 4.3: Autofit targets focused leaf ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.loadCommands = function() { return Promise.resolve(); };
globalThis.selectCommand = function() {};
globalThis.loadVttyHttpForPanel = function() {};

// ──────────────────────────────────────────────────────────────
// FIX43-001: autofitTerminalSize measures the focused leaf, not first
// ──────────────────────────────────────────────────────────────
console.log('FIX43-001: autofit measures focused leaf vtty-container');
{
    // Set up a split panel with two leaves
    const panelId = 'panel-test';
    const rootLeafId = panelId;
    const branchLeafId = panelId + '-L1';

    state.panels = [{
        id: panelId,
        _focusedLeafId: branchLeafId,
        selectedCmdId: 'cmd-1',
        selectedInstUrl: 'http://localhost:9090',
        split: { direction: 'horizontal', splitRatio: 0.5, activeSide: 'panel', branch: { id: branchLeafId } },
    }];
    state._focusedPanelId = panelId;
    state.fontSize = 10;
    state.connections = [];

    // Build panel DOM with two vtty-containers
    const panelEl = document.createElement('div');
    panelEl.id = panelId;

    // Root leaf vtty (400x300)
    const vttyRoot = document.createElement('div');
    vttyRoot.className = 'vtty-container';
    vttyRoot.id = 'vtty-' + rootLeafId;
    vttyRoot.setAttribute('data-leaf-id', rootLeafId);
    // Mock getBoundingClientRect
    Object.defineProperty(vttyRoot, 'getBoundingClientRect', { value: () => ({ width: 400, height: 300 }) });
    panelEl.appendChild(vttyRoot);

    // Branch leaf vtty (200x500) — this is the focused one
    const vttyBranch = document.createElement('div');
    vttyBranch.className = 'vtty-container';
    vttyBranch.id = 'vtty-' + branchLeafId;
    vttyBranch.setAttribute('data-leaf-id', branchLeafId);
    Object.defineProperty(vttyBranch, 'getBoundingClientRect', { value: () => ({ width: 200, height: 500 }) });
    panelEl.appendChild(vttyBranch);

    const viewVtty = document.getElementById('view-vtty');
    if (viewVtty) viewVtty.appendChild(panelEl);
    else { document.body.appendChild(panelEl); }

    // Create autofitHint
    let hint = document.getElementById('autofitHint');
    if (!hint) { hint = document.createElement('span'); hint.id = 'autofitHint'; document.body.appendChild(hint); }

    // Create spawn fields
    let rowsField = document.getElementById('spawnRows');
    if (!rowsField) { rowsField = document.createElement('input'); rowsField.id = 'spawnRows'; document.body.appendChild(rowsField); }
    let colsField = document.getElementById('spawnCols');
    if (!colsField) { colsField = document.createElement('input'); colsField.id = 'spawnCols'; document.body.appendChild(colsField); }

    // Call autofit
    autofitTerminalSize();

    // The focused leaf is branch (200x500), not root (400x300)
    // With fontSize=10: cols = floor(200 / (10*0.6)) = floor(33.3) = 33
    //                   rows = floor(500 / (10*1.2)) = floor(41.6) = 41
    // If it was using root (buggy): cols = floor(400/6) = 66, rows = floor(300/12) = 25
    const hintText = hint.textContent;
    assert(hintText.includes('200x500'), 'FIX43-001a: autofit measured focused leaf (200x500), not root (400x300)');
    assertEq(String(colsField.value), '33', 'FIX43-001b: cols computed from focused leaf width');
    assertEq(String(rowsField.value), '41', 'FIX43-001c: rows computed from focused leaf height');
}

// ──────────────────────────────────────────────────────────────
// FIX43-002: Non-split panel still works (no regression)
// ──────────────────────────────────────────────────────────────
console.log('FIX43-002: Non-split panel autofit works');
{
    const panelId2 = 'panel-nosplit';
    state.panels = [{
        id: panelId2,
        _focusedLeafId: null,
        selectedCmdId: 'cmd-2',
        selectedInstUrl: 'http://localhost:9090',
        split: null,
    }];
    state._focusedPanelId = panelId2;
    state.fontSize = 12;

    const panelEl2 = document.createElement('div');
    panelEl2.id = panelId2;
    const vtty2 = document.createElement('div');
    vtty2.className = 'vtty-container';
    vtty2.id = 'vtty-' + panelId2;
    vtty2.setAttribute('data-leaf-id', panelId2);
    Object.defineProperty(vtty2, 'getBoundingClientRect', { value: () => ({ width: 600, height: 400 }) });
    panelEl2.appendChild(vtty2);
    document.body.appendChild(panelEl2);

    autofitTerminalSize();

    const hintText2 = document.getElementById('autofitHint').textContent;
    // cols = floor(600 / (12*0.6)) = floor(83.3) = 83
    // rows = floor(400 / (12*1.2)) = floor(27.7) = 27
    assert(hintText2.includes('600x400'), 'FIX43-002a: non-split panel measured correctly');
    assertEq(String(document.getElementById('spawnCols').value), '83', 'FIX43-002b: cols correct for non-split');
    assertEq(String(document.getElementById('spawnRows').value), '27', 'FIX43-002c: rows correct for non-split');
}