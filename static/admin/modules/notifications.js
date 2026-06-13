// ─── Notifications, Sound, Auto-Restart, Resource Polling ───
// Browser notifications on command exit, sound toggle, auto-restart pinned commands,
// and resource polling for CPU/memory display.
(function() {
    'use strict';

// ─── Browser Notification on Command Exit ───
const _notifiedExits = new Set();

function notifyCommandEnded(cmdId) {
    if (!cmdId || _notifiedExits.has(cmdId)) return;
    _notifiedExits.add(cmdId);

    // Find command name and exit code
    let cmdName = cmdId;
    let exitCode = null;
    for (const inst of state.connections) {
        if (inst._commands) {
            const cmd = inst._commands.find(c => c.id === cmdId);
            if (cmd) { cmdName = cmd.name || cmdId; exitCode = cmd.exit_code; break; }
        }
    }

    // Play sound notification
    if (state.soundEnabled) {
        playExitSound(exitCode === 0);
    }

    if ('Notification' in window) {
        if (Notification.permission === 'granted') {
            new Notification('vrw: Command exited', { body: cmdName, icon: '/favicon.ico' });
        } else if (Notification.permission !== 'denied') {
            Notification.requestPermission().then(perm => {
                if (perm === 'granted') {
                    new Notification('vrw: Command exited', { body: cmdName, icon: '/favicon.ico' });
                }
            });
        }
    }
}

// Also detect command exits via polling — notify when a previously-alive command exits
// Auto-restart pinned commands on exit (with debounce to avoid restart loops)
const _autoRestartDebounce = new Map(); // cmdName → timeout ID

function checkForExitedCommands() {
    const pinnedNames = getPinnedNames();
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false && !_notifiedExits.has(cmd.id)) {
                notifyCommandEnded(cmd.id);
                // Auto-restart pinned commands
                const cmdName = cmd.name || cmd.id;
                if (pinnedNames.includes(cmdName)) {
                    _autoRestartCommand(inst.url, cmd, cmdName);
                }
            }
        }
    }
}

function _autoRestartCommand(instUrl, cmd, cmdName) {
    // Debounce: don't restart the same command name more than once every 10s
    // to avoid rapid restart loops on commands that exit immediately
    if (_autoRestartDebounce.has(cmdName)) return;
    _autoRestartDebounce.set(cmdName, setTimeout(() => {
        _autoRestartDebounce.delete(cmdName);
    }, 10000));

    restartCommandById(instUrl, cmd.id).then(() => {
        // Show a brief indicator that auto-restart happened
        const indicator = document.getElementById('autoRestartIndicator');
        if (indicator) {
            indicator.textContent = 'Auto-restarted: ' + cmdName;
            indicator.classList.remove('hidden');
            setTimeout(() => { indicator.classList.add('hidden'); }, 3000);
        }
    }).catch(() => {
        // Restart failed — remove debounce lock so it can retry
        const t = _autoRestartDebounce.get(cmdName);
        if (t) { clearTimeout(t); _autoRestartDebounce.delete(cmdName); }
    });
}


// ─── Resource Polling ───
async function pollResources() {
    // Fetch all alive commands' resources in PARALLEL (not serial).
    const promises = [];
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false) continue;
            promises.push((async () => {
                try {
                    const json = await api.getCommandResources(inst.url, cmd.id);
                    if (json.status === 'ok' && json.data) {
                        state._resourceCache[cmd.id] = json.data;
                    }
                } catch (e) {
                    // Silently ignore — resources are optional
                }
            })());
        }
    }
    await Promise.all(promises);
    // Update sidebar resource text without full DOM rebuild
    updateSidebarResourceText();
}

/// Update the .cmd-detail-inline text in sidebar command items to reflect
/// the latest resource data from state._resourceCache. This avoids a full
/// DOM rebuild (which the fingerprint optimization would skip anyway).
function updateSidebarResourceText() {
    for (const inst of state.connections) {
        if (!inst._commands) continue;
        for (const cmd of inst._commands) {
            if (cmd.alive === false) continue;
            const res = state._resourceCache[cmd.id];
            const item = document.querySelector(`.cmd-item[data-cmd-id="${cmd.id}"]`);
            if (!item) continue;
            const isFrozen = cmd.frozen === true;
            const runtimeStr = cmd.runtime_secs > 0
                ? (cmd.runtime_secs < 60 ? Math.floor(cmd.runtime_secs) + 's'
                   : cmd.runtime_secs < 3600 ? Math.floor(cmd.runtime_secs / 60) + 'm ' + Math.floor(cmd.runtime_secs % 60) + 's'
                   : Math.floor(cmd.runtime_secs / 3600) + 'h ' + Math.floor((cmd.runtime_secs % 3600) / 60) + 'm')
                : '';
            const frozenBadge = isFrozen ? 'PAUSED' : '';
            // Compact: runtime · cpu% · memM · pid  (numeric only, no labels — must match
            // the format used in renderCmdList to avoid visual flipping)
            const detailParts = [];
            if (runtimeStr) detailParts.push(runtimeStr);
            if (frozenBadge) detailParts.push(frozenBadge);
            if (res && res.cpu_percent != null) detailParts.push(res.cpu_percent.toFixed(1) + '%');
            if (res && res.memory_mb != null) {
                const mb = res.memory_mb;
                detailParts.push(mb >= 1024 ? (mb / 1024).toFixed(1) + 'G' : mb.toFixed(1) + 'M');
            }
            if (cmd.pid) detailParts.push(String(cmd.pid));

            // Find or create the detail row
            let detailRow = item.querySelector('.cmd-detail-row');
            if (detailParts.length === 0) {
                if (detailRow) detailRow.remove();
            } else {
                if (!detailRow) {
                    detailRow = document.createElement('div');
                    detailRow.className = 'cmd-detail-row';
                    item.appendChild(detailRow);
                }
                detailRow.innerHTML = detailParts.join(' · ');
            }
        }
    }
}


// ─── Sound Notifications ───
function initSoundToggle() {
    const btn = document.getElementById('soundBtn');
    if (!btn) return;
    if (state.soundEnabled) btn.classList.add('sound-btn-active');
}

function toggleSoundNotifications() {
    state.soundEnabled = !state.soundEnabled;
    localStorage.setItem('vrw_sound', state.soundEnabled.toString());
    const btn = document.getElementById('soundBtn');
    if (btn) btn.classList.toggle('sound-btn-active', state.soundEnabled);
}

function playExitSound(success) {
    try {
        const ctx = new (window.AudioContext || window.webkitAudioContext)();
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.connect(gain);
        gain.connect(ctx.destination);
        if (success) {
            osc.frequency.value = 880;
            osc.type = 'sine';
        } else {
            osc.frequency.value = 440;
            osc.type = 'square';
        }
        gain.gain.value = 0.1;
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
        osc.start(ctx.currentTime);
        osc.stop(ctx.currentTime + 0.5);
    } catch (e) { /* ignore — audio not supported */ }
}

    // Expose to global scope
    window.pollResources = pollResources;
    window.updateSidebarResourceText = updateSidebarResourceText;
    window.checkForExitedCommands = checkForExitedCommands;
    window.notifyCommandEnded = notifyCommandEnded;
    window.initSoundToggle = initSoundToggle;
    window.toggleSoundNotifications = toggleSoundNotifications;
    window.playExitSound = playExitSound;
})();
