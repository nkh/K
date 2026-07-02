/// test_bug_touchpad_scroll.js — Fix 7.1
/// Verify that .vtty-container has touch-action: pan-y so touchpad
/// scrolling works correctly in all browsers.
'use strict';

console.log('\n=== Fix 7.1: Touchpad scroll CSS verification ===\n');

const fs = require('fs');
const css = fs.readFileSync(__dirname + '/../style.css', 'utf8');

// TPS-001: .vtty-container has touch-action CSS property
{
    // Match the main .vtty-container rule (not :root overrides or nested selectors)
    // Use a regex that requires .vtty-container at start of line or after comma/space
    const vttyRules = css.split(/\n/).filter(line => /^[^.]*\.vtty-container\s*\{/.test(line));
    // Combine all matching rule bodies
    const mainRule = vttyRules.find(r => /overflow/.test(r));
    assert(mainRule, 'TPS-001a: .vtty-container rule with overflow exists');
    
    const hasTouchAction = /touch-action\s*:/.test(mainRule);
    assert(hasTouchAction, 'TPS-001b: .vtty-container has touch-action property');
    
    const hasPanY = /touch-action\s*:\s*[^;]*pan-y/.test(mainRule);
    assert(hasPanY, 'TPS-001c: touch-action includes pan-y for vertical scrolling');
}

// TPS-002: .vtty-container has overflow: auto (not hidden)
{
    const vttyRules = css.split(/\n/).filter(line => /^[^.]*\.vtty-container\s*\{/.test(line));
    const mainRule = vttyRules.find(r => /overflow/.test(r));
    assert(mainRule, 'TPS-002a: .vtty-container rule found');
    const hasOverflowAuto = /overflow\s*:\s*auto/.test(mainRule);
    assert(hasOverflowAuto, 'TPS-002b: .vtty-container has overflow: auto');
}

// TPS-003: No touch-action: none on vtty or its parents
{
    const noTouchNone = !/touch-action\s*:\s*none/.test(css);
    assert(noTouchNone, 'TPS-003: No element has touch-action: none (would block scrolling)');
}

console.log('\n[Fix 7.1: Touchpad Scroll] Tests complete');