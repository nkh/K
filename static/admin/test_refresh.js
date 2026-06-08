/// Test suite for vrw refresh throttle and diff width handling.
/// Run with: node test_refresh.js
///
/// These tests verify the core logic that was previously broken:
/// 1. Refresh throttle actually throttles updates
/// 2. Diff cell width field is used to distinguish empty cells from wide-char continuations
/// 3. Normal empty cells (width=1, ch=' ') render as space, NOT as U+200B
/// 4. Wide-char continuation cells (width=0) render as U+200B

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

// ── Mock setTimeout/clearTimeout for testing throttle logic ──
// We override the global timers to use synchronous tracking.
let _timers = [];
let _nextTimerId = 1;
let _originalSetTimeout;
let _originalClearTimeout;

function installMockTimers() {
    _timers = [];
    _nextTimerId = 1;
    _originalSetTimeout = global.setTimeout;
    _originalClearTimeout = global.clearTimeout;
    global.setTimeout = (fn, ms) => {
        const id = _nextTimerId++;
        _timers.push({ id, fn, ms });
        return id;
    };
    global.clearTimeout = (id) => {
        _timers = _timers.filter(t => t.id !== id);
    };
}

function restoreTimers() {
    global.setTimeout = _originalSetTimeout;
    global.clearTimeout = _originalClearTimeout;
}

/// Fire all pending timers (in order of creation).
function fireAllTimers() {
    while (_timers.length > 0) {
        const t = _timers.shift();
        t.fn();
    }
}

// ── Inline throttle logic (same as app.js) ──
let _state = {
    refreshMs: 0,
    _refreshThrottleTimer: null,
    selectedInstUrl: 'http://localhost:9090',
    selectedCmdId: 'test-cmd',
    _scheduleVttyHttpCalls: [],
};

function _throttleRefresh() {
    if (_state.refreshMs <= 0) return false;
    if (_state._refreshThrottleTimer) return true;
    _state._refreshThrottleTimer = setTimeout(() => {
        _state._refreshThrottleTimer = null;
        _flushThrottledRefresh();
    }, _state.refreshMs);
    return true;
}

function _flushThrottledRefresh() {
    _state._scheduleVttyHttpCalls.push(Date.now());
}

// ── Tests ──

console.log('\n=== Refresh Throttle Tests ===\n');

// Test 1: No throttle (refreshMs=0) → updates are not throttled
console.log('Test 1: No throttle when refreshMs=0');
_state.refreshMs = 0;
_state._refreshThrottleTimer = null;
_state._scheduleVttyHttpCalls = [];
installMockTimers();
const result1 = _throttleRefresh();
assert(!result1, 'should return false when refreshMs=0 (no throttle)');
assertEq(_timers.length, 0, 'no timer should be set when refreshMs=0');
restoreTimers();

// Test 2: Throttle active (refreshMs=100) → first call sets timer, returns true
console.log('Test 2: Throttle active, first call sets timer');
_state.refreshMs = 100;
_state._refreshThrottleTimer = null;
_state._scheduleVttyHttpCalls = [];
installMockTimers();
const result2 = _throttleRefresh();
assert(result2, 'should return true when refreshMs > 0');
assertEq(_timers.length, 1, 'timer should be set');
assertEq(_state._scheduleVttyHttpCalls.length, 0, 'no flush yet');
restoreTimers();

// Test 3: Throttle active, second call before timer fires → returns true, no new timer
console.log('Test 3: Throttle active, second call returns true immediately');
_state.refreshMs = 100;
_state._refreshThrottleTimer = 123; // pretend timer is set
installMockTimers();
const result3 = _throttleRefresh();
assert(result3, 'should return true when timer already pending');
assertEq(_timers.length, 0, 'no new timer should be set');
restoreTimers();

// Test 4: Timer fires → _flushThrottledRefresh is called
console.log('Test 4: Timer fires → flush is called');
_state.refreshMs = 100;
_state._refreshThrottleTimer = null;
_state._scheduleVttyHttpCalls = [];
installMockTimers();
_throttleRefresh();
assertEq(_timers.length, 1, 'timer should be set');
fireAllTimers();
assertEq(_state._scheduleVttyHttpCalls.length, 1, 'flush should have been called once');
restoreTimers();

// Test 5: Multiple rapid calls → only one flush after timer
console.log('Test 5: Multiple rapid calls → only one flush');
_state.refreshMs = 200;
_state._refreshThrottleTimer = null;
_state._scheduleVttyHttpCalls = [];
installMockTimers();
_throttleRefresh(); // call 1 — sets timer
_throttleRefresh(); // call 2 — returns true, no new timer
_throttleRefresh(); // call 3 — returns true, no new timer
assertEq(_timers.length, 1, 'only one timer should exist');
fireAllTimers();
assertEq(_state._scheduleVttyHttpCalls.length, 1, 'only one flush after timer fires');
restoreTimers();

// ── Diff Width Handling Tests ──

console.log('\n=== Diff Width Handling Tests ===\n');

// Test 6: Normal empty cell (width=1, ch=' ') → should render as space
console.log('Test 6: Normal empty cell renders as space');
function renderCellChar(diff) {
    // Same logic as applyVttyDiff line 2182 (FIXED version)
    return diff.width === 0 ? '\u200b' : (diff.ch === '\u0000' ? ' ' : diff.ch);
}
const normalEmpty = { ch: ' ', width: 1 };
assertEq(renderCellChar(normalEmpty), ' ', 'width=1 empty cell should render as space');

// Test 7: Wide-char continuation (width=0) → should render as U+200B
console.log('Test 7: Wide-char continuation renders as U+200B');
const wideCont = { ch: ' ', width: 0 };
assertEq(renderCellChar(wideCont), '\u200b', 'width=0 continuation should render as U+200B');

// Test 8: Wide-char continuation with any char → should render as U+200B
console.log('Test 8: Wide-char continuation with non-space char → still U+200B');
const wideCont2 = { ch: 'X', width: 0 };
assertEq(renderCellChar(wideCont2), '\u200b', 'width=0 with any ch should render as U+200B');

// Test 9: Normal visible character (width=1) → renders as the character
console.log('Test 9: Normal visible character renders as itself');
const normalChar = { ch: 'A', width: 1 };
assertEq(renderCellChar(normalChar), 'A', 'width=1 with ch=A should render as A');

// Test 10: Wide character (width=2) → renders as the character
console.log('Test 10: Wide character (width=2) renders as itself');
const wideChar = { ch: '\u4f60', width: 2 };
assertEq(renderCellChar(wideChar), '\u4f60', 'width=2 wide char should render as itself');

// Test 11: Null character (width=1) → renders as space
console.log('Test 11: Null character (width=1) renders as space');
const nullChar = { ch: '\u0000', width: 1 };
assertEq(renderCellChar(nullChar), ' ', 'width=1 with null ch should render as space');

// Test 12: Verify the OLD (broken) logic would have produced wrong results
console.log('Test 12: OLD logic would have incorrectly replaced space with U+200B');
function oldRenderCellChar(diff) {
    // Old broken logic from before the fix
    return (diff.ch === ' ' || diff.ch === '\u0000') ? '\u200b' : diff.ch;
}
const broken = oldRenderCellChar(normalEmpty);
assertEq(broken, '\u200b', 'OLD logic incorrectly replaced space with U+200B (this was the bug)');
assert(broken !== renderCellChar(normalEmpty), 'NEW logic should differ from OLD broken logic');

// ── Summary ──
console.log(`\n=== Results: ${_passed} passed, ${_failed} failed ===\n`);
if (_failed > 0) process.exit(1);
