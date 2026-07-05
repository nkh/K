/// test/test_bug_header_focus_color.js — Fix 2.1
///   "Selecting a pane changes the pane header background color"
///
/// BUG: CSS rules `.panel.focused > .panel-header` and
/// `.split-pane.focused > .panel-header` override the server-assigned
/// header background with color-mix() when a pane is focused.
/// The header should keep its server color regardless of focus state.
/// Focus is indicated solely by box-shadow (inset border), not header color.
///
/// FIX: Remove the two offending CSS rules from style.css.

require('./setup');

console.log('\n=== Fix 2.1: Header color must not change on focus ===\n');

const fs = require('fs');
const path = require('path');

const cssPath = path.join(__dirname, '..', 'style.css');
const css = fs.readFileSync(cssPath, 'utf-8');

// ──────────────────────────────────────────────────────────────
// FIX21-001: No CSS rule sets background on .panel.focused > .panel-header
// ──────────────────────────────────────────────────────────────
console.log('FIX21-001: No focused panel header background override');
{
    // Match any CSS rule that targets .panel.focused > .panel-header
    // and sets a background property (the offending color-mix rule).
    const ruleRegex = /\.panel\.focused\s*>\s*\.panel-header\s*\{[^}]*background\s*:/;
    assert(!ruleRegex.test(css),
        'FIX21-001: style.css must not override .panel.focused > .panel-header background');
}

// ──────────────────────────────────────────────────────────────
// FIX21-002: No CSS rule sets background on .split-pane.focused > .panel-header
// ──────────────────────────────────────────────────────────────
console.log('FIX21-002: No focused split-pane header background override');
{
    const ruleRegex = /\.split-pane\.focused\s*>\s*\.panel-header\s*\{[^}]*background\s*:/;
    assert(!ruleRegex.test(css),
        'FIX21-002: style.css must not override .split-pane.focused > .panel-header background');
}

// ──────────────────────────────────────────────────────────────
// FIX21-003: Focus indicator still uses box-shadow (not removed)
// ──────────────────────────────────────────────────────────────
console.log('FIX21-003: Focus box-shadow indicators still present');
{
    // The .panel.focused rule with box-shadow should still exist
    const panelFocus = /\.panel\.focused\s*\{[^}]*box-shadow\s*:/;
    assert(panelFocus.test(css),
        'FIX21-003: .panel.focused box-shadow rule must still exist');
    const splitFocus = /\.split-pane\.focused\s*\{[^}]*box-shadow\s*:/;
    assert(splitFocus.test(css),
        'FIX21-003: .split-pane.focused box-shadow rule must still exist');
}