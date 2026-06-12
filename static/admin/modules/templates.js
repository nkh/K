// ─── Command Templates ───
// Server-side templates (from vrw config [[templates]]) and user-defined templates
// (stored in localStorage). Templates provide one-click command spawning.
(function() {
    'use strict';

// ─── Command Templates ───
// Server-side templates are loaded from the vrw config file ([[templates]]).
// User templates are stored in localStorage and are editable in the web UI.
let _serverTemplates = []; // cached from /api/templates

function getServerTemplates() {
    return _serverTemplates;
}

async function fetchServerTemplates() {
    try {
        const json = await api.getTemplates();
        if (json.status === 'ok') {
            _serverTemplates = json.data || [];
        }
    } catch { /* ignore — use cached */ }
}

function getUserTemplates() {
    try {
        return JSON.parse(localStorage.getItem('vrw_templates') || '[]');
    } catch { return []; }
}

function saveUserTemplates(templates) {
    localStorage.setItem('vrw_templates', JSON.stringify(templates));
}

function renderTemplates() {
    const container = document.getElementById('templateList');
    if (!container) return;

    const server = getServerTemplates();
    const user = getUserTemplates();
    const hasAny = server.length > 0 || user.length > 0;

    if (!hasAny) {
        container.innerHTML = '<div style="padding:0.5rem;color:var(--text-muted);font-size:0.7rem;text-align:center;">No templates configured. Add templates in your config file under [[templates]].</div>';
        return;
    }

    let html = '';

    // Server templates section
    if (server.length > 0) {
        html += '<div style="font-size:0.6rem;color:var(--text-muted);padding:0.2rem 0.3rem;text-transform:uppercase;letter-spacing:0.05em;">From config</div>';
        html += server.map((t, i) => {
            const detail = [t.cmd, t.args].filter(Boolean).join(' ');
            const extras = [];
            if (t.workdir) extras.push('dir: ' + t.workdir);
            if (t.certificate) extras.push('cert: ' + t.certificate);
            if (t.rows || t.cols) extras.push((t.rows || '?') + 'x' + (t.cols || '?'));
            const extraStr = extras.length > 0 ? extras.join(' | ') : '';
            return `<div class="template-card" data-action="SpawnServerTemplate" data-index="${i}" title="Click to spawn this command">
                <div style="display:flex;align-items:center;gap:0.3rem;">
                    <div class="template-name">${escHtml(t.name)}</div>
                    <span style="font-size:0.5rem;background:var(--accent);color:#fff;padding:0 0.25rem;border-radius:2px;">config</span>
                </div>
                <div class="template-cmd">${escHtml(detail)}</div>
                ${extraStr ? `<div style="font-size:0.6rem;color:var(--text-muted);padding-left:0.2rem;">${escHtml(extraStr)}</div>` : ''}
            </div>`;
        }).join('');
    }

    // User templates section
    if (user.length > 0) {
        html += '<div style="font-size:0.6rem;color:var(--text-muted);padding:0.3rem 0.3rem 0.1rem;text-transform:uppercase;letter-spacing:0.05em;">Custom</div>';
        html += user.map((t, i) => `
            <div class="template-card" data-action="SpawnUserTemplate" data-index="${i}" title="Click to spawn this command">
                <div class="template-name">${escHtml(t.name)}</div>
                <div class="template-cmd">${escHtml(t.cmd)}${t.args ? ' ' + escHtml(t.args) : ''}</div>
                <div class="template-actions">
                    <button class="btn btn-xs btn-danger" data-action="DeleteUserTemplate" data-index="${i}" title="Delete">&#x2715;</button>
                </div>
            </div>
        `).join('');
    }

    container.innerHTML = html;
}

function spawnServerTemplate(index) {
    const t = getServerTemplates()[index];
    if (!t) return;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect ? instSelect.value : (window._userSpawnInstUrl || getBaseUrl());
    const args = t.args ? t.args.split(/\s+/) : [];
    const body = { cmd: t.cmd, args };
    // Convert env from ["KEY=VALUE", ...] to { "KEY": "VALUE", ... }
    if (t.env && t.env.length > 0) {
        const envObj = {};
        for (const entry of t.env) {
            const eqIdx = entry.indexOf('=');
            if (eqIdx > 0) envObj[entry.substring(0, eqIdx)] = entry.substring(eqIdx + 1);
        }
        body.env = envObj;
    }
    if (t.workdir) body.workdir = t.workdir;
    if (t.certificate) body.certificate = t.certificate;
    if (t.rows) body.rows = t.rows;
    if (t.cols) body.cols = t.cols;
    api.spawnCommand(instUrl, body).then(json => {
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
            const cmdTab = document.querySelector('.sidebar-tab');
            if (cmdTab) switchSidebarTab('commands', cmdTab);
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    }).catch(e => alert('Spawn failed: ' + e.message));
}

function spawnUserTemplate(index) {
    const user = getUserTemplates();
    const t = user[index];
    if (!t) return;
    const instSelect = document.getElementById('spawnInstance');
    const instUrl = instSelect ? instSelect.value : (window._userSpawnInstUrl || getBaseUrl());
    const args = t.args ? t.args.split(/\s+/) : [];
    const body = { cmd: t.cmd, args };
    api.spawnCommand(instUrl, body).then(json => {
        if (json.status === 'ok') {
            const newId = json.data && json.data.id ? json.data.id : null;
            if (newId) {
                state.selectedInstUrl = instUrl;
                _cacheTerminalForSwitch();
                state._pendingSelectId = newId;
            }
            loadCommands();
            const cmdTab = document.querySelector('.sidebar-tab');
            if (cmdTab) switchSidebarTab('commands', cmdTab);
        } else {
            alert('Spawn failed: ' + (json.error || 'unknown'));
        }
    }).catch(e => alert('Spawn failed: ' + e.message));
}

function deleteUserTemplate(index) {
    const templates = getUserTemplates();
    templates.splice(index, 1);
    saveUserTemplates(templates);
    renderTemplates();
}

function showAddTemplateForm() {
    const form = document.getElementById('templateAddForm');
    if (form) form.style.display = '';
}

function hideAddTemplateForm() {
    const form = document.getElementById('templateAddForm');
    if (form) form.style.display = 'none';
    document.getElementById('templateName').value = '';
    document.getElementById('templateCmd').value = '';
    document.getElementById('templateArgs').value = '';
}

function saveTemplate() {
    const name = document.getElementById('templateName').value.trim();
    const cmd = document.getElementById('templateCmd').value.trim();
    const args = document.getElementById('templateArgs').value.trim();
    if (!name || !cmd) { alert('Name and command are required'); return; }
    const templates = getUserTemplates();
    templates.push({ name, cmd, args });
    saveUserTemplates(templates);
    hideAddTemplateForm();
    renderTemplates();
}

    // Expose to global scope
    window.fetchServerTemplates = fetchServerTemplates;
    window.getServerTemplates = getServerTemplates;
    window.getUserTemplates = getUserTemplates;
    window.saveUserTemplates = saveUserTemplates;
    window.renderTemplates = renderTemplates;
    window.spawnServerTemplate = spawnServerTemplate;
    window.spawnUserTemplate = spawnUserTemplate;
    window.deleteUserTemplate = deleteUserTemplate;
    window.showAddTemplateForm = showAddTemplateForm;
    window.hideAddTemplateForm = hideAddTemplateForm;
    window.saveTemplate = saveTemplate;
})();
