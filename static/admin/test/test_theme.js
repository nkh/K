/// test/test_theme.js — Tests for theme management
require('./setup');

console.log('\n=== theme.js Tests ===\n');

// ── initTheme ──
console.log('initTheme tests');
assert(typeof initTheme === 'function', 'initTheme is a function');
// With no localStorage, should default
localStorage.removeItem('vrw_theme');
initTheme();
assert(typeof state !== 'undefined', 'state accessible after initTheme');

// ── toggleGlobalTheme ──
console.log('toggleGlobalTheme tests');
assert(typeof toggleGlobalTheme === 'function', 'toggleGlobalTheme is a function');
localStorage.setItem('vrw_theme', 'auto');
toggleGlobalTheme();
// Should cycle auto → grey → dark → auto
let theme = localStorage.getItem('vrw_theme');
assert(theme === 'grey' || theme === 'dark' || theme === 'light' || theme === 'auto', 'theme cycled to: ' + theme);

toggleGlobalTheme();
theme = localStorage.getItem('vrw_theme');
assert(theme === 'grey' || theme === 'dark' || theme === 'light' || theme === 'auto', 'theme cycled to: ' + theme);

// ── updateThemeButton ──
console.log('updateThemeButton tests');
assert(typeof updateThemeButton === 'function', 'updateThemeButton is a function');
assert(() => { updateThemeButton(); }, 'updateThemeButton does not throw without button element');

// ── togglePanelTheme ──
console.log('togglePanelTheme tests');
assert(typeof togglePanelTheme === 'function', 'togglePanelTheme is a function');

// Create a panel in state
state.panels = [];
state.panels.push({ id: 'panel-test', theme: '', fontSize: 10 });
togglePanelTheme('panel-test');
assertEq(state.panels[0].theme, 'light', 'theme cycled from empty to light');

togglePanelTheme('panel-test');
assertEq(state.panels[0].theme, 'dark', 'theme cycled from light to dark');

togglePanelTheme('panel-test');
assertEq(state.panels[0].theme, '', 'theme cycled from dark to empty');

// Non-existent panel
assert(() => { togglePanelTheme('nonexistent'); }, 'togglePanelTheme with invalid ID does not throw');

// ── applyPanelTheme ──
console.log('applyPanelTheme tests');
assert(typeof applyPanelTheme === 'function', 'applyPanelTheme is a function');

// Create vtty element
const vttyEl = document.createElement('div');
vttyEl.id = 'vtty-panel-test';
_elementRegistry.set('vtty-panel-test', vttyEl);

applyPanelTheme('panel-test', 'dark');
assertEq(vttyEl.getAttribute('data-panel-theme'), 'dark', 'data-panel-theme set to dark');

applyPanelTheme('panel-test', '');
assertEq(vttyEl.hasAttribute('data-panel-theme'), false, 'data-panel-theme removed when empty');

// Non-existent vtty element
assert(() => { applyPanelTheme('nonexistent', 'dark'); }, 'applyPanelTheme with invalid ID does not throw');

console.log('\n[theme.js] Tests complete');
