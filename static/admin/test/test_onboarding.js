/// test/test_onboarding.js — Tests for onboarding tutorial
require('./setup');

console.log('\n=== onboarding.js Tests ===\n');

resetTestState();

// ── checkOnboarding ──
console.log('checkOnboarding tests');
if (typeof checkOnboarding === 'function') {
    localStorage.removeItem('vrw_onboarding_done');
    assert(() => { checkOnboarding(); }, 'checkOnboarding does not throw');
}

// ── openOnboarding ──
console.log('openOnboarding tests');
if (typeof openOnboarding === 'function') {
    const overlay = document.createElement('div');
    overlay.id = 'onboardingOverlay';
    assert(() => { openOnboarding(); }, 'openOnboarding does not throw');
}

// ── closeOnboarding ──
console.log('closeOnboarding tests');
if (typeof closeOnboarding === 'function') {
    assert(() => { closeOnboarding(); }, 'closeOnboarding does not throw');
}

// ── nextOnboardingStep ──
console.log('nextOnboardingStep tests');
if (typeof nextOnboardingStep === 'function') {
    assert(() => { nextOnboardingStep(); }, 'nextOnboardingStep does not throw');
}

// ── renderOnboardingStep ──
console.log('renderOnboardingStep tests');
if (typeof renderOnboardingStep === 'function') {
    assert(() => { renderOnboardingStep(); }, 'renderOnboardingStep does not throw');
}

// ── onboarding steps ──
console.log('onboarding steps data');
if (typeof _onboardingSteps !== 'undefined') {
    assert(Array.isArray(_onboardingSteps), '_onboardingSteps is an array');
    if (_onboardingSteps.length > 0) {
        assert(typeof _onboardingSteps[0].title === 'string', 'step has title');
        assert(typeof _onboardingSteps[0].body === 'string', 'step has body');
        assert(typeof _onboardingSteps[0].target === 'string', 'step has target');
    }
}

// ── showShortcuts ──
console.log('showShortcuts tests');
if (typeof showShortcuts === 'function') {
    assert(() => { showShortcuts(); }, 'showShortcuts does not throw');
}

// ── closeShortcuts ──
console.log('closeShortcuts tests');
if (typeof closeShortcuts === 'function') {
    assert(() => { closeShortcuts(); }, 'closeShortcuts does not throw');
}

console.log('\n[onboarding.js] Tests complete');
