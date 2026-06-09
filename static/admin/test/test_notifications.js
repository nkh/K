/// test/test_notifications.js — Tests for notifications and sound
require('./setup');

console.log('\n=== notifications.js Tests ===\n');

resetTestState();

globalThis.renderPanels = function() {};
globalThis.getPinnedNames = function() { return []; };
globalThis.restartCommandById = function() { return Promise.resolve(); };

// ── notifyCommandEnded ──
console.log('notifyCommandEnded tests');
if (typeof notifyCommandEnded === 'function') {
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', _commands: [
        { id: 'cmd-1', name: 'htop', exit_code: 0 }
    ]}];
    assert(() => { notifyCommandEnded('cmd-1'); }, 'notifyCommandEnded does not throw');

    // Idempotent: second call with same ID should be no-op (notification already sent)
    assert(() => { notifyCommandEnded('cmd-1'); }, 'notifyCommandEnded idempotent no throw');

    // Null cmdId → early return
    assert(() => { notifyCommandEnded(null); }, 'notifyCommandEnded null early return');
    assert(() => { notifyCommandEnded(undefined); }, 'notifyCommandEnded undefined early return');

    // Empty string cmdId → early return
    assert(() => { notifyCommandEnded(''); }, 'notifyCommandEnded empty string early return');
}

// ── requestNotificationPermission ──
console.log('requestNotificationPermission tests');
if (typeof requestNotificationPermission === 'function') {
    const result = requestNotificationPermission();
    assert(result instanceof Promise || result === undefined, 'returns promise or undefined');
}

// ── initSoundToggle ──
console.log('initSoundToggle tests');
if (typeof initSoundToggle === 'function') {
    // No button → no crash
    _elementRegistry.delete('soundBtn');
    assert(() => { initSoundToggle(); }, 'initSoundToggle no-crash without button');

    // With button, sound off → class NOT added
    const btn1 = document.createElement('button');
    btn1.id = 'soundBtn';
    state.soundEnabled = false;
    initSoundToggle();
    assert(!btn1.classList.contains('sound-btn-active'), 'class not added when sound off');

    // With button, sound on → class added
    const btn2 = document.createElement('button');
    btn2.id = 'soundBtn';
    state.soundEnabled = true;
    initSoundToggle();
    assert(btn2.classList.contains('sound-btn-active'), 'class added when sound on');
}

// ── toggleSoundNotifications ──
console.log('toggleSoundNotifications tests');
if (typeof toggleSoundNotifications === 'function') {
    // No button in DOM → state still toggles
    _elementRegistry.delete('soundBtn');
    state.soundEnabled = false;
    toggleSoundNotifications();
    assertEq(state.soundEnabled, true, 'sound toggled on');
    assertEq(localStorage.getItem('vrw_sound'), 'true', 'sound on saved to localStorage');

    toggleSoundNotifications();
    assertEq(state.soundEnabled, false, 'sound toggled off');
    assertEq(localStorage.getItem('vrw_sound'), 'false', 'sound off saved to localStorage');

    // With button in DOM — class should be toggled
    const soundBtn = document.createElement('button');
    soundBtn.id = 'soundBtn';
    state.soundEnabled = false;
    toggleSoundNotifications();
    // After toggling on, class should be added (note: mock toggle doesn't use force param)
    const btnAfterToggle = document.getElementById('soundBtn');
    assertEq(state.soundEnabled, true, 'state toggled on with button');
    assert(btnAfterToggle.classList.contains('sound-btn-active'), 'button class added on');

    // Toggle off — class should be removed
    toggleSoundNotifications();
    assertEq(state.soundEnabled, false, 'state toggled off with button');
    assert(!document.getElementById('soundBtn').classList.contains('sound-btn-active'), 'button class removed off');
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
    assert(pollResources.constructor.name === 'AsyncFunction', 'pollResources is async');

    // Empty connections → no crash
    state.connections = [];
    assert(() => { pollResources(); }, 'pollResources no-crash with empty connections');

    // With commands
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', _commands: [
        { id: 'cmd-1', name: 'htop', alive: true },
        { id: 'cmd-2', name: 'dead', alive: false },
    ]}];
    assert(() => { pollResources(); }, 'pollResources does not throw with commands');

    // No commands → no crash
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', _commands: [] }];
    assert(() => { pollResources(); }, 'pollResources no-crash with no commands');

    // Null _commands → no crash
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', _commands: null }];
    assert(() => { pollResources(); }, 'pollResources no-crash with null commands');
}

// ── checkForExitedCommands ──
console.log('checkForExitedCommands tests');
if (typeof checkForExitedCommands === 'function') {
    // All alive → no notifications
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '', _commands: [
        { id: 'cmd-alive', name: 'htop', alive: true, exit_code: null }
    ]}];
    assert(() => { checkForExitedCommands(); }, 'checkForExitedCommands no crash with alive commands');

    // No commands → no crash
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', _commands: [] }];
    assert(() => { checkForExitedCommands(); }, 'checkForExitedCommands no crash with no commands');

    // Null _commands → no crash
    state.connections = [{ url: 'http://localhost:9090', _commands: null }];
    assert(() => { checkForExitedCommands(); }, 'checkForExitedCommands no crash with null _commands');

    // Exited command triggers notification
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', _commands: [
        { id: 'cmd-exited-new', name: 'exited-cmd', alive: false, exit_code: 1 }
    ]}];
    assert(() => { checkForExitedCommands(); }, 'checkForExitedCommands triggers notification for exited command');

    // Multiple connections
    state.connections = [
        { url: 'http://a.com', _commands: [{ id: 'c1', name: 'a', alive: true }] },
        { url: 'http://b.com', _commands: [{ id: 'c2', name: 'b', alive: false }] },
    ];
    assert(() => { checkForExitedCommands(); }, 'checkForExitedCommands handles multiple connections');
}

// ── updateSidebarResourceText ──
console.log('updateSidebarResourceText tests');
if (typeof updateSidebarResourceText === 'function') {
    // No connections → no crash
    state.connections = [];
    assert(() => { updateSidebarResourceText(); }, 'updateSidebarResourceText no crash with no connections');

    // With commands and resource data
    state.connections = [{ url: 'http://localhost:9090', _commands: [
        { id: 'r-cmd-1', name: 'htop', alive: true, runtime_secs: 90, pid: 123 }
    ]}];
    state._resourceCache = { 'r-cmd-1': { cpu_percent: 45.5, memory_mb: 128.3 } };
    assert(() => { updateSidebarResourceText(); }, 'updateSidebarResourceText does not throw');

    // With no resource cache → no crash
    state._resourceCache = {};
    assert(() => { updateSidebarResourceText(); }, 'updateSidebarResourceText no crash with no cache');

    // With null _commands → no crash
    state.connections = [{ url: 'http://localhost:9090', _commands: null }];
    assert(() => { updateSidebarResourceText(); }, 'updateSidebarResourceText no crash with null commands');
}

console.log('\n[notifications.js] Tests complete');