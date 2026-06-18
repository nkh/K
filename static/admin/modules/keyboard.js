// ─── Keyboard & Mouse Handling ───
// Global listeners for terminal keyboard/click/wheel/mouse interaction.
(function() {
    'use strict';

const _INPUT_TAGS = ['INPUT', 'TEXTAREA', 'SELECT'];
function _inInput(e) { return _INPUT_TAGS.includes(e.target.tagName); }

// ─── Key escape-sequence maps (shared + exported) ───
const _KEY_MAP = {
    'Enter': '\r', 'Backspace': '\x7f', 'Tab': '\t', 'Escape': '\x1b',
    'Home': '\x1b[H', 'End': '\x1b[F', 'Delete': '\x1b[3~',
    'ArrowUp': '\x1b[A', 'ArrowDown': '\x1b[B', 'ArrowRight': '\x1b[C', 'ArrowLeft': '\x1b[D',
    'PageUp': '\x1b[5~', 'PageDown': '\x1b[6~', 'Insert': '\x1b[2~',
    'F1': '\x1bOP', 'F2': '\x1bOQ', 'F3': '\x1bOR', 'F4': '\x1bOS',
    'F5': '\x1b[15~', 'F6': '\x1b[17~', 'F7': '\x1b[18~', 'F8': '\x1b[19~',
    'F9': '\x1b[20~', 'F10': '\x1b[21~', 'F11': '\x1b[23~', 'F12': '\x1b[24~',
};
const _CTRL_MAP = { '[': '\x1b', '\\': '\x1c', ']': '\x1d', '^': '\x1e', '_': '\x1f' };

// ─── Helpers ───
function _handleEscape() {
    closeContextMenu();
    const cp = document.getElementById('cmdPicker');
    if (cp) { releaseCurrentFocusTrap(); cp.remove(); return true; }
    const panel = getSelectedPanel();
    if (panel) vttySearchClose(panel.id);
    closeContextMenu();
    closeShortcuts();
    return true;
}

function _openSearch(panelId) {
    const sb = document.getElementById('searchBar-' + panelId);
    if (!sb) return;
    sb.classList.add('visible');
    const vc = document.getElementById(panelId)?.querySelector('.vtty-container');
    if (vc) trapFocus(vc);
    const si = document.getElementById('searchInput-' + panelId);
    if (si) { si.focus(); si.select(); }
}

function _withPanel(fn) {
    return (e) => { const p = getSelectedPanel(); if (p) { e.preventDefault(); fn(p.id); } };
}

function _getPanelObj(e) {
    const vc = e.target.closest('.vtty-container');
    if (!vc || state.currentView !== 'vtty') return null;
    const pe = vc.closest('.panel');
    if (!pe) return null;
    const panelObj = state.panels.find(p => p.id === pe.id) || null;
    if (panelObj?.split) {
        const side = vc.getAttribute('data-split-side');
        if (side && panelObj.split.activeSide !== side) {
            panelObj.split.activeSide = side;
        }
    }
    return panelObj;
}

function _saveScrollback(offset) {
    const key = 'vrw_scrollback_' + state.selectedCmdId;
    offset > 0 ? sessionStorage.setItem(key, String(offset)) : sessionStorage.removeItem(key);
}

function _updateScrollbackUI(po, panelEl) {
    const btn = panelEl.querySelector('.scroll-bottom-btn');
    if (btn) btn.classList.toggle('visible', po.scrollbackOffset > 0);
    const ind = document.getElementById('scrollbackIndicator');
    if (ind) {
        ind.classList.toggle('hidden', po.scrollbackOffset <= 0);
        if (po.scrollbackOffset > 0) ind.textContent = 'SCROLLBACK -' + po.scrollbackOffset + ' rows';
    }
}

function _loadPanel(po) {
    loadVttyHttpForPanel(po.id, po.selectedInstUrl, state.selectedCmdId);
}

// ─── Keyboard shortcut bindings ───
// Default shortcuts — can be overridden by user-defined shortcuts (localStorage).
// Each entry: { key, ctrl?, shift?, alt?, meta?, noInput?, action, label?, id? }
// 'id' is a stable name used for user customization (e.g. 'split-vertical').

function _switchWindowByIndex(e, idx) {
    e.preventDefault();
    if (!state.windows || !state.windows.length) return;
    if (idx < state.windows.length) switchWindow(state.windows[idx].id);
}

const _defaultShortcuts = [
    { id: 'escape', key: 'Escape', action: _handleEscape, label: 'Dismiss popup / close search' },
    { id: 'context-menu', key: 'ContextMenu', shift: 'F10', action(e) {
        e.preventDefault();
        const t = document.activeElement;
        if (!t) return;
        if (t.classList.contains('panel-header') && t.dataset.panelId) {
            const r = t.getBoundingClientRect();
            showPanelContextMenu({ preventDefault(){}, clientX: r.left + r.width/2, clientY: r.bottom }, t.dataset.panelId);
        }
        if (t.classList.contains('cmd-item') && t.dataset.instUrl) {
            const r = t.getBoundingClientRect();
            showCmdContextMenu({ preventDefault(){}, clientX: r.left + r.width/2, clientY: r.bottom }, t.dataset.instUrl, t.dataset.cmdId, t.dataset.cmdName, t.dataset.cmdAlive === 'true');
        }
    }, label: 'Context menu' },
    { id: 'copy', key: 'c', ctrl: true, shift: true, action: _withPanel(copyTerminalSelection), label: 'Copy selection' },
    { id: 'selection-mode', key: 's', ctrl: true, shift: true, action: _withPanel(toggleSelectionMode), label: 'Toggle selection mode' },
    { id: 'selection-mode-alt', key: 's', alt: true, action: _withPanel(toggleSelectionMode), label: 'Toggle selection mode (Alt)' },
    { id: 'shortcuts-help', key: '?', noInput: true, action: showShortcuts, label: 'Show shortcuts' },
    { id: 'export', key: 'e', ctrl: true, shift: true, noInput: true, action: _withPanel(exportTerminal), label: 'Export terminal' },
    { id: 'restart', key: 'r', ctrl: true, shift: true, noInput: true, action: _withPanel(restartCommand), label: 'Restart command' },
    { id: 'panel-theme', key: 't', alt: true, noInput: true, action(e) { e.preventDefault(); const id = getActivePanelId(); if (id) togglePanelTheme(id); }, label: 'Toggle panel theme' },
    { id: 'new-panel', key: 'n', alt: true, noInput: true, action(e) { e.preventDefault(); addPanel(); }, label: 'New panel' },
    { id: 'split-vertical', key: '|', alt: true, noInput: true, action(e) {
        e.preventDefault();
        const id = getActivePanelId();
        if (id) { const p = state.panels.find(x => x.id === id); if (p && !p.split) splitPanel(id, 'vertical'); }
    }, label: 'Split pane vertically' },
    { id: 'split-horizontal', key: '-', alt: true, noInput: true, action(e) {
        e.preventDefault();
        const id = getActivePanelId();
        if (id) { const p = state.panels.find(x => x.id === id); if (p && !p.split) splitPanel(id, 'horizontal'); }
    }, label: 'Split pane horizontally' },
    { id: 'unsplit', key: 'u', alt: true, noInput: true, action(e) {
        e.preventDefault();
        const id = getActivePanelId();
        if (id) { const p = state.panels.find(x => x.id === id); if (p && p.split) unsplitPanel(id); }
    }, label: 'Remove split' },
    { id: 'new-window', key: 'w', alt: true, noInput: true, action(e) { e.preventDefault(); createWindow(); }, label: 'New window' },
    { id: 'close-window', key: 'W', alt: true, noInput: true, action(e) {
        e.preventDefault();
        if (state.activeWindowId) closeWindow(state.activeWindowId);
    }, label: 'Close window' },
    { id: 'win-1', key: '1', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 0); }, label: 'Switch to window 1' },
    { id: 'win-2', key: '2', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 1); }, label: 'Switch to window 2' },
    { id: 'win-3', key: '3', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 2); }, label: 'Switch to window 3' },
    { id: 'win-4', key: '4', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 3); }, label: 'Switch to window 4' },
    { id: 'win-5', key: '5', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 4); }, label: 'Switch to window 5' },
    { id: 'win-6', key: '6', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 5); }, label: 'Switch to window 6' },
    { id: 'win-7', key: '7', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 6); }, label: 'Switch to window 7' },
    { id: 'win-8', key: '8', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 7); }, label: 'Switch to window 8' },
    { id: 'win-9', key: '9', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 8); }, label: 'Switch to window 9' },
    { id: 'nav-prev', key: 'ArrowLeft', alt: true, noInput: true, action(e) {
        const p = getSelectedPanel(), po = p && state.panels.find(x => x.id === p.id);
        if (!(po && po.focused)) { e.preventDefault(); navigatePrevCommand(); }
    }, label: 'Previous command' },
    { id: 'nav-next', key: 'ArrowRight', alt: true, noInput: true, action(e) {
        const p = getSelectedPanel(), po = p && state.panels.find(x => x.id === p.id);
        if (!(po && po.focused)) { e.preventDefault(); navigateNextCommand(); }
    }, label: 'Next command' },
];

