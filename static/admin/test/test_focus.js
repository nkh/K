/// test/test_focus.js — Tests for focus management
require('./setup');

console.log('\n=== focus.js Tests ===\n');

// ── trapFocus ──
console.log('trapFocus tests');
assert(typeof trapFocus === 'function', 'trapFocus is a function');

// Create a mock container with focusable elements
const container = document.createElement('div');
container.id = 'test-modal';
const input1 = document.createElement('input');
input1.id = 'input-1';
input1.tabIndex = 0;
const input2 = document.createElement('input');
input2.id = 'input-2';
input2.tabIndex = 0;
const btn = document.createElement('button');
btn.id = 'btn-1';
btn.tabIndex = 0;
container.appendChild(input1);
container.appendChild(input2);
container.appendChild(btn);

const release = trapFocus(container);
assert(typeof release === 'function', 'trapFocus returns release function');

// ── releaseCurrentFocusTrap ──
console.log('releaseCurrentFocusTrap tests');
assert(typeof releaseCurrentFocusTrap === 'function', 'releaseCurrentFocusTrap is a function');
releaseCurrentFocusTrap(); // Should not throw

// Release via returned function
release(); // Should not throw

console.log('\n[focus.js] Tests complete');
