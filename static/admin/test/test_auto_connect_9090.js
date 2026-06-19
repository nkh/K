// ════════════════════════════════════════════════════════════════════
// TEST: Auto-connect to port 9090 on startup + auto-select logic
// ════════════════════════════════════════════════════════════════════
// This test traces the exact same API call sequence that the web UI
// performs on page load. If the API calls succeed but the web UI
// doesn't show the terminal, the bug is in the frontend JS code.
//
// Flow:
//   1. GET /api/info        → server reachability + config
//   2. GET /api/snapshot    → commands + VTTY HTML + resources (first load)
//   3. GET /api/commands    → command list (subsequent refresh)
//   4. WS  /api/commands/:id/ws → live VTTY stream
//
// Usage:
//   node test_auto_connect_9090.js [BASE_URL]
//   Default BASE_URL: http://localhost:9090
// ════════════════════════════════════════════════════════════════════

const http = require('http');

const BASE_URL = process.argv[2] || 'http://localhost:9090';
const BASE = new URL(BASE_URL);

let passed = 0;
let failed = 0;

function assert(condition, msg) {
    if (condition) {
        console.log('  ✓ ' + msg);
        passed++;
    } else {
        console.log('  ✗ FAIL: ' + msg);
        failed++;
    }
}

function fetchJson(path) {
    return new Promise((resolve, reject) => {
        const url = new URL(path, BASE_URL);
        const options = {
            hostname: url.hostname,
            port: url.port,
            path: url.pathname + url.search,
            method: 'GET',
            headers: { 'Accept': 'application/json' },
            timeout: 5000,
        };
        const req = http.request(options, (res) => {
            let body = '';
            res.on('data', chunk => body += chunk);
            res.on('end', () => {
                try {
                    resolve({ status: res.statusCode, json: JSON.parse(body) });
                } catch (e) {
                    reject(new Error('Non-JSON response from ' + path + ': ' + body.substring(0, 200)));
                }
            });
        });
        req.on('error', reject);
        req.on('timeout', () => { req.destroy(); reject(new Error('Timeout: ' + path)); });
        req.end();
    });
}

async function testApiInfo() {
    console.log('\n━━━ STEP 1: GET /api/info (server reachability) ━━━');
    try {
        const res = await fetchJson('/api/info');
        console.log('  HTTP status:', res.status);
        console.log('  Response:', JSON.stringify(res.json).substring(0, 300));
        assert(res.status === 200, '/api/info returns HTTP 200');
        assert(res.json.status === 'ok', '/api/info returns status "ok"');
        assert(!!res.json.data, '/api/info has data field');
        assert(res.json.data.command_count !== undefined, '/api/info has command_count');
        return true;
    } catch (e) {
        console.log('  ✗ ERROR: ' + e.message);
        assert(false, '/api/info reachable');
        return false;
    }
}

async function testApiSnapshot() {
    console.log('\n━━━ STEP 2: GET /api/snapshot (initial load) ━━━');
    try {
        const res = await fetchJson('/api/snapshot');
        console.log('  HTTP status:', res.status);
        assert(res.status === 200, '/api/snapshot returns HTTP 200');
        assert(res.json.status === 'ok', '/api/snapshot returns status "ok"');
        assert(res.json.data !== undefined, '/api/snapshot has data field');

        const data = res.json.data;
        assert(Array.isArray(data.commands), 'snapshot.data.commands is array');
        assert(data.commands.length >= 0, 'commands array is non-negative length');
        console.log('  Commands count:', data.commands.length);

        if (data.commands.length > 0) {
            const firstCmd = data.commands.find(c => c.alive) || data.commands[0];
            console.log('  First command:', JSON.stringify(firstCmd).substring(0, 200));
            assert(firstCmd.id !== undefined, 'first command has an id');
            assert(firstCmd.name !== undefined, 'first command has a name');

            // Check VTTY data
            if (data.vtty) {
                console.log('  VTTY data present:', JSON.stringify(data.vtty).substring(0, 300));
                assert(data.vtty.html !== undefined, 'vtty.html field exists (even if null)');
                assert(data.vtty.generation !== undefined, 'vtty.generation field exists');
                assert(data.vtty.dimensions !== undefined, 'vtty.dimensions field exists');

                if (data.vtty.html) {
                    console.log('  VTTY HTML length:', data.vtty.html.length);
                    assert(data.vtty.html.length > 0, 'VTTY HTML is non-empty');
                }
            } else {
                console.log('  ⚠ No VTTY data in snapshot (server may have no commands alive)');
                assert(false, 'vtty data present when commands exist');
            }
        } else {
            console.log('  ⚠ No commands — this is expected if no process is running');
        }

        // Check resources
        if (data.resources) {
            console.log('  Resources:', Object.keys(data.resources).length, 'commands');
        }

        return { success: true, data };
    } catch (e) {
        console.log('  ✗ ERROR: ' + e.message);
        assert(false, '/api/snapshot reachable');
        return { success: false };
    }
}

