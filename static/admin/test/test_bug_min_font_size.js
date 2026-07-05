/// test/test_bug_min_font_size.js — Fix 4.1
///   "Font size, allow smaller"
///
/// BUG: Math.max(8, ...) prevents font sizes below 8px.
/// Users with high-resolution displays may need smaller fonts.
///
/// FIX: Lower the minimum font size from 8 to 2.

require('./setup');

console.log('\n=== Fix 4.1: Allow smaller font sizes ===\n');

// ──────────────────────────────────────────────────────────────
// FIX41-001: changeFontSize allows going below 8
// ──────────────────────────────────────────────────────────────
console.log('FIX41-001: changeFontSize allows values below 8');
{
    state.fontSize = 8;
    changeFontSize(-1);
    assertEq(state.fontSize, 7, 'FIX41-001a: fontSize can be 7');
    assert(state.fontSize >= 2, 'FIX41-001b: fontSize floor is 2, not 8');
}

// ──────────────────────────────────────────────────────────────
// FIX41-002: changeFontSize respects lower bound of 2
// ──────────────────────────────────────────────────────────────
console.log('FIX41-002: changeFontSize lower bound is 2');
{
    state.fontSize = 2;
    changeFontSize(-1);
    assertEq(state.fontSize, 2, 'FIX41-002a: fontSize clamped at 2');
    changeFontSize(-10);
    assertEq(state.fontSize, 2, 'FIX41-002b: fontSize stays at 2 with large negative delta');
}

// ──────────────────────────────────────────────────────────────
// FIX41-003: changeFontSize upper bound still 28 (no regression)
// ──────────────────────────────────────────────────────────────
console.log('FIX41-003: Upper bound still 28');
{
    state.fontSize = 27;
    changeFontSize(1);
    assertEq(state.fontSize, 28, 'FIX41-003a: fontSize maxes at 28');
    changeFontSize(10);
    assertEq(state.fontSize, 28, 'FIX41-003b: fontSize stays at 28 with large delta');
}

// ──────────────────────────────────────────────────────────────
// FIX41-004: changePanelFontSize also allows below 8
// ──────────────────────────────────────────────────────────────
console.log('FIX41-004: changePanelFontSize allows below 8');
{
    state.panels = [{ id: 'p1', fontSize: 8 }];
    changePanelFontSize('p1', -1);
    assertEq(state.panels[0].fontSize, 7, 'FIX41-004a: panel fontSize can be 7');
}

// ──────────────────────────────────────────────────────────────
// FIX41-005: changePanelFontSize lower bound is 2
// ──────────────────────────────────────────────────────────────
console.log('FIX41-005: changePanelFontSize lower bound is 2');
{
    state.panels = [{ id: 'p2', fontSize: 2 }];
    changePanelFontSize('p2', -5);
    assertEq(state.panels[0].fontSize, 2, 'FIX41-005a: panel fontSize clamped at 2');
}