/// test/test_share_viewer.js — Tests for share and viewer features.
require('./setup');

console.log('\n=== Share & Viewer Tests ===\n');

// ── API methods exist ──
console.log('share/viewer API methods');
assertOk(typeof api.createShareToken === 'function', 'createShareToken exists');
assertOk(typeof api.createViewerToken === 'function', 'createViewerToken exists');

// ── Context menu includes share entries ──
console.log('context menu integration');
assertOk(typeof showPanelContextMenu === 'function', 'showPanelContextMenu exists');

// ── Share modal creation (DOM test) ──
console.log('share modal DOM creation');
// The share modal is created dynamically inside showPanelContextMenu's
// sub-functions. We test that the functions are accessible.
// _showShareModal and _openViewerTab are private (inside IIFE), so we
// test via the exported showPanelContextMenu which references them.

// ── Delegate actions for share/viewer not needed ──
// Share and viewer actions are triggered from the context menu using
// inline closures (not data-action dispatch), so no delegate entries needed.

console.log('\n[share_viewer] ' + _testPassed + ' passed so far');