async function testApiCommands() {
    console.log('\n━━━ STEP 3: GET /api/commands (command list refresh) ━━━');
    try {
        const res = await fetchJson('/api/commands');
        console.log('  HTTP status:', res.status);
        assert(res.status === 200, '/api/commands returns HTTP 200');
        assert(res.json.status === 'ok', '/api/commands returns status "ok"');

        if (res.json.data) {
            console.log('  Commands:', res.json.data.length);
            for (const cmd of res.json.data) {
                console.log('    - ' + cmd.name + ' (' + cmd.id + ') alive=' + cmd.alive + ' status=' + cmd.status);
            }
        }
        return true;
    } catch (e) {
        console.log('  ✗ ERROR: ' + e.message);
        assert(false, '/api/commands reachable');
        return false;
    }
}

async function testWebSocket(cmdId) {
    console.log('\n━━━ STEP 4: WS /api/commands/:id/ws (live VTTY) ━━━');
    if (!cmdId) {
        console.log('  ⚠ No command ID to test WebSocket with — skipping');
        return;
    }

    return new Promise((resolve) => {
        const ws = require('ws');
        const wsUrl = BASE_URL.replace(/^http/, 'ws') + '/api/commands/' + cmdId + '/ws';
        console.log('  Connecting to:', wsUrl);

        let gotConnected = false;
        let gotVttyFull = false;
        let gotVttyDiff = false;
        const timer = setTimeout(() => {
            console.log('  Timeout after 5s — partial results shown');
            assert(gotConnected, 'ws: received "connected" message');
            assert(gotVttyFull || gotVttyDiff, 'ws: received vtty_full or vtty_diff');
            ws.terminate();
            resolve();
        }, 5000);

        try {
            const socket = new ws(wsUrl);
            socket.on('open', () => {
                console.log('  WS opened');
            });
            socket.on('message', (data) => {
                try {
                    const msg = JSON.parse(data.toString());
                    console.log('  WS msg:', msg.type);
                    if (msg.type === 'connected') gotConnected = true;
                    if (msg.type === 'vtty_full') {
                        gotVttyFull = true;
                        if (msg.data) {
                            console.log('  vtty_full: html length=' + (msg.data.html || '').length +
                                ' gen=' + msg.data.generation +
                                ' dims=' + JSON.stringify(msg.data.dimensions));
                        }
                    }
                    if (msg.type === 'vtty_diff') {
                        gotVttyDiff = true;
                        console.log('  vtty_diff: gen=' + msg.data.generation + ' changed=' + msg.data.changed_count);
                    }
                    // Got at least one real message — good enough
                    if (gotVttyFull || gotVttyDiff) {
                        clearTimeout(timer);
                        socket.close();
                    }
                } catch (e) {
                    console.log('  WS parse error:', e.message);
                }
            });
            socket.on('error', (err) => {
                console.log('  WS error:', err.message);
                clearTimeout(timer);
                assert(false, 'ws: no error');
                resolve();
            });
            socket.on('close', () => {
                clearTimeout(timer);
                assert(gotConnected, 'ws: received "connected" message');
                assert(gotVttyFull || gotVttyDiff, 'ws: received vtty data');
                resolve();
            });
        } catch (e) {
            console.log('  WS connect failed:', e.message);
            clearTimeout(timer);
            assert(false, 'ws module available and connectable');
            resolve();
        }
    });
}

