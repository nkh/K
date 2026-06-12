// ─── Keyboard & Mouse Handling ───
// Global keydown/click/wheel/mouse event listeners for terminal interaction,
// direct key sending to PTY, scrollback navigation, and mouse event forwarding.
(function() {
    'use strict';

// ─── Keyboard handling ───
document.addEventListener('keydown', (e) => {
    // Direct terminal keyboard input: when a panel is focused,
    // capture keystrokes and send them to the PTY directly.
    if (state.currentView === 'vtty') {
        const panel = getSelectedPanel();
        if (panel) {
            const panelObj = state.panels.find(p => p.id === panel.id);
            if (panelObj && panelObj.focused && state.selectedCmdId) {
                // Skip if user is in a search input
                const searchBar = document.getElementById('searchBar-' + panel.id);
                if (searchBar && searchBar.classList.contains('visible') &&
                    document.activeElement && document.activeElement.id === 'searchInput-' + panel.id) {
                    // Let search input handle the key
                } else if (e.key === 'Escape') {
                    // Close Add Panel modal if open
                    const panelModal = document.getElementById('panelModal');
                    if (panelModal && panelModal.style.display !== 'none') {
                        closePanelModal();
                        return;
                    }
                    // Close Command Picker if open
                    const cmdPicker = document.getElementById('cmdPicker');
                    if (cmdPicker) {
                        releaseCurrentFocusTrap();
                        cmdPicker.remove();
                        return;
                    }
                    vttySearchClose(panel.id);
                    closeContextMenu();
                    closeShortcuts();
                    return;
                } else if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
                    e.preventDefault();
                    const sb = document.getElementById('searchBar-' + panel.id);
                    if (sb) {
                        sb.classList.add('visible');
                        // Trap focus inside the search bar
                        const vttyContainer = panel.querySelector('.vtty-container');
                        if (vttyContainer) trapFocus(vttyContainer);
                        const si = document.getElementById('searchInput-' + panel.id);
                        if (si) { si.focus(); si.select(); }
                    }
                    return;
                } else {
                    e.preventDefault();
                    sendDirectKey(e, panelObj);
                    return;
                }
            }
        }
    }

    // Focus key input when not in an input field and a command is selected
    if (state.currentView === 'vtty' && state.selectedCmdId &&
        !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panel = getSelectedPanel();
        if (panel) {
            const input = document.getElementById('keyInput-' + panel.id);
            if (input) input.focus();
        }
    }
    // Ctrl+F — open terminal search bar
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        const vttyContainer = e.target.closest && e.target.closest('.vtty-container');
        if (vttyContainer || state.currentView === 'vtty') {
            e.preventDefault();
            const panel = getSelectedPanel();
            if (panel) {
                const searchBar = document.getElementById('searchBar-' + panel.id);
                if (searchBar) {
                    searchBar.classList.add('visible');
                    // Trap focus inside the search bar area
                    const vtty = panel.querySelector('.vtty-container');
                    if (vtty) trapFocus(vtty);
                    const searchInput = document.getElementById('searchInput-' + panel.id);
                    if (searchInput) { searchInput.focus(); searchInput.select(); }
                }
            }
        }
    }
    // Shift+F10 or ContextMenu key — open context menu on focused cmd-item or panel-header
    if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
        e.preventDefault();
        const target = document.activeElement;
        if (!target) return;
        // Panel header context menu
        if (target.classList.contains('panel-header') && target.dataset.panelId) {
            const rect = target.getBoundingClientRect();
            showPanelContextMenu({ preventDefault: () => {}, clientX: rect.left + rect.width / 2, clientY: rect.bottom }, target.dataset.panelId);
        }
        // Command item context menu
        if (target.classList.contains('cmd-item') && target.dataset.instUrl) {
            const rect = target.getBoundingClientRect();
            showCmdContextMenu({ preventDefault: () => {}, clientX: rect.left + rect.width / 2, clientY: rect.bottom }, target.dataset.instUrl, target.dataset.cmdId, target.dataset.cmdName, target.dataset.cmdAlive === 'true');
        }
    }
    // Escape — close terminal search bar, panel modal, command picker, shortcuts
    if (e.key === 'Escape') {
        // Close Add Panel modal if open
        const panelModal = document.getElementById('panelModal');
        if (panelModal && panelModal.style.display !== 'none') {
            closePanelModal();
            return;
        }
        // Close Command Picker if open
        const cmdPicker = document.getElementById('cmdPicker');
        if (cmdPicker) {
            releaseCurrentFocusTrap();
            cmdPicker.remove();
            return;
        }
        const panel = getSelectedPanel();
        if (panel) {
            vttySearchClose(panel.id);
        }
        closeContextMenu();
        closeShortcuts();
    }
    // Ctrl+Shift+C — copy terminal selection
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'C' || e.key === 'c')) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            copyTerminalSelection(panel.id);
            return;
        }
    }
    // Ctrl+Shift+S — toggle selection mode
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'S' || e.key === 's')) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            toggleSelectionMode(panel.id);
            return;
        }
    }
    // Alt+S — toggle selection mode (alternative shortcut)
    if (e.altKey && (e.key === 's' || e.key === 'S') && !e.ctrlKey && !e.metaKey) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            toggleSelectionMode(panel.id);
            return;
        }
    }
    // ? — show keyboard shortcuts
    if (e.key === '?' && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        showShortcuts();
    }
    // Ctrl+Shift+E — export terminal as text
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'E' || e.key === 'e')) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            exportTerminal(panel.id);
            return;
        }
    }
    // Ctrl+Shift+R — restart command (only when not in input)
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'R' || e.key === 'r') && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panel = getSelectedPanel();
        if (panel) {
            e.preventDefault();
            restartCommand(panel.id);
            return;
        }
    }
    // Alt+T — toggle panel theme (only when not in input)
    if (e.altKey && (e.key === 't' || e.key === 'T') && !e.ctrlKey && !e.metaKey && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panelId = getActivePanelId();
        if (panelId) {
            e.preventDefault();
            togglePanelTheme(panelId);
            return;
        }
    }
    // Alt+N — add new panel (only when not in input)
    if (e.altKey && (e.key === 'n' || e.key === 'N') && !e.ctrlKey && !e.metaKey && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        e.preventDefault();
        addPanel();
        return;
    }
    // Alt+Left / Alt+Right — navigate prev/next command (only when not focused on terminal)
    if (e.altKey && !e.ctrlKey && !e.metaKey && !['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName)) {
        const panel = getSelectedPanel();
        const panelObj = panel && state.panels.find(p => p.id === panel.id);
        if (e.key === 'ArrowLeft' && !(panelObj && panelObj.focused)) {
            e.preventDefault();
            navigatePrevCommand();
            return;
        }
        if (e.key === 'ArrowRight' && !(panelObj && panelObj.focused)) {
            e.preventDefault();
            navigateNextCommand();
            return;
        }
    }
});


