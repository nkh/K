// ─── vrw Web UI Entry Point ───
// This file loads all modular JS files and runs initialization.
// Load order matters: dependencies must be loaded before dependents.
//
// Module dependency graph (load order):
//   state.js        ← (no deps)
//   eventbus.js     ← (no deps)
//   utils.js        ← (no deps)
//   focus.js        ← (no deps)
//   theme.js        ← state
//   sidebar.js      ← utils, state
//   panels.js       ← utils, state, focus
//   commands.js     ← utils, state, panels, sidebar
//   websocket.js    ← utils, state
//   vtty.js         ← utils, state
//   spawn.js        ← utils, state, commands
//   logs.js         ← utils, state
//   keyboard.js     ← utils, state, focus, search
//   search.js       ← utils, state, focus
//   notifications.js ← utils, state, commands
//   templates.js    ← utils, state, commands, spawn
//   dragdrop.js     ← utils, state, panels, commands
//   workspaces.js   ← utils, state, panels, logs, commands
//   misc.js         ← all above
//   app.js          ← all above (initialization)
//
// DO NOT reorder these script tags in index.html — the load order is significant.

(function init() {
    // Initialize event delegation FIRST — replaces all inline onclick handlers
    initDelegation();

    initTheme();
    document.getElementById('authToken').value = state.authToken;
    applyFontSize();
    initBottombar();
    initSoundToggle();
    _syncRefreshMsUI();

    // Mark resize inputs as user-edited when manually changed, so that
    // server-reported dimensions don't overwrite the user's values.
    const stResizeRows = document.getElementById('stResizeRows');
    const stResizeCols = document.getElementById('stResizeCols');
    if (stResizeRows) stResizeRows.addEventListener('input', () => { stResizeRows._userEdited = true; });
    if (stResizeCols) stResizeCols.addEventListener('input', () => { stResizeCols._userEdited = true; });

    // Event delegation for command list — handles kill buttons without inline onclick
    document.getElementById('commandList').addEventListener('click', (e) => {
        const killBtn = e.target.closest('.cmd-kill-btn');
        if (killBtn) {
            e.stopPropagation();
            if (killBtn.disabled) return; // still respect disabled attribute as a last resort
            if (killBtn.dataset.cmdRetained === 'true' && killBtn.dataset.cmdAlive !== 'true') {
                purgeKeptCommand(killBtn.dataset.instUrl, killBtn.dataset.cmdId, '');
            } else {
                killCommand(killBtn.dataset.instUrl, killBtn.dataset.cmdId);
            }
        }
    });

    // Parse URL arguments for multi-instance
    const params = new URLSearchParams(window.location.search);
    const instances = params.getAll('instance');
    if (instances.length > 0) {
        // Multiple instances from URL params — add as connections
        state.connections = instances.map((u, i) => ({
            url: u,
            label: params.getAll('label')[i] || `Instance ${i + 1}`,
            token: params.getAll('token')[i] || '',
            reachable: undefined,
        }));
    } else {
        // Default: auto-connect to current origin
        state.connections = [{
            url: window.location.origin,
            label: '',  // will be derived from URL by _getServerLabel
            token: '',
            reachable: undefined,
        }];
    }

    // Create initial panels
    addConnection(state.connections[0].url, state.connections[0].label, state.connections[0].token);
    addPanelDirect();

    // Restore saved server connections from localStorage
    const restoredConnections = _restoreConnections();

    // Health-check restored connections: remove ones that don't respond
    // after 5 retries at 500ms intervals
    healthCheckConnections(restoredConnections);

    // Restore panel layout from localStorage
    const savedLayout = localStorage.getItem('vrw_panel_layout');
    if (savedLayout) {
        state.panelLayout = savedLayout;
    }

    // Restore number of panels from localStorage
    const savedPanelCount = parseInt(localStorage.getItem('vrw_panel_count'));
    if (savedPanelCount && savedPanelCount > 1 && state.panels.length < savedPanelCount) {
        for (let i = 1; i < savedPanelCount; i++) {
            addPanelDirect();
        }
    }

    // Fetch server names for restored connections
    if (restoredConnections) {
        for (const connUrl of restoredConnections) {
            const conn = state.connections.find(c => c.url === connUrl);
            if (conn) _fetchServerName(conn);
        }
    }

    // ── Start refresh ──
    startRefresh();
    loadCertificates();
    fetchServerTemplates();
    fetchEnvironments();
    fetchServerConfig();
    applyUpdateModeUI();
    updateSidebarTabsVisibility();
    fetchPeers();
    // Check if mobile layout should be active
    state._mobileTabbedLayout = window.innerWidth <= 768;

    // Auto-collapse sidebar on small screens
    if (window.innerWidth <= 768) {
        const sidebar = document.getElementById('sidebar');
        sidebar.classList.add('collapsed');
        sidebar.style.width = '';
    }

    // Auto-fit terminal on window resize (debounced)
    let _resizeTimer = null;
    window.addEventListener('resize', () => {
        if (_resizeTimer) clearTimeout(_resizeTimer);
        _resizeTimer = setTimeout(() => {
            const sidebar = document.getElementById('sidebar');
            if (window.innerWidth <= 768) {
                sidebar.classList.add('collapsed');
                sidebar.style.width = '';
            }
            const wasMobile = state._mobileTabbedLayout;
            state._mobileTabbedLayout = window.innerWidth <= 768;
            if (wasMobile !== state._mobileTabbedLayout) {
                renderPanels();
            }
            autoFitActiveTerminal();
        }, 300);
    });

    // ── Command-name URL routing ──
    const pathname = window.location.pathname.replace(/^\/+|\/+$/g, '');
    if (pathname && pathname !== 'admin' && !pathname.startsWith('api/')) {
        lookupAndSelectCommand(pathname);
    }

    // ── Sidebar resize ──
    const sidebarHandle = document.getElementById('sidebarResizeHandle');
    if (sidebarHandle) {
        let startX, startWidth;
        const sidebar = document.getElementById('sidebar');
        sidebarHandle.addEventListener('mousedown', (e) => {
            e.preventDefault();
            startX = e.clientX;
            startWidth = sidebar.offsetWidth;
            sidebarHandle.classList.add('active');
            document.body.style.cursor = 'col-resize';
            document.body.style.userSelect = 'none';
            const onMove = (e) => {
                const newWidth = Math.max(150, Math.min(9999, startWidth + e.clientX - startX));
                sidebar.style.width = newWidth + 'px';
                // Toggle wide mode for inline details
                sidebar.classList.toggle('sidebar-wide', newWidth >= 400);
            };
            const onUp = () => {
                sidebarHandle.classList.remove('active');
                document.body.style.cursor = '';
                document.body.style.userSelect = '';
                document.removeEventListener('mousemove', onMove);
                document.removeEventListener('mouseup', onUp);
            };
            document.addEventListener('mousemove', onMove);
            document.addEventListener('mouseup', onUp);
        });
    }
})();
