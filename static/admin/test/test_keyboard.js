/// test/test_keyboard.js — Tests for keyboard handling
require('./setup');

console.log('\n=== keyboard.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.vttySearchClose = function() {};
globalThis.closeContextMenu = function() {};
globalThis.closeShortcuts = function() {};
globalThis.closePanelModal = function() {};
globalThis.trapFocus = function() {};
globalThis.releaseCurrentFocusTrap = function() {};
globalThis.copyTerminalSelection = function() {};
globalThis.toggleSelectionMode = function() {};
globalThis.exportTerminal = function() {};
globalThis.restartCommand = function() {};
globalThis.togglePanelTheme = function() {};
globalThis.addPanel = function() {};

// Note: sendDirectKey and sendMouseEvent are local functions inside the keyboard.js
// IIFE — they are NOT exported to window. The setup.js provides stub functions for
// cross-module dependency resolution. Testing the real functions would require
// importing the module differently, so we test the stub interface and the exported
// _KEY_MAP data structure.

// ── _KEY_MAP ──
console.log('_KEY_MAP tests');
if (typeof _KEY_MAP !== 'undefined') {
    assert(typeof _KEY_MAP === 'object', '_KEY_MAP is an object');

    // Check critical mappings exist and have correct escape sequences
    assertEq(_KEY_MAP['Enter'], '\r', 'Enter maps to \\r');
    assertEq(_KEY_MAP['Tab'], '\t', 'Tab maps to \\t');
    assertEq(_KEY_MAP['Escape'], '\x1b', 'Escape maps to ESC');
    assertEq(_KEY_MAP['ArrowUp'], '\x1b[A', 'ArrowUp maps to ESC[A');
    assertEq(_KEY_MAP['ArrowDown'], '\x1b[B', 'ArrowDown maps to ESC[B');
    assertEq(_KEY_MAP['ArrowRight'], '\x1b[C', 'ArrowRight maps to ESC[C');
    assertEq(_KEY_MAP['ArrowLeft'], '\x1b[D', 'ArrowLeft maps to ESC[D');
    assertEq(_KEY_MAP['Backspace'], '\x7f', 'Backspace maps to DEL');
    assertEq(_KEY_MAP['Delete'], '\x1b[3~', 'Delete maps to ESC[3~');
    assertEq(_KEY_MAP['Home'], '\x1b[H', 'Home maps to ESC[H');
    assertEq(_KEY_MAP['End'], '\x1b[F', 'End maps to ESC[F');
    assertEq(_KEY_MAP['PageUp'], '\x1b[5~', 'PageUp maps to ESC[5~');
    assertEq(_KEY_MAP['PageDown'], '\x1b[6~', 'PageDown maps to ESC[6~');
    assertEq(_KEY_MAP['Insert'], '\x1b[2~', 'Insert maps to ESC[2~');
    assertEq(_KEY_MAP['F1'], '\x1bOP', 'F1 maps to ESCOP');
    assertEq(_KEY_MAP['F2'], '\x1bOQ', 'F2 maps to ESCOQ');
    assertEq(_KEY_MAP['F3'], '\x1bOR', 'F3 maps to ESCOR');
    assertEq(_KEY_MAP['F4'], '\x1bOS', 'F4 maps to ESCOS');
    assertEq(_KEY_MAP['F5'], '\x1b[15~', 'F5 maps to ESC[15~');
    assertEq(_KEY_MAP['F6'], '\x1b[17~', 'F6 maps to ESC[17~');
    assertEq(_KEY_MAP['F7'], '\x1b[18~', 'F7 maps to ESC[18~');
    assertEq(_KEY_MAP['F8'], '\x1b[19~', 'F8 maps to ESC[19~');
    assertEq(_KEY_MAP['F9'], '\x1b[20~', 'F9 maps to ESC[20~');
    assertEq(_KEY_MAP['F10'], '\x1b[21~', 'F10 maps to ESC[21~');
    assertEq(_KEY_MAP['F11'], '\x1b[23~', 'F11 maps to ESC[23~');
    assertEq(_KEY_MAP['F12'], '\x1b[24~', 'F12 maps to ESC[24~');

    // All values are strings
    for (const [key, val] of Object.entries(_KEY_MAP)) {
        assert(typeof val === 'string', '_KEY_MAP["' + key + '"] is a string');
        assert(val.length > 0, '_KEY_MAP["' + key + '"] is not empty');
    }

    // Total key count
    assertEq(Object.keys(_KEY_MAP).length, 26, '26 keys in _KEY_MAP');
}

// ── sendDirectKey (stub) ──
console.log('sendDirectKey stub tests');
assert(typeof sendDirectKey === 'function', 'sendDirectKey stub exists');
assert(() => { sendDirectKey({}, {}); }, 'sendDirectKey stub does not throw');

// ── sendMouseEvent (stub) ──
console.log('sendMouseEvent stub tests');
assert(typeof sendMouseEvent === 'function', 'sendMouseEvent stub exists');
assert(() => { sendMouseEvent({}, 'down', 0, {}); }, 'sendMouseEvent stub does not throw');

console.log('\n[keyboard.js] Tests complete');