// ─── User-definable shortcut system ───
// Custom shortcuts are stored in localStorage as vrw_custom_shortcuts.
// Format: { "split-vertical": { key: "|", alt: true }, "new-panel": { key: "p", alt: true, ctrl: true }, ... }
// Only the key/modifier fields are customizable; the action is always from the default.

function _loadCustomShortcuts() {
    try {
        const raw = localStorage.getItem('vrw_custom_shortcuts');
        return raw ? JSON.parse(raw) : {};
    } catch { return {}; }
}

function _saveCustomShortcut(id, binding) {
    const customs = _loadCustomShortcuts();
    if (!binding || (!binding.key && !binding.shift)) {
        delete customs[id];
    } else {
        customs[id] = { key: binding.key };
        if (binding.ctrl) customs[id].ctrl = true;
        if (binding.shift) customs[id].shift = true;
        if (binding.alt) customs[id].alt = true;
        if (binding.meta) customs[id].meta = true;
    }
    localStorage.setItem('vrw_custom_shortcuts', JSON.stringify(customs));
    _rebuildShortcuts();
}

function _rebuildShortcuts() {
    const customs = _loadCustomShortcuts();
    const active = new Set();  // track which default ids are overridden
    const merged = [];
    // Apply custom overrides
    for (const def of _defaultShortcuts) {
        if (!def.id) { merged.push(def); continue; }
        if (customs[def.id]) {
            const c = customs[def.id];
            merged.push({ ...def, key: c.key, ctrl: !!c.ctrl, shift: !!c.shift, alt: !!c.alt, meta: !!c.meta, _custom: true });
            active.add(def.id);
        } else {
            merged.push(def);
        }
    }
    _shortcuts = merged;
}

