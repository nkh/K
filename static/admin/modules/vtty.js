// ─── VTTY Display ───
(function() {
    'use strict';
function updateVttyDisplay(data) {
    // Pause DOM updates while the user is actively scrolling
    if (state._userScrolling) {
        state._pendingVttyData = data;
        state._pendingVttyDirty = true;
        return;
    }
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    // Level 2: Skip redundant DOM updates if generation hasn't changed.
    const cmdId = state.selectedCmdId;
    if (cmdId && data.generation !== undefined) {
        if (state._lastGeneration[cmdId] === data.generation) {
            // Generation unchanged — only update metadata, skip DOM replacement
            updateVttyMetadata(data, panel, vttyEl);
            return;
        }
        state._lastGeneration[cmdId] = data.generation;
    }

    if (data.html !== undefined && data.html !== null) {
        // Level 1: Save scroll position before innerHTML replacement
        const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
        const oldScrollHeight = vttyEl.scrollHeight;

        pre.innerHTML = data.html;

        // Level 3: Rebuild cell grid after full HTML replacement
        if (state._level3Enabled && data.dimensions) {
            buildCellGrid(cmdId, pre, data.dimensions.rows, data.dimensions.cols);
        }

        // Level 1: Restore scroll position after DOM replacement.
        // If user was at bottom, snap to new bottom (auto-scroll).
        // Otherwise, adjust for content height change to maintain view position.
        if (wasAtBottom) {
            vttyEl.scrollTop = vttyEl.scrollHeight;
        } else {
            vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
        }
    }

    updateVttyMetadata(data, panel, vttyEl);
}

// ─── Per-Panel VTTY Display ───
// These functions route VTTY updates to a specific panel's DOM,
// rather than always targeting the focused panel.

function updateVttyDisplayForPanel(panelObj, panelEl, data) {
    const vttyEl = panelEl.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    const cmdId = panelObj.selectedCmdId;
    if (cmdId && data.generation !== undefined) {
        if (state._lastGeneration[cmdId] === data.generation) {
            updateVttyMetadataForPanel(panelObj, panelEl, vttyEl, data);
            return;
        }
        state._lastGeneration[cmdId] = data.generation;
    }

    if (data.html !== undefined && data.html !== null) {
        const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
        const oldScrollHeight = vttyEl.scrollHeight;
        pre.innerHTML = data.html;
        if (state._level3Enabled && data.dimensions) {
            buildCellGrid(cmdId, pre, data.dimensions.rows, data.dimensions.cols);
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
        cursorEl.style.display = '';
    } else if (cursorEl) {
        cursorEl.style.display = 'none';
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

    // Skip if generation unchanged (only update cursor/dimensions/mouse metadata)
    if (data.generation !== undefined && state._lastGeneration[cmdId] === data.generation) {
        if (data.cursor || data.dimensions || data.mouse_tracking !== undefined) {
            updateVttyMetadataForPanel(panelObj, panelEl, vttyEl, data);
        }
        return;
    }
    if (data.generation !== undefined) {
        state._lastGeneration[cmdId] = data.generation;
    }

    // If full HTML is embedded (e.g. from vtty_dirty fallback), use it directly
    if (data.html !== undefined) {
        const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
        const oldScrollHeight = vttyEl.scrollHeight;
        pre.innerHTML = data.html;
        if (state._level3Enabled && data.dimensions) {
            buildCellGrid(cmdId, pre, data.dimensions.rows, data.dimensions.cols);
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

    const cg = state._cellGrids[cmdId];
    if (!cg || !data.cells || !data.cells.length) {
        // No grid or no cells — fall back to full HTML fetch
        scheduleVttyHttpForPanel(panelObj.id, panelObj.selectedInstUrl, cmdId, 0);
        return;
    }

    // Check for dimension mismatch — if dimensions changed, need full resync
    const dims = data.dimensions || {};
    if (dims.rows !== cg.rows || dims.cols !== cg.cols) {
        delete state._cellGrids[cmdId];
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
                    // Fast path: single-char span — update directly
                    const ch = c.width === 0 ? '\u200b' : (c.ch === '\u0000' ? ' ' : c.ch);
                    entry.span.textContent = _htmlEscapeChar(ch);
                    entry.span.setAttribute('style', _cellStyle(c));
                    const wCls = c.width === 0 ? 'c w0' : c.width === 2 ? 'c w2' : 'c w1';
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
function scheduleVttyHttpForPanel(panelId, instUrl, cmdId, delayMs) {
    if (state._vttyHttpTimer) clearTimeout(state._vttyHttpTimer);
    state._vttyHttpTimer = setTimeout(() => {
        state._vttyHttpTimer = null;
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
        const res = await fetch(apiUrl(endpoint, { url: instUrl }), {
            headers: authHeadersForInstance({ url: instUrl }),
        });
        if (!res.ok) return;
        const json = await res.json();
        if (json.status === 'ok' && json.data) {
            updateVttyDisplayForPanel(panelObj, panelEl, json.data);
        }
    } catch (e) {
        // Silently ignore fetch errors (server might be unreachable)
    }
}

/// Update cursor, dimensions, mouse state, etc. without touching the DOM content.
/// Called both after innerHTML replacement and when generation is unchanged (skip path).
function updateVttyMetadata(data, panel, vttyEl) {
    // Cursor position
    const cursor = data.cursor || {};
    const dims = data.dimensions || {};
    document.getElementById('cursorPos').textContent = `Cursor: ${cursor.row + 1},${cursor.col + 1}`;
    document.getElementById('termDims').textContent = `${dims.rows}x${dims.cols}`;

    // Show cursor indicator (hide when in scrollback or app hid it via ?25l)
    const panelObj = state.panels.find(p => p.id === panel.id);
    const inScrollback = panelObj && panelObj.scrollbackOffset > 0;
    const cursorHidden = data.cursor_visible === false;
    const cursorEl = vttyEl ? vttyEl.querySelector('.cursor-indicator') : null;
    if (cursorEl && cursor.row !== undefined && !inScrollback && !cursorHidden) {
        const charW = state.fontSize * 0.6;
        const charH = state.fontSize * 1.2;
        cursorEl.style.top = (cursor.row * charH) + 'px';
        cursorEl.style.left = (cursor.col * charW) + 'px';
        cursorEl.style.width = charW + 'px';
        cursorEl.style.height = charH + 'px';
        cursorEl.style.display = '';
    } else if (cursorEl) {
        cursorEl.style.display = 'none';
    }

    // Track mouse state from the server response
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

    state._termRows = dims.rows;
    state._termCols = dims.cols;
}

// ─── Level 3: Cell Grid for Incremental DOM Patching ───
// Builds a 2D array of span element references from the <pre> DOM tree,
// indexed as grid[row][col]. Each row is terminated by a \n text node in
// the HTML produced by VttyRenderer::to_html().
//
// This grid enables O(1) lookup for any (row, col) cell, allowing
// applyVttyDiff() to patch individual cells without destroying the entire
// DOM tree (no innerHTML replacement).

function buildCellGrid(cmdId, pre, rows, cols) {
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

    state._cellGrids[cmdId] = { grid, rows, cols };
}

// Generate the inline style string for a cell, matching the server's
// VttyRenderer::to_html() format exactly. This ensures visual consistency
// between full HTML replacement and incremental diff patching.
function _cellStyle(diff) {
    let fg = diff.fg;
    let bg = diff.bg;

    // Handle reverse video: swap fg and bg
    if (diff.reverse) {
        [fg, bg] = [bg, fg];
    }

    // Width in ch units: matches server-side run_len * cell_ch.
    // For single-cell updates (diff patching), run_len is always 1.
    const cellW = diff.width || 1;
    let style = 'width:' + (cellW > 0 ? cellW + 'ch' : '0') + ';color:#' + _hex(fg[0]) + _hex(fg[1]) + _hex(fg[2]) + ';background:#' + _hex(bg[0]) + _hex(bg[1]) + _hex(bg[2]);

    if (diff.bold) style += ';font-weight:bold';
    if (diff.italic) style += ';font-style:italic';
    if (diff.underline && diff.strikethrough) {
        style += ';text-decoration:underline line-through';
    } else if (diff.underline) {
        style += ';text-decoration:underline';
    } else if (diff.strikethrough) {
        style += ';text-decoration:line-through';
    }
    if (diff.blink) style += ';animation:blink 1s step-end infinite';

    return style;
}

// NOTE: _hex() and _htmlEscapeChar() are defined in utils.js and exported via window.
// They are intentionally NOT redefined here to avoid duplication.

// Apply an incremental diff from the server directly to the DOM.
// This updates only the changed cells, avoiding a full innerHTML replacement.
//
// The diff data has the format:
//   { generation, cursor, dimensions, changed_count, cells: [...] }
// Each cell: { row, col, ch, fg: [r,g,b], bg: [r,g,b], bold, italic, ... }

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
    const ch = diff.width === 0 ? '\u200b' : (diff.ch === '\u0000' ? ' ' : diff.ch);
    span.textContent = _htmlEscapeChar(ch);
    span.setAttribute('style', _cellStyle(diff));
    const wCls = diff.width === 0 ? 'c w0' : diff.width === 2 ? 'c w2' : 'c w1';
    span.className = wCls;

    // Update grid entry for the target cell
    entry.len = 1;
    entry.idx = 0;
}
function applyVttyDiff(data) {
    // Pause DOM updates while the user is actively scrolling
    if (state._userScrolling) {
        state._pendingVttyData = data;
        state._pendingVttyDirty = true;
        return;
    }
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    const cmdId = state.selectedCmdId;
    if (!cmdId) return;

    // Level 2: Skip if generation unchanged
    if (data.generation !== undefined && state._lastGeneration[cmdId] === data.generation) {
        updateVttyMetadata(data, panel, vttyEl);
        return;
    }
    if (data.generation !== undefined) {
        state._lastGeneration[cmdId] = data.generation;
    }

    // Check if we have a cell grid for this command
    const cg = state._cellGrids[cmdId];
    if (!cg || !data.cells || !data.cells.length) {
        // No grid or no cells — fall back to full HTML fetch
        scheduleVttyHttp(state.selectedInstUrl, cmdId, 0);
        return;
    }

    // Check for dimension mismatch — if dimensions changed, we need a full resync
    const dims = data.dimensions || {};
    if (dims.rows !== cg.rows || dims.cols !== cg.cols) {
        // Dimensions changed — fall back to full HTML fetch
        delete state._cellGrids[cmdId];
        scheduleVttyHttp(state.selectedInstUrl, cmdId, 0);
        return;
    }

    // Save scroll position (Level 1)
    const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
    const oldScrollHeight = vttyEl.scrollHeight;

    // Apply each cell diff
    for (let i = 0; i < data.cells.length; i++) {
        const c = data.cells[i];
        if (c.row < cg.grid.length && c.col < cg.grid[c.row].length) {
            const entry = cg.grid[c.row][c.col];
            if (entry) {
                // Cell grid entries are { span, idx, len } objects from RLE expansion.
                // If the span contains only this cell (len===1), update directly.
                // Otherwise, split the merged span so this cell gets its own element.
                if (entry.len === 1) {
                    // Fast path: single-char span — update directly
                    // width=0 → wide-char continuation (zero-width space).
                    // width=1 with space → normal empty cell (actual space).
                    const ch = c.width === 0 ? '\u200b' : (c.ch === '\u0000' ? ' ' : c.ch);
                    entry.span.textContent = _htmlEscapeChar(ch);
                    entry.span.setAttribute('style', _cellStyle(c));
                    // Update width class to match new cell width
                    const wCls = c.width === 0 ? 'c w0' : c.width === 2 ? 'c w2' : 'c w1';
                    entry.span.className = wCls;
                } else {
                    // Slow path: split the merged span at the target position.
                    _splitAndUpdateCell(cg, c.row, c.col, c);
                }
            }
        }
    }

    // Level 1: Restore scroll position
    if (wasAtBottom) {
        vttyEl.scrollTop = vttyEl.scrollHeight;
    } else {
        vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
    }

    // Update metadata (cursor, dimensions, etc.)
    updateVttyMetadata(data, panel, vttyEl);
}

// ─── Debounced VTTY HTTP Fetch ───
// Prevents request flooding when multiple code paths (dirty signals, onclose,
// periodic refresh, sendKeys) all want to refresh the VTTY display.
// Only the last call within the debounce window actually fires.
function scheduleVttyHttp(instUrl, cmdId, delayMs) {
    // Legacy wrapper: delegate to per-panel
    const panelId = getActivePanelId();
    if (panelId) scheduleVttyHttpForPanel(panelId, instUrl, cmdId, delayMs);
}

/// Pre-fetch VTTY HTML for instant initial display.
/// Unlike loadVttyHttp, this does NOT check generation (first load, no cache)
/// and does NOT defer to pending state.  It writes directly into the <pre>.
async function _prefetchVttyHtml(instUrl, cmdId) {
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    const pre = vttyEl ? vttyEl.querySelector('pre') : null;
    if (!pre) return;

    try {
        const res = await fetch(apiUrl(`/api/commands/${cmdId}/vtty/html`, { url: instUrl }),
            { headers: authHeadersForInstance({ url: instUrl }) });
        if (!res.ok) return;
        const json = await res.json();
        if (json.status === 'ok' && json.data && json.data.html !== undefined) {
            pre.innerHTML = json.data.html;
            // Store generation for subsequent incremental updates
            if (json.data.generation !== undefined) {
                state._lastGeneration[cmdId] = json.data.generation;
            }
            // Build cell grid for Level 3 incremental diffing
            if (state._level3Enabled && json.data.dimensions) {
                buildCellGrid(cmdId, pre, json.data.dimensions.rows, json.data.dimensions.cols);
            }
            // Update metadata (cursor, dimensions, etc.)
            updateVttyMetadataFromHttp(json.data, panel,
                state.panels.find(p => p.id === panel.id), 0);
            // Start the push/poll update mode now that initial content is displayed
            const panelObj = state.panels.find(p => p.id === panel.id);
            if (panelObj) startPanelUpdateMode(panelObj.id);
        }
    } catch (e) {
        console.error('Failed to pre-fetch VTTY HTML:', e);
    }
}

async function loadVttyHttp(instUrl, cmdId) {
    const panel = getSelectedPanel();
    if (!panel) return;

    // Get panel state for scrollback offset
    const panelObj = state.panels.find(p => p.id === panel.id);
    const sbOffset = panelObj ? panelObj.scrollbackOffset : 0;

    // If viewing a specific buffer, use the buffer endpoint
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
        const res = await fetch(apiUrl(endpoint, { url: instUrl }), { headers: authHeadersForInstance({ url: instUrl }) });
        if (!res.ok) {
            console.warn('VTTY HTTP fetch failed:', res.status, res.statusText);
            return;
        }
        const json = await res.json();
        if (json.status === 'ok' && json.data) {
            // Level 2: Skip redundant DOM updates if generation hasn't changed.
            if (json.data.generation !== undefined && state._lastGeneration[cmdId] === json.data.generation) {
                // Only update metadata (cursor position, dimensions, etc.)
                updateVttyMetadataFromHttp(json.data, panel, panelObj, sbOffset);
                return;
            }
            if (json.data.generation !== undefined) {
                state._lastGeneration[cmdId] = json.data.generation;
            }

            const vttyEl = panel.querySelector('.vtty-container');
            const pre = vttyEl ? vttyEl.querySelector('pre') : null;
            if (pre && json.data.html !== undefined) {
                // Pause DOM updates while the user is actively scrolling
                if (state._userScrolling) {
                    state._pendingVttyData = json.data;
                    state._pendingVttyDirty = true;
                    return;
                }
                // Level 1: Save scroll position before innerHTML replacement
                const wasAtBottom = vttyEl.scrollHeight - vttyEl.scrollTop - vttyEl.clientHeight < 50;
                const oldScrollHeight = vttyEl.scrollHeight;

                pre.innerHTML = json.data.html;

                // Level 3: Rebuild cell grid after full HTML replacement
                if (state._level3Enabled && json.data.dimensions) {
                    buildCellGrid(cmdId, pre, json.data.dimensions.rows, json.data.dimensions.cols);
                } else {
                    // Clear stale grid if dimensions not available
                    delete state._cellGrids[cmdId];
                }

                // Level 1: Restore scroll position.
                // Only auto-scroll when user was viewing the bottom.
                if (wasAtBottom) {
                    vttyEl.scrollTop = vttyEl.scrollHeight;
                } else {
                    vttyEl.scrollTop += vttyEl.scrollHeight - oldScrollHeight;
                }
            }

            updateVttyMetadataFromHttp(json.data, panel, panelObj, sbOffset);
        }
    } catch (e) {
        console.error('Failed to load VTTY:', e);
    }
}

/// Update cursor, dimensions, mouse state, alt screen badge, and scrollback indicator
/// from an HTTP response, without touching the DOM content. Shared by both the
/// generation-skip path and the full-update path in loadVttyHttp.
function updateVttyMetadataFromHttp(data, panel, panelObj, sbOffset) {
    const vttyEl = panel.querySelector('.vtty-container');
    const cursor = data.cursor || {};
    const dims = data.dimensions || {};
    document.getElementById('cursorPos').textContent = `Cursor: ${(cursor.row + 1) || '-'},${(cursor.col + 1) || '-'}`;
    document.getElementById('termDims').textContent = `${dims.rows || '-'}x${dims.cols || '-'}`;

    // Update alt screen badge
    const badge = document.getElementById('altScreenBadge-' + panel.id);
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
            cursorEl.style.display = 'none';
        } else {
            cursorEl.style.display = '';
        }
    }

    // Show/hide scrollback indicator in bottom bar
    const sbIndicator = document.getElementById('scrollbackIndicator');
    if (sbIndicator) {
        sbIndicator.style.display = sbOffset > 0 ? '' : 'none';
        sbIndicator.textContent = sbOffset > 0 ? 'SCROLLBACK -' + sbOffset + ' rows' : 'SCROLLBACK';
    }
}

function switchBuffer(view) {
    state.bufferView = view;
    if (!state.selectedCmdId) return;

    // Reset scrollback when switching buffer views
    state.panels.forEach(p => p.scrollbackOffset = 0);
    // Clear stored scrollback since we reset
    sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);

    if (view === 'current') {
        // Re-enable the active update mode for live updates
        startUpdateMode();
    } else {
        // Disconnect WS / stop poll — we're viewing a static snapshot
        stopUpdateMode();
        loadVttyHttp(state.selectedInstUrl, state.selectedCmdId);
    }
}



    window.updateVttyDisplay = updateVttyDisplay;
    window.updateVttyDisplayForPanel = updateVttyDisplayForPanel;
    window.updateVttyMetadataForPanel = updateVttyMetadataForPanel;
    window.applyVttyDiffForPanel = applyVttyDiffForPanel;
    window.scheduleVttyHttpForPanel = scheduleVttyHttpForPanel;
    window.loadVttyHttpForPanel = loadVttyHttpForPanel;
    window.buildCellGrid = buildCellGrid;
    window.applyVttyDiff = applyVttyDiff;
    window.scheduleVttyHttp = scheduleVttyHttp;
    window._prefetchVttyHtml = _prefetchVttyHtml;
    window.loadVttyHttp = loadVttyHttp;
    window.updateVttyMetadata = updateVttyMetadata;
    window.updateVttyMetadataFromHttp = updateVttyMetadataFromHttp;
    window.switchBuffer = switchBuffer;
})();
