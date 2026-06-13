// ─── Panels: Drag & Drop ───
(function() {
    'use strict';

// ─── Drag-and-Drop Panel Reorder ───
let _draggedPanelId = null;

function onPanelDragStart(e, panelId) {
    _draggedPanelId = panelId;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', panelId);
    setTimeout(() => { const el = document.getElementById(panelId); if (el) el.classList.add('dragging'); }, 0);
}

function onPanelDragOver(e) {
    e.preventDefault();
    e.dataTransfer.dropEffect = _draggedPanelId ? 'move' : 'copy';
    const panel = e.target.closest('.panel');
    if (!panel || panel.id === _draggedPanelId) return;
    const rect = panel.getBoundingClientRect(), midX = rect.left + rect.width / 2;
    panel.classList.remove('drag-over-left', 'drag-over-right');
    panel.classList.add(e.clientX < midX ? 'drag-over-left' : 'drag-over-right');
}

function onPanelDragLeave(e) {
    const panel = e.target.closest('.panel');
    if (panel) panel.classList.remove('drag-over-left', 'drag-over-right');
}

function onPanelDrop(e, targetPanelId) {
    e.preventDefault(); if (e.stopPropagation) e.stopPropagation();
    if (!_draggedPanelId) {
        try {
            const cmdData = JSON.parse(e.dataTransfer.getData('application/x-cmd'));
            if (cmdData?.cmdId) { document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left', 'drag-over-right')); _openCommandInNewPane(cmdData.instUrl, cmdData.cmdId, cmdData.cmdName); return; }
        } catch {}
        onPanelDragEnd(e); return;
    }
    if (_draggedPanelId === targetPanelId) { onPanelDragEnd(e); return; }
    const container = document.getElementById('view-vtty');
    const draggedEl = document.getElementById(_draggedPanelId), targetEl = document.getElementById(targetPanelId);
    if (!draggedEl || !targetEl || !container) { onPanelDragEnd(e); return; }
    const rect = targetEl.getBoundingClientRect(), midX = rect.left + rect.width / 2;
    container.insertBefore(draggedEl, e.clientX < midX ? targetEl : targetEl.nextSibling);
    const handle = draggedEl.nextElementSibling;
    if (handle?.classList.contains('panel-resize-handle')) {
        container.removeChild(handle);
        container.insertBefore(handle, draggedEl.nextElementSibling);
    }
    const newOrder = [];
    container.querySelectorAll('.panel').forEach(el => { const p = state.panels.find(pp => pp.id === el.id); if (p) newOrder.push(p); });
    state.panels = newOrder;
    localStorage.setItem('vrw_panel_order', JSON.stringify(newOrder.map(p => p.id)));
    onPanelDragEnd(e);
}

function onPanelAreaDragOver(e) { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; }

function onPanelAreaDrop(e) {
    e.preventDefault();
    try { const d = JSON.parse(e.dataTransfer.getData('application/x-cmd')); if (d?.cmdId) _openCommandInNewPane(d.instUrl, d.cmdId, d.cmdName); } catch {}
}

function onPanelDragEnd() {
    _draggedPanelId = null;
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('dragging', 'drag-over-left', 'drag-over-right'));
}

// ─── Drag-and-Drop (Sidebar Commands) ───
let _draggedCmd = null;

function onCmdDragStart(e, instUrl, cmdId, cmdName) {
    _draggedCmd = { instUrl, cmdId, cmdName };
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
    if (!container) return;
    if (!s.placeholder) {
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
    if (s.overPane !== wasOverPane) {
        document.querySelectorAll('.panel').forEach(p => p.classList.toggle('drag-over-left', s.overPane));
        if (!s.overPane) container.querySelectorAll('.cmd-item').forEach(el => el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom'));
    }
    if (s.overPane) return;
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
    const { srcEl, placeholder, instUrl, cmdId, cmdName, overPane } = _reorderState;
    if (srcEl) {
        ['position','left','top','width','zIndex','opacity','pointerEvents'].forEach(p => srcEl.style[p] = '');
        srcEl.classList.remove('cmd-dragging');
    }
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('drag-over-left'));
    const container = document.getElementById('commandList');
    if (container) container.querySelectorAll('.cmd-item').forEach(el => el.classList.remove('cmd-drag-over-top', 'cmd-drag-over-bottom'));
    if (overPane && placeholder) { placeholder.remove(); _openCommandInNewPane(instUrl, cmdId, cmdName); _reorderState = null; return; }
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
        onPanelDragStart, onPanelDragOver, onPanelDragLeave, onPanelDrop, onPanelDragEnd,
        onPanelAreaDragOver, onPanelAreaDrop,
        onCmdDragStart, getCmdOrder, setCmdOrder, getOrderedCmds,
    });
})();