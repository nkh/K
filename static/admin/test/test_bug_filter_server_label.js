/// test/test_bug_filter_server_label.js — Fix 3.1
///   "Filter commands also has the server"
///
/// BUG: The command filter only matches command name, args, and PID.
/// Typing a server name/label does not filter commands.
///
/// FIX: Also match the filter against the server label (_shortLabel).

require('./setup');

console.log('\n=== Fix 3.1: Filter must match server label ===\n');

resetTestState();

// Mocks
globalThis.renderPanels = function() {};
globalThis.loadCommands = function() { return Promise.resolve(); };
globalThis.selectCommand = function() {};
globalThis.loadVttyHttpForPanel = function() {};

// Set up state: two servers, one command each
state.connections = [
    { url: 'http://production.example.com:9090', label: 'Production Server', token: '', reachable: true,
      _commands: [{ id: 'cmd-1', name: 'nginx', alive: true, args: [], pid: 1001, spawn_order: 0 }] },
    { url: 'http://localhost:9090', label: 'Local Dev', token: '', reachable: true,
      _commands: [{ id: 'cmd-2', name: 'node-app', alive: true, args: [], pid: 2002, spawn_order: 1 }] },
];
state.panels = [];
state._focusedPanelId = null;
state._sidebarSort = 'name';
state.activeTab = 'servers';
state._resourceCache = {};

// Create the commandList container and filter input (used by _buildSidebar)
let container = document.getElementById('commandList');
if (!container) {
    container = document.createElement('div');
    container.id = 'commandList';
    document.body.appendChild(container);
}

let fi = document.getElementById('cmdFilter');
if (!fi) {
    fi = document.createElement('input');
    fi.id = 'cmdFilter';
    document.body.appendChild(fi);
}

// ──────────────────────────────────────────────────────────────
// FIX31-001: Filtering by server label shows matching commands
// ──────────────────────────────────────────────────────────────
console.log('FIX31-001: Filter by server label "Production" shows only production cmd');
{
    fi.value = 'Production';
    _buildSidebar();
    const html = container.innerHTML;

    assert(html.includes('nginx'), 'FIX31-001a: production cmd "nginx" visible when filtering by server label');
    assert(!html.includes('node-app'), 'FIX31-001b: local cmd "node-app" hidden when filtering by server label');
}

// ──────────────────────────────────────────────────────────────
// FIX31-002: Filtering by partial server label works
// ──────────────────────────────────────────────────────────────
console.log('FIX31-002: Filter by partial server label "local" shows only local cmd');
{
    fi.value = 'local';
    _buildSidebar();
    const html = container.innerHTML;

    assert(!html.includes('nginx'), 'FIX31-002a: production cmd hidden when filtering by "local"');
    assert(html.includes('node-app'), 'FIX31-002b: local cmd "node-app" visible when filtering by "local"');
}

// ──────────────────────────────────────────────────────────────
// FIX31-003: Filtering by command name still works (no regression)
// ──────────────────────────────────────────────────────────────
console.log('FIX31-003: Filter by command name "node" still works');
{
    fi.value = 'node';
    _buildSidebar();
    const html = container.innerHTML;

    assert(!html.includes('nginx'), 'FIX31-003a: production cmd hidden when filtering by cmd name "node"');
    assert(html.includes('node-app'), 'FIX31-003b: local cmd "node-app" visible when filtering by cmd name "node"');
}

// ──────────────────────────────────────────────────────────────
// FIX31-004: Empty filter shows all commands (no regression)
// ──────────────────────────────────────────────────────────────
console.log('FIX31-004: Empty filter shows all commands');
{
    fi.value = '';
    _buildSidebar();
    const html = container.innerHTML;

    assert(html.includes('nginx'), 'FIX31-004a: production cmd visible with empty filter');
    assert(html.includes('node-app'), 'FIX31-004b: local cmd visible with empty filter');
}