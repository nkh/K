// ─── VTTY Display ───
(function() {
    'use strict';

// ─── VTTY Display & Incremental Updates ───
// Handles full HTML replacement, cell-level diff patching (Level 3),
// cursor/metadata updates, scrollback, and split pane VTTY.

function updateVttyDisplayForPanel(panelObj, panelEl, data) {
    const vttyEl = panelEl.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    const cmdId = panelObj.selectedCmdId;
    const genKey = panelObj.id + '/' + cmdId;
    if (cmdId && data.generation !== undefined) {
        if (state._lastGeneration[genKey] === data.generation) {
            updateVttyMetadataForPanel(panelObj, panelEl, vttyEl, data);
            return;
        }
        state._lastGeneration[genKey] = data.generation;
    }

    if (data.html !== undefined && data.html !== null) {
        const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
        const oldScrollHeight = vttyEl.scrollHeight;
        pre.innerHTML = data.html;
        if (state._level3Enabled && data.dimensions) {
            buildCellGrid(genKey, pre, data.dimensions.rows, data.dimensions.cols);
        }
        if (wasAtBottom) {
            vttyEl.scrollTop = vttyEl.scrollHeight;
        } else {
            vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
        }
    }

    updateVttyMetadataForPanel(panelObj, panelEl, vttyEl, data);
}

function updateVttyMetadataForPanel(panelObj, panelEl, vttyEl, data) {
    const cursor = data.cursor || {};
    const dims = data.dimensions || {};
    // Sync toolbar resize inputs with actual server dimensions so that
    // Max Fit / Max Font / manual resize always start from the real values.
    if (dims.rows && dims.cols && panelObj.id === state._focusedPanelId) {
        const ri = document.getElementById('stResizeRows');
        const ci = document.getElementById('stResizeCols');
        // Only update if the inputs haven't been manually edited by the user
        // (i.e., they still contain the last server-reported values or defaults).
        if (ri && !ri._userEdited) ri.value = dims.rows;
        if (ci && !ci._userEdited) ci.value = dims.cols;
    }
    // Only update bottombar if this is the focused panel
    if (panelObj.id === state._focusedPanelId) {
        document.getElementById('cursorPos').textContent = `Cursor: ${cursor.row + 1},${cursor.col + 1}`;
        document.getElementById('termDims').textContent = `${dims.rows}x${dims.cols}`;
    }
    const inScrollback = panelObj.scrollbackOffset > 0;
    const cursorHidden = data.cursor_visible === false;
    const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
    if (cursorEl && cursor.row !== undefined && !inScrollback && !cursorHidden) {
        const charW = panelObj.fontSize * 0.6;
        const charH = panelObj.fontSize * 1.2;
        cursorEl.style.top = (cursor.row * charH) + 'px';
        cursorEl.style.left = (cursor.col * charW) + 'px';
        cursorEl.style.width = charW + 'px';
        cursorEl.style.height = charH + 'px';
        cursorEl.classList.remove('hidden');
    } else if (cursorEl) {
        cursorEl.classList.add('hidden');
    }
    panelObj.mouseTracking = !!data.mouse_tracking;
    panelObj.mouseSgr = !!data.mouse_sgr;
    if (vttyEl) {
        const mt = panelObj.mouseTracking;
        vttyEl.classList.toggle('selectable', !mt);
        const pre = vttyEl.querySelector('pre');
        if (pre && dims.rows && dims.cols) {
            pre._vttyRows = dims.rows;
            pre._vttyCols = dims.cols;
        }
    }
}

/// Per-panel version of applyVttyDiff.
/// Handles vtty_diff messages with data.cells array (cell-level diffs).
/// Falls back to full HTML fetch when no cell grid exists or dimensions changed.
function applyVttyDiffForPanel(panelObj, panelEl, data) {
    const cmdId = panelObj.selectedCmdId;
    if (!cmdId) return;
    const vttyEl = panelEl.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    const genKey = panelObj.id + '/' + cmdId;
    // Skip if generation unchanged (only update cursor/dimensions/mouse metadata)
    if (data.generation !== undefined && state._lastGeneration[genKey] === data.generation) {
        if (data.cursor || data.dimensions || data.mouse_tracking !== undefined) {
            updateVttyMetadataForPanel(panelObj, panelEl, vttyEl, data);
        }
        return;
    }
    if (data.generation !== undefined) {
        state._lastGeneration[genKey] = data.generation;
    }

    // If full HTML is embedded (e.g. from vtty_dirty fallback), use it directly
    if (data.html !== undefined) {
        const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
        const oldScrollHeight = vttyEl.scrollHeight;
        pre.innerHTML = data.html;
        if (state._level3Enabled && data.dimensions) {
            buildCellGrid(cgKey, pre, data.dimensions.rows, data.dimensions.cols);
        }
        if (wasAtBottom) {
            vttyEl.scrollTop = vttyEl.scrollHeight;
        } else {
            vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
        }
        updateVttyMetadataForPanel(panelObj, panelEl, vttyEl, data);
        return;
    }

    // Level 3 cell-level incremental diff
    if (!state._level3Enabled) {
        // Level 1/2: no cell grid — fall back to full HTML fetch
        scheduleVttyHttpForPanel(panelObj.id, panelObj.selectedInstUrl, cmdId, 0);
        return;
    }

    const cgKey = panelObj.id + '/' + cmdId;
    const cg = state._cellGrids[cgKey];
    if (!cg || !data.cells || !data.cells.length) {
        // No grid or no cells — fall back to full HTML fetch
        scheduleVttyHttpForPanel(panelObj.id, panelObj.selectedInstUrl, cmdId, 0);
        return;
    }

    // Check for dimension mismatch — if dimensions changed, need full resync
    const dims = data.dimensions || {};
    if (dims.rows !== cg.rows || dims.cols !== cg.cols) {
        delete state._cellGrids[cgKey];
        scheduleVttyHttpForPanel(panelObj.id, panelObj.selectedInstUrl, cmdId, 0);
        return;
    }

    // Save scroll position
    const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
    const oldScrollHeight = vttyEl.scrollHeight;

    // Apply each cell diff using the cell grid
    for (let i = 0; i < data.cells.length; i++) {
        const c = data.cells[i];
        if (c.row < cg.grid.length && c.col < cg.grid[c.row].length) {
            const entry = cg.grid[c.row][c.col];
            if (entry) {
                if (entry.len === 1) {
                    // Fast path: single-char span — update directly.
                    // Server sends { row, col, cell: { ch, fg, bg, ..., width } }.
                    const cell = c.cell;
                    const ch = cell.width === 0 ? '\u200b' : (cell.ch === '\u0000' ? ' ' : cell.ch);
                    entry.span.textContent = _htmlEscapeChar(ch);
                    entry.span.setAttribute('style', _cellStyle(c));
                    const wCls = cell.width === 0 ? 'c w0' : cell.width === 2 ? 'c w2' : 'c w1';
                    entry.span.className = wCls;
                } else {
                    // Slow path: split the merged span at the target position
                    _splitAndUpdateCell(cg, c.row, c.col, c);
                }
            }
        }
    }

    // Restore scroll position
    if (wasAtBottom) {
        vttyEl.scrollTop = vttyEl.scrollHeight;
    } else {
        vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
    }

    updateVttyMetadataForPanel(panelObj, panelEl, vttyEl, data);
}

/// Per-panel version of scheduleVttyHttp.
/// Uses a per-panel timer key to avoid clobbering HTTP fetches across panels.
function scheduleVttyHttpForPanel(panelId, instUrl, cmdId, delayMs) {
    const timerKey = '_vttyHttpTimer_' + panelId;
    if (state[timerKey]) clearTimeout(state[timerKey]);
    state[timerKey] = setTimeout(() => {
        state[timerKey] = null;
        loadVttyHttpForPanel(panelId, instUrl, cmdId);
    }, delayMs);
}

/// Per-panel version of loadVttyHttp.
async function loadVttyHttpForPanel(panelId, instUrl, cmdId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;

    const sbOffset = panelObj.scrollbackOffset;

    let endpoint;
    if (state.bufferView !== 'current') {
        const screenParam = `?screen=${state.bufferView}`;
        endpoint = `/api/commands/${cmdId}/vtty/buffer${screenParam}`;
    } else if (sbOffset > 0) {
        endpoint = `/api/commands/${cmdId}/vtty/html?scrollback_offset=${sbOffset}`;
    } else {
        endpoint = `/api/commands/${cmdId}/vtty/html`;
    }

    try {
        let json;
        if (endpoint === `/api/commands/${cmdId}/vtty/html`) {
            json = await api.getVttyHtml(instUrl, cmdId);
        } else {
            json = await api.getJson(endpoint, instUrl);
        }
        if (json.status === 'ok' && json.data) {
            updateVttyDisplayForPanel(panelObj, panelEl, json.data);
        }
    } catch (e) {
        // Silently ignore fetch errors (server might be unreachable)
    }
}

// ─── Level 3: Cell Grid for Incremental DOM Patching ───
// Builds a 2D array of span element references from the <pre> DOM tree,
// indexed as grid[row][col]. Each row is terminated by a \n text node in
// the HTML produced by VttyRenderer::to_html().
//
// This grid enables O(1) lookup for any (row, col) cell, allowing
// applyVttyDiff() to patch individual cells without destroying the entire
// DOM tree (no innerHTML replacement).

function buildCellGrid(gridKey, pre, rows, cols) {
    const grid = [];
    let currentRow = [];
    for (const child of pre.childNodes) {
        if (child.nodeType === Node.TEXT_NODE) {
            // Text nodes with only whitespace/newline mark row boundaries.
            // The server's to_html() emits a single '\n' between rows.
            if (child.textContent.includes('\n')) {
                // Split by newlines — each \n ends a row
                const parts = child.textContent.split('\n');
                for (let i = 0; i < parts.length - 1; i++) {
                    if (currentRow.length > 0 || i > 0) {
                        grid.push(currentRow);
                        currentRow = [];
                    }
                }
                // Trailing text (if any) is part of the next row — but there
                // shouldn't be any in the server's output format.
            }
        } else if (child.nodeType === Node.ELEMENT_NODE && child.tagName === 'SPAN') {
            // Server uses RLE: a single span may contain multiple characters.
            // Expand into per-cell entries for the cell grid.
            const text = child.textContent;
            // Use Array.from to iterate code points, not UTF-16 code units.
            // Supplementary-plane emoji (e.g. 😊) are 2 UTF-16 code units
            // but 1 terminal cell; indexing by code unit breaks the grid.
            const chars = Array.from(text);
            for (let i = 0; i < chars.length; i++) {
                currentRow.push({ span: child, idx: i, len: chars.length });
            }
        }
    }
    // Push the last row
    if (currentRow.length > 0) {
        grid.push(currentRow);
    }

    state._cellGrids[gridKey] = { grid, rows, cols };
}

// Generate the inline style string for a cell, matching the server's
// VttyRenderer::to_html() format exactly. This ensures visual consistency
// between full HTML replacement and incremental diff patching.
function _cellStyle(diff) {
    const c = diff.cell;
    let fg = c.fg;
    let bg = c.bg;

    // Handle reverse video: swap fg and bg
    if (c.reverse) {
        [fg, bg] = [bg, fg];
    }

    // Width in ch units: matches server-side run_len * cell_ch.
    // For single-cell updates (diff patching), run_len is always 1.
    const cellW = c.width || 1;
    let style = 'width:' + (cellW > 0 ? cellW + 'ch' : '0') + ';color:#' + _hex(fg[0]) + _hex(fg[1]) + _hex(fg[2]) + ';background:#' + _hex(bg[0]) + _hex(bg[1]) + _hex(bg[2]);

    if (c.bold) style += ';font-weight:bold';
    if (c.italic) style += ';font-style:italic';
    if (c.underline && c.strikethrough) {
        style += ';text-decoration:underline line-through';
    } else if (c.underline) {
        style += ';text-decoration:underline';
    } else if (c.strikethrough) {
        style += ';text-decoration:line-through';
    }
    if (c.blink) style += ';animation:blink 1s step-end infinite';

    return style;
}

// NOTE: _hex() and _htmlEscapeChar() are defined in utils.js and exported via window.
// They are intentionally NOT redefined here to avoid duplication.

// Apply an incremental diff from the server directly to the DOM.
// This updates only the changed cells, avoiding a full innerHTML replacement.
//
// The diff data has the format:
//   { generation, cursor, dimensions, changed_count, cells: [...] }
// Each cell: { row, col, cell: { ch, fg: [r,g,b], bg: [r,g,b], bold, italic, ... } }

/// Split a merged (RLE) span at the target cell position so that cell
/// gets its own individual <span> element.  Updates the cell grid entries
/// for all affected positions (before, target, and after the split).
///
/// Before: <span class="c w1" style="width:5ch;color:...">ABCDE</span>
///                         ^--- target at idx=2 (cell 'C')
/// After:  <span class="c w1" style="width:2ch;color:...">AB</span><span class="c w1" style="width:1ch;color:new">C'</span><span class="c w1" style="width:2ch;color:...">DE</span>
function _splitAndUpdateCell(cg, row, col, diff) {
    const entry = cg.grid[row][col];
    if (!entry || entry.len <= 1) return;

    const span = entry.span;
    const idx = entry.idx;
    const text = span.textContent;
    const origStyle = span.getAttribute('style') || '';
    const origClass = span.getAttribute('class') || 'c';
    // Determine cell width from class: w0→0, w1→1, w2→2
    const cellCh = origClass.includes('w2') ? 2 : origClass.includes('w0') ? 0 : 1;
    // Use Array.from for code point-aware splitting
    const chars = Array.from(text);

    // Characters before the target
    const before = chars.slice(0, idx).join('');
    const beforeLen = before.length; // code point count
    // Characters after the target
    const after = chars.slice(idx + 1).join('');
    const afterLen = after.length;

    // Helper: rebuild style with correct width for a given character count
    function _rebuildStyle(orig, charCount) {
        // Remove leading width:Nch or width:0 from origStyle
        const stripped = orig.replace(/^width:[^;]*;?/, '');
        const w = charCount * cellCh;
        return 'width:' + (w > 0 ? w + 'ch' : '0') + ';' + stripped;
    }

    // Create "after" span if there are trailing characters
    if (after.length > 0) {
        const afterSpan = document.createElement('span');
        afterSpan.className = origClass;
        afterSpan.setAttribute('style', _rebuildStyle(origStyle, afterLen));
        afterSpan.textContent = after;
        span.parentNode.insertBefore(afterSpan, span.nextSibling);
        // Update grid entries for characters after the target
        for (let k = col + 1; k < cg.grid[row].length; k++) {
            const e = cg.grid[row][k];
            if (e && e.span === span && e.idx > idx) {
                e.span = afterSpan;
                e.idx = e.idx - idx - 1;
                e.len = afterSpan.textContent.length;
            }
        }
    }

    // Create "before" span if there are leading characters
    if (before.length > 0) {
        const beforeSpan = document.createElement('span');
        beforeSpan.className = origClass;
        beforeSpan.setAttribute('style', _rebuildStyle(origStyle, beforeLen));
        beforeSpan.textContent = before;
        span.parentNode.insertBefore(beforeSpan, span);
        // Update grid entries for characters before the target
        for (let k = col - 1; k >= 0; k--) {
            const e = cg.grid[row][k];
            if (e && e.span === span && e.idx < idx) {
                e.span = beforeSpan;
                e.len = beforeSpan.textContent.length;
            }
        }
    }

    // Update the target cell in place
    const ch = diff.cell.width === 0 ? '\u200b' : (diff.cell.ch === '\u0000' ? ' ' : diff.cell.ch);
    span.textContent = _htmlEscapeChar(ch);
    span.setAttribute('style', _cellStyle(diff));
    const wCls = diff.cell.width === 0 ? 'c w0' : diff.cell.width === 2 ? 'c w2' : 'c w1';
    span.className = wCls;

    // Update grid entry for the target cell
    entry.len = 1;
    entry.idx = 0;
}

/// Update cursor, dimensions, mouse state, alt screen badge, and scrollback indicator
/// from an HTTP response, without touching the DOM content.
function updateVttyMetadataFromHttp(data, panelEl, panelObj, sbOffset) {
    const vttyEl = panelEl.querySelector('.vtty-container');
    const cursor = data.cursor || {};
    const dims = data.dimensions || {};
    document.getElementById('cursorPos').textContent = `Cursor: ${(cursor.row + 1) || '-'},${(cursor.col + 1) || '-'}`;
    document.getElementById('termDims').textContent = `${dims.rows || '-'}x${dims.cols || '-'}`;

    // Update alt screen badge
    const badge = document.getElementById('altScreenBadge-' + panelObj.id);
    if (badge) {
        badge.classList.toggle('visible', !!data.alternate_screen);
    }

    // Update mouse state
    if (panelObj) {
        panelObj.mouseTracking = !!data.mouse_tracking;
        panelObj.mouseSgr = !!data.mouse_sgr;
    }

    // Toggle selectable class on vtty container (enable text selection when mouse tracking is off)
    if (vttyEl) {
        const mt = panelObj ? panelObj.mouseTracking : false;
        vttyEl.classList.toggle('selectable', !mt);
        // Store dimensions on <pre> for screenshot filename generation
        const pre = vttyEl.querySelector('pre');
        if (pre && dims.rows && dims.cols) {
            pre._vttyRows = dims.rows;
            pre._vttyCols = dims.cols;
        }
    }

    // Hide cursor when in scrollback view or app hid it via ?25l
    const cursorVisible = data.cursor_visible !== false;
    const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
    if (cursorEl) {
        if (sbOffset > 0 || !cursorVisible) {
            cursorEl.classList.add('hidden');
        } else {
            cursorEl.classList.remove('hidden');
        }
    }

    // Show/hide scrollback indicator in bottom bar
    const sbIndicator = document.getElementById('scrollbackIndicator');
    if (sbIndicator) {
        sbIndicator.classList.toggle('hidden', sbOffset <= 0);
        sbIndicator.textContent = sbOffset > 0 ? 'SCROLLBACK -' + sbOffset + ' rows' : 'SCROLLBACK';
    }
}

    window.updateVttyDisplayForPanel = updateVttyDisplayForPanel;
    window.updateVttyMetadataForPanel = updateVttyMetadataForPanel;
    window.applyVttyDiffForPanel = applyVttyDiffForPanel;
    window.scheduleVttyHttpForPanel = scheduleVttyHttpForPanel;
    window.loadVttyHttpForPanel = loadVttyHttpForPanel;
    window.buildCellGrid = buildCellGrid;
    window.updateVttyMetadataFromHttp = updateVttyMetadataFromHttp;


function _applyScrollHtml(vttyEl, pre, html) {
    const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
    const oldScrollHeight = vttyEl.scrollHeight;
    pre.innerHTML = html;
    if (wasAtBottom) vttyEl.scrollTop = vttyEl.scrollHeight;
    else vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
}

function scheduleSecondaryVttyHttp(panelObj, delayMs) {
    if (!panelObj || !panelObj.split) return;
    const leaf = panelObj.split.branch;
    if (!leaf.cmdId || !leaf.instUrl) return;
    const timerKey = '_secondaryVttyHttpTimer_' + panelObj.id;
    if (state[timerKey]) clearTimeout(state[timerKey]);
    state[timerKey] = setTimeout(() => {
        state[timerKey] = null;
        _loadLeafVttyHttp(panelObj);
    }, delayMs);
}

async function _loadLeafVttyHttp(panelObj) {
    if (!panelObj || !panelObj.split) return;
    const leaf = panelObj.split.branch;
    const vttyEl = document.getElementById('vtty-' + leaf.id);
    if (!vttyEl) return;
    try {
        const json = await api.getVttyHtml(leaf.instUrl, leaf.cmdId);
        if (json.status === 'ok' && json.data) updateSecondaryVttyDisplay(panelObj, vttyEl, json.data);
    } catch (e) { /* ignore */ }
}

function updateSecondaryVttyDisplay(panelObj, vttyEl, data) {
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;
    const leaf = panelObj.split.branch;
    const cmdId = leaf.cmdId;
    const genKey = '_secondaryGen_' + cmdId;
    if (cmdId && data.generation !== undefined) {
        if (state[genKey] === data.generation) { _updateLeafVttyMetadata(leaf, vttyEl, data); return; }
        state[genKey] = data.generation;
    }
    if (data.html !== undefined && data.html !== null) _applyScrollHtml(vttyEl, pre, data.html);
    _updateLeafVttyMetadata(leaf, vttyEl, data);
}

function _updateLeafVttyMetadata(leaf, vttyEl, data) {
    const cursor = data.cursor || {};
    const dims = data.dimensions || {};
    const inScrollback = leaf.scrollbackOffset > 0;
    const cursorHidden = data.cursor_visible === false;
    const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
    if (cursorEl && cursor.row !== undefined && !inScrollback && !cursorHidden) {
        const charW = 10 * 0.6;  // panelObj.fontSize may not apply here, use 10 as default
        const charH = 10 * 1.2;
        cursorEl.style.top = (cursor.row * charH) + 'px';
        cursorEl.style.left = (cursor.col * charW) + 'px';
        cursorEl.style.width = charW + 'px';
        cursorEl.style.height = charH + 'px';
        cursorEl.classList.remove('hidden');
    } else if (cursorEl) {
        cursorEl.classList.add('hidden');
    }
    leaf.mouseTracking = !!data.mouse_tracking;
    leaf.mouseSgr = !!data.mouse_sgr;
    if (vttyEl) {
        vttyEl.classList.toggle('selectable', !leaf.mouseTracking);
        const pre = vttyEl.querySelector('pre');
        if (pre && dims.rows && dims.cols) { pre._vttyRows = dims.rows; pre._vttyCols = dims.cols; }
    }
}

function applySecondaryVttyDiff(panelObj, vttyEl, data) {
    // Incremental diff for ALL leaves.
    const leaf = panelObj.split.branch;
    const cmdId = leaf.cmdId;
    if (!cmdId) return;
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;
    const genKey = leaf.id + '/' + cmdId;
    // Generation check — use the same global _lastGeneration cache
    if (data.generation !== undefined && state._lastGeneration[genKey] === data.generation) {
        if (data.cursor || data.dimensions || data.mouse_tracking !== undefined)
            _updateLeafVttyMetadata(leaf, vttyEl, data);
        return;
    }
    if (data.generation !== undefined) state._lastGeneration[genKey] = data.generation;
    // Full HTML diff
    if (data.html !== undefined) {
        _applyScrollHtml(vttyEl, pre, data.html);
        _updateLeafVttyMetadata(leaf, vttyEl, data);
        return;
    }
    // Cell-level diff support for leaves
    if (state._level3Enabled && data.dimensions) {
        const cgKey = leaf.id + '/' + cmdId;
        buildCellGrid(cgKey, pre, data.dimensions.rows, data.dimensions.cols);
    }
    // If we got here without html/cells, fall back to HTTP fetch
    loadVttyHttpForPanel(leaf.id, leaf.instUrl, leaf.cmdId);
}

    window.scheduleSecondaryVttyHttp = scheduleSecondaryVttyHttp;
    window.updateSecondaryVttyDisplay = updateSecondaryVttyDisplay;
    window.applySecondaryVttyDiff = applySecondaryVttyDiff;
    window._cellStyle = _cellStyle;
    window._splitAndUpdateCell = _splitAndUpdateCell;
})();