let _shortcuts = [];
_rebuildShortcuts();

// ─── Shortcut matching (extracted so it can be called from focused-terminal path) ───
function _tryShortcut(e) {
    for (const s of _shortcuts) {
        const keyMatch = e.key === s.key || (s.shift && typeof s.shift === 'string' && e.shiftKey && e.key === s.shift);
        if (!keyMatch) continue;
        const ctrlOk = !s.ctrl || e.ctrlKey || e.metaKey;
        const altOk = s.alt ? e.altKey : !e.altKey;
        const shiftOk = !s.shift || e.shiftKey;
        const metaOk = !s.meta || e.metaKey;
        if (ctrlOk && altOk && shiftOk && metaOk) {
            if (!(s.noInput && _inInput(e))) { s.action(e); return true; }
        }
    }
    return false;
}

// ─── Keyboard handling ───
document.addEventListener('keydown', (e) => {
    // Direct terminal keyboard input: capture keystrokes and send to PTY
    if (state.currentView === 'vtty') {
        const panel = getSelectedPanel();
        if (panel) {
            const panelObj = state.panels.find(p => p.id === panel.id);
            if (panelObj && panelObj.focused && state.selectedCmdId) {
                const searchBar = document.getElementById('searchBar-' + panel.id);
                if (searchBar && searchBar.classList.contains('visible') &&
                    document.activeElement && document.activeElement.id === 'searchInput-' + panel.id) {
                    // Let search input handle the key
                } else if (e.key === 'Escape') {
                    _handleEscape(); return;
                } else if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
                    e.preventDefault(); _openSearch(panel.id); return;
                } else if (_tryShortcut(e)) {
                    // A shortcut matched (e.g. Alt+|, Alt+-, Alt+n) — don't send to terminal
                    return;
                } else {
                    e.preventDefault(); sendDirectKey(e, panelObj); return;
                }
            }
        }
    }

    // Focus key input when not in an input field and a command is selected
    if (state.currentView === 'vtty' && state.selectedCmdId && !_inInput(e)) {
        const panel = getSelectedPanel();
        if (panel) { const input = document.getElementById('keyInput-' + panel.id); if (input) input.focus(); }
    }

    // Ctrl+F — open terminal search bar (general, for non-focused paths)
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        const vttyContainer = e.target.closest && e.target.closest('.vtty-container');
        if (vttyContainer || state.currentView === 'vtty') {
            e.preventDefault();
            const panel = getSelectedPanel();
            if (panel) _openSearch(panel.id);
            return;
        }
    }

    // Shortcut table dispatch (for non-focused paths)
    _tryShortcut(e);
});