// ─── Direct key sending (when terminal is focused) ───
// Encodes a KeyboardEvent into escape sequences and sends to the PTY.
async function sendDirectKey(e, panelObj) {
    if (!state.selectedCmdId || !panelObj.selectedInstUrl) return;

    // Map common special keys to escape sequences
    const keyMap = {
        'Enter': '\r',
        'Backspace': '\x7f',
        'Tab': '\t',
        'Escape': '\x1b',
        'Home': '\x1b[H',
        'End': '\x1b[F',
        'Delete': '\x1b[3~',
        'ArrowUp': '\x1b[A',
        'ArrowDown': '\x1b[B',
        'ArrowRight': '\x1b[C',
        'ArrowLeft': '\x1b[D',
        'PageUp': '\x1b[5~',
        'PageDown': '\x1b[6~',
        'Insert': '\x1b[2~',
        'F1': '\x1bOP',
        'F2': '\x1bOQ',
        'F3': '\x1bOR',
        'F4': '\x1bOS',
        'F5': '\x1b[15~',
        'F6': '\x1b[17~',
        'F7': '\x1b[18~',
        'F8': '\x1b[19~',
        'F9': '\x1b[20~',
        'F10': '\x1b[21~',
        'F11': '\x1b[23~',
        'F12': '\x1b[24~',
    };

    let seq = '';
    if (e.ctrlKey && !e.altKey && !e.metaKey) {
        // Ctrl+letter
        if (e.key.length === 1 && e.key >= 'a' && e.key <= 'z') {
            seq = String.fromCharCode(e.key.charCodeAt(0) - 96);
        } else if (e.key === '[') seq = '\x1b'; // Ctrl+[ = ESC
        else if (e.key === '\\') seq = '\x1c';
        else if (e.key === ']') seq = '\x1d';
        else if (e.key === '^') seq = '\x1e';
        else if (e.key === '_') seq = '\x1f';
    } else if (e.altKey && !e.ctrlKey && !e.metaKey) {
        // Alt+letter = ESC + letter
        if (e.key.length === 1) seq = '\x1b' + e.key;
    } else if (keyMap[e.key]) {
        seq = keyMap[e.key];
    } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
        // Regular printable character
        seq = e.key;
    }

    if (!seq) return;

    try {
        const json = await api.sendKeys(panelObj.selectedInstUrl, state.selectedCmdId, { keys: seq });
        if (json.status === 'ok') {
            // Trigger a refresh
            scheduleVttyHttp(panelObj.selectedInstUrl, state.selectedCmdId, 50);
        }
    } catch (err) {
        console.error('Direct key send error:', err);
    }
}

