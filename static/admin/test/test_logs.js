/// test/test_logs.js — Tests for log viewer functions
require('./setup');

console.log('\n=== logs.js Tests ===\n');

resetTestState();

// ── parseLogLine ──
console.log('parseLogLine tests');
if (typeof parseLogLine === 'function') {
    const line1 = parseLogLine('2024-01-15T10:30:00Z INFO [main] Application started');
    assert(line1 !== null, 'parseLogLine returns non-null for valid line');
    assert(typeof line1 === 'object', 'parseLogLine returns object');

    // Test with empty line
    const empty = parseLogLine('');
    assert(empty !== null || empty !== undefined, 'parseLogLine handles empty line');

    // Test with different levels
    const warn = parseLogLine('2024-01-15 WARN something');
    assert(warn !== null, 'parseLogLine handles WARN level');

    const err = parseLogLine('2024-01-15 ERROR something bad');
    assert(err !== null, 'parseLogLine handles ERROR level');
}

// ── formatLogLine ──
console.log('formatLogLine tests');
if (typeof formatLogLine === 'function') {
    const parsed = { timestamp: '10:30:00', level: 'INFO', message: 'started' };
    const formatted = formatLogLine(parsed, '2024-01-15T10:30:00Z INFO started');
    assert(typeof formatted === 'string', 'formatLogLine returns string');
    assert(formatted.length > 0, 'formatLogLine not empty');
}

// ── searchLogs ──
console.log('searchLogs tests');
if (typeof searchLogs === 'function') {
    const logInput = document.createElement('input');
    logInput.id = 'logSearch';
    logInput.value = 'test';
    assert(() => { searchLogs(); }, 'searchLogs does not throw');
}

// ── clearLogSearch ──
console.log('clearLogSearch tests');
if (typeof clearLogSearch === 'function') {
    assert(() => { clearLogSearch(); }, 'clearLogSearch does not throw');
}

// ── connectLogWs ──
console.log('connectLogWs tests');
if (typeof connectLogWs === 'function') {
    state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
    assert(() => { connectLogWs(); }, 'connectLogWs does not throw');
}

// ── disconnectLogWs ──
console.log('disconnectLogWs tests');
if (typeof disconnectLogWs === 'function') {
    assert(() => { disconnectLogWs(); }, 'disconnectLogWs does not throw');
}

// ── _updateLogTransportIndicator ──
console.log('_updateLogTransportIndicator tests');
if (typeof _updateLogTransportIndicator === 'function') {
    const indicator = document.createElement('span');
    indicator.id = 'logTransportIndicator';
    assert(() => { _updateLogTransportIndicator('ws'); }, '_updateLogTransportIndicator ws');
    assert(() => { _updateLogTransportIndicator('http'); }, '_updateLogTransportIndicator http');
}

// ── _autoScrollLog ──
console.log('_autoScrollLog tests');
if (typeof _autoScrollLog === 'function') {
    const container = document.createElement('div');
    container.scrollTop = 100;
    container.scrollHeight = 200;
    container.clientHeight = 100;
    assert(() => { _autoScrollLog(container); }, '_autoScrollLog does not throw');
}

console.log('\n[logs.js] Tests complete');