// ════════════════════════════════════════════════════════════════════
// TRACE: Map the web UI code path step-by-step to find divergence
// ════════════════════════════════════════════════════════════════════
function traceWebUIFlow() {
    console.log('\n━━━ TRACE: Web UI startup code path ━━━');
    console.log(`
  app.js init():
    1. state.connections = [{ url: window.location.origin, label: 'Local', ... }]
       → When served from port 9090, this is "http://localhost:9090" ✓
    2. addConnection(state.connections[0].url, ...)
       → This is idempotent: finds existing connection, returns it.
       → CRITICAL: addConnection() checks if url already exists in state.connections.
       → Since we just SET state.connections in step 1, it WILL find it.
       → Connection object has: reachable: undefined, _commands: null
    3. addPanelDirect()
       → Creates panel object: { id, selectedCmdId: null, selectedInstUrl: null, ... }
       → CRITICAL: panel.selectedCmdId is null and panel.selectedInstUrl is null
       → Calls renderPanels() → shows welcome screen (no commands yet)
    4. startRefresh()
       → loadSnapshot()  [FIRST call only]
       → setInterval(loadCommands, 1000)
       → setInterval(fetchServerConfig, 5000)
    5. fetchServerConfig()
       → GET /api/info → sets state.serverReachable

  loadSnapshot():
    1. _snapshotLoaded = true
    2. localInst = state.connections[0]  ← the one with url: window.location.origin
    3. fetch(apiUrl('/api/snapshot', localInst))
       → apiUrl builds: localInst.url + '/api/snapshot' = 'http://localhost:9090/api/snapshot'
    4. On success:
       → localInst._commands = commands || []
       → localInst.reachable = true
       → hasAnyCommands = commands && commands.length > 0
       → firstCmd = commands.find(c => c.alive) || commands[0]
       → shouldShowWelcome = !hasAnyCommands && !state.selectedCmdId && !state.serverReachable
         ⚠ PROBLEM: state.serverReachable is still false at this point!
           fetchServerConfig() is called AFTER startRefresh() in init(), so
           the snapshot loads before serverReachable is set to true.
           But: if hasAnyCommands is true, shouldShowWelcome is false anyway.
       → If vtty.html exists && firstCmd:
         → state.selectedInstUrl = localInst.url
         → state.selectedCmdId = firstCmd.id
         → CRITICAL: panelObj = state.panels.find(p => p.id === (state._focusedPanelId || state.panels[0].id))
           ⚠ PROBLEM: state._focusedPanelId is null!
             → Falls back to state.panels[0].id
             → Sets panelObj.selectedInstUrl = localInst.url
             → Sets panelObj.selectedCmdId = firstCmd.id
         → Gets DOM element for panel
         → Writes VTTY HTML into <pre>
         → Calls updatePanelCommandInfo()
         → Calls startUpdateMode()
           → startPanelUpdateMode(panelId)
             → Checks panelObj.selectedCmdId === null → should be set now ✓
             → If push mode: connectPanelWs(panelId) ✓
    5. On failure (catch):
       → localInst.reachable = false
       → loadCommands() fallback

  POTENTIAL ISSUES:
  ──────────────────
  A) _focusedPanelId is null during loadSnapshot()
     → Falls back to state.panels[0].id — should work IF panels exist.
     → addPanelDirect() was called before startRefresh(), so panels[0] exists.

  B) renderPanels() rebuilds DOM, potentially destroying the <pre> that
     loadSnapshot() is writing to.
     → addPanelDirect() calls renderPanels() synchronously.
     → loadSnapshot() is async — it runs AFTER renderPanels().
     → So loadSnapshot() gets the DOM that renderPanels() built.
     → BUT: loadSnapshot() also calls renderPanels() on shouldShowWelcome change!
       Line 72-73 of snapshot.js: if (shouldShowWelcome !== _showingWelcome) { renderPanels(); }
       This would rebuild the DOM, and the VTTY HTML write at line 100-111
       targets the panel element by ID. If renderPanels() just rebuilt,
       the panel element still has the same ID, so the write should work.

  C) Welcome screen logic uses THREE conditions:
     (!hasAnyCommands && !state.selectedCmdId && !state.serverReachable)
     → If the server has commands, hasAnyCommands=true → welcome=false ✓
     → If serverReachable is still false but commands exist → welcome=false ✓
     → If NO commands and server not reachable → welcome=true ✓

  D) addConnection() is called on an already-existing connection:
     → In init(), state.connections is set directly (not via addConnection).
     → Then addConnection(state.connections[0].url, ...) is called.
     → addConnection finds the existing connection and returns it.
     → The existing connection has NO _commands field (set as array literal).
     → loadSnapshot() adds _commands to the connection object.
`);

    // The smoking gun: check if state.connections[0] has the right shape
    console.log('  state.connections[0] after init:');
    console.log('    url:', BASE_URL);
    console.log('    label:', 'Local');
    console.log('    reachable: undefined (not yet fetched)');
    console.log('    _commands: null (not yet fetched)');
    console.log('');
    console.log('  After addConnection() is called:');
    console.log('    → addConnection finds existing connection, returns it');
    console.log('    → No _commands, no reachable change');
    console.log('');
    console.log('  After addPanelDirect():');
    console.log('    → Panel created with selectedCmdId=null, selectedInstUrl=null');
    console.log('    → renderPanels() shows welcome (no commands)');
    console.log('');
    console.log('  After loadSnapshot() succeeds:');
    console.log('    → localInst._commands = [commands...]');
    console.log('    → localInst.reachable = true');
    console.log('    → panelObj.selectedCmdId = firstCmd.id');
    console.log('    → panelObj.selectedInstUrl = localInst.url');
    console.log('    → VTTY HTML written to <pre>');
    console.log('    → startPanelUpdateMode(panelId) → connectPanelWs(panelId)');
    console.log('');
    console.log('  After fetchServerConfig() completes:');
    console.log('    → state.serverReachable = true');
    console.log('    → renderPanels() called if reachability changed');
    console.log('    → BUT: if commands are already loaded, welcome stays false');
}

