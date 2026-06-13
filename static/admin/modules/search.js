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

    // Get the text content of the terminal
    const panel = document.getElementById(panelId);
    const pre = panel ? panel.querySelector('pre') : null;
    if (!pre) { countEl.textContent = '0/0'; return; }

    // Remove previous highlights
    vttyRemoveHighlights(pre);

    if (!query) {
        vttySearchState.matches = [];
        countEl.textContent = '';
        return;
    }

    // Find all text nodes and mark matches
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
    // Walk through text and highlight matches
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    const fragment = document.createDocumentFragment();
    let lastIdx = 0;
    let matchIdx = 0;
    let pos = 0;

    // We need to rebuild using the pre's innerHTML which has spans for ANSI
    // Instead, work at the text level using a tree walker on text nodes
    const walker = document.createTreeWalker(pre, NodeFilter.SHOW_TEXT, null);
    const textNodes = [];
    while (walker.nextNode()) textNodes.push(walker.currentNode);

    if (textNodes.length === 0) return;

    // Simple approach: highlight by rebuilding innerHTML with mark spans
    // Get the full innerHTML and do string replacement on text portions
    let html = pre.innerHTML;
    const escaped = escHtml(query);
    // Use a regex that matches the query text (case insensitive) but only
    // within text content, not inside HTML tags
    const regex = new RegExp('(?![^<]*>)(' + escaped.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + ')', 'gi');
    const results = [];
    let match;
    while ((match = regex.exec(html)) !== null) {
        results.push(match.index);
    }

    // Apply highlights in reverse order to preserve indices
    for (let i = results.length - 1; i >= 0; i--) {
        const idx = results[i];
        const originalLen = query.length;
        // Find the end of the match in the HTML
        const endIdx = html.indexOf(match[1], idx) + match[1].length;
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
    const mark = panel.querySelector('mark.vtty-search-highlight.current');
    if (mark) mark.classList.remove('current');
    const marks = panel.querySelectorAll('mark.vtty-search-highlight');
    if (marks[idx]) {
        marks[idx].classList.add('current');
        marks[idx].scrollIntoView({ block: 'center', behavior: 'smooth' });
    }
}

function vttySearchNext(panelId) {
    if (vttySearchState.matches.length === 0) return;
    vttySearchState.matchIndex = (vttySearchState.matchIndex + 1) % vttySearchState.matches.length;
    vttyScrollToMatch(panelId, vttySearchState.matchIndex);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = (vttySearchState.matchIndex + 1) + '/' + vttySearchState.matches.length;
    _updateSearchProgress(panelId, vttySearchState.matchIndex, vttySearchState.matches.length);
}

function vttySearchPrev(panelId) {
    if (vttySearchState.matches.length === 0) return;
    vttySearchState.matchIndex = (vttySearchState.matchIndex - 1 + vttySearchState.matches.length) % vttySearchState.matches.length;
    vttyScrollToMatch(panelId, vttySearchState.matchIndex);
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = (vttySearchState.matchIndex + 1) + '/' + vttySearchState.matches.length;
    _updateSearchProgress(panelId, vttySearchState.matchIndex, vttySearchState.matches.length);
}

function _updateSearchProgress(panelId, currentIdx, totalMatches) {
    const bar = document.getElementById('searchProgress-' + panelId);
    if (!bar) return;
    if (totalMatches <= 1) {
        bar.classList.add('hidden');
        return;
    }
    bar.classList.remove('hidden');
    const pct = ((currentIdx + 1) / totalMatches) * 100;
    bar.style.background = `linear-gradient(to right, var(--accent) ${pct}%, var(--border) ${pct}%)`;
}

function vttySearchClose(panelId) {
    releaseCurrentFocusTrap();
    const searchBar = document.getElementById('searchBar-' + panelId);
    if (searchBar) searchBar.classList.remove('visible');
    const panel = document.getElementById(panelId);
    const pre = panel ? panel.querySelector('pre') : null;
    if (pre) vttyRemoveHighlights(pre);
    vttySearchState.matches = [];
    vttySearchState.matchIndex = 0;
    const countEl = document.getElementById('searchCount-' + panelId);
    if (countEl) countEl.textContent = '';
    // Return focus to the VTTY container
    if (panel) {
        const vtty = panel.querySelector('.vtty-container');
        if (vtty) vtty.focus();
    }
}


