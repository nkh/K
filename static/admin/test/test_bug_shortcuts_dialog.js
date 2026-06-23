/// test/test_bug_shortcuts_dialog.js — Fix 2.2
///   "Keyboard shortcut dialog not long enough"
///
/// BUG: The shortcuts dialog has no max-height or overflow, so with many
/// shortcuts it grows beyond the viewport and becomes unusable.
///
/// FIX: Add max-height to .shortcuts-panel, overflow-y to .shortcuts-scroll
/// wrapper, and a flex layout so the Close button stays pinned.

require('./setup');

console.log('\n=== Fix 2.2: Shortcuts dialog has scrollbar ===\n');

const fs = require('fs');
const path = require('path');

const cssPath = path.join(__dirname, '..', 'style.css');
const css = fs.readFileSync(cssPath, 'utf-8');

// ──────────────────────────────────────────────────────────────
// FIX22-001: .shortcuts-panel has max-height
// ──────────────────────────────────────────────────────────────
console.log('FIX22-001: .shortcuts-panel has max-height');
{
    const ruleRegex = /\.shortcuts-panel\s*\{[^}]*max-height\s*:/;
    assert(ruleRegex.test(css),
        'FIX22-001: .shortcuts-panel must have max-height');
}

// ──────────────────────────────────────────────────────────────
// FIX22-002: .shortcuts-scroll has overflow-y: auto
// ──────────────────────────────────────────────────────────────
console.log('FIX22-002: .shortcuts-scroll has overflow-y: auto');
{
    const ruleRegex = /\.shortcuts-scroll\s*\{[^}]*overflow-y\s*:\s*auto/;
    assert(ruleRegex.test(css),
        'FIX22-002: .shortcuts-scroll must have overflow-y: auto');
}

// ──────────────────────────────────────────────────────────────
// FIX22-003: .shortcuts-footer exists for pinned close button
// ──────────────────────────────────────────────────────────────
console.log('FIX22-003: .shortcuts-footer exists');
{
    const ruleRegex = /\.shortcuts-footer\s*\{/;
    assert(ruleRegex.test(css),
        'FIX22-003: .shortcuts-footer rule must exist');
}

// ──────────────────────────────────────────────────────────────
// FIX22-004: Close button is outside the scroll wrapper in HTML
// ──────────────────────────────────────────────────────────────
console.log('FIX22-004: Close button outside scroll wrapper');
{
    const srcPath = path.join(__dirname, '..', 'modules', 'misc.js');
    const src = fs.readFileSync(srcPath, 'utf-8');
    // The template must have shortcuts-scroll wrapper around the table
    // and shortcuts-footer div with the Close button after it
    assert(src.includes('shortcuts-scroll'), 'FIX22-004a: shortcuts-scroll wrapper in HTML template');
    assert(src.includes('shortcuts-footer'), 'FIX22-004b: shortcuts-footer in HTML template');
    // Verify close button is inside shortcuts-footer, not shortcuts-scroll
    const scrollIdx = src.indexOf('shortcuts-scroll');
    const footerIdx = src.indexOf('shortcuts-footer');
    assert(scrollIdx < footerIdx, 'FIX22-004c: scroll wrapper comes before footer');
    // The footer div should contain the Close button text
    const afterFooter = src.substring(footerIdx, footerIdx + 100);
    assert(afterFooter.includes('Close'), 'FIX22-004d: footer contains Close button');
}