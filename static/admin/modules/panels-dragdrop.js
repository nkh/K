// ─── Panels: Drag & Drop ───
(function() {
    'use strict';

// ─── Drag-and-Drop Panel Reorder (mousedown on header) ───
let _panelDrag = null;

function _panelDragMouseDown(e) {
    // Only left click, not on buttons/inputs
    if (e.button !== 0) return;
    if (e.target.closest('button') || e.target.closest('input') || e.target.closest('select')) return;
    const header = e.target.closest('.panel-header');
    if (!header) return;
    const panelEl = header.closest('.panel');
    if (!panelEl) return;
    e.preventDefault();
    const rect = panelEl.getBoundingClientRect();
    _panelDrag = {
        panelId: panelEl.id,
        startX: e.clientX,
        startY: e.clientY,
        lastX: e.clientX,
        lastY: e.clientY,
        rect: rect,
        offsetX: e.clientX - rect.left,
        offsetY: e.clientY - rect.top,
        started: false,
        placeholder: null,
    };
    document.addEventListener('mousemove', _panelDragMouseMove);
    document.addEventListener('mouseup', _panelDragMouseUp);
}

function _panelDragMouseMove(e) {
    if (!_panelDrag) return;
    const d = _panelDrag;
    d.lastX = e.clientX;
    d.lastY = e.clientY;
    // Require 4px movement before starting drag
    if (!d.started) {
        if (Math.abs(e.clientX - d.startX) < 4 && Math.abs(e.clientY - d.startY) < 4) return;
        d.started = true;
        const el = document.getElementById(d.panelId);
        if (!el) { _panelDragMouseUp(); return; }
        // Create placeholder
        const ph = document.createElement('div');
        ph.className = 'panel';
        ph.style.cssText = 'border:2px dashed var(--accent);opacity:0.3;min-height:100px;';
        d.placeholder = ph;
        el.parentNode.insertBefore(ph, el);
        Object.assign(el.style, {
            position: 'fixed', left: d.rect.left + 'px', top: d.rect.top + 'px',
            width: d.rect.width + 'px', height: d.rect.height + 'px',
            zIndex: '1000', opacity: '0.85', pointerEvents: 'none',
        });
        el.classList.add('dragging');
    }
    const el = document.getElementById(d.panelId);
    if (el) {
        el.style.left = (e.clientX - d.offsetX) + 'px';
        el.style.top = (e.clientY - d.offsetY) + 'px';
    }
    // Highlight drop target
    document.querySelectorAll('.panel').forEach(p => {
        if (p.id === d.panelId || p.classList.contains('dragging')) return;
        p.classList.remove('drag-over-left', 'drag-over-right');
    });
    const under = document.elementFromPoint(e.clientX, e.clientY);
    const targetPanel = under?.closest('.panel');
    if (targetPanel && targetPanel.id !== d.panelId && !targetPanel.classList.contains('dragging')) {
        const rect = targetPanel.getBoundingClientRect();
        const midX = rect.left + rect.width / 2;
        targetPanel.classList.add(e.clientX < midX ? 'drag-over-left' : 'drag-over-right');
    }
}

function _panelDragMouseUp() {
    document.removeEventListener('mousemove', _panelDragMouseMove);
    document.removeEventListener('mouseup', _panelDragMouseUp);
    if (!_panelDrag) return;
    const d = _panelDrag;
    const el = document.getElementById(d.panelId);
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left', 'drag-over-right', 'dragging'));
    if (el && d.started) {
        // Restore element style
        ['position','left','top','width','height','zIndex','opacity','pointerEvents'].forEach(p => el.style[p] = '');
        // Find drop target using last known mouse position
        const under = document.elementFromPoint(d.lastX, d.lastY);
        const targetPanel = under?.closest('.panel');
        const container = document.getElementById('view-vtty');
        if (targetPanel && targetPanel.id !== d.panelId && container) {
            const rect = targetPanel.getBoundingClientRect();
            const midX = rect.left + rect.width / 2;
            const insertBefore = d.lastX < midX;
            // Remove placeholder first
            if (d.placeholder && d.placeholder.parentNode) d.placeholder.remove();
            // Insert element relative to the target panel
            if (insertBefore) {
                container.insertBefore(el, targetPanel);
            } else {
                // Insert after target panel (and its resize handle if any)
                const nextEl = targetPanel.nextElementSibling;
                if (nextEl?.classList.contains('panel-resize-handle')) {
                    container.insertBefore(el, nextEl.nextElementSibling);
                } else {
                    container.insertBefore(el, targetPanel.nextElementSibling);
                }
            }
            // Update state order
            const newOrder = [];
            container.querySelectorAll('.panel').forEach(p => { const pp = state.panels.find(x => x.id === p.id); if (pp) newOrder.push(pp); });
            state.panels = newOrder;
            localStorage.setItem('vrw_panel_order', JSON.stringify(newOrder.map(p => p.id)));
        } else {
            if (d.placeholder && d.placeholder.parentNode) d.placeholder.remove();
        }
    } else if (d.placeholder) {
        d.placeholder.remove();
    }
    _panelDrag = null;
}

// Setup header drag delegation (done once)
let _panelDragDelegated = false;
function setupPanelHeaderDrag() {
    if (_panelDragDelegated) return;
    _panelDragDelegated = true;
    const container = document.getElementById('view-vtty');
    if (container) container.addEventListener('mousedown', _panelDragMouseDown);
}

// ─── Command drag from sidebar (keep HTML5 for sidebar→panel) ───
function onPanelDragOver(e) { e.preventDefault(); }

function onPanelDrop(e, targetPanelId) {
    e.preventDefault(); if (e.stopPropagation) e.stopPropagation();
    try {
        const cmdData = JSON.parse(e.dataTransfer.getData('application/x-cmd'));
        if (cmdData?.cmdId) {
            const panelObj = state.panels.find(p => p.id === targetPanelId);
            if (!panelObj) return;
            // Check if the drop landed on a specific leaf in a split panel
            const leafEl = e.target && e.target.closest ? e.target.closest('[data-leaf-id]') : null;
            if (panelObj.split && leafEl) {
                const leafId = leafEl.getAttribute('data-leaf-id');
                if (leafId && leafId !== panelObj.id) {
                    // Dropped on a specific branch leaf — assign to it
                    const found = (typeof _findLeafState === 'function') ? _findLeafState(panelObj, leafId) : null;
                    if (found && found.leaf) {
                        _selectLeafCommand(panelObj, found.leaf, cmdData.instUrl, cmdData.cmdId);
                    }
                    return;
                }
            }
            // Dropped on a pane (root or non-split) — assign command to it
            _pushPanelHistory(panelObj);
            _selectCommandForPanel(panelObj, cmdData.instUrl, cmdData.cmdId);
            return;
        }
    } catch {}
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left', 'drag-over-right'));
}

function onPanelDragLeave(e) {
    const panel = e.target.closest('.panel');
    if (panel) panel.classList.remove('drag-over-left', 'drag-over-right');
}

function onPanelDragEnd() {
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('dragging', 'drag-over-left', 'drag-over-right'));
}

function onPanelAreaDragOver(e) { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; }

function onPanelAreaDrop(e) {
    e.preventDefault();
    // This fires when dropping on the panel-area container itself (not on a specific pane).
    // Assign to the currently focused pane instead of creating a new one.
    try {
        const d = JSON.parse(e.dataTransfer.getData('application/x-cmd'));
        if (d?.cmdId) {
            const panelObj = state.panels.find(p => p.id === state._focusedPanelId) || state.panels[0];
            if (panelObj) { _pushPanelHistory(panelObj); _selectCommandForPanel(panelObj, d.instUrl, d.cmdId); }
        }
    } catch {}
}

// ─── Drag-and-Drop (Sidebar Commands) ───
function onCmdDragStart(e, instUrl, cmdId, cmdName) {
    e.dataTransfer.effectAllowed = 'copy';
    e.dataTransfer.setData('text/plain', cmdId);
    e.dataTransfer.setData('application/x-cmd', JSON.stringify({ instUrl, cmdId, cmdName }));
    if (e.target?.style) { e.target.style.opacity = '0.5'; setTimeout(() => { if (e.target?.style) e.target.style.opacity = ''; }, 0); }
}

// ─── Sidebar Command Reorder (mousedown-based) ───
function getCmdOrder() { try { return JSON.parse(localStorage.getItem('vrw_cmd_order') || '{}'); } catch { return {}; } }
function setCmdOrder(order) { localStorage.setItem('vrw_cmd_order', JSON.stringify(order)); }
function getOrderedCmds(instUrl, items) {
    const instOrder = getCmdOrder()[instUrl];
    if (!instOrder) return items;
    const ordered = [], remaining = [];
    for (const item of items) {
        const idx = instOrder.indexOf(item.cmd.id);
        (idx >= 0 ? ordered : remaining).push(idx >= 0 ? { item, idx } : item);
    }
    ordered.sort((a, b) => a.idx - b.idx);
    return [...ordered.map(x => x.item), ...remaining];
}

let _reorderState = null;

function _cmdReorderMouseDown(e, instUrl, cmdId, cmdName) {
    if (e.button !== 0) return;
    e.preventDefault(); e.stopPropagation();
    const srcEl = e.target.closest('.cmd-item');
    if (!srcEl) return;
    const rect = srcEl.getBoundingClientRect();
    _reorderState = { instUrl, cmdId, cmdName: cmdName || cmdId, srcEl, startY: e.clientY, startRect: rect, placeholder: null, offsetY: e.clientY - rect.top, overPane: false };
    document.addEventListener('mousemove', _cmdReorderMouseMove);
    document.addEventListener('mouseup', _cmdReorderMouseUp);
}

function _cmdReorderMouseMove(e) {
    if (!_reorderState) return;
    const s = _reorderState;
    if (Math.abs(e.clientY - s.startY) < 4 && !s.placeholder) return;
    const container = document.getElementById('commandList');
    if (!s.placeholder) {
        if (!container) return;
        const ph = document.createElement('div');
        ph.className = 'cmd-reorder-placeholder';
        ph.style.cssText = 'border-top:2px solid var(--accent);margin:0;pointer-events:none;';
        s.placeholder = ph;
        s.srcEl.parentNode.insertBefore(ph, s.srcEl);
        Object.assign(s.srcEl.style, { position: 'fixed', left: s.startRect.left + 'px', width: s.startRect.width + 'px', zIndex: '1000', opacity: '0.85', pointerEvents: 'none' });
        s.srcEl.classList.add('cmd-dragging');
    }
    s.srcEl.style.top = (e.clientY - s.offsetY) + 'px';
    s.srcEl.classList.add('hidden');
    const underEl = document.elementFromPoint(e.clientX, e.clientY);
    s.srcEl.classList.remove('hidden');
    const overPanel = underEl?.closest('.panel');
    const overArea = underEl?.closest('#view-vtty') && !underEl.closest('#sidebar');
    const wasOverPane = s.overPane;
    s.overPane = !!(overPanel || overArea);
    // Track the specific target panel and leaf for proper drop handling
    s._targetPanelId = overPanel?.id || null;
    s._targetLeafId = underEl?.closest('[data-leaf-id]')?.getAttribute('data-leaf-id') || null;
    if (s.overPane !== wasOverPane) {
        document.querySelectorAll('.panel').forEach(p => p.classList.toggle('drag-over-left', s.overPane));
        if (!s.overPane && container) container.querySelectorAll('.cmd-item').forEach(el => el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom'));
    }
    if (s.overPane) return;
    if (!container) return;
    container.querySelectorAll('.cmd-item').forEach(el => el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom'));
    const target = underEl?.closest('.cmd-item');
    if (!target || target === s.srcEl) return;
    const midY = target.getBoundingClientRect().top + target.getBoundingClientRect().height / 2;
    target.classList.add(e.clientY < midY ? 'cmd-drag-over-top' : 'cmd-drag-over-bottom');
    target.parentNode.insertBefore(s.placeholder, e.clientY < midY ? target : target.nextElementSibling);
}

function _cmdReorderMouseUp() {
    document.removeEventListener('mousemove', _cmdReorderMouseMove);
    document.removeEventListener('mouseup', _cmdReorderMouseUp);
    if (!_reorderState) return;
    const { srcEl, placeholder, instUrl, cmdId, cmdName, overPane, _targetPanelId, _targetLeafId } = _reorderState;
    if (srcEl) {
        ['position','left','top','width','zIndex','opacity','pointerEvents'].forEach(p => srcEl.style[p] = '');
        srcEl.classList.remove('cmd-dragging');
    }
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left'));
    const container = document.getElementById('commandList');
    if (container) container.querySelectorAll('.cmd-item').forEach(el => el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom'));
    if (overPane) {
        if (placeholder) placeholder.remove();
        // Find the target panel and handle drop properly (assign to panel/split side)
        const targetPanelObj = _targetPanelId ? state.panels.find(p => p.id === _targetPanelId) : null;
        if (targetPanelObj) {
            if (targetPanelObj.split) {
                // Use the leaf under the cursor if detected, otherwise fall back to focused leaf
                const dropLeafId = _targetLeafId || (typeof _getFocusedLeafId === 'function' ? _getFocusedLeafId(targetPanelObj) : targetPanelObj.id);
                if (dropLeafId && dropLeafId !== targetPanelObj.id) {
                    const found = (typeof _findLeafState === 'function') ? _findLeafState(targetPanelObj, dropLeafId) : null;
                    if (found && found.leaf) {
                        _selectLeafCommand(targetPanelObj, found.leaf, instUrl, cmdId);
                    }
                } else {
                    _pushPanelHistory(targetPanelObj);
                    _selectCommandForPanel(targetPanelObj, instUrl, cmdId);
                }
            } else {
                // Always assign to the target pane — never split or open a new pane
                _pushPanelHistory(targetPanelObj);
                _selectCommandForPanel(targetPanelObj, instUrl, cmdId);
            }
        } else {
            // FIX: Assign to focused panel instead of creating a new pane (Bug 3)
            const focusedPanel = state.panels.find(pp => pp.id === state._focusedPanelId) || state.panels[0];
            if (focusedPanel) { _pushPanelHistory(focusedPanel); _selectCommandForPanel(focusedPanel, instUrl, cmdId); }
        }
        _reorderState = null;
        return;
    }
    if (placeholder && container) {
        const nextEl = placeholder.nextElementSibling;
        const targetCmdId = nextEl?.classList.contains('cmd-item') ? nextEl.dataset.cmdId : null;
        placeholder.remove();
        if (targetCmdId && targetCmdId !== cmdId) {
            const order = getCmdOrder();
            let instOrder = order[instUrl] || [];
            instOrder = instOrder.filter(id => id !== cmdId);
            instOrder.splice((instOrder.indexOf(targetCmdId) + 1 || instOrder.length + 1) - 1, 0, cmdId);
            order[instUrl] = instOrder;
            setCmdOrder(order); loadCommands();
        }
    }
    _reorderState = null;
}

    // ── Exports ──
    Object.assign(window, {
        onPanelDragOver, onPanelDrop, onPanelDragLeave, onPanelDragEnd,
        onPanelAreaDragOver, onPanelAreaDrop,
        onCmdDragStart, getCmdOrder, setCmdOrder, getOrderedCmds,
        setupPanelHeaderDrag,
    });
})();