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
    window.closeShortcuts();
    return true;
}

// Find which leaf ID is focused within a panel.
// FIX: Use the explicit _focusedLeafId (set by click/keyboard) instead of
// walking the tree via activeSide. Tree walking via activeSide was unreliable
// because activeSide wasn't always updated correctly after unsplit operations.
function _getFocusedLeafId(panel) {
    if (!panel) return null;
    // Use the explicit _focusedLeafId, NOT tree walking via activeSide.
    // This fixes the bug where tree walking returns the wrong pane.
    if (!panel._focusedLeafId) return panel.id;
    // Validate that the focused leaf still exists in the tree.
    // MUST check both panel.split AND panel._rootSplit — they are parallel
    // split trees. Missing either means _focusedLeafId is silently dropped.
    if (panel._focusedLeafId !== panel.id && (panel.split || panel._rootSplit)) {
        const found = _findLeafState(panel, panel._focusedLeafId);
        if (found && found.leaf) return panel._focusedLeafId;
    }
    return panel.id;
}

// Given a vtty-container element, find the leaf state it belongs to
function _getLeafFromVtty(vc, panelObj) {
    if (!panelObj) return null;
    // Check if this vtty belongs to a leaf in the split tree.
    // MUST check both split trees — _rootSplit leaves are valid targets.
    const leafId = vc.getAttribute('data-leaf-id');
    if (!leafId) return { leaf: panelObj, isPanelLeaf: true };
    if (leafId === panelObj.id) return { leaf: panelObj, isPanelLeaf: true };
    if (panelObj.split || panelObj._rootSplit) {
        const found = _findLeafState(panelObj, leafId);
        if (found) return { leaf: found.leaf, isPanelLeaf: false };
    }
    return { leaf: panelObj, isPanelLeaf: true };
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

// _withPanel resolves the target ID for panel-level operations.
// For split panels, returns the focused leaf ID instead of the panel ID,
// so that shortcuts like copy/export/restart target the correct leaf.
function _withPanel(fn) {
    return (e) => {
        const p = getSelectedPanel();
        if (!p) return;
        e.preventDefault();
        const panelObj = state.panels.find(pp => pp.id === p.id);
        const targetId = (panelObj && (panelObj.split || panelObj._rootSplit))
            ? (panelObj._focusedLeafId || p.id)
            : p.id;
        fn(targetId);
    };
}

function _getPanelObj(e) {
    const vc = e.target.closest('.vtty-container');
    if (!vc || state.currentView !== 'vtty') return null;
    const pe = vc.closest('.panel');
    if (!pe) return null;
    const panelObj = state.panels.find(p => p.id === pe.id) || null;
    if (!panelObj) return null;
    // Track which leaf is active in the split tree.
    // MUST check both split trees — _rootSplit leaves are valid targets too.
    const leafId = vc.getAttribute('data-leaf-id');
    if ((panelObj.split || panelObj._rootSplit) && leafId) {
        // Update activeSide for the appropriate split level
        _setActiveSideForLeaf(panelObj, leafId);
    }
    return panelObj;
}

// Set the activeSide at each level of the split tree to reflect which leaf is focused
function _setActiveSideForLeaf(panel, leafId) {
    // Track focused leaf per panel for visual highlighting
    panel._focusedLeafId = leafId;
    if (leafId === panel.id) {
        if (panel.split) panel.split.activeSide = 'panel';
        if (panel._rootSplit) panel._rootSplit.activeSide = 'panel';
        return;
    }
    // Check if the leaf is in the root's own split tree (_rootSplit)
    if (panel._rootSplit) {
        const foundInRoot = _setActiveSideInNode(panel._rootSplit, leafId);
        if (foundInRoot) {
            // Leaf found in root split tree — also set top-level split to 'panel' side
            if (panel.split) panel.split.activeSide = 'panel';
            return;
        }
    }
    if (!panel.split) return;
    _setActiveSideInNode(panel.split, leafId);
}

function _setActiveSideInNode(splitNode, leafId) {
    if (!splitNode || !splitNode.branch) return false;
    if (splitNode.branch.id === leafId) {
        splitNode.activeSide = 'branch';
        return true;
    }
    // The leaf might be deeper in the branch's tree
    if (splitNode.branch.split) {
        const found = _setActiveSideInNode(splitNode.branch.split, leafId);
        if (found) {
            // If found deeper, this level's activeSide should be 'branch'
            splitNode.activeSide = 'branch';
            return true;
        }
    }
    return false;
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

// ─── Prefix key system ───
// A prefix key (default: Ctrl+A) activates "prefix mode". The next keystroke
// is then checked against prefix-bound shortcuts. If no match within 1 second,
// prefix mode cancels automatically. This is modelled after screen(1)/tmux(1).
//
// Prefix shortcuts have { prefix: true } — they only match when prefix mode is active.
// The prefix key itself is a regular shortcut with { isPrefix: true }.

let _prefixActive = false;
let _prefixTimer = null;
const _PREFIX_TIMEOUT_MS = 1000;

function _activatePrefix() {
    _prefixActive = true;
    if (_prefixTimer) clearTimeout(_prefixTimer);
    _prefixTimer = setTimeout(_cancelPrefix, _PREFIX_TIMEOUT_MS);
    // Show visual indicator
    let indicator = document.getElementById('prefixIndicator');
    if (!indicator) {
        indicator = document.createElement('div');
        indicator.id = 'prefixIndicator';
        indicator.style.cssText = 'position:fixed;bottom:0.5rem;right:0.5rem;background:var(--accent);color:var(--color-on-accent);padding:0.15rem 0.5rem;border-radius:3px;font-size:var(--ui-fs);font-family:var(--font-mono);z-index:99999;pointer-events:none;opacity:0.9;';
        document.body.appendChild(indicator);
    }
    indicator.textContent = state._prefixLabel || 'PREFIX';
}

function _cancelPrefix() {
    _prefixActive = false;
    if (_prefixTimer) { clearTimeout(_prefixTimer); _prefixTimer = null; }
    const indicator = document.getElementById('prefixIndicator');
    if (indicator) indicator.remove();
}

// ─── Keyboard shortcut bindings ───
// Default shortcuts — can be overridden by user-defined shortcuts (localStorage).
// Each entry: { key, ctrl?, shift?, alt?, meta?, noInput?, action, label?, id? }
// 'id' is a stable name used for user customization (e.g. 'split-vertical').
// 'prefix: true' — only matches when prefix mode is active (after Ctrl+A).
// 'isPrefix: true' — this shortcut IS the prefix key (activates prefix mode).

function _switchWindowByIndex(e, idx) {
    e.preventDefault();
    if (!state.windows || !state.windows.length) return;
    if (idx < state.windows.length) switchWindow(state.windows[idx].id);
}

function _splitAction(direction) {
    return function(e) {
        e.preventDefault();
        const id = getActivePanelId();
        if (id) { const p = state.panels.find(x => x.id === id); if (p) {
            const leafId = _getFocusedLeafId(p);
            splitPanel(id, direction, leafId);
        }}
    };
}

const _defaultShortcuts = [
    { id: 'escape', key: 'Escape', action: _handleEscape, label: 'Dismiss popup / close search' },
    { id: 'context-menu', key: 'ContextMenu', shift: 'F10', action(e) {
        e.preventDefault();
        const t = document.activeElement;
        if (!t) return;
        if (t.classList.contains('panel-header') && t.dataset.panelId) {
            const r = t.getBoundingClientRect();
            const leafId = t.dataset.leafId || t.dataset.panelId;
            showPanelContextMenu({ preventDefault(){}, clientX: r.left + r.width/2, clientY: r.bottom }, t.dataset.panelId, leafId);
        }
        if (t.classList.contains('cmd-item') && t.dataset.instUrl) {
            const r = t.getBoundingClientRect();
            showCmdContextMenu({ preventDefault(){}, clientX: r.left + r.width/2, clientY: r.bottom }, t.dataset.instUrl, t.dataset.cmdId, t.dataset.cmdName, t.dataset.cmdAlive === 'true');
        }
    }, label: 'Context menu' },
    { id: 'copy', key: 'c', ctrl: true, shift: true, action: _withPanel(copyTerminalSelection), label: 'Copy selection' },
    { id: 'selection-mode', key: 's', ctrl: true, shift: true, action: _withPanel(toggleSelectionMode), label: 'Toggle selection mode' },
    { id: 'selection-mode-alt', key: 's', alt: true, action: _withPanel(toggleSelectionMode), label: 'Toggle selection mode (Alt)' },
    { id: 'shortcuts-help', key: '?', noInput: true, action: window.showShortcuts, label: 'Show shortcuts' },
    { id: 'export', key: 'e', ctrl: true, shift: true, noInput: true, action: _withPanel(exportTerminal), label: 'Export terminal' },
    { id: 'restart', key: 'r', ctrl: true, shift: true, noInput: true, action: _withPanel(restartCommand), label: 'Restart command' },

    // ── Prefix key ──
    { id: 'prefix', key: 'a', ctrl: true, isPrefix: true, action(e) { e.preventDefault(); _activatePrefix(); }, label: 'Prefix key (enter command mode)' },

    // ── Prefix-mode shortcuts (after Ctrl+A) ──
    { id: 'p-split-vertical', key: '|', prefix: true, noInput: true, action: _splitAction('horizontal'), label: 'Split pane vertically (side by side)' },
    { id: 'p-split-horizontal', key: '-', prefix: true, noInput: true, action: _splitAction('vertical'), label: 'Split pane horizontally (top/bottom)' },
    { id: 'p-unsplit', key: 'd', ctrl: true, prefix: true, noInput: true, action(e) {
        e.preventDefault();
        const id = getActivePanelId();
        if (id) { const p = state.panels.find(x => x.id === id); if (p && (p.split || p._rootSplit)) {
            const leafId = _getFocusedLeafId(p);
            unsplitPanel(id, leafId);
        }}
    }, label: 'Close pane (remove split)' },
    { id: 'p-new-panel', key: 'c', prefix: true, noInput: true, action(e) { e.preventDefault(); addPanel(); }, label: 'New panel' },
    { id: 'p-panel-theme', key: 't', prefix: true, noInput: true, action(e) { e.preventDefault(); const id = getActivePanelId(); if (id) togglePanelTheme(id); }, label: 'Toggle panel theme' },
    { id: 'p-new-window', key: 'w', prefix: true, noInput: true, action(e) { e.preventDefault(); createWindow(); }, label: 'New window' },
    { id: 'p-close-window', key: 'W', prefix: true, noInput: true, action(e) {
        e.preventDefault();
        if (state.activeWindowId) closeWindow(state.activeWindowId);
    }, label: 'Close window' },
    { id: 'p-win-1', key: '1', prefix: true, noInput: true, action(e) { _switchWindowByIndex(e, 0); }, label: 'Switch to window 1' },
    { id: 'p-win-2', key: '2', prefix: true, noInput: true, action(e) { _switchWindowByIndex(e, 1); }, label: 'Switch to window 2' },
    { id: 'p-win-3', key: '3', prefix: true, noInput: true, action(e) { _switchWindowByIndex(e, 2); }, label: 'Switch to window 3' },
    { id: 'p-win-4', key: '4', prefix: true, noInput: true, action(e) { _switchWindowByIndex(e, 3); }, label: 'Switch to window 4' },
    { id: 'p-win-5', key: '5', prefix: true, noInput: true, action(e) { _switchWindowByIndex(e, 4); }, label: 'Switch to window 5' },
    { id: 'p-win-6', key: '6', prefix: true, noInput: true, action(e) { _switchWindowByIndex(e, 5); }, label: 'Switch to window 6' },
    { id: 'p-win-7', key: '7', prefix: true, noInput: true, action(e) { _switchWindowByIndex(e, 6); }, label: 'Switch to window 7' },
    { id: 'p-win-8', key: '8', prefix: true, noInput: true, action(e) { _switchWindowByIndex(e, 7); }, label: 'Switch to window 8' },
    { id: 'p-win-9', key: '9', prefix: true, noInput: true, action(e) { _switchWindowByIndex(e, 8); }, label: 'Switch to window 9' },

    // ── Legacy Alt+ shortcuts (kept as alternatives) ──
    { id: 'panel-theme', key: 't', alt: true, noInput: true, action(e) { e.preventDefault(); const id = getActivePanelId(); if (id) togglePanelTheme(id); }, label: 'Toggle panel theme (Alt+T)' },
    { id: 'new-panel', key: 'n', alt: true, noInput: true, action(e) { e.preventDefault(); addPanel(); }, label: 'New panel (Alt+N)' },
    { id: 'split-vertical', key: '|', alt: true, noInput: true, action: _splitAction('horizontal'), label: 'Split vertically (Alt+|)' },
    { id: 'split-horizontal', key: '-', alt: true, noInput: true, action: _splitAction('vertical'), label: 'Split horizontally (Alt+-)' },
    { id: 'unsplit', key: 'd', ctrl: true, alt: true, noInput: true, action(e) {
        e.preventDefault();
        const id = getActivePanelId();
        if (id) { const p = state.panels.find(x => x.id === id); if (p && (p.split || p._rootSplit)) {
            const leafId = _getFocusedLeafId(p);
            unsplitPanel(id, leafId);
        }}
    }, label: 'Close pane (Ctrl+D / Alt+Ctrl+D)' },
    { id: 'new-window', key: 'w', alt: true, noInput: true, action(e) { e.preventDefault(); createWindow(); }, label: 'New window (Alt+W)' },
    { id: 'close-window', key: 'W', alt: true, noInput: true, action(e) {
        e.preventDefault();
        if (state.activeWindowId) closeWindow(state.activeWindowId);
    }, label: 'Close window (Alt+W)' },
    { id: 'win-1', key: '1', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 0); }, label: 'Window 1 (Alt+1)' },
    { id: 'win-2', key: '2', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 1); }, label: 'Window 2 (Alt+2)' },
    { id: 'win-3', key: '3', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 2); }, label: 'Window 3 (Alt+3)' },
    { id: 'win-4', key: '4', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 3); }, label: 'Window 4 (Alt+4)' },
    { id: 'win-5', key: '5', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 4); }, label: 'Window 5 (Alt+5)' },
    { id: 'win-6', key: '6', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 5); }, label: 'Window 6 (Alt+6)' },
    { id: 'win-7', key: '7', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 6); }, label: 'Window 7 (Alt+7)' },
    { id: 'win-8', key: '8', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 7); }, label: 'Window 8 (Alt+8)' },
    { id: 'win-9', key: '9', alt: true, noInput: true, action(e) { _switchWindowByIndex(e, 8); }, label: 'Window 9 (Alt+9)' },
    { id: 'nav-prev', key: 'ArrowLeft', alt: true, noInput: true, action(e) {
        const p = getSelectedPanel(), po = p && state.panels.find(x => x.id === p.id);
        if (!(po && po.focused)) { e.preventDefault(); navigatePrevCommand(); }
    }, label: 'Previous command' },
    { id: 'nav-next', key: 'ArrowRight', alt: true, noInput: true, action(e) {
        const p = getSelectedPanel(), po = p && state.panels.find(x => x.id === p.id);
        if (!(po && po.focused)) { e.preventDefault(); navigateNextCommand(); }
    }, label: 'Next command' },
    { id: 'screenshot', key: 'p', alt: true, noInput: true, action(e) { e.preventDefault(); const id = getActivePanelId(); if (id) screenshotPanel(id); }, label: 'Screenshot panel (Alt+P)' },
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
        if (binding.prefix) customs[id].prefix = true;
        if (binding.isPrefix) customs[id].isPrefix = true;
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
        // Skip prefix-only shortcuts when not in prefix mode
        if (s.prefix && !_prefixActive) continue;
        // Skip non-prefix shortcuts when in prefix mode (except prefix key itself and Escape)
        if (!s.prefix && !s.isPrefix && _prefixActive && s.key !== 'Escape') continue;

        const keyMatch = e.key === s.key || (s.shift && typeof s.shift === 'string' && e.shiftKey && e.key === s.shift);
        if (!keyMatch) continue;
        // For prefix shortcuts, block modifiers that aren't part of the shortcut definition.
        // Allow Shift for keys that require it (e.g. | = Shift+\, ! = Shift+1).
        // Allow Ctrl if the shortcut specifically declares ctrl:true (e.g. Ctrl+D to close).
        if (s.prefix) {
            const badCtrl = e.ctrlKey && !s.ctrl;
            const badAlt = e.altKey && !s.alt;
            const badMeta = e.metaKey && !s.meta;
            const badShift = e.shiftKey && !(s.shift || (typeof s.shift === 'string'));
            if (badCtrl || badAlt || badMeta || badShift) continue;
        }
        const ctrlOk = !s.ctrl || e.ctrlKey || e.metaKey;
        const altOk = s.alt ? e.altKey : !e.altKey;
        const shiftOk = !s.shift || e.shiftKey;
        const metaOk = !s.meta || e.metaKey;
        if (ctrlOk && altOk && shiftOk && metaOk) {
            if (!(s.noInput && _inInput(e))) {
                // Cancel prefix after executing a prefix-mode shortcut
                if (_prefixActive) _cancelPrefix();
                s.action(e);
                return true;
            }
        }
    }
    // If prefix was active but no match found, cancel it
    if (_prefixActive && !e.ctrlKey && !e.altKey && !e.metaKey) _cancelPrefix();
    return false;
}

