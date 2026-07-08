// ─── Panels: Context Menu ───
(function() {
    'use strict';

function _addCtxSep(menu) {
    const s = document.createElement('div');
    s.className = 'ctx-menu-sep'; s.setAttribute('role', 'separator');
    menu.appendChild(s);
}

let _ctxMenuFocusedIndex = -1;

function closeContextMenu() {
    const el = document.getElementById('ctxMenu');
    if (el) el.remove();
    _ctxMenuFocusedIndex = -1;
}

function _createCtxMenuItem(label, onClick, isDanger) {
    const div = document.createElement('div');
    div.className = 'ctx-menu-item' + (isDanger ? ' danger' : '');
    div.setAttribute('role', 'menuitem'); div.setAttribute('tabindex', '-1');
    div.textContent = label;
    div.addEventListener('click', () => { onClick(); closeContextMenu(); });
    return div;
}

function _positionCtxMenu(menu, x, y) {
    menu.style.left = x + 'px'; menu.style.top = y + 'px';
    document.body.appendChild(menu);
    const rect = menu.getBoundingClientRect();
    if (rect.right > window.innerWidth) menu.style.left = (window.innerWidth - rect.width - 4) + 'px';
    if (rect.bottom > window.innerHeight) menu.style.top = (window.innerHeight - rect.height - 4) + 'px';
}

function _setupCtxMenuListeners(menu) {
    document.addEventListener('click', closeContextMenu, { once: true });
    menu.addEventListener('keydown', (e) => {
        const items = menu.querySelectorAll('.ctx-menu-item');
        if (!items.length) return;
        if (e.key === 'ArrowDown') { e.preventDefault(); _ctxMenuFocusedIndex = (_ctxMenuFocusedIndex + 1) % items.length; }
        else if (e.key === 'ArrowUp') { e.preventDefault(); _ctxMenuFocusedIndex = (_ctxMenuFocusedIndex - 1 + items.length) % items.length; }
        else if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); if (_ctxMenuFocusedIndex >= 0) items[_ctxMenuFocusedIndex].click(); return; }
        else if (e.key === 'Escape') { e.preventDefault(); closeContextMenu(); return; }
        else if (e.key === 'Tab') { e.preventDefault(); closeContextMenu(); return; }
        else return;
        _focusCtxMenuItem(items);
    });
    _ctxMenuFocusedIndex = 0;
    const firstItem = menu.querySelector('.ctx-menu-item');
    if (firstItem) firstItem.focus();
}

function _focusCtxMenuItem(items) {
    items.forEach((item, i) => { item.classList.toggle('ctx-menu-focused', i === _ctxMenuFocusedIndex); if (i === _ctxMenuFocusedIndex) item.focus(); });
}