// ─── Direct key sending (when terminal is focused) ───
async function sendDirectKey(e, panelObj) {
    // For split panes, determine which side's command to send to
    let cmdId = panelObj.selectedCmdId;
    let instUrl = panelObj.selectedInstUrl;
    if (panelObj.split && panelObj.split.activeSide === 'secondary') {
        cmdId = panelObj.split.secondaryCmdId;
        instUrl = panelObj.split.secondaryInstUrl;
    }
    if (!cmdId || !instUrl) return;

    let seq = '';
    if (e.ctrlKey && !e.altKey && !e.metaKey) {
        if (e.key.length === 1 && e.key >= 'a' && e.key <= 'z')
            seq = String.fromCharCode(e.key.charCodeAt(0) - 96);
        else if (_CTRL_MAP[e.key]) seq = _CTRL_MAP[e.key];
    } else if (e.altKey && !e.ctrlKey && !e.metaKey) {
        if (e.key.length === 1) seq = '\x1b' + e.key;
    } else if (_KEY_MAP[e.key]) {
        seq = _KEY_MAP[e.key];
    } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
        seq = e.key;
    }
    if (!seq) return;

    try {
        const json = await api.sendKeys(instUrl, cmdId, { keys: seq });
        if (json.status === 'ok')
            scheduleVttyHttpForPanel(panelObj.id, instUrl, cmdId, 50);
    } catch (err) { console.error('Direct key send error:', err); }
}

// ─── Click-to-focus terminal ───
document.addEventListener('click', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (vttyContainer && state.currentView === 'vtty') {
        const panelEl = vttyContainer.closest('.panel');
        if (!panelEl) return;
        const panelObj = state.panels.find(p => p.id === panelEl.id);
        if (!panelObj) return;
        if (e.target.closest('button') || e.target.closest('input')) return;

        if (panelObj.focused) {
            panelObj.focused = false;
            vttyContainer.style.outline = '';
        } else {
            state.panels.forEach(p => p.focused = false);
            document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
            panelObj.focused = true;
            vttyContainer.style.outline = '2px solid var(--accent)';
            vttyContainer.setAttribute('tabindex', '0');
            vttyContainer.focus();
        }
    } else if (!vttyContainer) {
        state.panels.forEach(p => p.focused = false);
        document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
    }
});

// ─── Mouse wheel / scrollback handling ───
let _wheelScrollRafId = null, _wheelScrollPanel = null, _wheelScrollAccum = 0;

