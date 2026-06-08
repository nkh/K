// ─── Onboarding Tutorial & Keyboard Shortcuts Help ───
// First-run tutorial overlay and shortcuts reference panel.
(function() {
    'use strict';

// ─── Onboarding Tutorial ───
const ONBOARDING_KEY = 'vrw_onboarding_done';
const ONBOARDING_STEPS = [
    { target: '#sidebar', title: 'Sidebar', body: 'Browse servers, running commands, and spawn new commands from here. Drag commands by their grip handle to reorder them. Pin important commands with the ◉ button.' },
    { target: '#tab-servers', title: 'Servers Tab', body: 'View all connected vrw instances. Click the connection indicator to add new servers. Resource polling shows live CPU and memory usage.' },
    { target: '#tab-spawn', title: 'Spawn Tab', body: 'Launch commands on any connected server. Set environment variables, working directory, and terminal size. Press Tab for path completion.' },
    { target: '#sharedToolbar', title: 'Shared Toolbar', body: 'Controls for the focused panel: restart, resize font, toggle selection mode, copy, export, screenshot, and layout presets.' },
    { target: '#view-vtty', title: 'Terminal Panels', body: 'Each panel shows a terminal. Click the panel header to focus it. Double-click the command name to rename the panel. Use the ☰ Commands button for a unified command manager.' },
    { target: '#bottomBar', title: 'Status Bar', body: 'Shows the active command name, arguments, PID, runtime, cursor position, terminal dimensions, and scrollback indicator. Toggle it with the Status button.' },
    { target: null, title: 'Keyboard Shortcuts', body: 'Ctrl+F — search terminal\nCtrl+Shift+C — copy selection\nCtrl+Shift+R — restart command\nAlt+S — toggle selection mode\nAlt+N — new panel\nAlt+T — toggle theme\nPress ? (on focus) to see all shortcuts' },
    { target: null, title: 'You\'re all set!', body: 'Right-click panels and commands for more options. Drag commands from the sidebar onto panels. Pin commands to auto-restart them on exit. Check the ☰ Commands button to manage all commands at once.' },
];

let _onboardingStep = 0;

function checkOnboarding() {
    if (localStorage.getItem(ONBOARDING_KEY)) return;
    // Only show after a short delay to let the UI settle
    setTimeout(() => {
        const sidebar = document.getElementById('sidebar');
        const viewVtty = document.getElementById('view-vtty');
        if (sidebar && viewVtty) openOnboarding();
    }, 1500);
}

function openOnboarding() {
    _onboardingStep = 0;
    document.getElementById('onboardingOverlay').style.display = '';
    document.getElementById('onboardingDontShow').checked = false;
    renderOnboardingStep();
}

function closeOnboarding() {
    document.getElementById('onboardingOverlay').style.display = 'none';
    if (document.getElementById('onboardingDontShow').checked) {
        localStorage.setItem(ONBOARDING_KEY, '1');
    }
}

function nextOnboardingStep() {
    _onboardingStep++;
    if (_onboardingStep >= ONBOARDING_STEPS.length) {
        closeOnboarding();
        return;
    }
    renderOnboardingStep();
}

function renderOnboardingStep() {
    const step = ONBOARDING_STEPS[_onboardingStep];
    const total = ONBOARDING_STEPS.length;
    document.getElementById('onboardingStep').textContent = (_onboardingStep + 1) + '/' + total;
    document.getElementById('onboardingTitle').textContent = step.title;
    // Support newlines in body text
    document.getElementById('onboardingBody').innerHTML = escHtml(step.body).replace(/\n/g, '<br>');
    const nextBtn = document.getElementById('onboardingNextBtn');
    nextBtn.textContent = _onboardingStep === total - 1 ? 'Done' : 'Next';

    // Position spotlight on target element
    const spotlight = document.getElementById('onboardingSpotlight');
    const tooltip = document.getElementById('onboardingTooltip');
    if (step.target) {
        const el = document.querySelector(step.target);
        if (el) {
            const rect = el.getBoundingClientRect();
            spotlight.style.display = 'block';
            spotlight.style.top = (rect.top - 4) + 'px';
            spotlight.style.left = (rect.left - 4) + 'px';
            spotlight.style.width = (rect.width + 8) + 'px';
            spotlight.style.height = (rect.height + 8) + 'px';

            // Position tooltip below or beside the spotlight
            const tooltipMaxWidth = Math.min(350, window.innerWidth - 40);
            tooltip.style.maxWidth = tooltipMaxWidth + 'px';
            if (rect.bottom + 16 + 200 < window.innerHeight) {
                tooltip.style.top = (rect.bottom + 12) + 'px';
                tooltip.style.left = Math.max(12, Math.min(rect.left, window.innerWidth - tooltipMaxWidth - 12)) + 'px';
            } else {
                tooltip.style.top = Math.max(12, rect.top - 200) + 'px';
                tooltip.style.left = Math.max(12, Math.min(rect.left, window.innerWidth - tooltipMaxWidth - 12)) + 'px';
            }
            return;
        }
    }
    // No target — center the tooltip
    spotlight.style.display = 'none';
    tooltip.style.top = '50%';
    tooltip.style.left = '50%';
    tooltip.style.transform = 'translate(-50%, -50%)';
    setTimeout(() => { tooltip.style.transform = ''; }, 0);
}

// ─── Keyboard Shortcuts Help ───
function showShortcuts() {
    closeShortcuts();
    const overlay = document.createElement('div');
    overlay.className = 'shortcuts-overlay';
    overlay.id = 'shortcutsOverlay';
    overlay.onclick = (e) => { if (e.target === overlay) closeShortcuts(); };
    overlay.innerHTML = `<div class="shortcuts-panel">
        <h2>Keyboard Shortcuts</h2>
        <table>
            <tr><td>?</td><td>Show this help</td></tr>
            <tr><td>Ctrl+F</td><td>Search in terminal</td></tr>
            <tr><td>Ctrl+Shift+C</td><td>Copy terminal selection</td></tr>
            <tr><td>Ctrl+Shift+S / Alt+S</td><td>Toggle selection mode</td></tr>
            <tr><td>Ctrl+Shift+E</td><td>Export terminal as text</td></tr>
            <tr><td>Ctrl+Shift+R</td><td>Restart command</td></tr>
            <tr><td>Escape</td><td>Close search / menu</td></tr>
            <tr><td>Alt+Left / Alt+Right</td><td>Navigate prev/next command</td></tr>
            <tr><td>Alt+T</td><td>Toggle panel theme</td></tr>
            <tr><td>Alt+N</td><td>Add new panel</td></tr>
            <tr><td>Any key</td><td>Focus key input (when not in a field)</td></tr>
            <tr><td>Enter</td><td>Send keystrokes to terminal</td></tr>
        </table>
        <p style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.5rem;">Click on the terminal to focus the key input field.</p>
        <div style="text-align:right;margin-top:0.75rem;">
            <button class="btn" onclick="closeShortcuts()">Close</button>
        </div>
    </div>`;
    document.body.appendChild(overlay);
    // Trap focus inside the shortcuts panel and focus the close button
    const shortcutsPanel = overlay.querySelector('.shortcuts-panel');
    if (shortcutsPanel) trapFocus(shortcutsPanel);
    const closeBtn = overlay.querySelector('button');
    if (closeBtn) closeBtn.focus();
}

function closeShortcuts() {
    releaseCurrentFocusTrap();
    const el = document.getElementById('shortcutsOverlay');
    if (el) el.remove();
}

    // Expose to global scope
    window.checkOnboarding = checkOnboarding;
    window.openOnboarding = openOnboarding;
    window.closeOnboarding = closeOnboarding;
    window.nextOnboardingStep = nextOnboardingStep;
    window.showShortcuts = showShortcuts;
    window.closeShortcuts = closeShortcuts;
})();