// ─── Scroll to Bottom ───
function scrollTerminalBottom(panelId) {
    // Check if this is a secondary pane of a split panel
    const isSecondary = panelId.endsWith('-secondary');
    if (isSecondary) {
        const primaryPanelId = panelId.slice(0, -'-secondary'.length);
        const vtty = document.getElementById('vtty-' + panelId);
        if (vtty) {
            vtty.scrollTop = vtty.scrollHeight;
        }
        const panelObj = state.panels.find(p => p.id === primaryPanelId);
        if (panelObj && panelObj.split && panelObj.split.secondaryScrollbackOffset > 0) {
            panelObj.split.secondaryScrollbackOffset = 0;
            if (panelObj.split.secondaryCmdId) {
                _loadSecondaryVttyHttp(panelObj);
            }
        }
        return;
    }

    const panelEl = document.getElementById(panelId);
    if (!panelEl) return;
    const vtty = panelEl.querySelector('.vtty-container');
    if (vtty) {
        vtty.scrollTop = vtty.scrollHeight;
    }
    // Reset scrollback offset and re-fetch
    const panelObj = state.panels.find(p => p.id === panelId);
    if (panelObj && panelObj.scrollbackOffset > 0) {
        panelObj.scrollbackOffset = 0;
        // Clear stored scrollback since we reset
        if (state.selectedCmdId) {
            sessionStorage.removeItem('vrw_scrollback_' + state.selectedCmdId);
        }
        const sbIndicator = document.getElementById('scrollbackIndicator');
        if (sbIndicator) sbIndicator.classList.add('hidden');
        if (state.selectedCmdId && panelObj.selectedInstUrl) {
            loadVttyHttp(panelObj.selectedInstUrl, state.selectedCmdId);
        }
    }
}


// ─── Global Search ───
// When the search overlay opens, all panel VTTY updates are paused so text
// doesn't shift under the user's eyes. Optionally, the commands themselves
// can be frozen (SIGSTOP). On cancel, everything resumes. On result click,
// the selected panel stays frozen so the matched text remains stable.

function _freezeAllPanelsForSearch() {
    _searchFrozenPanelIds.clear();
    _searchFrozenCmdIds = [];
    for (const panel of state.panels) {
        if (panel.selectedInstUrl && panel.selectedCmdId) {
            stopPanelUpdateMode(panel.id);
            _searchFrozenPanelIds.add(panel.id);
        }
    }
}

async function _thawAllPanelsFromSearch() {
    for (const panelId of _searchFrozenPanelIds) {
        const panelObj = state.panels.find(p => p.id === panelId);
        if (panelObj && panelObj.selectedInstUrl && panelObj.selectedCmdId) {
            startPanelUpdateMode(panelId);
        }
    }
    _searchFrozenPanelIds.clear();
    // Thaw any commands that were frozen during search
    for (const entry of _searchFrozenCmdIds) {
        try {
            await api.thaw(entry.instUrl, entry.cmdId);
        } catch (e) { /* ignore */ }
    }
    _searchFrozenCmdIds = [];
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

    // Collect all commands across all instances
    let cmds = [];
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            const res = state._resourceCache[cmd.id] || {};
            cmds.push({ ...cmd, instUrl: inst.url, cpu: res.cpu_percent || 0, mem: res.memory_mb || 0 });
        }
    }

    // Filter
    if (filter) {
        cmds = cmds.filter(c => {
            const name = (c.name || c.id).toLowerCase();
            const args = (c.args || []).join(' ').toLowerCase();
            return name.includes(filter) || args.includes(filter);
        });
    }

    // Sort
    if (sortBy === 'name') cmds.sort((a, b) => (a.name || a.id).localeCompare(b.name || b.id));
    else if (sortBy === 'runtime') cmds.sort((a, b) => (b.runtime_secs || 0) - (a.runtime_secs || 0));
    else if (sortBy === 'cpu') cmds.sort((a, b) => b.cpu - a.cpu);
    else if (sortBy === 'mem') cmds.sort((a, b) => b.mem - a.mem);

    // Stats
    const alive = cmds.filter(c => c.alive !== false).length;
    const total = cmds.length;
    const totalCpu = cmds.reduce((s, c) => s + c.cpu, 0);
    const totalMem = cmds.reduce((s, c) => s + c.mem, 0);
    footer.textContent = total + ' commands (' + alive + ' running) | CPU: ' + totalCpu.toFixed(1) + '% | Mem: ' + totalMem.toFixed(1) + 'MB';

    // Render rows
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
        const exitCode = cmd.exit_code;
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
    const modal = document.getElementById('globalSearchModal');
    modal.classList.add('hidden');
    _thawAllPanelsFromSearch();
}