// ─── Keyboard handling ───
document.addEventListener('keydown', (e) => {
    // Direct terminal keyboard input: capture keystrokes and send to PTY
    if (state.currentView === 'vtty') {
        const panel = getSelectedPanel();
        if (panel) {
            const panelObj = state.panels.find(p => p.id === panel.id);
            // Check if the terminal is focused (click-to-focus) and has a command to send to.
            // For split panels, the focused leaf's command may differ from state.selectedCmdId.
            const hasCommand = state.selectedCmdId
                || (panelObj && (panelObj.split || panelObj._rootSplit) && panelObj._focusedLeafId);
            if (panelObj && panelObj.focused && hasCommand) {
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

    // FIX: Try shortcuts BEFORE auto-focusing keyInput.
    // Previously, auto-focus ran first, making _inInput(e) return true,
    // which caused noInput:true shortcuts (Alt+|, Alt+-, etc.) to be skipped.
    _tryShortcut(e);

    // Focus key input when not in an input field and a command is selected
    // (only if no shortcut was matched above)
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
});

// ─── Direct key sending (when terminal is focused) ───
async function sendDirectKey(e, panelObj) {
    // Find which vtty container is focused to determine the target leaf
    const focusedVtty = document.activeElement?.closest?.('.vtty-container')
        || panelObj.focused && document.querySelector(`#vtty-${panelObj.id}`)?.closest('.panel')?.querySelector('.vtty-container[tabindex="0"]');
    const leafInfo = focusedVtty ? _getLeafFromVtty(focusedVtty, panelObj) : { leaf: panelObj, isPanelLeaf: true };
    const leaf = leafInfo.leaf;
    const cmdId = leafInfo.isPanelLeaf ? panelObj.selectedCmdId : leaf.cmdId;
    const instUrl = leafInfo.isPanelLeaf ? panelObj.selectedInstUrl : leaf.instUrl;
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

        // Track which leaf is active in split panes
        const leafId = vttyContainer.getAttribute('data-leaf-id');
        // MUST check both split trees — _rootSplit leaves are valid targets too.
        if ((panelObj.split || panelObj._rootSplit) && leafId) {
            _setActiveSideForLeaf(panelObj, leafId);
        }

        // FIX: Always focus on click (no toggle). Single click selects + focuses.
        // If already focused on THIS container, re-focus it (handles blur cases).
        state.panels.forEach(p => p.focused = false);
        document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
        panelObj.focused = true;
        vttyContainer.style.outline = '2px solid var(--accent)';
        vttyContainer.setAttribute('tabindex', '0');
        vttyContainer.focus();
    } else if (!vttyContainer) {
        state.panels.forEach(p => p.focused = false);
        document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
    }
});

function _loadLeaf(po, leafId) {
    // Load VTTY content for a specific leaf in a split pane
    const vc = document.getElementById('vtty-' + leafId);
    if (!vc) return;
    const leafInfo = _getLeafFromVtty(vc, po);
    const cmdId = leafInfo.isPanelLeaf ? po.selectedCmdId : leafInfo.leaf.cmdId;
    const instUrl = leafInfo.isPanelLeaf ? po.selectedInstUrl : leafInfo.leaf.instUrl;
    if (!cmdId || !instUrl) return;
    if (leafInfo.isPanelLeaf) {
        loadVttyHttpForPanel(po.id, instUrl, cmdId);
    } else if (typeof _loadLeafVttyHttpDirect === 'function') {
        _loadLeafVttyHttpDirect(leafInfo.leaf);
    }
}

// ─── Mouse wheel / scrollback handling ───
let _wheelScrollRafId = null, _wheelScrollPanel = null, _wheelScrollAccum = 0;

document.addEventListener('wheel', (e) => {
    const panelObj = _getPanelObj(e);
    if (!panelObj || !state.selectedCmdId) return;
    const panelEl = e.target.closest('.vtty-container')?.closest('.panel');
    if (!panelEl) return;
    if (panelObj.selectionMode) return;

    const vc = e.target.closest('.vtty-container');
    const leafId = vc ? (vc.getAttribute('data-leaf-id') || panelObj.id) : panelObj.id;

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
            _loadLeaf(panelObj, leafId);
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
        _loadLeaf(p, leafId);
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
    // Find the target leaf from the event
    const vc = e.target.closest('.vtty-container');
    const leafInfo = vc ? _getLeafFromVtty(vc, panelObj) : { leaf: panelObj, isPanelLeaf: true };
    const leaf = leafInfo.leaf;
    const cmdId = leafInfo.isPanelLeaf ? panelObj.selectedCmdId : leaf.cmdId;
    const instUrl = leafInfo.isPanelLeaf ? panelObj.selectedInstUrl : leaf.instUrl;
    if (!cmdId || !instUrl) return;
    const vttyEl = document.getElementById(leafInfo.isPanelLeaf ? panelObj.id : leaf.id)?.querySelector('.vtty-container') || vc;
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

Object.assign(window, { _KEY_MAP, _defaultShortcuts, _loadCustomShortcuts, _saveCustomShortcut, _rebuildShortcuts, _getFocusedLeafId, _getLeafFromVtty, _setActiveSideForLeaf, _activatePrefix, _cancelPrefix, _prefixActive: (() => _prefixActive) });
})();
