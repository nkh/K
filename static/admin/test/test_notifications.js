/// test/test_notifications.js — Tests for notifications and sound
require('./setup');

console.log('\n=== notifications.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};

// ── notifyCommandEnded ──
console.log('notifyCommandEnded tests');
if (typeof notifyCommandEnded === 'function') {
    assert(() => { notifyCommandEnded('cmd-1'); }, 'notifyCommandEnded does not throw');
}

// ── requestNotificationPermission ──
console.log('requestNotificationPermission tests');
if (typeof requestNotificationPermission === 'function') {
    // Returns a promise
    const result = requestNotificationPermission();
    assert(result instanceof Promise || result === undefined, 'returns promise or undefined');
}

// ── initSoundToggle ──
console.log('initSoundToggle tests');
if (typeof initSoundToggle === 'function') {
    const btn = document.createElement('button');
    btn.id = 'soundBtn';
    assert(() => { initSoundToggle(); }, 'initSoundToggle does not throw');
}

// ── toggleSoundNotifications ──
console.log('toggleSoundNotifications tests');
if (typeof toggleSoundNotifications === 'function') {
    state.soundEnabled = false;
    toggleSoundNotifications();
    assertEq(state.soundEnabled, true, 'sound toggled on');
    toggleSoundNotifications();
    assertEq(state.soundEnabled, false, 'sound toggled off');
}

// ── playExitSound ──
console.log('playExitSound tests');
if (typeof playExitSound === 'function') {
    assert(() => { playExitSound(true); }, 'playExitSound success does not throw');
    assert(() => { playExitSound(false); }, 'playExitSound failure does not throw');
}

// ── pollResources ──
console.log('pollResources tests');
if (typeof pollResources === 'function') {
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', _commands: [] }];
    assert(() => { pollResources(); }, 'pollResources does not throw');
}

// ── checkForExitedCommands ──
console.log('checkForExitedCommands tests');
if (typeof checkForExitedCommands === 'function') {
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', _commands: [
        { id: 'cmd-1', name: 'htop', alive: true, exit_code: null }
    ]}];
    assert(() => { checkForExitedCommands(); }, 'checkForExitedCommands does not throw');
}

// ── updateSidebarResourceText ──
console.log('updateSidebarResourceText tests');
if (typeof updateSidebarResourceText === 'function') {
    assert(() => { updateSidebarResourceText(); }, 'updateSidebarResourceText does not throw');
}

console.log('\n[notifications.js] Tests complete');