// ─── Click-to-focus terminal ───
// Clicking on the VTTY container focuses the terminal for direct keyboard input.
// A second click on an already-focused terminal blurs it.
document.addEventListener('click', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (vttyContainer && state.currentView === 'vtty') {
        const panelEl = vttyContainer.closest('.panel');
        if (panelEl) {
            const panelObj = state.panels.find(p => p.id === panelEl.id);
            if (panelObj) {
                // Check if click is on a button inside the vtty container (search bar, scroll btn)
                if (e.target.closest('button') || e.target.closest('input')) return;

                if (panelObj.focused) {
                    // Already focused — blur
                    panelObj.focused = false;
                    vttyContainer.style.outline = '';
                } else {
                    // Focus this panel's terminal
                    state.panels.forEach(p => p.focused = false);
                    document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
                    panelObj.focused = true;
                    vttyContainer.style.outline = '2px solid var(--accent)';
                    vttyContainer.setAttribute('tabindex', '0');
                    vttyContainer.focus();
                }
            }
        }
    } else if (!vttyContainer) {
        // Click outside any terminal — blur all
        state.panels.forEach(p => p.focused = false);
        document.querySelectorAll('.vtty-container').forEach(v => v.style.outline = '');
    }
});

// ─── Mouse wheel handling on terminal ───
// Level 1 optimization: Don't block native scroll when viewing the live buffer.
// Only intercept wheel events at the top edge (scroll into scrollback history)
// or when mouse tracking is enabled (forward to PTY).
//
// When in scrollback view (scrollbackOffset > 0), scroll wheel navigates
// scrollback history via server-side offset (debounced with rAF).
//
// Native scroll provides smooth inertia and momentum — the browser handles
// repaint timing, which is far more efficient than per-tick HTTP round-trips.
let _wheelScrollRafId = null;
let _wheelScrollPanel = null;   // panel object for the pending rAF callback
let _wheelScrollAccum = 0;      // accumulated signed vertical delta

