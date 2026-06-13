// ─── Drag-and-Drop ───
// Sidebar command drag-to-panel, sidebar command reorder (mousedown-based),
// and open-command-in-new-pane helper.
(function() {
    'use strict';

// ─── Drag-and-Drop: Sidebar Commands to Panels ───
let _draggedCmd = null; // { instUrl, cmdId, cmdName }

function onCmdDragStart(e, instUrl, cmdId, cmdName) {
    _draggedCmd = { instUrl, cmdId, cmdName };
    e.dataTransfer.effectAllowed = 'copy';
    e.dataTransfer.setData('text/plain', cmdId);
    e.dataTransfer.setData('application/x-cmd', JSON.stringify({ instUrl, cmdId, cmdName }));
    if (e.target && e.target.style) e.target.style.opacity = '0.5';
    setTimeout(() => { if (e.target && e.target.style) e.target.style.opacity = ''; }, 0);
}

// Make panels accept command drops from sidebar (drop target is the panel header only)
function initPanelDropTargets() {
    document.querySelectorAll('.panel-header').forEach(headerEl => {
        const panelEl = headerEl.closest('.panel');
        if (!panelEl) return;
        headerEl.addEventListener('dragover', (e) => {
            e.preventDefault();
            e.dataTransfer.dropEffect = 'copy';
            panelEl.classList.add('drag-over-left');
        });
        headerEl.addEventListener('dragleave', (e) => {
            panelEl.classList.remove('drag-over-left');
        });
        headerEl.addEventListener('drop', (e) => {
            e.preventDefault();
            e.stopPropagation();
            panelEl.classList.remove('drag-over-left');
            try {
                const data = JSON.parse(e.dataTransfer.getData('application/x-cmd'));
                if (data && data.cmdId) {
                    // Always create a new panel for the dropped command
                    _openCommandInNewPane(data.instUrl, data.cmdId, data.cmdName);
                }
            } catch (err) { /* ignore invalid drops */ }
            _draggedCmd = null;
        });
    });
}


// ─── Drag-and-Drop: Sidebar Command Reorder (mousedown-based) ───
// Commands can be reordered within the sidebar by dragging the grab handle.
// Uses mousedown/mousemove/mouseup instead of nested HTML5 DnD because nested
// draggable elements (cmd-item draggable for panel-drop + grab-handle draggable
// for reorder) is a well-known anti-pattern that fails silently in most browsers.
// The custom order is persisted in localStorage as 'vrw_cmd_order'.
// { instUrl: [cmdId1, cmdId2, ...] }
function getCmdOrder() {
    try { return JSON.parse(localStorage.getItem('vrw_cmd_order') || '{}'); } catch { return {}; }
}
function setCmdOrder(order) {
    localStorage.setItem('vrw_cmd_order', JSON.stringify(order));
}
function getOrderedCmds(instUrl, items) {
    const order = getCmdOrder();
    const instOrder = order[instUrl];
    if (!instOrder) return items;
    // items are { inst, cmd, cmdName } objects; order by cmd.id
    const ordered = [];
    const remaining = [];
    for (const item of items) {
        const idx = instOrder.indexOf(item.cmd.id);
        if (idx >= 0) {
            ordered.push({ item, idx });
        } else {
            remaining.push(item);
        }
    }
    ordered.sort((a, b) => a.idx - b.idx);
    return [...ordered.map(x => x.item), ...remaining];
}

// mousedown-based reorder state
let _reorderState = null; // { instUrl, cmdId, cmdName, srcEl, startY, startRect, placeholder, offsetY, overPane }

function _cmdReorderMouseDown(e, instUrl, cmdId, cmdName) {
    // Only left-click
    if (e.button !== 0) return;
    e.preventDefault(); // prevent text selection
    e.stopPropagation(); // don't trigger cmd-item onclick

    const srcEl = e.target.closest('.cmd-item');
    if (!srcEl) return;

    const rect = srcEl.getBoundingClientRect();
    _reorderState = {
        instUrl,
        cmdId,
        cmdName: cmdName || cmdId,
        srcEl,
        startY: e.clientY,
        startRect: rect,
        placeholder: null,
        offsetY: e.clientY - rect.top,
        overPane: false,
    };

    document.addEventListener('mousemove', _cmdReorderMouseMove);
    document.addEventListener('mouseup', _cmdReorderMouseUp);
}

function _cmdReorderMouseMove(e) {
    if (!_reorderState) return;

    const dy = e.clientY - _reorderState.startY;
    // Minimum 4px before starting visual drag
    if (Math.abs(dy) < 4 && !_reorderState.placeholder) return;

    const container = document.getElementById('commandList');
    if (!container) return;

    // First move: create placeholder and make source float
    if (!_reorderState.placeholder) {
        const srcEl = _reorderState.srcEl;
        _reorderState.placeholder = document.createElement('div');
        _reorderState.placeholder.style.cssText = 'border-top:2px solid var(--accent);margin:0;pointer-events:none;';
        _reorderState.placeholder.className = 'cmd-reorder-placeholder';
        srcEl.parentNode.insertBefore(_reorderState.placeholder, srcEl);
        srcEl.style.position = 'fixed';
        srcEl.style.left = _reorderState.startRect.left + 'px';
        srcEl.style.top = (e.clientY - _reorderState.offsetY) + 'px';
        srcEl.style.width = _reorderState.startRect.width + 'px';
        srcEl.style.zIndex = '1000';
        srcEl.style.opacity = '0.85';
        srcEl.style.pointerEvents = 'none';
        srcEl.classList.add('cmd-dragging');
    }

    // Move the floating element
    _reorderState.srcEl.style.top = (e.clientY - _reorderState.offsetY) + 'px';

    // Find the element we're hovering over (use elementFromPoint to see what's
    // under the floating ghost).
    _reorderState.srcEl.classList.add('hidden');
    const underEl = document.elementFromPoint(e.clientX, e.clientY);
    _reorderState.srcEl.classList.remove('hidden');

    // Check if hovering over the pane area (for drop-to-open feature)
    const overPanel = underEl ? underEl.closest('.panel') : null;
    const overPanelArea = underEl ? underEl.closest('#view-vtty') : null;
    const wasOverPane = _reorderState.overPane;
    _reorderState.overPane = !!(overPanel || (overPanelArea && !underEl.closest('#sidebar')));

    // Toggle pane drop indicator
    if (_reorderState.overPane && !wasOverPane) {
        // Entered pane area — show drop indicator
        document.querySelectorAll('.panel').forEach(p => p.classList.add('drag-over-left'));
        // Clear sidebar indicators
        container.querySelectorAll('.cmd-item').forEach(el => {
            el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom');
        });
    } else if (!_reorderState.overPane && wasOverPane) {
        // Left pane area — remove drop indicator
        document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left'));
    }

    // If over a panel, don't try to reorder in sidebar
    if (_reorderState.overPane) return;

    // Clear old sidebar indicators
    container.querySelectorAll('.cmd-item').forEach(el => {
        el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom');
    });

    const target = underEl ? underEl.closest('.cmd-item') : null;
    if (!target || target === _reorderState.srcEl) return;

    // Move placeholder to indicate drop position
    const rect = target.getBoundingClientRect();
    const midY = rect.top + rect.height / 2;
    if (e.clientY < midY) {
        target.classList.add('cmd-drag-over-top');
        target.parentNode.insertBefore(_reorderState.placeholder, target);
    } else {
        target.classList.add('cmd-drag-over-bottom');
        const next = target.nextElementSibling;
        target.parentNode.insertBefore(_reorderState.placeholder, next);
    }
}

function _cmdReorderMouseUp(e) {
    document.removeEventListener('mousemove', _cmdReorderMouseMove);
    document.removeEventListener('mouseup', _cmdReorderMouseUp);

    if (!_reorderState) return;

    const container = document.getElementById('commandList');
    const placeholder = _reorderState.placeholder;
    const srcEl = _reorderState.srcEl;
    const droppedOnPane = _reorderState.overPane;

    // Clean up visual state on the source element
    if (srcEl) {
        srcEl.style.position = '';
        srcEl.style.left = '';
        srcEl.style.top = '';
        srcEl.style.width = '';
        srcEl.style.zIndex = '';
        srcEl.style.opacity = '';
        srcEl.style.pointerEvents = '';
        srcEl.classList.remove('cmd-dragging');
    }
    // Clean up sidebar indicators
    if (container) {
        container.querySelectorAll('.cmd-item').forEach(el => {
            el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom');
        });
    }
    // Clean up pane drop indicators
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left'));

    // ── Drop on pane area: create new panel with this command ──
    if (droppedOnPane && placeholder) {
        placeholder.remove();
        _openCommandInNewPane(_reorderState.instUrl, _reorderState.cmdId, _reorderState.cmdName);
        _reorderState = null;
        return;
    }

    // ── Drop on sidebar: perform reorder ──
    if (placeholder && container) {
        const targetItem = placeholder.nextElementSibling;
        const targetCmdId = targetItem && targetItem.classList.contains('cmd-item')
            ? targetItem.dataset.cmdId
            : null;

        // Remove placeholder before doing DOM operations
        placeholder.remove();

        // Only reorder if we moved to a different position
        if (targetCmdId && targetCmdId !== _reorderState.cmdId) {
            const order = getCmdOrder();
            let instOrder = order[_reorderState.instUrl] || [];
            // Remove source from current position
            instOrder = instOrder.filter(id => id !== _reorderState.cmdId);
            // Find target position
            const targetIdx = instOrder.indexOf(targetCmdId);
            instOrder.splice(targetIdx >= 0 ? targetIdx : instOrder.length, 0, _reorderState.cmdId);
            order[_reorderState.instUrl] = instOrder;
            setCmdOrder(order);
            loadCommands();
        } else if (placeholder.parentNode) {
            // Moved but dropped back to same spot — just remove placeholder
            placeholder.remove();
        }
    }

    _reorderState = null;
}

// ─── Open command in a new pane (used by grab-handle drop-to-pane) ───
function _openCommandInNewPane(instUrl, cmdId, cmdName) {
    // Create a new empty panel
    const newPanel = addPanelDirect();
    if (!newPanel) return;
    // Focus it and assign the command
    focusPanel(newPanel.id);
    newPanel.selectedInstUrl = instUrl;
    newPanel.selectedCmdId = cmdId;
    // Sync global state
    state.selectedInstUrl = instUrl;
    state.selectedCmdId = cmdId;
    state._pendingVttyData = null;
    state._pendingVttyDirty = false;
    state.bufferView = 'current';
    _restoreCachedDom(cmdId);
    updatePanelCommandInfo();
    updateTerminalDisconnectedOverlay();
    updateSidebarSelection();
    // Fetch VTTY content and start push/poll
    loadVttyHttpForPanel(newPanel.id, instUrl, cmdId);
    startPanelUpdateMode(newPanel.id);
}

    // Expose to global scope
    window.initPanelDropTargets = initPanelDropTargets;
    window.onCmdDragStart = onCmdDragStart;
    window.getCmdOrder = getCmdOrder;
    window.setCmdOrder = setCmdOrder;
    window.getOrderedCmds = getOrderedCmds;
    window._openCommandInNewPane = _openCommandInNewPane;
})();
