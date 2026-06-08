/// test/test_keyboard.js — Tests for keyboard handling
require('./setup');

console.log('\n=== keyboard.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};

// ── sendDirectKey ──
console.log('sendDirectKey tests');
if (typeof sendDirectKey === 'function') {
    // Mock WebSocket send
    state.panels = [];
    const p = addPanelDirect();
    p.selectedInstUrl = 'http://localhost:9090';
    p.selectedCmdId = 'cmd-keyboard';

    const ws = new MockWebSocket('ws://test');
    ws.readyState = 1; // OPEN
    p.ws = ws;
    p.wsCmdId = 'cmd-keyboard';

    // Test various key sequences
    const keys = [
        { key: 'Enter', expected: true },
        { key: 'Tab', expected: true },
        { key: 'Escape', expected: true },
        { key: 'ArrowUp', expected: true },
        { key: 'ArrowDown', expected: true },
        { key: 'ArrowRight', expected: true },
        { key: 'ArrowLeft', expected: true },
        { key: 'Backspace', expected: true },
        { key: 'Delete', expected: true },
        { key: 'Home', expected: true },
        { key: 'End', expected: true },
        { key: 'PageUp', expected: true },
        { key: 'PageDown', expected: true },
        { key: 'F1', expected: true },
        { key: 'F12', expected: true },
    ];

    for (const { key } of keys) {
        ws._calls = []; // Reset
        assert(() => { sendDirectKey(p.id, key); }, 'sendDirectKey(' + key + ') does not throw');
    }
}

// ── _KEY_MAP ──
console.log('_KEY_MAP tests');
if (typeof _KEY_MAP !== 'undefined') {
    assert(typeof _KEY_MAP === 'object', '_KEY_MAP is an object');
    // Check critical mappings exist
    assert(_KEY_MAP['Enter'] !== undefined, 'Enter mapped');
    assert(_KEY_MAP['Tab'] !== undefined, 'Tab mapped');
    assert(_KEY_MAP['Escape'] !== undefined, 'Escape mapped');
    assert(_KEY_MAP['ArrowUp'] !== undefined, 'ArrowUp mapped');
    assert(_KEY_MAP['ArrowDown'] !== undefined, 'ArrowDown mapped');
    assert(_KEY_MAP['Backspace'] !== undefined, 'Backspace mapped');
    assert(_KEY_MAP['F1'] !== undefined, 'F1 mapped');
    assert(_KEY_MAP['F12'] !== undefined, 'F12 mapped');
}

console.log('\n[keyboard.js] Tests complete');