document.addEventListener('wheel', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') return;

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // If selection mode is active, let browser handle wheel natively (no scrollback, no PTY)
    if (panelObj.selectionMode) return;

    // If the child has mouse tracking enabled, forward wheel events to the PTY
    if (panelObj.mouseTracking) {
        e.preventDefault();
        const wheelEvent = e.deltaY < 0 ? 'wheel_up' : 'wheel_down';
        sendMouseEvent(panelObj, wheelEvent, 0, e);
        return;
    }

    // ── Live buffer view (scrollbackOffset === 0) ──
    // Allow native scroll. Only intercept when user scrolls up past the top
    // edge, which means they want to enter scrollback history.
    if (panelObj.scrollbackOffset === 0) {
        const atTop = vttyContainer.scrollTop <= 0;
        if (e.deltaY < 0 && atTop) {
            // User scrolled up at the top edge — enter scrollback history
            e.preventDefault();
            panelObj.scrollbackOffset += 3;
            sessionStorage.setItem('vrw_scrollback_' + state.selectedCmdId, panelObj.scrollbackOffset.toString());
            loadVttyHttp(panelObj.selectedInstUrl, state.selectedCmdId);
            // Show scrollback indicator
            const sbIndicator = document.getElementById('scrollbackIndicator');
            if (sbIndicator) { sbIndicator.style.display = ''; sbIndicator.textContent = 'SCROLLBACK -' + panelObj.scrollbackOffset + ' rows'; }
            const btn = panelEl.querySelector('.scroll-bottom-btn');
            if (btn) btn.classList.add('visible');
        }
        // else: let browser handle native scroll (no preventDefault)
        return;
    }

    // ── Scrollback history view (scrollbackOffset > 0) ──
    e.preventDefault();

    // Accumulate scroll delta — will be processed in the next animation frame.
    // This coalesces rapid wheel ticks into a single HTTP round-trip.
    _wheelScrollPanel = panelObj;
    _wheelScrollAccum += e.deltaY;

    if (_wheelScrollRafId) cancelAnimationFrame(_wheelScrollRafId);
    _wheelScrollRafId = requestAnimationFrame(() => {
        _wheelScrollRafId = null;
        const p = _wheelScrollPanel;
        if (!p) return;

        // Snapshot and reset the accumulator before processing.
        const accum = _wheelScrollAccum;
        _wheelScrollAccum = 0;

        // Convert accumulated pixel delta to scrollback lines.
        // ~100px of scroll ≈ 3 lines (same ratio as the previous per-tick behavior).
        const lines = Math.max(1, Math.round(Math.abs(accum) / 100) * 3);

        if (accum > 0) {
            // Wheel down: decrease scrollback offset (move toward live view)
            const newOffset = Math.max(0, p.scrollbackOffset - lines);
            if (newOffset === 0) {
                // Reached the live buffer — restore native scroll
                p.scrollbackOffset = 0;
                sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
                loadVttyHttpForPanel(panel.id, p.selectedInstUrl, p.selectedCmdId);
                // Scroll to bottom after returning to live view
                const vtty = panelEl.querySelector('.vtty-container');
                if (vtty) vtty.scrollTop = vtty.scrollHeight;
            } else {
                p.scrollbackOffset = newOffset;
                sessionStorage.setItem('vrw_scrollback_' + state.selectedCmdId, p.scrollbackOffset.toString());
                loadVttyHttpForPanel(panel.id, p.selectedInstUrl, p.selectedCmdId);
            }
        } else {
            // Wheel up: increase scrollback offset (move into history)
            p.scrollbackOffset += lines;
            sessionStorage.setItem('vrw_scrollback_' + state.selectedCmdId, p.scrollbackOffset.toString());
            loadVttyHttpForPanel(panel.id, p.selectedInstUrl, p.selectedCmdId);
        }

        // Update scroll-to-bottom button visibility and scrollback indicator
        const btn = panelEl.querySelector('.scroll-bottom-btn');
        if (btn) btn.classList.toggle('visible', p.scrollbackOffset > 0);
        const sbIndicator = document.getElementById('scrollbackIndicator');
        if (sbIndicator) {
            sbIndicator.style.display = p.scrollbackOffset > 0 ? '' : 'none';
            if (p.scrollbackOffset > 0) sbIndicator.textContent = 'SCROLLBACK -' + p.scrollbackOffset + ' rows';
        }
    });
}, { passive: false });

// ─── Mouse event forwarding to PTY ───
// Forwards mousedown, mouseup, mousemove events to the PTY when the child
// has enabled mouse tracking mode. Events are sent as escape sequences via
// POST /api/commands/:id/mouse.

let _mouseDownButton = null; // Track which button is pressed

document.addEventListener('mousedown', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') {
        _mouseDownButton = null;
        return;
    }

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // Skip if clicking on buttons/inputs inside vtty container
    if (e.target.closest('button') || e.target.closest('input')) return;

    // If selection mode is active, skip PTY forwarding — let browser handle selection
    if (panelObj.selectionMode) return;

    // If mouse tracking is enabled, forward the event to PTY
    if (panelObj.mouseTracking) {
        e.preventDefault();
        _mouseDownButton = e.button; // 0=left, 1=middle, 2=right
        sendMouseEvent(panelObj, 'down', e.button, e);
    }
});