async function _toggleSearchFreezeCommands() {
    const freeze = document.getElementById('searchFreezeToggle').checked;
    if (freeze) {
        // Freeze all running commands across all servers
        for (const inst of state.connections) {
            if (!inst._commands) continue;
            for (const cmd of inst._commands) {
                if (!cmd.alive || cmd.frozen) continue;
                try {
                    await api.freeze(inst.url, cmd.id);
                    _searchFrozenCmdIds.push({ instUrl: inst.url, cmdId: cmd.id, wasFrozen: false });
                } catch (e) { /* skip */ }
            }
        }
    } else {
        // Thaw all commands we froze
        for (const entry of _searchFrozenCmdIds) {
            if (!entry.wasFrozen) {
                try {
                    await api.thaw(entry.instUrl, entry.cmdId);
                } catch (e) { /* ignore */ }
            }
        }
        _searchFrozenCmdIds = [];
    }
}

function onSearchResultClick(instUrl, cmdId, cmdName) {
    const modal = document.getElementById('globalSearchModal');
    modal.classList.add('hidden');

    // Select the command in the focused panel
    const activePanelId = getActivePanelId();
    selectCommand(instUrl, cmdId, cmdName);

    // Thaw all OTHER panels and commands, but keep the selected panel frozen
    const keepFrozenId = activePanelId;
    for (const panelId of _searchFrozenPanelIds) {
        if (panelId !== keepFrozenId) {
            const panelObj = state.panels.find(p => p.id === panelId);
            if (panelObj && panelObj.selectedInstUrl && panelObj.selectedCmdId) {
                startPanelUpdateMode(panelId);
            }
        }
    }
    // Thaw all frozen commands
    for (const entry of _searchFrozenCmdIds) {
        if (!entry.wasFrozen) {
            api.thaw(entry.instUrl, entry.cmdId).catch(() => {});
        }
    }
    _searchFrozenCmdIds = [];
    _searchFrozenPanelIds.clear();

    // Keep only the active panel frozen
    if (keepFrozenId) {
        _searchFrozenPanelIds.add(keepFrozenId);
    }

    // Show a frozen indicator on the panel so the user knows updates are paused
    updateFrozenIndicator();
}

function updateFrozenIndicator() {
    // Remove any existing frozen indicators
    document.querySelectorAll('.search-frozen-indicator').forEach(el => el.remove());
    for (const panelId of _searchFrozenPanelIds) {
        const panelEl = document.getElementById(panelId);
        if (!panelEl) continue;
        const indicator = document.createElement('div');
        indicator.className = 'search-frozen-indicator';
        indicator.textContent = 'VTTY frozen (click to unfreeze)';
        indicator.onclick = () => {
            _searchFrozenPanelIds.delete(panelId);
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
            } catch (e) { /* skip */ }
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

    // Expose to global scope
    window.vttySearch = vttySearch;
    window.vttyApplyHighlights = vttyApplyHighlights;
    window.vttyRemoveHighlights = vttyRemoveHighlights;
    window.vttySearchClose = vttySearchClose;
    window.vttySearchNext = vttySearchNext;
    window.vttySearchPrev = vttySearchPrev;
    window.scrollTerminalBottom = scrollTerminalBottom;
    window.openGlobalSearch = openGlobalSearch;
    window.closeGlobalSearch = closeGlobalSearch;
    window.executeGlobalSearch = executeGlobalSearch;
    window.onSearchResultClick = onSearchResultClick;
    window._selectAndViewCmd = function(instUrl, cmdId, cmdName) {
        selectCommand(instUrl, cmdId, cmdName);
        closeCmdManager();
    };
    window.updateFrozenIndicator = updateFrozenIndicator;
    window._toggleSearchFreezeCommands = _toggleSearchFreezeCommands;
    window.closeCmdManager = closeCmdManager;
    window.renderCmdManagerList = renderCmdManagerList;
    window.cmdManagerKillAll = cmdManagerKillAll;
})();
