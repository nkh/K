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

// ── applyVttyDiffForPanel with server-format cell data ──
// This test verifies the critical fix: the server sends cell diffs with
// a nested structure { row, col, cell: { ch, fg, bg, ..., width } },
// and the fast path in applyVttyDiffForPanel must read ch and width
// from the nested cell object, not from the top level.
//
// The test simulates the state that exists after a vtty_full message has
// been rendered and buildCellGrid() has populated state._cellGrids:
//   - A <pre> with <span> children (one per terminal cell)
//   - A cell grid mapping (row, col) → { span, idx, len }
//   - The panel's selectedCmdId and selectedInstUrl set
//
// Then it sends a vtty_diff in the exact format the server produces and
// verifies that the correct character and CSS class are applied.
console.log('applyVttyDiffForPanel server-format cell data tests');
if (typeof applyVttyDiffForPanel === 'function') {
    state.panels = [];
    const p = addPanelDirect();
    p.selectedCmdId = 'test-cmd-diff';
    p.selectedInstUrl = 'http://localhost:9090';
    p.fontSize = 10;

    // Create the panel DOM: panel > .vtty-container > pre > span chars
    const panelEl = document.createElement('div');
    panelEl.id = p.id;
    const vttyEl = document.createElement('div');
    vttyEl.className = 'vtty-container';
    // Mock scrollHeight/scrollTop for scroll position logic
    vttyEl.scrollHeight = 100;
    vttyEl.clientHeight = 100;
    vttyEl.scrollTop = 100;
    const pre = document.createElement('pre');
    vttyEl.appendChild(pre);
    panelEl.appendChild(vttyEl);
    document.body.appendChild(panelEl);

    // Simulate 3 cells in a single row: "ABC"
    // After buildCellGrid, each cell gets its own grid entry.
    // For the fast path (len === 1), each span holds exactly 1 character.
    const spanA = document.createElement('span');
    spanA.textContent = 'A';
    spanA.className = 'c w1';
    spanA.setAttribute('style', 'width:1ch;color:#e0e0e0;background:#1a1a2e');
    pre.appendChild(spanA);

    const spanB = document.createElement('span');
    spanB.textContent = 'B';
    spanB.className = 'c w1';
    spanB.setAttribute('style', 'width:1ch;color:#e0e0e0;background:#1a1a2e');
    pre.appendChild(spanB);

    const spanC = document.createElement('span');
    spanC.textContent = 'C';
    spanC.className = 'c w1';
    spanC.setAttribute('style', 'width:1ch;color:#e0e0e0;background:#1a1a2e');
    pre.appendChild(spanC);

    // Manually set up the cell grid (normally done by buildCellGrid)
    // Key must be panelId/cmdId (per-panel cell grids)
    state._cellGrids[p.id + '/test-cmd-diff'] = {
        grid: [
            [
                { span: spanA, idx: 0, len: 1 },
                { span: spanB, idx: 0, len: 1 },
                { span: spanC, idx: 0, len: 1 },
            ]
        ],
        rows: 1,
        cols: 3,
    };
    // Set a previous generation so the diff is not skipped
    state._lastGeneration[p.id + '/test-cmd-diff'] = 1;

    // Send a vtty_diff in the exact server format.
    // Change cell (0,1) from 'B' to 'X' with bold red on black.
    const diffData = {
        generation: 2,
        cursor: { row: 0, col: 3 },
        dimensions: { rows: 1, cols: 3 },
        changed_count: 1,
        cells: [
            {
                row: 0,
                col: 1,
                cell: {
                    ch: 'X',
                    fg: [255, 0, 0],
                    bg: [0, 0, 0],
                    bold: true,
                    italic: false,
                    underline: false,
                    blink: false,
                    reverse: false,
                    invisible: false,
                    strikethrough: false,
                    width: 1,
                },
            },
        ],
    };

    applyVttyDiffForPanel(p, panelEl, diffData);

    // Verify: spanB must now show 'X' (not 'undefined' or 'B')
    assertEq(spanB.textContent, 'X', 'fast path reads ch from cell.ch (nested)');
    // Verify: spanB must have bold in its style
    assert(spanB.getAttribute('style').includes('bold'), 'fast path applies bold style from cell');
    // Verify: spanB must have the correct fg color (red = ff0000)
    assert(spanB.getAttribute('style').includes('#ff0000'), 'fast path applies fg color from cell.fg');
    // Verify: spanB must still have w1 class (width 1)
    assertEq(spanB.className, 'c w1', 'fast path sets correct width class from cell.width');

    // Now test wide character (width=2) — should get 'c w2' class
    state._lastGeneration['test-cmd-diff'] = 2;
    const wideDiffData = {
        generation: 3,
        cursor: { row: 0, col: 3 },
        dimensions: { rows: 1, cols: 3 },
        changed_count: 1,
        cells: [
            {
                row: 0,
                col: 2,
                cell: {
                    ch: '\u4e16',  // 世 — wide character
                    fg: [0, 255, 0],
                    bg: [0, 0, 0],
                    bold: false, italic: false, underline: false,
                    blink: false, reverse: false, invisible: false,
                    strikethrough: false,
                    width: 2,
                },
            },
        ],
    };
    applyVttyDiffForPanel(p, panelEl, wideDiffData);
    assertEq(spanC.textContent, '\u4e16', 'fast path handles wide char from cell.ch');
    assertEq(spanC.className, 'c w2', 'fast path sets w2 class for wide char (cell.width=2)');

    // Test zero-width continuation character — should get 'c w0' class and \u200b
    state._lastGeneration['test-cmd-diff'] = 3;
    const zwDiffData = {
        generation: 4,
        cursor: { row: 0, col: 3 },
        dimensions: { rows: 1, cols: 3 },
        changed_count: 1,
        cells: [
            {
                row: 0,
                col: 2,
                cell: {
                    ch: '\u0000',  // null char → rendered as space
                    fg: [255, 255, 255],
                    bg: [0, 0, 0],
                    bold: false, italic: false, underline: false,
                    blink: false, reverse: false, invisible: false,
                    strikethrough: false,
                    width: 0,  // zero-width continuation
                },
            },
        ],
    };
    applyVttyDiffForPanel(p, panelEl, zwDiffData);
    assertEq(spanC.textContent, '\u200b', 'fast path renders zero-width cell as \\u200b');
    assertEq(spanC.className, 'c w0', 'fast path sets w0 class for zero-width cell (cell.width=0)');

    // Cleanup
    document.body.removeChild(panelEl);
    delete state._cellGrids[p.id + '/test-cmd-diff'];
    delete state._lastGeneration[p.id + '/test-cmd-diff'];
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

// ── Secondary pane VTTY display (moved from websocket.js) ──
console.log('secondary VTTY display tests');
if (typeof updateSecondaryVttyDisplay === 'function') {
    const metaP = addPanelDirect();
    metaP.split = { direction: 'horizontal', splitRatio: 0.5, activeSide: 'primary', secondary: { id: metaP.id + '-s1', cmdId: 's1', instUrl: null, scrollbackOffset: 0, mouseTracking: false, mouseSgr: false } };
    metaP.fontSize = 10;

    const vttyEl = document.createElement('div');
    vttyEl.className = 'vtty-container';
    const cursorEl = document.createElement('div');
    cursorEl.className = 'cursor-indicator';
    cursorEl.classList.add('hidden');
    const pre = document.createElement('pre');
    vttyEl.appendChild(cursorEl);
    vttyEl.appendChild(pre);

    // Full display with cursor + metadata
    updateSecondaryVttyDisplay(metaP, vttyEl, {
        html: '<span>hello</span>', generation: 1,
        cursor: { row: 5, col: 10, cursor_visible: true },
        dimensions: { rows: 24, cols: 80 },
        mouse_tracking: true, mouse_sgr: true,
    });
    assertEq(pre.innerHTML, '<span>hello</span>', 'secondary display html set');
    assert(!cursorEl.classList.contains('hidden'), 'secondary cursor shown');
    assert(cursorEl.style.top.includes('px'), 'secondary cursor top set');
    assertEq(metaP.split.secondary.mouseTracking, true, 'secondary mouse tracking set');
    assertEq(metaP.split.secondary.mouseSgr, true, 'secondary mouse sgr set');
    assertEq(pre._vttyRows, 24, 'secondary vttyRows stored');
    assertEq(pre._vttyCols, 80, 'secondary vttyCols stored');

    // Same generation → skip HTML, still update metadata
    updateSecondaryVttyDisplay(metaP, vttyEl, { generation: 1, cursor_visible: false });
    assertEq(pre.innerHTML, '<span>hello</span>', 'same gen skips html');
    assert(cursorEl.classList.contains('hidden'), 'cursor hidden via metadata-only update');

    // Cursor hidden in scrollback
    metaP.split.secondary.scrollbackOffset = 10;
    updateSecondaryVttyDisplay(metaP, vttyEl, { html: '<span>scroll</span>', generation: 2, cursor: { row: 1, col: 1, cursor_visible: true } });
    assert(cursorEl.classList.contains('hidden'), 'cursor hidden in scrollback');
    metaP.split.secondary.scrollbackOffset = 0;
}

if (typeof applySecondaryVttyDiff === 'function') {
    const diffP = addPanelDirect();
    diffP.split = { direction: 'horizontal', splitRatio: 0.5, activeSide: 'primary', secondary: { id: diffP.id + '-s1', cmdId: 's1', instUrl: null } };
    const diffVtty = document.createElement('div');
    diffVtty.className = 'vtty-container';
    const diffPre = document.createElement('pre');
    diffVtty.appendChild(diffPre);

    // No cmdId → no-op
    const noCmdP = addPanelDirect();
    noCmdP.split = { direction: 'horizontal', splitRatio: 0.5, activeSide: 'primary', secondary: { id: noCmdP.id + '-s1', cmdId: null, instUrl: null } };
    assert(() => { applySecondaryVttyDiff(noCmdP, diffVtty, {}); }, 'applySecondaryVttyDiff no-cmd no crash');

    // HTML fallback
    assert(() => { applySecondaryVttyDiff(diffP, diffVtty, { html: '<span>diff</span>', generation: 1 }); }, 'applySecondaryVttyDiff html fallback does not throw');
    assertEq(diffPre.innerHTML, '<span>diff</span>', 'diff html applied');
}

if (typeof scheduleSecondaryVttyHttp === 'function') {
    const schedP = addPanelDirect();
    schedP.split = { direction: 'horizontal', splitRatio: 0.5, activeSide: 'primary', secondary: { id: schedP.id + '-s1', cmdId: 's1', instUrl: 'http://localhost:9090' } };
    assert(() => { scheduleSecondaryVttyHttp(schedP, 50); }, 'scheduleSecondaryVttyHttp does not throw');

    const noSplitP = addPanelDirect();
    assert(() => { scheduleSecondaryVttyHttp(noSplitP, 50); }, 'scheduleSecondaryVttyHttp no-split no crash');
}

console.log('\n[vtty.js] Tests complete');
