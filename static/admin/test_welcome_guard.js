/// Test suite for the renderPanels() structural guard and welcome-panel transition.
/// Run with: node test_welcome_guard.js
///
/// Tests the bug fixed in commit b48ffef and its follow-up:
/// The structural guard in renderPanels() must NOT skip rebuilds when
/// the welcome state changes. Additionally, _showingWelcome must be
/// set BEFORE renderPanels() is called, otherwise the guard sees
/// no change and incorrectly skips the rebuild.
///
/// The bug manifested as: sidebar shows commands while main area is
/// stuck on "vrw is not running" welcome screen.

let _passed = 0;
let _failed = 0;

function assert(condition, msg) {
    if (!condition) {
        _failed++;
        console.error(`  FAIL: ${msg}`);
        process.exitCode = 1;
    } else {
        _passed++;
    }
}

function assertEq(actual, expected, msg) {
    if (actual !== expected) {
        _failed++;
        console.error(`  FAIL: ${msg} — expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
        process.exitCode = 1;
    } else {
        _passed++;
    }
}

// ── Reproduce the structural guard logic from renderPanels() ──

let _lastRenderedPanelCount = -1;
let _lastRenderedPanelIds = '';
let _lastShowingWelcome = true;
let _showingWelcome = true;
let _renderCount = 0; // Track how many times a full rebuild happens

// Simulated state
let panels = [];
let instanceUrls = [];
let serverReachable = false;
let selectedCmdId = null;

/// Simulates the renderPanels() structural guard and rebuild logic.
/// Returns true if a full rebuild happened, false if skipped.
function simulateRenderPanels() {
    const container = { innerHTML: '' };
    const hasMultiplePanels = panels.length > 1;

    const currentPanelIds = panels.map(p => p.id).join(',');
    const structuralUnchanged =
        _lastRenderedPanelCount === panels.length &&
        _lastRenderedPanelIds === currentPanelIds;

    if (structuralUnchanged && _lastShowingWelcome === _showingWelcome) {
        // FAST PATH: skip rebuild
        return false;
    }

    // FULL REBUILD
    _renderCount++;

    // Check if there are any commands
    let hasAnyCommands = false;
    for (const inst of instanceUrls) {
        if (inst._commands && inst._commands.length > 0) {
            hasAnyCommands = true;
            break;
        }
    }

    // Welcome condition (same as renderPanels line 3079)
    if (panels.length === 1 && !hasAnyCommands && !selectedCmdId && !serverReachable) {
        _showingWelcome = true;
        container.innerHTML = '<div class="welcome-panel">vrw is not running</div>';
    } else {
        _showingWelcome = false;
        for (const panel of panels) {
            container.innerHTML += `<div class="panel" id="${panel.id}"><pre></pre></div>`;
        }
    }

    _lastRenderedPanelCount = panels.length;
    _lastRenderedPanelIds = currentPanelIds;
    _lastShowingWelcome = _showingWelcome;

    return true;
}

/// Simulates loadSnapshot()'s welcome check (FIXED version: sets _showingWelcome before renderPanels).
/// With the bug, _showingWelcome is NOT set before calling renderPanels().
function simulateLoadSnapshotFixed(commands) {
    const primaryInst = instanceUrls[0];
    primaryInst._commands = commands || [];
    primaryInst.reachable = true;

    const hasAnyCommands = commands && commands.length > 0;
    const shouldShowWelcome = (panels.length === 1 && !hasAnyCommands && !selectedCmdId && !serverReachable);

    // FIXED: set _showingWelcome BEFORE calling renderPanels
    if (shouldShowWelcome !== _showingWelcome) {
        _showingWelcome = shouldShowWelcome;
        simulateRenderPanels();
    }
}

/// Simulates loadSnapshot()'s welcome check (BUGGY version: does NOT set _showingWelcome).
function simulateLoadSnapshotBuggy(commands) {
    const primaryInst = instanceUrls[0];
    primaryInst._commands = commands || [];
    primaryInst.reachable = true;

    const hasAnyCommands = commands && commands.length > 0;
    const shouldShowWelcome = (panels.length === 1 && !hasAnyCommands && !selectedCmdId && !serverReachable);

    // BUG: _showingWelcome NOT updated before renderPanels
    if (shouldShowWelcome !== _showingWelcome) {
        simulateRenderPanels();
    }
}

// ── Tests ──

console.log('\n=== Welcome Panel Guard Tests ===\n');

// Test 1: Initial render with no commands → shows welcome
console.log('Test 1: Initial render with no commands shows welcome panel');
panels = [{ id: 'panel-1', instUrl: 'http://localhost:9090' }];
instanceUrls = [{ url: 'http://localhost:9090', _commands: [], reachable: undefined }];
serverReachable = false;
selectedCmdId = null;
_lastRenderedPanelCount = -1;
_lastRenderedPanelIds = '';
_lastShowingWelcome = true;
_showingWelcome = true;
_renderCount = 0;

const r1 = simulateRenderPanels();
assert(r1, 'should do full rebuild on first render');
assertEq(_showingWelcome, true, 'should show welcome when no commands and server unreachable');
assertEq(_renderCount, 1, 'should have 1 rebuild');
assertEq(_lastShowingWelcome, true, '_lastShowingWelcome should be true');

// Test 2: Commands arrive — FIXED version correctly transitions away from welcome
console.log('Test 2: Commands arrive — fixed version transitions from welcome to panels');
instanceUrls[0]._commands = [{ id: 'cmd-1', name: 'htop', alive: true }];
_renderCount = 0;

simulateLoadSnapshotFixed(instanceUrls[0]._commands);
assertEq(_renderCount, 1, 'fixed: should rebuild when commands arrive (guard bypassed)');
assertEq(_showingWelcome, false, 'fixed: should NOT show welcome after commands arrive');
assertEq(_lastShowingWelcome, false, 'fixed: _lastShowingWelcome should be false');

// Test 3: Commands arrive — BUGGY version gets stuck on welcome
console.log('Test 3: Commands arrive — buggy version gets stuck on welcome');
// Reset to simulate initial state
panels = [{ id: 'panel-1', instUrl: 'http://localhost:9090' }];
instanceUrls = [{ url: 'http://localhost:9090', _commands: [], reachable: undefined }];
serverReachable = false;
selectedCmdId = null;
_lastRenderedPanelCount = -1;
_lastRenderedPanelIds = '';
_lastShowingWelcome = true;
_showingWelcome = true;
_renderCount = 0;

// First render: welcome panel
simulateRenderPanels();
assertEq(_showingWelcome, true, 'initial state: showing welcome');
assertEq(_lastShowingWelcome, true, 'initial state: _lastShowingWelcome is true');
assertEq(_renderCount, 1, 'initial render happened');

// Commands arrive via loadSnapshot (buggy path)
_renderCount = 0;
instanceUrls[0]._commands = [{ id: 'cmd-1', name: 'htop', alive: true }];
simulateLoadSnapshotBuggy(instanceUrls[0]._commands);
assertEq(_renderCount, 0, 'buggy: structural guard INCORRECTLY skips rebuild');
assertEq(_showingWelcome, true, 'buggy: _showingWelcome still true (never updated)');
// This is the bug: sidebar shows commands but main area shows welcome

// Test 4: fetchServerConfig arrives after buggy loadSnapshot — also stuck
console.log('Test 4: fetchServerConfig after buggy loadSnapshot — also stuck');
_renderCount = 0;
serverReachable = true;
simulateRenderPanels();
// Now serverReachable=true, but hasAnyCommands is true, so welcome condition is false
// The guard: structuralUnchanged=true, _showingWelcome=true, _lastShowingWelcome=true
// → guard fires → SKIPS!
assertEq(_renderCount, 0, 'buggy: fetchServerConfig also cannot break through the guard');
assertEq(_showingWelcome, true, 'buggy: still showing welcome even though server is reachable and has commands!');

// Test 5: Verify the full scenario with the fix — everything works
console.log('Test 5: Full scenario with fix — loadSnapshot then fetchServerConfig');
// Reset
panels = [{ id: 'panel-1', instUrl: 'http://localhost:9090' }];
instanceUrls = [{ url: 'http://localhost:9090', _commands: [], reachable: undefined }];
serverReachable = false;
selectedCmdId = null;
_lastRenderedPanelCount = -1;
_lastRenderedPanelIds = '';
_lastShowingWelcome = true;
_showingWelcome = true;
_renderCount = 0;

// Step 1: Initial render → welcome
simulateRenderPanels();
assertEq(_showingWelcome, true, 'step 1: welcome shown');
assertEq(_renderCount, 1, 'step 1: rebuilt');

// Step 2: loadSnapshot with commands (fixed)
_renderCount = 0;
instanceUrls[0]._commands = [{ id: 'cmd-1', name: 'htop', alive: true }];
simulateLoadSnapshotFixed(instanceUrls[0]._commands);
assertEq(_renderCount, 1, 'step 2: rebuilt when commands arrive');
assertEq(_showingWelcome, false, 'step 2: welcome dismissed, panels shown');

// Step 3: fetchServerConfig — serverReachable changes
_renderCount = 0;
const prevRenderCount = _renderCount;
serverReachable = true;
// fetchServerConfig calls renderPanels() but guard sees no change
// (already showing panels, structure unchanged) → correctly skips
simulateRenderPanels();
assertEq(_showingWelcome, false, 'step 3: still showing panels');

// Test 6: Multiple panels — welcome should not show even if one instance is unreachable
console.log('Test 6: Multiple panels — no welcome when there are commands');
panels = [
    { id: 'panel-1', instUrl: 'http://localhost:9090' },
    { id: 'panel-2', instUrl: 'http://localhost:9091' },
];
instanceUrls = [
    { url: 'http://localhost:9090', _commands: [{ id: 'cmd-1', name: 'htop' }], reachable: true },
    { url: 'http://localhost:9091', _commands: [], reachable: false },
];
serverReachable = true;
selectedCmdId = 'cmd-1';
_lastRenderedPanelCount = -1;
_lastRenderedPanelIds = '';
_lastShowingWelcome = true;
_showingWelcome = true;
_renderCount = 0;

simulateRenderPanels();
assertEq(_showingWelcome, false, 'multi-panel: should NOT show welcome when panels.length > 1');

// Test 7: Guard correctly skips when nothing changed
console.log('Test 7: Guard correctly skips rebuild when nothing changed');
_renderCount = 0;
const r7 = simulateRenderPanels();
assertEq(r7, false, 'should skip rebuild when structure and welcome state unchanged');
assertEq(_renderCount, 0, 'no rebuild should have happened');

// Test 8: Guard fires when welcome state changes even with same structure
console.log('Test 8: Guard fires when welcome state changes (same structure)');
_renderCount = 0;
// Simulate losing all commands and server going down
instanceUrls[0]._commands = [];
instanceUrls[1]._commands = [];
selectedCmdId = null;
panels = [{ id: 'panel-1', instUrl: 'http://localhost:9090' }]; // back to 1 panel
instanceUrls = [{ url: 'http://localhost:9090', _commands: [], reachable: false }];
serverReachable = false;
_lastRenderedPanelCount = panels.length; // same count
_lastRenderedPanelIds = panels.map(p => p.id).join(','); // same IDs
_showingWelcome = false; // currently showing panels
_lastShowingWelcome = false;

// Now set _showingWelcome to true (simulating loadSnapshot detecting no commands)
_showingWelcome = true;
const r8 = simulateRenderPanels();
assert(r8, 'should rebuild when _showingWelcome changes even with same panel structure');
assertEq(_renderCount, 1, 'rebuild should have happened');

// ── Summary ──
console.log(`\n=== Results: ${_passed} passed, ${_failed} failed ===\n`);
if (_failed > 0) process.exit(1);
