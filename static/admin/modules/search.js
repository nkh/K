// ─── Search ───
// Terminal search (within a single VTTY panel), global search (across all commands),
// command manager dialog, scroll-to-bottom, and panel freeze/thaw for search stability.
(function() {
    'use strict';

// ─── Terminal Search ───
const vttySearchState = { matchIndex: 0, matches: [], panelId: null };

function vttySearch(panelId) {
    const input = document.getElementById('searchInput-' + panelId);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (!input || !countEl) return;
    const query = input.value;
    vttySearchState.panelId = panelId;
    vttySearchState.matchIndex = 0;

    const panel = document.getElementById(panelId);
    const pre = panel ? panel.querySelector('pre') : null;
    if (!pre) { countEl.textContent = '0/0'; return; }

    vttyRemoveHighlights(pre);

    if (!query) {
        vttySearchState.matches = [];
        countEl.textContent = '';
        return;
    }

    const text = pre.textContent || '';
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    vttySearchState.matches = [];

    let pos = 0;
    while ((pos = lowerText.indexOf(lowerQuery, pos)) !== -1) {
        vttySearchState.matches.push(pos);
        pos += lowerQuery.length;
    }

    if (vttySearchState.matches.length > 0) {
        vttyApplyHighlights(pre, text, query);
        vttyScrollToMatch(panelId, 0);
        countEl.textContent = '1/' + vttySearchState.matches.length;
        _updateSearchProgress(panelId, 0, vttySearchState.matches.length);
    } else {
        countEl.textContent = '0/0';
        _updateSearchProgress(panelId, 0, 0);
    }
}

function vttyApplyHighlights(pre, text, query) {
    let html = pre.innerHTML;
    const escaped = escHtml(query);
    const regex = new RegExp('(?![^<]*>)(' + escaped.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ')', 'gi');
    const matches = [];
    let m;
    while ((m = regex.exec(html)) !== null) matches.push({ index: m.index, text: m[1] });

    for (let i = matches.length - 1; i >= 0; i--) {
        const { index: idx, text: matchText } = matches[i];
        const endIdx = html.indexOf(matchText, idx) + matchText.length;
        if (endIdx <= idx) continue;
        const cls = i === 0 ? 'vtty-search-highlight current' : 'vtty-search-highlight';
        html = html.substring(0, idx) + '<mark class="' + cls + '" data-match-idx="' + i + '">' + html.substring(idx, endIdx) + '</mark>' + html.substring(endIdx);
    }
    pre.innerHTML = html;
}

function vttyRemoveHighlights(pre) {
    const marks = pre.querySelectorAll('mark.vtty-search-highlight');
    marks.forEach(mark => {
        const parent = mark.parentNode;
        parent.replaceChild(document.createTextNode(mark.textContent), mark);
        parent.normalize();
    });
}

function vttyScrollToMatch(panelId, idx) {
    const panel = document.getElementById(panelId);
    if (!panel) return;
    const marks = panel.querySelectorAll('mark.vtty-search-highlight');
    const cur = panel.querySelector('mark.vtty-search-highlight.current');
    if (cur) cur.classList.remove('current');
    if (marks[idx]) {
        marks[idx].classList.add('current');
        marks[idx].scrollIntoView({ block: 'center', behavior: 'smooth' });
    }
}

function _vttySearchStep(panelId, direction) {
    if (vttySearchState.matches.length === 0) return;
    const total = vttySearchState.matches.length;
    vttySearchState.matchIndex = (vttySearchState.matchIndex + direction + total) % total;
    vttyScrollToMatch(panelId, vttySearchState.matchIndex);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = (vttySearchState.matchIndex + 1) + '/' + total;
    _updateSearchProgress(panelId, vttySearchState.matchIndex, total);
}

function vttySearchNext(panelId) { _vttySearchStep(panelId, 1); }
function vttySearchPrev(panelId) { _vttySearchStep(panelId, -1); }

function _updateSearchProgress(panelId, currentIdx, totalMatches) {
    const bar = document.getElementById('searchProgress-' + panelId);
    if (!bar || totalMatches <= 1) { if (bar) bar.classList.add('hidden'); return; }
    bar.classList.remove('hidden');
    const pct = ((currentIdx + 1) / totalMatches) * 100;
    bar.style.background = `linear-gradient(to right, var(--accent) ${pct}%, var(--border) ${pct}%)`;
}

function vttySearchClose(panelId) {
    releaseCurrentFocusTrap();
    const panel = document.getElementById(panelId);
    const searchBar = document.getElementById('searchBar-' + panelId);
    if (searchBar) searchBar.classList.remove('visible');
    const pre = panel?.querySelector('pre');
    if (pre) vttyRemoveHighlights(pre);
    vttySearchState.matches = [];
    vttySearchState.matchIndex = 0;
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = '';
    const vtty = panel?.querySelector('.vtty-container');
    if (vtty) vtty.focus();
}

// ─── Scroll to Bottom ───
function scrollTerminalBottom(panelId) {
    const isSecondary = panelId.endsWith('-secondary');
    if (isSecondary) {
        const primaryPanelId = panelId.slice(0, -'-secondary'.length);
        const vtty = document.getElementById('vtty-' + panelId);
        if (vtty) vtty.scrollTop = vtty.scrollHeight;
        const panelObj = state.panels.find(p => p.id === primaryPanelId);
        if (panelObj && panelObj.split && panelObj.split.secondaryScrollbackOffset > 0) {
            panelObj.split.secondaryScrollbackOffset = 0;
            if (panelObj.split.secondaryCmdId) _loadSecondaryVttyHttp(panelObj);
        }
        return;
    }

    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const vtty = panelEl.querySelector('.vtty-container');
    if (vtty) vtty.scrollTop = vtty.scrollHeight;
    const panelObj = state.panels.find(p => p.id === panelId);
    if (panelObj && panelObj.scrollbackOffset > 0) {
        panelObj.scrollbackOffset = 0;
        if (state.selectedCmdId) sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
        const sbIndicator = document.getElementById('scrollbackIndicator');
        if (sbIndicator) sbIndicator.classList.add('hidden');
        if (state.selectedCmdId && panelObj.selectedInstUrl) {
            loadVttyHttpForPanel(panelObj.id, panelObj.selectedInstUrl, state.selectedCmdId);
        }
    }
}

// ─── Global Search ───
function _freezeAllPanelsForSearch() {
    state._searchFrozenPanelIds.clear();
    state._searchFrozenCmdIds = [];
    for (const panel of state.panels) {
        if (panel.selectedInstUrl && panel.selectedCmdId) {
            stopPanelUpdateMode(panel.id);
            state._searchFrozenPanelIds.add(panel.id);
        }
    }
}

async function _thawAllPanelsFromSearch() {
    for (const panelId of state._searchFrozenPanelIds) {
        const panelObj = state.panels.find(p => p.id === panelId);
        if (panelObj && panelObj.selectedInstUrl && panelObj.selectedCmdId) {
            startPanelUpdateMode(panelId);
        }
    }
    state._searchFrozenPanelIds.clear();
    for (const entry of state._searchFrozenCmdIds) {
        try { await api.thaw(entry.instUrl, entry.cmdId); } catch (e) {}
    }
    state._searchFrozenCmdIds = [];
}

// ─── Command Manager Dialog ───
function closeCmdManager() {
    document.getElementById('cmdManagerModal').classList.add('hidden');
}

function renderCmdManagerList() {
    const container = document.getElementById('cmdManagerList');
    const filter = (document.getElementById('cmdManagerFilter').value || '').toLowerCase();
    const sortBy = document.getElementById('cmdManagerSort').value;
    const footer = document.getElementById('cmdManagerFooter');

    let cmds = [];
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            const res = state._resourceCache[cmd.id] || {};
            cmds.push({ ...cmd, instUrl: inst.url, cpu: res.cpu_percent || 0, mem: res.memory_mb || 0 });
        }
    }

    if (filter) {
        cmds = cmds.filter(c => {
            const name = (c.name || c.id).toLowerCase();
            const args = (c.args || []).join(' ').toLowerCase();
            return name.includes(filter) || args.includes(filter);
        });
    }

    if (sortBy === 'name') cmds.sort((a, b) => (a.name || a.id).localeCompare(b.name || b.id));
    else if (sortBy === 'runtime') cmds.sort((a, b) => (b.runtime_secs || 0) - (a.runtime_secs || 0));
    else if (sortBy === 'cpu') cmds.sort((a, b) => b.cpu - a.cpu);
    else if (sortBy === 'mem') cmds.sort((a, b) => b.mem - a.mem);

    const alive = cmds.filter(c => c.alive !== false).length;
    const total = cmds.length;
    const totalCpu = cmds.reduce((s, c) => s + c.cpu, 0);
    const totalMem = cmds.reduce((s, c) => s + c.mem, 0);
    footer.textContent = total + ' commands (' + alive + ' running) | CPU: ' + totalCpu.toFixed(1) + '% | Mem: ' + totalMem.toFixed(1) + 'MB';

    if (cmds.length === 0) {
        container.innerHTML = '<div class="cmd-manager-empty">No commands found</div>';
        return;
    }

    let html = '<div class="cmd-manager-header"><span class="cm-col cm-name">Name</span><span class="cm-col cm-status">Status</span><span class="cm-col cm-runtime">Runtime</span><span class="cm-col cm-res">CPU</span><span class="cm-col cm-res">Mem</span><span class="cm-col cm-server">Server</span><span class="cm-col cm-actions">Actions</span></div>';
    for (const cmd of cmds) {
        const isAlive = cmd.alive !== false;
        const name = cmd.name || cmd.id;
        const args = (cmd.args || []).join(' ');
        const runtime = cmd.runtime_secs != null ? formatRuntime(cmd.runtime_secs) : '-';
        const statusClass = isAlive ? 'cm-running' : 'cm-exited';
        const statusText = isAlive ? (cmd.frozen ? 'frozen' : 'running') : ('exit ' + (cmd.exit_code != null ? cmd.exit_code : '?'));
        const kept = cmd.exit && cmd.exit.retain_on_exit;
        const pinned = getPinnedNames().includes(name);
        const serverLabel = cmd.instUrl.replace(/^https?:\/\//, '').replace(/\/$/, '');

        html += `<div class="cmd-manager-row${isAlive ? '' : ' cm-row-dead'}" data-cmd-id="${escHtml(cmd.id)}" data-inst-url="${escHtml(cmd.instUrl)}">
            <span class="cm-col cm-name" title="${escHtml(name + (args ? ' ' + args : ''))}"><span class="cm-cmd-name">${escHtml(name)}</span>${args ? '<span class="cm-cmd-args">' + escHtml(args) + '</span>' : ''}</span>
            <span class="cm-col cm-status ${statusClass}">${statusText}</span>
            <span class="cm-col cm-runtime">${escHtml(runtime)}</span>
            <span class="cm-col cm-res">${cmd.cpu.toFixed(1)}%</span>
            <span class="cm-col cm-res">${cmd.mem.toFixed(1)}MB</span>
            <span class="cm-col cm-server" title="${escHtml(cmd.instUrl)}">${escHtml(serverLabel)}</span>
            <span class="cm-col cm-actions">
                ${isAlive ? `<button class="btn btn-xs" data-action="RestartCommandById" data-inst-url="${escHtml(cmd.instUrl)}" data-cmd-id="${escHtml(cmd.id)}" title="Restart">&#x21BB;</button>` : ''}
                ${isAlive ? `<button class="btn btn-xs" data-action="ToggleKeepCmd" data-inst-url="${escHtml(cmd.instUrl)}" data-cmd-id="${escHtml(cmd.id)}" title="${kept ? 'Unkeep' : 'Keep'}">${kept ? '★' : '☆'}</button>` : ''}
                <button class="btn btn-xs ${pinned ? 'btn-primary' : ''}" data-action="TogglePinCmd" data-cmd-name="${escHtml(name)}" title="Pin/Unpin">${pinned ? '◉' : '◎'}</button>
                ${isAlive ? `<button class="btn btn-xs btn-danger" data-action="KillCommand" data-inst-url="${escHtml(cmd.instUrl)}" data-cmd-id="${escHtml(cmd.id)}" title="Kill">&#x2715;</button>` : ''}
                <button class="btn btn-xs" data-action="SelectAndViewCmd" data-inst-url="${escHtml(cmd.instUrl)}" data-cmd-id="${escHtml(cmd.id)}" data-cmd-name="${escHtml(name)}" title="View">&#x25B6;</button>
            </span>
        </div>`;
    }
    container.innerHTML = html;
}

async function cmdManagerKillAll() {
    if (!confirm('Kill all running commands on all servers?')) return;
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive !== false) {
                try { await api.purge(inst.url, cmd.id); } catch {}
            }
        }
    }
    loadCommands();
    renderCmdManagerList();
}

function openGlobalSearch() {
    _freezeAllPanelsForSearch();
    const modal = document.getElementById('globalSearchModal');
    modal.classList.remove('hidden');
    const input = document.getElementById('globalSearchInput');
    input.value = '';
    input.focus();
    document.getElementById('searchFreezeToggle').checked = false;
    document.getElementById('globalSearchResults').innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;font-size:0.75rem;">Type a query and press Enter to search across all command output</div>';
}

function closeGlobalSearch() {
    document.getElementById('globalSearchModal').classList.add('hidden');
    _thawAllPanelsFromSearch();
}

async function _toggleSearchFreezeCommands() {
    const freeze = document.getElementById('searchFreezeToggle').checked;
    if (freeze) {
        for (const inst of state.connections) {
            if (!inst._commands) continue;
            for (const cmd of inst._commands) {
                if (!cmd.alive || cmd.frozen) continue;
                try {
                    await api.freeze(inst.url, cmd.id);
                    state._searchFrozenCmdIds.push({ instUrl: inst.url, cmdId: cmd.id, wasFrozen: false });
                } catch (e) {}
            }
        }
    } else {
        for (const entry of state._searchFrozenCmdIds) {
            if (!entry.wasFrozen) {
                try { await api.thaw(entry.instUrl, entry.cmdId); } catch (e) {}
            }
        }
        state._searchFrozenCmdIds = [];
    }
}

function onSearchResultClick(instUrl, cmdId, cmdName) {
    document.getElementById('globalSearchModal').classList.add('hidden');
    const activePanelId = getActivePanelId();
    selectCommand(instUrl, cmdId, cmdName);

    // Thaw all OTHER panels and commands, but keep the selected panel frozen
    for (const panelId of state._searchFrozenPanelIds) {
        if (panelId !== activePanelId) {
            const panelObj = state.panels.find(p => p.id === panelId);
            if (panelObj && panelObj.selectedInstUrl && panelObj.selectedCmdId) {
                startPanelUpdateMode(panelId);
            }
        }
    }
    for (const entry of state._searchFrozenCmdIds) {
        if (!entry.wasFrozen) api.thaw(entry.instUrl, entry.cmdId).catch(() => {});
    }
    state._searchFrozenCmdIds = [];
    state._searchFrozenPanelIds.clear();
    if (activePanelId) state._searchFrozenPanelIds.add(activePanelId);

    updateFrozenIndicator();
}

function updateFrozenIndicator() {
    document.querySelectorAll('.search-frozen-indicator').forEach(el => el.remove());
    for (const panelId of state._searchFrozenPanelIds) {
        const panelEl = document.getElementById(panelId);
        if (!panelEl) continue;
        const indicator = document.createElement('div');
        indicator.className = 'search-frozen-indicator';
        indicator.textContent = 'VTTY frozen (click to unfreeze)';
        indicator.onclick = () => {
            state._searchFrozenPanelIds.delete(panelId);
            indicator.remove();
            const panelObj = state.panels.find(p => p.id === panelId);
            if (panelObj && panelObj.selectedInstUrl && panelObj.selectedCmdId) {
                startPanelUpdateMode(panelId);
            }
        };
        panelEl.style.position = 'relative';
        panelEl.appendChild(indicator);
    }
}

async function executeGlobalSearch() {
    const query = document.getElementById('globalSearchInput').value.trim();
    if (!query) return;
    const resultsContainer = document.getElementById('globalSearchResults');
    resultsContainer.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;font-size:0.75rem;">Searching...</div>';
    let allResults = [];
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            try {
                const json = await api.getVttyText(inst.url, cmd.id);
                if (json.status !== 'ok' || !json.data || !json.data.text) continue;
                const lines = json.data.text.split('\n');
                const cmdName = cmd.name || cmd.id;
                const matchingLines = [];
                lines.forEach((line, idx) => {
                    if (line.toLowerCase().includes(query.toLowerCase())) {
                        matchingLines.push({ lineNum: idx + 1, text: line.trim() });
                    }
                });
                if (matchingLines.length > 0) {
                    allResults.push({ cmdName, cmdId: cmd.id, instUrl: inst.url, lines: matchingLines.slice(0, 50) });
                }
            } catch (e) {}
        }
    }
    if (allResults.length === 0) {
        resultsContainer.innerHTML = '<div style="padding:1rem;color:var(--text-muted);text-align:center;font-size:0.75rem;">No results found</div>';
        return;
    }
    resultsContainer.innerHTML = allResults.map(group => `
        <div class="search-result-group">
            <div class="search-result-header" data-action="OnSearchResultClick" data-inst-url="${escHtml(group.instUrl)}" data-cmd-id="${escHtml(group.cmdId)}" data-cmd-name="${escHtml(group.cmdName)}">
                ${escHtml(group.cmdName)} <span style="color:var(--text-muted);font-size:0.6rem;">(${group.lines.length} matches)</span>
            </div>
            ${group.lines.map(l => `<div class="search-result-line" title="${escHtml(l.text)}"><span style="color:var(--text-muted);">${l.lineNum}:</span> ${escHtml(l.text)}</div>`).join('')}
        </div>
    `).join('');
}

Object.assign(window, {
    vttySearch, vttyApplyHighlights, vttyRemoveHighlights,
    vttySearchClose, vttySearchNext, vttySearchPrev,
    scrollTerminalBottom, openGlobalSearch, closeGlobalSearch,
    executeGlobalSearch, onSearchResultClick,
    updateFrozenIndicator, _toggleSearchFreezeCommands,
    closeCmdManager, renderCmdManagerList, cmdManagerKillAll,
    _selectAndViewCmd(instUrl, cmdId, cmdName) {
        selectCommand(instUrl, cmdId, cmdName);
        closeCmdManager();
    },
});
})();
