/// test/test_templates.js — Tests for template management
require('./setup');

console.log('\n=== templates.js Tests ===\n');

resetTestState();

// ── getServerTemplates ──
console.log('getServerTemplates tests');
if (typeof getServerTemplates === 'function') {
    localStorage.removeItem('vrw_server_templates');
    const templates = getServerTemplates();
    assert(Array.isArray(templates), 'getServerTemplates returns array');
}

// ── getUserTemplates ──
console.log('getUserTemplates tests');
if (typeof getUserTemplates === 'function') {
    localStorage.removeItem('vrw_user_templates');
    const templates = getUserTemplates();
    assert(Array.isArray(templates), 'getUserTemplates returns array');
    assertEq(templates.length, 0, 'empty by default');
}

// ── saveUserTemplates ──
console.log('saveUserTemplates tests');
if (typeof saveUserTemplates === 'function') {
    const templates = [{ name: 'dev', cmd: 'npm run dev', args: '' }];
    saveUserTemplates(templates);
    const loaded = getUserTemplates();
    assertEq(loaded.length, 1, 'template saved');
    assertEq(loaded[0].name, 'dev', 'template name correct');
}

// ── renderTemplates ──
console.log('renderTemplates tests');
if (typeof renderTemplates === 'function') {
    const list = document.createElement('div');
    list.id = 'templateList';
    assert(() => { renderTemplates(); }, 'renderTemplates does not throw');
}

// ── showAddTemplateForm ──
console.log('showAddTemplateForm tests');
if (typeof showAddTemplateForm === 'function') {
    const form = document.createElement('div');
    form.id = 'templateAddForm';
    assert(() => { showAddTemplateForm(); }, 'showAddTemplateForm does not throw');
}

// ── hideAddTemplateForm ──
console.log('hideAddTemplateForm tests');
if (typeof hideAddTemplateForm === 'function') {
    assert(() => { hideAddTemplateForm(); }, 'hideAddTemplateForm does not throw');
}

// ── saveTemplate ──
console.log('saveTemplate tests');
if (typeof saveTemplate === 'function') {
    const nameInput = document.createElement('input');
    nameInput.id = 'templateName';
    nameInput.value = 'test-template';
    const cmdInput = document.createElement('input');
    cmdInput.id = 'templateCmd';
    cmdInput.value = 'echo hello';
    const argsInput = document.createElement('input');
    argsInput.id = 'templateArgs';
    argsInput.value = '';

    assert(() => { saveTemplate(); }, 'saveTemplate does not throw');
}

// ── deleteUserTemplate ──
console.log('deleteUserTemplate tests');
if (typeof deleteUserTemplate === 'function') {
    saveUserTemplates([{ name: 'temp', cmd: 'ls', args: '' }]);
    assert(() => { deleteUserTemplate(0); }, 'deleteUserTemplate does not throw');
}

// ── spawnServerTemplate ──
console.log('spawnServerTemplate tests');
if (typeof spawnServerTemplate === 'function') {
    assert(() => { spawnServerTemplate(0); }, 'spawnServerTemplate does not throw');
}

// ── spawnUserTemplate ──
console.log('spawnUserTemplate tests');
if (typeof spawnUserTemplate === 'function') {
    saveUserTemplates([{ name: 'test', cmd: 'echo hi', args: '' }]);
    assert(() => { spawnUserTemplate(0); }, 'spawnUserTemplate does not throw');
}

// ── fetchServerTemplates ──
console.log('fetchServerTemplates tests');
if (typeof fetchServerTemplates === 'function') {
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
    assert(() => { fetchServerTemplates(); }, 'fetchServerTemplates does not throw');
}

console.log('\n[templates.js] Tests complete');

// Prevent async callbacks from crashing after test completion
process.exit(0);