document.addEventListener('wheel', (e) => {
    const panelObj = _getPanelObj(e);
    if (!panelObj || !state.selectedCmdId) return;
    const panelEl = e.target.closest('.vtty-container')?.closest('.panel');
    if (!panelEl) return;
    if (panelObj.selectionMode) return;

    const vc = e.target.closest('.vtty-container');

    if (panelObj.mouseTracking) {
        e.preventDefault();
        sendMouseEvent(panelObj, e.deltaY < 0 ? 'wheel_up' : 'wheel_down', 0, e);
        return;
    }

    // Live buffer view — allow native scroll, only intercept at top edge
    if (panelObj.scrollbackOffset === 0) {
        if (e.deltaY < 0 && vc.scrollTop <= 0) {
            e.preventDefault();
            panelObj.scrollbackOffset += 3;
            _saveScrollback(panelObj.scrollbackOffset);
            _loadPanel(panelObj);
            _updateScrollbackUI(panelObj, panelEl);
        }
        return;
    }

    // Scrollback history view — coalesce rapid wheel ticks via rAF
    e.preventDefault();
    _wheelScrollPanel = panelObj;
    _wheelScrollAccum += e.deltaY;
    if (_wheelScrollRafId) cancelAnimationFrame(_wheelScrollRafId);
    _wheelScrollRafId = requestAnimationFrame(() => {
        _wheelScrollRafId = null;
        const p = _wheelScrollPanel;
        if (!p) return;
        const accum = _wheelScrollAccum;
        _wheelScrollAccum = 0;
        const lines = Math.max(1, Math.round(Math.abs(accum) / 100) * 3);

        if (accum > 0) {
            const newOffset = Math.max(0, p.scrollbackOffset - lines);
            p.scrollbackOffset = newOffset;
            if (newOffset === 0) {
                const vtty = panelEl.querySelector('.vtty-container');
                if (vtty) vtty.scrollTop = vtty.scrollHeight;
            }
        } else {
            p.scrollbackOffset += lines;
        }
        _saveScrollback(p.scrollbackOffset);
        _loadPanel(p);
        _updateScrollbackUI(p, panelEl);
    });
}, { passive: false });

// ─── Mouse event forwarding to PTY ───
let _mouseDownButton = null;

document.addEventListener('mousedown', (e) => {
    const po = _getPanelObj(e);
    if (!po || !state.selectedCmdId) return;
    if (e.target.closest('button') || e.target.closest('input')) return;
    if (po.selectionMode) return;
    if (po.mouseTracking) {
        e.preventDefault();
        _mouseDownButton = e.button;
        sendMouseEvent(po, 'down', e.button, e);
    }
});

document.addEventListener('mouseup', (e) => {
    const po = _getPanelObj(e);
    if (!po || !state.selectedCmdId) { _mouseDownButton = null; return; }

    if (po.selectionMode) {
        _mouseDownButton = null;
        // Copy-on-select
        setTimeout(() => {
            const text = window.getSelection()?.toString().trim();
            if (text) {
                const pe = e.target.closest('.vtty-container')?.closest('.panel');
                if (pe) copyTerminalSelection(pe.id);
            }
        }, 0);
        return;
    }

    if (po.mouseTracking && _mouseDownButton !== null) {
        e.preventDefault();
        sendMouseEvent(po, 'up', _mouseDownButton, e);
        _mouseDownButton = null;
    }
});

document.addEventListener('mousemove', (e) => {
    if (_mouseDownButton === null) return;
    const po = _getPanelObj(e);
    if (!po || !state.selectedCmdId || !po.mouseTracking || po.selectionMode) return;
    if (!po._lastMoveTime || Date.now() - po._lastMoveTime > 16) {
        po._lastMoveTime = Date.now();
        sendMouseEvent(po, 'move', _mouseDownButton, e);
    }
});

async function sendMouseEvent(panelObj, eventType, button, e) {
    // For split panes, determine which side's command to send to
    let cmdId = panelObj.selectedCmdId;
    let instUrl = panelObj.selectedInstUrl;
    if (panelObj.split && panelObj.split.activeSide === 'secondary') {
        cmdId = panelObj.split.secondaryCmdId;
        instUrl = panelObj.split.secondaryInstUrl;
    }
    if (!cmdId || !instUrl) return;
    const vttyEl = document.getElementById(panelObj.id)?.querySelector('.vtty-container');
    if (!vttyEl) return;

    const rect = vttyEl.getBoundingClientRect();
    const charW = state.fontSize * 0.6, charH = state.fontSize * 1.2;
    const x = Math.max(1, Math.floor((e.clientX - rect.left) / charW) + 1);
    const y = Math.max(1, Math.floor((e.clientY - rect.top) / charH) + 1);

    try {
        await api.sendMouse(instUrl, cmdId, { event: eventType, button, x, y });
        scheduleVttyHttpForPanel(panelObj.id, instUrl, cmdId, 30);
    } catch (err) { /* best-effort */ }
}

Object.assign(window, { _KEY_MAP, _defaultShortcuts, _loadCustomShortcuts, _saveCustomShortcut, _rebuildShortcuts });
})();
