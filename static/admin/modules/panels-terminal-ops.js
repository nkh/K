// ─── Panels: Terminal Operations ───
(function() {
    'use strict';

function _showCopyFeedback(pid) {
    const el = document.getElementById('copyFeedback-' + pid);
    if (el) { el.classList.add('visible'); setTimeout(() => el.classList.remove('visible'), 1200); }
}

function _getResizeDims(pid) {
    const rv = id => parseInt(document.getElementById(id)?.value) || 0;
    return { rows: rv('stResizeRows') || rv('resizeRows-' + pid) || 24, cols: rv('stResizeCols') || rv('resizeCols-' + pid) || 80 };
}

// ─── Export Terminal Output ───
function _resolveTargetLeaf(targetId) {
    // Resolve a panel ID or leaf ID to { panelObj, leaf, cmdId, instUrl }
    const panelObj = state.panels.find(p => p.id === targetId);
    if (!panelObj) return null;
    if (!(panelObj.split || panelObj._rootSplit) || targetId === panelObj.id) {
        return { panelObj, leaf: panelObj, cmdId: panelObj.selectedCmdId, instUrl: panelObj.selectedInstUrl, isPanelLeaf: true };
    }
    const found = typeof _findLeafState === 'function' ? _findLeafState(panelObj, targetId) : null;
    if (found && found.leaf) {
        return { panelObj, leaf: found.leaf, cmdId: found.leaf.cmdId, instUrl: found.leaf.instUrl, isPanelLeaf: false };
    }
    return { panelObj, leaf: panelObj, cmdId: panelObj.selectedCmdId, instUrl: panelObj.selectedInstUrl, isPanelLeaf: true };
}

function copyTerminalSelection(targetId) {
    let text = window.getSelection()?.toString().trim() || '';
    if (!text) { const pre = document.querySelector(`#vtty-${targetId} pre`); if (pre) text = pre.textContent || pre.innerText || ''; }
    if (!text) return;
    const panelId = state.panels.find(p => p.id === targetId) ? targetId : (state.panels.find(p => (p.split || p._rootSplit) && p._focusedLeafId === targetId) ? state.panels.find(p => p._focusedLeafId === targetId).id : targetId);
    navigator.clipboard.writeText(text).then(() => _showCopyFeedback(panelId)).catch(() => {
        const ta = document.createElement('textarea');
        ta.value = text; ta.style.cssText = 'position:fixed;opacity:0;';
        document.body.appendChild(ta); ta.select();
        try { document.execCommand('copy'); } catch {}
        document.body.removeChild(ta); _showCopyFeedback(panelId);
    });
}

function exportTerminal(targetId) {
    const resolved = _resolveTargetLeaf(targetId);
    if (!resolved) return;
    const pre = document.querySelector(`#vtty-${targetId} pre`);
    if (!pre) return;
    const text = pre.textContent || pre.innerText || '';
    const cmd = _findCmd(resolved.instUrl, resolved.cmdId);
    const cmdName = cmd ? (cmd.name || cmd.id).replace(/\//g, '_') : 'terminal';
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([text], { type: 'text/plain' }));
    a.download = cmdName + '.txt'; a.click(); URL.revokeObjectURL(a.href);
}

async function screenshotPanel(panelId) {
    const panelObj = state.panels.find(p => p.id === panelId);
    if (!panelObj) return;
    const instUrl = panelObj.selectedInstUrl, cmdId = panelObj.selectedCmdId;
    if (!instUrl || !cmdId) { alert('No command selected to screenshot.'); return; }
    const fontSize = state.serverScreenshotFontSize || 12;
    const fontName = state.serverScreenshotFontName || 'monospace';
    const params = new URLSearchParams({ font_size: fontSize });
    if (fontName !== 'monospace') params.set('font_name', fontName);
    try {
        const blob = await api.getVttyPng(instUrl, cmdId, Object.fromEntries(params));
        const cmd = _findCmd(instUrl, cmdId);
        const parts = cmd ? [cmd.name || 'unknown', ...(cmd.args || [])] : ['vrw'];
        const cmdInfo = parts.join(' ').replace(/[^a-zA-Z0-9_\-\.]/g, '_').substring(0, 120);
        const now = new Date(), pad = n => String(n).padStart(2, '0');
        const ts = `${now.getFullYear()}${pad(now.getMonth()+1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
        const pre = document.querySelector(`#vtty-${panelId} pre`);
        const dims = (pre && pre._vttyRows) ? pre._vttyRows + 'x' + pre._vttyCols : '';
        const a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = `vrw_${ts}${dims ? '_' + dims : ''}_${cmdInfo}.png`;
        a.click(); URL.revokeObjectURL(a.href);
    } catch (e) { alert('Screenshot failed: ' + e.message); }
}

async function sendKeysToPanel(panelId) {
    const panel = state.panels.find(p => p.id === panelId);
    if (!panel) return;
    const input = document.getElementById('stKeyInput') || document.getElementById('keyInput-' + panelId);
    if (!input || !input.value || !state.selectedCmdId) return;
    const keysValue = input.value;
    const cmdId = panel.selectedCmdId || state.selectedCmdId;
    const instUrl = panel.selectedInstUrl || state.selectedInstUrl;
    try {
        const json = await api.sendKeys(instUrl, cmdId, { keys: keysValue });
        input.value = '';
        if (json.status === 'ok') loadVttyHttpForPanel(panelId, instUrl, cmdId);
        else console.error('send_keys server error:', json.error);
    } catch (e) { console.error('send_keys error:', e); }
}

function showSpecialKeysHelp() {
    const old = document.getElementById('specialKeysModal');
    if (old) { old.remove(); return; }
    const overlay = document.createElement('div');
    overlay.id = 'specialKeysModal'; overlay.className = 'modal-overlay';
    overlay.onclick = (e) => { if (e.target === overlay) { releaseCurrentFocusTrap(); overlay.remove(); } };
    const rows = [
        ['Return / Enter', '<code>&lt;Enter&gt;</code> or <code>&lt;Return&gt;</code>', 'Send a newline (carriage return)'],
        ['Backspace', '<code>&lt;Backspace&gt;</code>', 'Delete character before cursor'],
        ['Tab', '<code>&lt;Tab&gt;</code>', 'Insert a tab character'],
        ['Escape', '<code>&lt;Esc&gt;</code>', 'Send the Escape character (0x1B)'],
        ['Space', '(space character)', 'Type a literal space'],
        ['Delete', '<code>&lt;Delete&gt;</code>', 'Delete character at cursor (forward delete)'],
        ['Insert', '<code>&lt;Insert&gt;</code>', 'Toggle insert/overwrite mode'],
        ['Home / End', '<code>&lt;Home&gt;</code> <code>&lt;End&gt;</code>', 'Jump to beginning / end of line'],
        ['Page Up / Down', '<code>&lt;PageUp&gt;</code> <code>&lt;PageDown&gt;</code>', 'Scroll up / down one page'],
        ['Arrow Keys', '<code>&lt;Up&gt;</code> <code>&lt;Down&gt;</code> <code>&lt;Left&gt;</code> <code>&lt;Right&gt;</code>', 'Cursor movement'],
        ['F1 – F12', '<code>&lt;F1&gt;</code> … <code>&lt;F12&gt;</code>', 'Function keys'],
        ['Ctrl + key', '<code>&lt;C-c&gt;</code> <code>&lt;C-a&gt;</code> …', 'Control modifier (lowercase). <code>&lt;C-c&gt;</code> = SIGINT'],
        ['Alt + key', '<code>&lt;A-x&gt;</code> <code>&lt;A-enter&gt;</code> …', 'Alt/Meta prefix (Escape + key)'],
    ];
    const p = 'padding:0.25rem 0.5rem;', th = p + 'color:var(--text-muted);font-weight:600;';
    const tbody = rows.map((r, i) => `<tr style="${i < rows.length - 1 ? 'border-bottom:1px solid var(--border);' : ''}"><td style="${p}">${r[0]}</td><td style="${p}">${r[1]}</td><td style="${p}color:var(--text-secondary);">${r[2]}</td></tr>`).join('');
    overlay.innerHTML = `<div class="modal" style="max-width:560px;max-height:80vh;overflow-y:auto;">
<h2 style="margin-bottom:0.5rem;">Special Keys Reference</h2>
<p style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.75rem;">Type special keys using <code style="background:var(--bg-tertiary);padding:0.1rem 0.3rem;border-radius:2px;">&lt;KeyName&gt;</code> syntax. Mix with text: <code style="background:var(--bg-tertiary);padding:0.1rem 0.3rem;border-radius:2px;">hello&lt;Enter&gt;world</code>.</p>
<table style="width:100%;font-size:0.75rem;border-collapse:collapse;">
<thead><tr style="border-bottom:1px solid var(--border);text-align:left;"><th style="${th}">Key</th><th style="${th}">Syntax</th><th style="${th}">Description</th></tr></thead>
<tbody>${tbody}</tbody></table>
<div style="margin-top:0.75rem;text-align:right;"><button class="btn btn-xs" data-action="CloseSpecialKeysModal">Close</button></div></div>`;
    document.body.appendChild(overlay);
    const modal = overlay.querySelector('.modal');
    if (modal) trapFocus(modal);
    overlay.querySelector('button')?.focus();
}

function copyCommandUrl(instUrl, cmdId, cmdName) {
    const base = cmdName.replace(/.*\//, '');
    navigator.clipboard.writeText(instUrl.replace(/^http/, 'http') + '/' + encodeURIComponent(base)).catch(() => {});
}

async function togglePauseCmd(instUrl, cmdId) {
    try { await _doFreezeThaw(instUrl, cmdId); loadCommands(); } catch (e) {}
}

// ─── Auto-fit Terminal on Window Resize ───
function autoFitActiveTerminal() {
    if (!state.selectedInstUrl || !state.selectedCmdId) return;
    const panel = getSelectedPanel();
    if (!panel) return;
    const vttyEl = panel.querySelector('.vtty-container');
    if (!vttyEl) return;
    // Skip if terminal has no real content yet (new panel, command not loaded)
    const pre = vttyEl.querySelector('pre');
    if (!pre || !pre.children.length) return;
    const rect = vttyEl.getBoundingClientRect();
    if (rect.width < 10 || rect.height < 10) return;
    const charW = state.fontSize * 0.6, charH = state.fontSize * 1.2;
    const cols = Math.max(20, Math.min(500, Math.floor(rect.width / charW)));
    const rows = Math.max(5, Math.min(200, Math.floor(rect.height / charH)));
    if (rows !== state._termRows || cols !== state._termCols) api.resize(state.selectedInstUrl, state.selectedCmdId, { rows, cols }).catch(() => {});
}

async function _resizePanelTo(panelId, rows, cols) {
    const p = state.panels.find(pp => pp.id === panelId);
    if (!p || !p.selectedCmdId) return false;
    const cmd = _findCmd(p.selectedInstUrl, p.selectedCmdId);
    if (cmd && cmd.status === 'exited') return false;
    try { await api.resize(p.selectedInstUrl, p.selectedCmdId, { rows, cols }); return true; } catch { return false; }
}

// Shared state objects for cross-module access (panels-toolbar reads these)
window._maxFitState = {};
window._maxFontState = {};

async function toggleMaxFit(panelId) {
    const { panelObj, vttyEl } = _findPanelVtty(panelId);
    if (!panelObj || !vttyEl) return;
    const st = _maxFitState[panelId];
    const btnIds = ['stMaxFitBtn', 'maxFitBtn-' + panelId];
    if (st?.active) {
        st.active = false;
        _setToggleBtn(btnIds, false, 'Auto-fit terminal to panel', 'Restore previous size');
        if (!(await _resizePanelTo(panelId, st.prevRows, st.prevCols))) delete _maxFitState[panelId];
    } else {
        const rect = vttyEl.getBoundingClientRect();
        if (rect.width < 10 || rect.height < 10) return;
        const cmd = _findCmd(panelObj.selectedInstUrl, panelObj.selectedCmdId);
        if (panelObj.selectedCmdId && cmd?.status === 'exited') return;
        const fs = panelObj.fontSize || state.fontSize;
        const maxCols = Math.max(20, Math.min(500, Math.floor(rect.width / (fs * 0.6))));
        const maxRows = Math.max(5, Math.min(200, Math.floor(rect.height / (fs * 1.2))));
        const { rows: curRows, cols: curCols } = _getResizeDims(panelId);
        _maxFitState[panelId] = { prevRows: curRows, prevCols: curCols, active: true };
        _setToggleBtn(btnIds, true, 'Auto-fit terminal to panel', 'Restore previous size');
        if (!(await _resizePanelTo(panelId, maxRows, maxCols))) delete _maxFitState[panelId];
    }
}

async function toggleMaxFont(panelId) {
    const { panelObj, vttyEl } = _findPanelVtty(panelId);
    if (!panelObj || !vttyEl) return;
    const st = _maxFontState[panelId];
    const btnIds = ['stMaxFontBtn', 'maxFontBtn-' + panelId];
    const { rows: curRows, cols: curCols } = _getResizeDims(panelId);
    if (st?.active) {
        st.active = false;
        _setToggleBtn(btnIds, false, 'Maximize font to fit', 'Restore previous font size');
        panelObj.fontSize = st.prevFontSize;
        localStorage.setItem('vrw_panel_font_' + panelId, String(panelObj.fontSize));
        vttyEl.style.fontSize = panelObj.fontSize + 'px';
        vttyEl.classList.toggle('thin-scrollbar', panelObj.fontSize < 10);
        delete _maxFontState[panelId];
    } else {
        const rect = vttyEl.getBoundingClientRect();
        if (rect.width < 10 || rect.height < 10) return;
        const maxFont = Math.max(8, Math.min(28, Math.min(Math.floor(rect.width / (curCols * 0.6)), Math.floor(rect.height / (curRows * 1.2)))));
        _maxFontState[panelId] = { prevFontSize: panelObj.fontSize, active: true };
        _setToggleBtn(btnIds, true, 'Maximize font to fit', 'Restore previous font size');
        panelObj.fontSize = maxFont;
        localStorage.setItem('vrw_panel_font_' + panelId, String(panelObj.fontSize));
        vttyEl.style.fontSize = panelObj.fontSize + 'px';
        vttyEl.classList.toggle('thin-scrollbar', panelObj.fontSize < 10);
    }
}

    // ── Exports ──
    Object.assign(window, {
        sendKeysToPanel, showSpecialKeysHelp,
        closeSpecialKeysModal: function() { releaseCurrentFocusTrap(); const m = document.getElementById('specialKeysModal'); if (m) m.remove(); },
        copyTerminalSelection, exportTerminal, screenshotPanel,
        copyCommandUrl, togglePauseCmd,
        autoFitActiveTerminal, toggleMaxFit, toggleMaxFont,
        _showCopyFeedback,
    });
})();