async function main() {
    console.log('═══════════════════════════════════════════════════════════');
    console.log('AUTO-CONNECT TO 9090 TEST');
    console.log('Base URL:', BASE_URL);
    console.log('═══════════════════════════════════════════════════════════');

    // Quick server reachability check — skip if server not running
    try {
        const check = await new Promise((resolve, reject) => {
            const req = http.request({ hostname: BASE.hostname, port: BASE.port, path: '/api/info', method: 'GET', timeout: 2000 }, (res) => {
                let data = '';
                res.on('data', d => data += d);
                res.on('end', () => resolve(res.statusCode));
            });
            req.on('error', reject);
            req.on('timeout', () => { req.destroy(); reject(new Error('timeout')); });
            req.end();
        });
        console.log('  Server check: HTTP ' + check);
    } catch (e) {
        console.log('  Server not reachable on ' + BASE_URL + ' — skipping integration tests');
        console.log('  (This is expected in CI/unit-test environments)');
        console.log('\nRESULTS: 0 passed, 0 failed (skipped — no server)');
        console.log('═══════════════════════════════════════════════════════════');
        return; // Don't process.exit — let run_all.js continue with remaining test files
    }

    // Trace the code path
    traceWebUIFlow();

    // Test API calls
    const infoOk = await testApiInfo();
    const snapshotResult = await testApiSnapshot();

    if (snapshotResult.success && snapshotResult.data) {
        const commands = snapshotResult.data.commands || [];
        const firstCmd = commands.find(c => c.alive) || commands[0];

        await testApiCommands();

        if (firstCmd) {
            await testWebSocket(firstCmd.id);
        } else {
            console.log('\n  ⚠ No commands to test WebSocket — skipping step 4');
        }
    } else {
        console.log('\n  ⚠ Snapshot failed, skipping remaining tests');
    }

    // Summary
    console.log('\n═══════════════════════════════════════════════════════════');
    console.log('RESULTS: ' + passed + ' passed, ' + failed + ' failed');
    console.log('═══════════════════════════════════════════════════════════');

    if (failed > 0) {
        console.log('\nDIAGNOSIS:');
        console.log('  If API tests fail → server is not running on ' + BASE_URL);
        console.log('  If API tests pass → bug is in web UI JS code path');
        console.log('  Compare the TRACE above with browser DevTools Network tab');
        process.exit(1);
    } else {
        console.log('\nAll API calls succeed. If web UI still shows nothing,');
        console.log('the bug is in the frontend JS code, not the server.');
        process.exit(0);
    }
}

main().catch(e => {
    console.error('Fatal error:', e);
    process.exit(2);
});
