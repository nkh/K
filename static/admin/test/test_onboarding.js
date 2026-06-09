/// test/test_onboarding.js — Tests for onboarding tutorial
require('./setup');

console.log('\n=== onboarding.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.trapFocus = function() {};
globalThis.releaseCurrentFocusTrap = function() {};
globalThis.escHtml = function(s) {
    if (!s) return '';
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
};

// ── checkOnboarding ──
console.log('checkOnboarding tests');
if (typeof checkOnboarding === 'function') {
    // Already completed → skips
    localStorage.setItem('vrw_onboarding_done', '1');
    assert(() => { checkOnboarding(); }, 'checkOnboarding skips when already done');
    localStorage.removeItem('vrw_onboarding_done');

    // Not completed → sets timeout (doesn't throw)
    assert(() => { checkOnboarding(); }, 'checkOnboarding does not throw when not done');
}

// ── openOnboarding ──
console.log('openOnboarding tests');
if (typeof openOnboarding === 'function') {
    const overlay = document.createElement('div');
    overlay.id = 'onboardingOverlay';
    overlay.style.display = 'none';
    const stepEl = document.createElement('span');
    stepEl.id = 'onboardingStep';
    const titleEl = document.createElement('h2');
    titleEl.id = 'onboardingTitle';
    const bodyEl = document.createElement('div');
    bodyEl.id = 'onboardingBody';
    const nextBtn = document.createElement('button');
    nextBtn.id = 'onboardingNextBtn';
    const dontShow = document.createElement('input');
    dontShow.id = 'onboardingDontShow';
    dontShow.type = 'checkbox';
    const spotlight = document.createElement('div');
    spotlight.id = 'onboardingSpotlight';
    const tooltip = document.createElement('div');
    tooltip.id = 'onboardingTooltip';

    openOnboarding();
    assertEq(overlay.style.display, '', 'overlay displayed');
    assertEq(dontShow.checked, false, 'dont-show checkbox unchecked');
    assert(stepEl.textContent.includes('/'), 'step counter shows progress');
}

// ── closeOnboarding ──
console.log('closeOnboarding tests');
if (typeof closeOnboarding === 'function') {
    const overlay = document.createElement('div');
    overlay.id = 'onboardingOverlay';
    overlay.style.display = '';
    const dontShow = document.createElement('input');
    dontShow.id = 'onboardingDontShow';
    dontShow.type = 'checkbox';

    // Without checkbox checked → no localStorage
    dontShow.checked = false;
    closeOnboarding();
    assertEq(overlay.style.display, 'none', 'overlay hidden');
    assertEq(localStorage.getItem('vrw_onboarding_done'), null, 'localStorage not set when not checked');

    // With checkbox checked → saves to localStorage
    overlay.style.display = '';
    dontShow.checked = true;
    closeOnboarding();
    assertEq(overlay.style.display, 'none', 'overlay hidden');
    assertEq(localStorage.getItem('vrw_onboarding_done'), '1', 'localStorage set when checked');
}

// ── nextOnboardingStep ──
console.log('nextOnboardingStep tests');
if (typeof nextOnboardingStep === 'function') {
    const overlay = document.createElement('div');
    overlay.id = 'onboardingOverlay';
    overlay.style.display = '';
    const stepEl = document.createElement('span');
    stepEl.id = 'onboardingStep';
    const titleEl = document.createElement('h2');
    titleEl.id = 'onboardingTitle';
    const bodyEl = document.createElement('div');
    bodyEl.id = 'onboardingBody';
    const nextBtn = document.createElement('button');
    nextBtn.id = 'onboardingNextBtn';
    const dontShow = document.createElement('input');
    dontShow.id = 'onboardingDontShow';
    dontShow.type = 'checkbox';
    const spotlight = document.createElement('div');
    spotlight.id = 'onboardingSpotlight';
    const tooltip = document.createElement('div');
    tooltip.id = 'onboardingTooltip';

    // Start from beginning
    openOnboarding();
    assert(titleEl.textContent.length > 0, 'title rendered');
    assert(bodyEl.textContent.length > 0, 'body rendered');

    // Advance through all steps
    const steps = typeof _onboardingSteps !== 'undefined' ? _onboardingSteps : [];
    for (let i = 1; i < steps.length; i++) {
        nextOnboardingStep();
        assert(overlay.style.display !== 'none' || i === steps.length - 1,
            'step ' + (i + 1) + ' rendered or is last');
    }

    // Last step should close
    nextOnboardingStep();
    assertEq(overlay.style.display, 'none', 'overlay closed after last step');
}

// ── renderOnboardingStep ──
console.log('renderOnboardingStep tests');
if (typeof renderOnboardingStep === 'function') {
    const stepEl = document.createElement('span');
    stepEl.id = 'onboardingStep';
    const titleEl = document.createElement('h2');
    titleEl.id = 'onboardingTitle';
    const bodyEl = document.createElement('div');
    bodyEl.id = 'onboardingBody';
    const nextBtn = document.createElement('button');
    nextBtn.id = 'onboardingNextBtn';
    const spotlight = document.createElement('div');
    spotlight.id = 'onboardingSpotlight';
    const tooltip = document.createElement('div');
    tooltip.id = 'onboardingTooltip';

    assert(() => { renderOnboardingStep(); }, 'renderOnboardingStep does not throw');
    assert(stepEl.textContent.includes('/'), 'step counter has format N/M');
    assert(titleEl.textContent.length > 0, 'title has content');
    assert(bodyEl.textContent.length > 0, 'body has content');

    // Newlines in body should be converted to <br> tags
    const steps = typeof _onboardingSteps !== 'undefined' ? _onboardingSteps : [];
    const stepWithNewline = steps.find(s => s.body.includes('\n'));
    if (stepWithNewline) {
        assert(bodyEl.innerHTML.includes('<br>'), 'newlines converted to <br>');
    }
}

// ── onboarding steps ──
console.log('onboarding steps data');
if (typeof _onboardingSteps !== 'undefined') {
    assert(Array.isArray(_onboardingSteps), '_onboardingSteps is an array');
    assert(_onboardingSteps.length > 0, 'has at least one step');
    assertEq(_onboardingSteps.length, 8, 'has 8 onboarding steps');

    if (_onboardingSteps.length > 0) {
        assert(typeof _onboardingSteps[0].title === 'string', 'step has title');
        assert(_onboardingSteps[0].title.length > 0, 'title is not empty');
        assert(typeof _onboardingSteps[0].body === 'string', 'step has body');
        assert(_onboardingSteps[0].body.length > 0, 'body is not empty');
    }

    // Check specific steps have expected content
    const sidebarStep = _onboardingSteps.find(s => s.title === 'Sidebar');
    assert(sidebarStep !== undefined, 'has Sidebar step');
    assert(sidebarStep.target === '#sidebar', 'Sidebar step targets #sidebar');

    const shortcutsStep = _onboardingSteps.find(s => s.title.includes('Keyboard Shortcuts'));
    assert(shortcutsStep !== undefined, 'has Keyboard Shortcuts step');
    assertEq(shortcutsStep.target, null, 'Keyboard Shortcuts has no target (centered)');

    // All steps have title and body
    for (let i = 0; i < _onboardingSteps.length; i++) {
        assert(_onboardingSteps[i].title !== undefined, 'step ' + i + ' has title');
        assert(_onboardingSteps[i].body !== undefined, 'step ' + i + ' has body');
    }
}

// ── showShortcuts ──
console.log('showShortcuts tests');
if (typeof showShortcuts === 'function') {
    // closeShortcuts removes the overlay
    globalThis.closeShortcuts = function() {
        const el = document.getElementById('shortcutsOverlay');
        if (el) el.remove();
    };

    assert(() => { showShortcuts(); }, 'showShortcuts does not throw');

    const overlay = document.getElementById('shortcutsOverlay');
    assert(overlay !== null, 'shortcuts overlay created');
    assert(overlay.className.includes('shortcuts-overlay'), 'has shortcuts-overlay class');
    assert(overlay.innerHTML.includes('Keyboard Shortcuts'), 'contains title');
    assert(overlay.innerHTML.includes('Ctrl+F'), 'contains Ctrl+F shortcut');
    assert(overlay.innerHTML.includes('Ctrl+Shift+C'), 'contains copy shortcut');
    assert(overlay.innerHTML.includes('Escape'), 'contains Escape shortcut');
    assert(overlay.innerHTML.includes('Alt+N'), 'contains Alt+N shortcut');
    assert(overlay.innerHTML.includes('Close'), 'contains Close button');

    // Cleanup
    closeShortcuts();
}

// ── closeShortcuts ──
console.log('closeShortcuts tests');
if (typeof closeShortcuts === 'function') {
    // No overlay → no crash
    assert(() => { closeShortcuts(); }, 'closeShortcuts no crash without overlay');

    // With overlay → removes it
    const overlay = document.createElement('div');
    overlay.id = 'shortcutsOverlay';
    document.body.appendChild(overlay);
    closeShortcuts();
    // getElementById auto-creates stubs, so check if the element was removed from body
    const remaining = document.getElementById('shortcutsOverlay');
    assert(remaining.parentElement !== document.body, 'overlay removed from body');
    _elementRegistry.delete('shortcutsOverlay'); // cleanup
}

console.log('\n[onboarding.js] Tests complete');