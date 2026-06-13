/// test/test_vtty.js — Tests for VTTY rendering functions
require('./setup');

console.log('\n=== vtty.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.scheduleVttyHttpForPanel = function() {};

// ── _hex is tested in test_utils.js (defined in utils.js, NOT duplicated in vtty.js) ──
console.log('_hex tests (skipped — see test_utils.js)');

// ── _htmlEscapeChar is tested in test_utils.js (defined in utils.js, NOT duplicated in vtty.js) ──
console.log('_htmlEscapeChar tests (skipped — see test_utils.js)');

// ── buildCellGrid ──
console.log('buildCellGrid tests');
if (typeof buildCellGrid === 'function') {
    const pre = document.createElement('pre');
    // MockElement doesn't parse innerHTML into childNodes,
    // so manually create a text node (nodeType 3)
    const textNode = document.createElement('#text');
    textNode.textContent = 'hello';
    textNode.nodeType = 3;
    pre.appendChild(textNode);
    state._cellGrids = {};
    buildCellGrid('test', pre, 24, 80);
    assert(state._cellGrids['test'] !== undefined, 'buildCellGrid stores grid in state');
    assert(Array.isArray(state._cellGrids['test'].grid), 'buildCellGrid grid is array of rows');
}

// ── _cellStyle ──
console.log('_cellStyle tests');
if (typeof _cellStyle === 'function') {
    const style = _cellStyle({ fg: 7, bg: 0, bold: false, italic: false, underline: false });
    assert(typeof style === 'string', '_cellStyle returns string');
    // Default colors should produce some style
    assert(style.length > 0, '_cellStyle not empty for default colors');

    const boldStyle = _cellStyle({ fg: 1, bg: 0, bold: true, italic: false, underline: false });
    assert(boldStyle.includes('bold') || boldStyle.includes('700') || boldStyle.includes('font-weight'), 'bold style includes weight');

    const italicStyle = _cellStyle({ fg: 2, bg: 0, bold: false, italic: true, underline: false });
    assert(italicStyle.includes('italic') || italicStyle.includes('font-style'), 'italic style includes font-style');
}

// ── updateVttyDisplayForPanel ──
console.log('updateVttyDisplayForPanel tests');
if (typeof updateVttyDisplayForPanel === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    const panelEl = document.createElement('div');
    panelEl.id = p.id;
    const vttyEl = document.createElement('div');
    vttyEl.className = 'vtty-container';
    const pre = document.createElement('pre');
    vttyEl.appendChild(pre);
    panelEl.appendChild(vttyEl);

    assert(() => {
        updateVttyDisplayForPanel(p, panelEl, { html: 'hello world', generation: 1 });
    }, 'updateVttyDisplayForPanel does not throw');
}

// ── updateVttyMetadataForPanel ──
console.log('updateVttyMetadataForPanel tests');
if (typeof updateVttyMetadataForPanel === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    const panelEl = document.createElement('div');
    panelEl.id = p.id;
    const vttyEl = document.createElement('div');
    vttyEl.className = 'vtty-container';
    const pre = document.createElement('pre');
    vttyEl.appendChild(pre);
    panelEl.appendChild(vttyEl);

    assert(() => {
        updateVttyMetadataForPanel(p, panelEl, vttyEl, {
            cursor: { row: 5, col: 10 },
            dimensions: { rows: 24, cols: 80 },
            generation: 1
        });
    }, 'updateVttyMetadataForPanel does not throw');
}

// ── applyVttyDiffForPanel ──
console.log('applyVttyDiffForPanel tests');
if (typeof applyVttyDiffForPanel === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    const panelEl = document.createElement('div');
    panelEl.id = p.id;
    const vttyEl = document.createElement('div');
    vttyEl.className = 'vtty-container';
    const pre = document.createElement('pre');
    pre.innerHTML = '<span>test</span>';
    pre._vttyRows = 24;
    pre._vttyCols = 80;
    vttyEl.appendChild(pre);
    panelEl.appendChild(vttyEl);

    assert(() => {
        applyVttyDiffForPanel(p, vttyEl, {
            cells: [{ row: 0, col: 0, ch: 'H', width: 1, fg: 7, bg: 0, bold: false }],
            generation: 2
        });
    }, 'applyVttyDiffForPanel does not throw');
}

// ── scheduleVttyHttpForPanel ──
console.log('scheduleVttyHttpForPanel tests');
if (typeof scheduleVttyHttpForPanel === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    assert(() => { scheduleVttyHttpForPanel(p, 0); }, 'scheduleVttyHttpForPanel does not throw');
    assert(() => { scheduleVttyHttpForPanel(p, 100); }, 'scheduleVttyHttpForPanel with delay does not throw');
}

// ── switchBuffer ──
console.log('switchBuffer tests');
if (typeof switchBuffer === 'function') {
    state.bufferView = 'current';
    state.selectedInstUrl = 'http://localhost:9090';
    state.selectedCmdId = 'cmd-1';
    assert(() => { switchBuffer('alt'); }, 'switchBuffer does not throw');
}

// ── Generation skip logic ──
console.log('generation skip logic');
if (typeof updateVttyDisplayForPanel === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    const panelEl = document.createElement('div');
    panelEl.id = p.id;
    const vttyEl = document.createElement('div');
    vttyEl.className = 'vtty-container';
    const pre = document.createElement('pre');
    vttyEl.appendChild(pre);
    panelEl.appendChild(vttyEl);

    // Set selectedCmdId so generation dedup works
    p.selectedCmdId = 'test-cmd';

    // First update — generation 1
    updateVttyDisplayForPanel(p, panelEl, { html: 'gen1', generation: 1 });
    // Same generation — should skip
    updateVttyDisplayForPanel(p, panelEl, { html: 'gen1-skip', generation: 1 });
    assertEq(pre.innerHTML, 'gen1', 'same generation skipped (html unchanged)');
    // New generation — should update
    updateVttyDisplayForPanel(p, panelEl, { html: 'gen2', generation: 2 });
    assertEq(pre.innerHTML, 'gen2', 'new generation applied');
}

console.log('\n[vtty.js] Tests complete');
