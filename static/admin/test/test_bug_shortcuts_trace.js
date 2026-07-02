/// test_bug_shortcuts_trace.js — Fix 8.1-8.2
/// Verify all shortcuts have valid action functions and that
/// commonly-needed shortcuts exist.
'use strict';

console.log('\n=== Fix 8.1-8.2: Shortcut trace and missing ones ===\n');

// SKT-001: All _defaultShortcuts have function actions
{
    const broken = [];
    for (const s of window._defaultShortcuts) {
        if (!s.action || typeof s.action !== 'function') {
            broken.push(s.id || s.label || JSON.stringify(s));
        }
    }
    assertEq(broken.length, 0,
        'SKT-001: All ' + window._defaultShortcuts.length + ' shortcuts have valid action functions');
}

// SKT-002: All shortcuts have an 'id' field (required for customization)
{
    const missing = window._defaultShortcuts.filter(s => !s.id);
    assertEq(missing.length, 0, 'SKT-002: All shortcuts have id field');
}

// SKT-003: All shortcuts have a 'label' field (for shortcuts dialog)
{
    const missing = window._defaultShortcuts.filter(s => !s.label);
    assertEq(missing.length, 0, 'SKT-003: All shortcuts have label field');
}

// SKT-004: Screenshot shortcut exists (Alt+P)
{
    const ss = window._defaultShortcuts.find(s => s.id === 'screenshot');
    assert(ss, 'SKT-004a: screenshot shortcut exists');
    if (ss) {
        assertEq(ss.key, 'p', 'SKT-004b: screenshot uses key p');
        assertEq(ss.alt, true, 'SKT-004c: screenshot uses Alt modifier');
    }
}

// SKT-005: No duplicate IDs in shortcuts
{
    const ids = window._defaultShortcuts.map(s => s.id);
    const unique = new Set(ids);
    assertEq(ids.length, unique.size, 'SKT-005: No duplicate shortcut IDs');
}

console.log('\n[Fix 8.1-8.2: Shortcut Trace] Tests complete');