function showCmdContextMenu(e, instUrl, cmdId, cmdName, isAlive, isRetained) {
    e.preventDefault(); closeContextMenu();
    const menu = document.createElement('div');
    menu.id = 'ctxMenu'; menu.className = 'ctx-menu'; menu.setAttribute('role', 'menu');
    menu.appendChild(_createCtxMenuItem('View Terminal', () => selectCommand(instUrl, cmdId, cmdName)));
    menu.appendChild(_createCtxMenuItem('Copy URL', () => copyCommandUrl(instUrl, cmdId, cmdName)));
    const groups = getCmdGroups(), groupNames = Object.keys(groups);
    if (groupNames.length > 0) {
        _addCtxSep(menu);
        for (const gName of groupNames) {
            const inGroup = groups[gName].includes(cmdName);
            menu.appendChild(_createCtxMenuItem((inGroup ? '✓ ' : '') + escHtml(gName), () => toggleCmdInGroup(gName, cmdName)));
        }
    }
    _addCtxSep(menu);
    if (isAlive) {
        menu.appendChild(_createCtxMenuItem(isRetained ? 'Unkeep' : 'Keep', () => toggleKeepCmd(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Pause/Resume', () => togglePauseCmd(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Restart', () => restartCommandById(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Kill', () => killCommand(instUrl, cmdId), true));
    } else {
        menu.appendChild(_createCtxMenuItem('Purge', () => purgeCommand(instUrl, cmdId, cmdName), true));
    }
    _positionCtxMenu(menu, e.clientX, e.clientY);
    _setupCtxMenuListeners(menu);
}

function showPanelContextMenu(e, panelId, leafId) {
    e.preventDefault(); closeContextMenu();
    const panel = state.panels.find(p => p.id === panelId);
    if (!panel) return;
    // Determine which leaf's command to show in the menu
    let instUrl, cmdId;
    if (leafId && leafId !== panelId && (panel.split || panel._rootSplit)) {
        const found = (typeof _findLeafState === 'function') ? _findLeafState(panel, leafId) : null;
        if (found && found.leaf) {
            instUrl = found.leaf.instUrl;
            cmdId = found.leaf.cmdId;
        }
    }
    if (!cmdId) {
        instUrl = panel.selectedInstUrl;
        cmdId = panel.selectedCmdId;
    }
    const menu = document.createElement('div');
    menu.id = 'ctxMenu'; menu.className = 'ctx-menu'; menu.setAttribute('role', 'menu');
    menu.appendChild(_createCtxMenuItem('Copy URL', () => {
        if (cmdId) { const cmd = _findCmd(instUrl, cmdId); copyCommandUrl(instUrl, cmdId, cmd ? (cmd.name || cmd.id) : cmdId); }
        else navigator.clipboard.writeText(instUrl).catch(() => {});
    }));
    if (cmdId) {
        menu.appendChild(_createCtxMenuItem('Pause/Resume', () => togglePauseCmd(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Restart', () => restartCommandById(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Kill', () => killCommand(instUrl, cmdId), true));
    }
    menu.appendChild(_createCtxMenuItem('Rename Panel', () => startRenamePanel(panelId)));
    if (state.panels.length > 1) {
        menu.appendChild(_createCtxMenuItem(panel.minimized ? 'Restore Panel' : 'Minimize Panel', () => toggleMinimizePanel(panelId)));
    }
    _addCtxSep(menu);
    if (!panel.split && !panel._rootSplit) {
        menu.appendChild(_createCtxMenuItem('Split Horizontal (Alt+-)', () => splitPanel(panelId, 'vertical')));
        menu.appendChild(_createCtxMenuItem('Split Vertical (Alt+|)', () => splitPanel(panelId, 'horizontal')));
    } else {
        const targetLeaf = leafId || panelId;
        menu.appendChild(_createCtxMenuItem('Split Horizontal (Alt+-)', () => {
            splitPanel(panelId, 'vertical', targetLeaf);
        }));
        menu.appendChild(_createCtxMenuItem('Split Vertical (Alt+|)', () => {
            splitPanel(panelId, 'horizontal', targetLeaf);
        }));
        menu.appendChild(_createCtxMenuItem('Remove Split (Ctrl+A Ctrl+D)', () => {
            unsplitPanel(panelId, targetLeaf);
        }));
    }
    _addCtxSep(menu);
    menu.appendChild(_createCtxMenuItem('New Window (Alt+w)', () => createWindow()));
    if (typeof state !== 'undefined' && state.windows && state.windows.length > 1) {
        menu.appendChild(_createCtxMenuItem('Close Window (Alt+W)', () => { if (state.activeWindowId) closeWindow(state.activeWindowId); }, true));
        _addCtxSep(menu);
        const winSub = document.createElement('div');
        winSub.className = 'ctx-menu-sub';
        for (let i = 0; i < state.windows.length && i < 9; i++) {
            const w = state.windows[i];
            const label = 'Window ' + escHtml(w.name) + (w.id === state.activeWindowId ? ' (active)' : '');
            menu.appendChild(_createCtxMenuItem(label, () => switchWindow(w.id)));
        }
    }
    _addCtxSep(menu);
    if (state.panels.length > 1) menu.appendChild(_createCtxMenuItem('Remove Panel', () => removePanel(panelId), true));
    _positionCtxMenu(menu, e.clientX, e.clientY);
    _setupCtxMenuListeners(menu);
}

    // ── Exports ──
    Object.assign(window, {
        closeContextMenu, showCmdContextMenu, showPanelContextMenu,
    });
})();