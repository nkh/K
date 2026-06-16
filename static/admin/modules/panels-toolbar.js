// ─── Panels: Toolbar ───
(function() {
    'use strict';

function _setText(id, text) { const el = document.getElementById(id); if (el) el.textContent = text; }

function _setToggleBtn(ids, active, offTitle, onTitle) {
    const btn = ids.map(id => document.getElementById(id)).find(Boolean);
    if (btn) { btn.classList.toggle('btn-primary', active); btn.title = active ? onTitle : offTitle; }
}

function focusPanel(panelId) {
    if (state._focusedPanelId === panelId) return;
    state._focusedPanelId = panelId;
    document.querySelectorAll('.panel').forEach(el => el.classList.toggle('focused', el.id === panelId));
    if (state._mobileTabbedLayout) {
        document.querySelectorAll('.panel').forEach(el => el.classList.toggle('hidden', el.id !== panelId));
        document.querySelectorAll('.mobile-tab').forEach(el => el.classList.toggle('active', el.getAttribute('data-panel') === panelId));
    }
    const panelObj = state.panels.find(p => p.id === panelId);
    if (panelObj) { state.selectedInstUrl = panelObj.selectedInstUrl; state.selectedCmdId = panelObj.selectedCmdId; }
    updateSharedToolbar();
}

function updateSharedToolbar() {
    const pid = getActivePanelId();
    const panelObj = state.panels.find(p => p.id === pid);
    if (!panelObj) return;
    _setText('stFontSize', panelObj.fontSize + 'px');
    const themeBtn = document.getElementById('stPanelThemeBtn');
    if (themeBtn) {
        themeBtn.textContent = panelObj.theme === 'light' ? '\u263E' : panelObj.theme === 'dark' ? '\u2600' : '\u25D0';
        themeBtn.title = 'Panel theme: ' + (panelObj.theme || 'inherit') + ' (click to toggle)';
    }
    const selectBtn = document.getElementById('stSelectBtn');
    if (selectBtn) { selectBtn.classList.toggle('btn-primary', panelObj.selectionMode); selectBtn.textContent = panelObj.selectionMode ? '\u2713 Select' : 'Select'; }
    _setText('stInstanceUrl', (panelObj.selectedInstUrl || '').replace(/^https?:\/\//, ''));
    _setText('stRefreshVal', state.refreshMs || 'off');
    const bufferSel = document.getElementById('stBufferSelect');
    if (bufferSel) bufferSel.value = state.bufferView || 'current';
    const resourceBadge = document.getElementById('stResourceBadge');
    if (resourceBadge && panelObj.selectedCmdId) {
        const res = state._resourceCache[panelObj.selectedCmdId];
        if (state.showResources && res && (res.cpu_percent != null || res.memory_mb != null)) {
            resourceBadge.classList.remove('hidden');
            resourceBadge.textContent = (res.cpu_percent != null ? 'CPU ' + res.cpu_percent.toFixed(1) + '%' : '') + (res.memory_mb != null ? ' MEM ' + res.memory_mb.toFixed(1) + 'MB' : '');
        } else { resourceBadge.classList.add('hidden'); }
    }
    const restartBtn = document.getElementById('stRestartBtn');
    if (restartBtn) restartBtn.classList.toggle('hidden', !panelObj.selectedCmdId);
    _setToggleBtn(['stMaxFitBtn', 'maxFitBtn-' + pid], !!(_maxFitState[pid]?.active), 'Auto-fit terminal to panel', 'Restore previous size');
    _setToggleBtn(['stMaxFontBtn', 'maxFontBtn-' + pid], !!(_maxFontState[pid]?.active), 'Maximize font to fit', 'Restore previous font size');
}

let _panelDelegated = false;
function _setupPanelDelegation() {
    if (_panelDelegated) return;
    _panelDelegated = true;
    const container = document.getElementById('view-vtty');
    if (!container) return;

    container.addEventListener('mousedown', (e) => {
        const divider = e.target.closest('.split-divider');
        if (divider) {
            e.preventDefault();
            const pid = divider.getAttribute('data-panel');
            const panelObj = state.panels.find(p => p.id === pid);
            if (!panelObj?.split) return;
            const splitContainer = divider.parentElement;
            const dir = panelObj.split.direction;
            divider.classList.add('active');
            const startPos = dir === 'horizontal' ? e.clientX : e.clientY;
            const cSize = dir === 'horizontal' ? splitContainer.offsetWidth : splitContainer.offsetHeight;
            const startRatio = panelObj.split.splitRatio || 0.5;
            const onMove = (ev) => {
                const pos = dir === 'horizontal' ? ev.clientX : ev.clientY;
                let ratio = Math.max(0.1, Math.min(0.9, startRatio + (pos - startPos) / cSize));
                panelObj.split.splitRatio = ratio;
                const panes = splitContainer.querySelectorAll('.split-pane');
                if (panes.length === 2) {
                    const p1 = (ratio * 100).toFixed(1), p2 = (100 - parseFloat(p1)).toFixed(1);
                    panes[0].style.flex = `0 0 ${p1}%`; panes[1].style.flex = `0 0 ${p2}%`;
                }
            };
            const onUp = () => { divider.classList.remove('active'); document.removeEventListener('mousemove', onMove); document.removeEventListener('mouseup', onUp); };
            document.addEventListener('mousemove', onMove);
            document.addEventListener('mouseup', onUp);
            return;
        }
        const panelEl = e.target.closest('.panel');
        if (!panelEl) return;
        const pid = panelEl.id;
        focusPanel(pid);
        const el = e.target.closest('.vtty-container') || e.target.closest('.split-header');
        if (el) {
            const side = el.getAttribute('data-split-side');
            if (side) { const p = state.panels.find(pp => pp.id === pid); if (p?.split) p.split.activeSide = side; }
        }
    });

    container.addEventListener('scroll', (e) => {
        const vtty = e.target.closest('.vtty-container');
        if (!vtty) return;
        const btn = vtty.querySelector('.scroll-bottom-btn');
        if (btn) btn.classList.toggle('visible', vtty.scrollHeight - vtty.scrollTop - vtty.clientHeight >= 50);
    }, true);
}

    // ── Exports ──
    Object.assign(window, {
        focusPanel, updateSharedToolbar,
        _setupPanelDelegation, _setToggleBtn,
    });
})();