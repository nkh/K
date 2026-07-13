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
    // Position off-DOM first to avoid layout thrash (append → measure → reposition)
    menu.style.visibility = 'hidden';
    menu.style.left = '0'; menu.style.top = '0';
    document.body.appendChild(menu);
    const rect = menu.getBoundingClientRect();
    menu.style.visibility = '';
    let left = x, top = y;
    if (left + rect.width > window.innerWidth) left = window.innerWidth - rect.width - 4;
    if (top + rect.height > window.innerHeight) top = window.innerHeight - rect.height - 4;
    if (left < 0) left = 4;
    if (top < 0) top = 4;
    menu.style.left = left + 'px'; menu.style.top = top + 'px';
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
        _addCtxSep(menu);
        menu.appendChild(_createCtxMenuItem('Share Terminal...', () => _showShareModal(instUrl, cmdId)));
        menu.appendChild(_createCtxMenuItem('Open in New Tab', () => _openViewerTab(instUrl, cmdId)));
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

    // ── Share Modal ──
    function _showShareModal(instUrl, cmdId) {
        closeContextMenu();
        const cmd = _findCmd(instUrl, cmdId);
        const cmdName = cmd ? (cmd.name || cmd.id) : cmdId;
        // Remove existing modal if any
        const existing = document.getElementById('shareModal');
        if (existing) existing.remove();
        const overlay = document.createElement('div');
        overlay.id = 'shareModal';
        overlay.className = 'modal-overlay';
        overlay.innerHTML = `<div class="modal" style="min-width:380px">
            <h2>Share Terminal</h2>
            <p style="font-size:var(--ui-fs);color:var(--text-secondary);margin-bottom:0.75rem;">Create a link that lets others view${' '}<strong>${escHtml(cmdName)}</strong>'s terminal in real-time.</p>
            <div class="field" style="margin-bottom:0.5rem;">
                <label style="display:flex;align-items:center;gap:0.3rem;font-size:var(--ui-fs);color:var(--text-secondary);cursor:pointer;">
                    <input type="checkbox" id="shareKeyboard" style="width:auto;"> Allow viewers to type (interactive)
                </label>
            </div>
            <div class="field" style="margin-bottom:0.5rem;">
                <label style="font-size:var(--ui-fs);color:var(--text-secondary);">Expires in</label>
                <select id="shareExpires" style="font-size:var(--ui-fs);padding:0.15rem 0.3rem;background:var(--bg-tertiary);color:var(--text-primary);border:1px solid var(--border);border-radius:var(--radius-sm);width:100%;">
                    <option value="1">1 hour</option>
                    <option value="4">4 hours</option>
                    <option value="24" selected>24 hours</option>
                    <option value="72">3 days</option>
                    <option value="168">1 week</option>
                    <option value="0">Never</option>
                </select>
            </div>
            <div class="field" id="shareResult" class="hidden" style="margin-bottom:0.5rem;display:none;">
                <label style="font-size:var(--ui-fs);color:var(--text-secondary);">Share link</label>
                <div style="display:flex;gap:0.2rem;">
                    <input type="text" id="shareUrl" readonly style="flex:1;font-size:var(--ui-fs);padding:0.15rem 0.3rem;background:var(--bg-tertiary);color:var(--text-primary);border:1px solid var(--border);border-radius:var(--radius-sm);">
                    <button class="btn btn-xs btn-primary" id="shareCopyBtn">Copy</button>
                </div>
            </div>
            <div class="actions">
                <button class="btn btn-xs" id="shareCancelBtn">Cancel</button>
                <button class="btn btn-xs btn-primary" id="shareCreateBtn">Create Link</button>
            </div>
        </div>`;
        document.body.appendChild(overlay);
        overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
        document.getElementById('shareCancelBtn').addEventListener('click', () => overlay.remove());
        document.getElementById('shareCreateBtn').addEventListener('click', async () => {
            const keyboard = document.getElementById('shareKeyboard').checked;
            const expires = parseInt(document.getElementById('shareExpires').value, 10);
            const btn = document.getElementById('shareCreateBtn');
            btn.textContent = 'Creating...'; btn.disabled = true;
            try {
                const json = await api.createShareToken(instUrl, cmdId, { keyboard, expires_hours: expires });
                if (json.status === 'ok') {
                    const baseUrl = window.location.origin;
                    const shareUrl = baseUrl + json.data.url;
                    const result = document.getElementById('shareResult');
                    result.style.display = '';
                    document.getElementById('shareUrl').value = shareUrl;
                    document.getElementById('shareCreateBtn').style.display = 'none';
                    document.getElementById('shareCopyBtn').addEventListener('click', () => {
                        navigator.clipboard.writeText(shareUrl).then(() => {
                            document.getElementById('shareCopyBtn').textContent = 'Copied!';
                            setTimeout(() => { document.getElementById('shareCopyBtn').textContent = 'Copy'; }, 2000);
                        });
                    });
                } else {
                    alert('Failed to create share link: ' + (json.error || 'unknown error'));
                    btn.textContent = 'Create Link'; btn.disabled = false;
                }
            } catch (e) {
                alert('Error: ' + (e.message || e));
                btn.textContent = 'Create Link'; btn.disabled = false;
            }
        });
        // Focus the create button
        document.getElementById('shareCreateBtn').focus();
    }

    function _openViewerTab(instUrl, cmdId) {
        closeContextMenu();
        api.createViewerToken(instUrl, cmdId).then(json => {
            if (json.status === 'ok' && json.data && json.data.token) {
                const cmd = _findCmd(instUrl, cmdId);
                const label = cmd ? encodeURIComponent(cmd.name || cmd.id) : '';
                const url = window.location.origin + '/viewer/' + json.data.token + (label ? '?label=' + label : '');
                window.open(url, '_blank');
            } else {
                alert('Failed to open viewer: ' + (json.error || 'unknown error'));
            }
        }).catch(e => alert('Error: ' + (e.message || e)));
    }

    // ── Exports ──
    Object.assign(window, {
        closeContextMenu, showCmdContextMenu, showPanelContextMenu,
    });
})();