document.addEventListener('mouseup', (e) => {
    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') {
        _mouseDownButton = null;
        return;
    }

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId) return;

    // If selection mode is active, skip PTY forwarding — auto-copy on select
    if (panelObj.selectionMode) {
        _mouseDownButton = null;
        // Copy-on-select: if user just selected text, copy it automatically
        setTimeout(() => {
            const sel = window.getSelection();
            const text = sel ? sel.toString().trim() : '';
            if (text) copyTerminalSelection(panelEl.id);
        }, 0);
        return;
    }

    if (panelObj.mouseTracking && _mouseDownButton !== null) {
        e.preventDefault();
        sendMouseEvent(panelObj, 'up', _mouseDownButton, e);
        _mouseDownButton = null;
    }
});

document.addEventListener('mousemove', (e) => {
    if (_mouseDownButton === null) return; // Only track during drag

    const vttyContainer = e.target.closest('.vtty-container');
    if (!vttyContainer || state.currentView !== 'vtty') return;

    const panelEl = vttyContainer.closest('.panel');
    if (!panelEl) return;

    const panelObj = state.panels.find(p => p.id === panelEl.id);
    if (!panelObj || !state.selectedCmdId || !panelObj.mouseTracking) return;

    // If selection mode is active, skip PTY forwarding
    if (panelObj.selectionMode) return;

    // Throttle mouse move events to avoid flooding
    if (!panelObj._lastMoveTime || Date.now() - panelObj._lastMoveTime > 16) {
        panelObj._lastMoveTime = Date.now();
        sendMouseEvent(panelObj, 'move', _mouseDownButton, e);
    }
});

// Send a mouse event to the PTY via the API
async function sendMouseEvent(panelObj, eventType, button, e) {
    if (!state.selectedCmdId || !panelObj.selectedInstUrl) return;

    // Calculate terminal cell coordinates from pixel position
    const vttyEl = document.getElementById(panelObj.id)?.querySelector('.vtty-container');
    if (!vttyEl) return;

    const rect = vttyEl.getBoundingClientRect();
    const charW = state.fontSize * 0.6;
    const charH = state.fontSize * 1.2;

    const x = Math.max(1, Math.floor((e.clientX - rect.left) / charW) + 1);
    const y = Math.max(1, Math.floor((e.clientY - rect.top) / charH) + 1);

    try {
        await api.sendMouse(panelObj.selectedInstUrl, state.selectedCmdId, {
            event: eventType,
            button: button,
            x: x,
            y: y,
        });
        // Refresh display after mouse events (the child may have reacted)
        scheduleVttyHttp(panelObj.selectedInstUrl, state.selectedCmdId, 30);
    } catch (err) {
        // Silently ignore — mouse events are best-effort
    }
}

    // No global window assignments needed — keyboard/mouse handlers are event listeners
    // Export the key map for testing
    const _KEY_MAP = {
        'Enter': '\r', 'Backspace': '\x7f', 'Tab': '\t', 'Escape': '\x1b',
        'Home': '\x1b[H', 'End': '\x1b[F', 'Delete': '\x1b[3~',
        'ArrowUp': '\x1b[A', 'ArrowDown': '\x1b[B', 'ArrowRight': '\x1b[C', 'ArrowLeft': '\x1b[D',
        'PageUp': '\x1b[5~', 'PageDown': '\x1b[6~', 'Insert': '\x1b[2~',
        'F1': '\x1bOP', 'F2': '\x1bOQ', 'F3': '\x1bOR', 'F4': '\x1bOS',
        'F5': '\x1b[15~', 'F6': '\x1b[17~', 'F7': '\x1b[18~', 'F8': '\x1b[19~',
        'F9': '\x1b[20~', 'F10': '\x1b[21~', 'F11': '\x1b[23~', 'F12': '\x1b[24~',
    };
    window._KEY_MAP = _KEY_MAP;
})();
