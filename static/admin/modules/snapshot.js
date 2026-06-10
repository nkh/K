// ─── Snapshot (Initial Load) ───
// Fast initial load: fetch commands, VTTY HTML, and resources in a SINGLE
// request from the primary instance. This replaces the old flow of
// loadCommands → _prefetchVttyHtml → pollResources (3+ serial round trips)
// with just 1 round trip.
//
// Dependencies: commands (loadCommands), panels (renderPanels), vtty (buildCellGrid,
//   updateVttyMetadataFromHttp), sidebar (_buildSidebar, updateDisconnectedUI),
//   websocket (startUpdateMode), commands (updatePanelCommandInfo, getSelectedPanel)
(function() {
    'use strict';

let _snapshotLoaded = false;
// Expose for test resets (getter/setter keeps local var in sync)
Object.defineProperty(window, '_snapshotLoaded', {
    get() { return _snapshotLoaded; },
    set(v) { _snapshotLoaded = v; },
    configurable: true,
});

async function loadSnapshot() {
    if (_snapshotLoaded) { loadCommands(); return; }
    _snapshotLoaded = true;

    const primaryInst = state.connections[0];
    if (!primaryInst) { loadCommands(); return; }

    try {
        const res = await fetch(apiUrl('/api/snapshot', primaryInst),
            { headers: authHeadersForInstance(primaryInst) });
        if (!res.ok) throw new Error('HTTP ' + res.status);
        const json = await res.json();
        if (json.status !== 'ok' || !json.data) throw new Error('bad snapshot');

        const { commands, vtty, resources } = json.data;

        // Store commands for the primary instance
        primaryInst._commands = commands || [];
        primaryInst.reachable = true;
        primaryInst._lastError = null;

        // Store resources in cache — sidebar will show them immediately
        if (resources) {
            for (const [cmdId, resData] of Object.entries(resources)) {
                state._resourceCache[cmdId] = resData;
            }
        }

        // Fetch peer instances in parallel (don't block the primary display)
        const peerPromises = state.connections.slice(1).map(async (inst) => {
            try {
                const r = await fetch(apiUrl('/api/commands', inst),
                    { headers: authHeadersForInstance(inst) });
                if (!r.ok) throw new Error('HTTP ' + r.status);
                const j = await r.json();
                inst._commands = j.status === 'ok' ? j.data : [];
                inst.reachable = true;
                inst._lastError = null;
            } catch (e) {
                inst._commands = inst._commands || [];
                inst.reachable = false;
                inst._lastError = 'connection lost (instance may have exited)';
            }
        });
        // Kick off peer fetches but don't await — render primary immediately
        const peersDone = Promise.all(peerPromises).then(() => {
            updateDisconnectedUI();
        });

        // ── Render terminal from embedded VTTY HTML ──
        const hasAnyCommands = commands && commands.length > 0;
        const firstCmd = hasAnyCommands
            ? (commands.find(c => c.alive) || commands[0])
            : null;
        const shouldShowWelcome = (!hasAnyCommands && !state.selectedCmdId && !state.serverReachable);

        if (shouldShowWelcome !== _showingWelcome) {
            _showingWelcome = shouldShowWelcome;
            renderPanels();
        }

        if (vtty && vtty.html !== undefined && firstCmd) {
            state.selectedInstUrl = primaryInst.url;
            state.selectedCmdId = firstCmd.id;
            state._pendingVttyData = null;
            state._pendingVttyDirty = false;
            state.bufferView = 'current';

            // CRITICAL: set per-panel selection fields on the panel OBJECT
            // (state.panels[]), not the DOM element. Without these,
            // startPanelUpdateMode() returns immediately (selectedCmdId === null)
            // and the per-panel WebSocket is never connected.
            const panelObj = state.panels.find(p => p.id === (state._focusedPanelId || state.panels[0].id));
            if (panelObj) {
                panelObj.selectedInstUrl = primaryInst.url;
                panelObj.selectedCmdId = firstCmd.id;
            }
            const panel = getSelectedPanel();

            // Store generation for subsequent incremental updates
            if (vtty.generation !== undefined) {
                state._lastGeneration[firstCmd.id] = vtty.generation;
            }

            // Write VTTY HTML directly into <pre> — NO second HTTP request
            const panelEl = document.getElementById(panelObj ? panelObj.id : (state._focusedPanelId || (state.panels[0] || {}).id));
            if (panelEl) {
                const vttyEl = panelEl.querySelector('.vtty-container');
                const pre = vttyEl ? vttyEl.querySelector('pre') : null;
                if (pre) {
                    pre.innerHTML = vtty.html;
                    if (state._level3Enabled && vtty.dimensions) {
                        buildCellGrid(firstCmd.id, pre, vtty.dimensions.rows, vtty.dimensions.cols);
                    }
                    updateVttyMetadataFromHttp(vtty, panelEl, panelObj, 0);
                }
            }

            updatePanelCommandInfo();
            updateTerminalDisconnectedOverlay();
            // Start push/poll for incremental updates
            startUpdateMode();
        } else {
            _showingWelcome = shouldShowWelcome;
            updateDisconnectedUI();
        }

        // Wait for peers to finish, then build the sidebar with full data
        await peersDone;
        // Build sidebar (includes resource data from cache)
        _buildSidebar();

    } catch (e) {
        primaryInst._commands = primaryInst._commands || [];
        primaryInst.reachable = false;
        primaryInst._lastError = 'connection lost';
        updateDisconnectedUI();
        // Fall back to regular loadCommands
        loadCommands();
    }
}

    window.loadSnapshot = loadSnapshot;
})();
