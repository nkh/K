/// test/test_utils.js — Tests for utility functions (formatRuntime, escHtml, parseSpawnArgs, etc.)
require('./setup');

console.log('\n=== utils.js Tests ===\n');

resetTestState();

// ── formatRuntime ──
console.log('formatRuntime tests');
assertEq(formatRuntime(0), '0s', '0 seconds → "0s"');
assertEq(formatRuntime(null), '', 'null → empty');
assertEq(formatRuntime(undefined), '', 'undefined → empty');
assertEq(formatRuntime(-1), '', 'negative → empty');
assertEq(formatRuntime(1), '1s', '1 second');
assertEq(formatRuntime(59), '59s', '59 seconds');
assertEq(formatRuntime(60), '1m 0s', '1 minute');
assertEq(formatRuntime(61), '1m 1s', '1 minute 1 second');
assertEq(formatRuntime(3599), '59m 59s', '59 min 59 sec');
assertEq(formatRuntime(3600), '1h 0m', '1 hour (no seconds)');
assertEq(formatRuntime(3661), '1h 1m', '1h 1m (no seconds)');
assertEq(formatRuntime(86400), '24h 0m', '24 hours');
assertEq(formatRuntime(90061), '25h 1m', '25 hours');

// ── escHtml ──
console.log('escHtml tests');
assertEq(escHtml(''), '', 'empty string');
assertEq(escHtml(null), '', 'null');
assertEq(escHtml(undefined), '', 'undefined');
assertEq(escHtml('hello'), 'hello', 'plain text unchanged');
assertEq(escHtml('<script>alert("xss")</script>'), '&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;', 'HTML tags escaped (textContent method escapes quotes too)');
assertEq(escHtml('a & b'), 'a &amp; b', 'ampersand escaped');
assertEq(escHtml("it's"), "it's", 'single quotes unchanged with textContent method');
assertEq(escHtml('a < b && c > d'), 'a &lt; b &amp;&amp; c &gt; d', 'mixed special chars');

// ── getBaseUrl ──
console.log('getBaseUrl tests');
// getBaseUrl returns state.connections[0].url or location.origin
state.connections = [{ url: 'http://localhost:9090' }];
assertEq(getBaseUrl(), 'http://localhost:9090', 'returns connections[0].url');
state.connections = [{ url: 'https://example.com:8080' }];
assertEq(getBaseUrl(), 'https://example.com:8080', 'returns different connection URL');
state.connections = [];
assertEq(getBaseUrl(), location.origin, 'falls back to location.origin when no connections');
state.connections = [{ url: 'http://localhost:9090' }];

// ── authHeaders ──
console.log('authHeaders tests');
let headers = authHeaders('');
assertEq(headers.Authorization, undefined, 'no token → no auth header');
headers = authHeaders('mytoken');
assertEq(headers.Authorization, 'Bearer mytoken', 'token → Bearer token');

// ── authHeadersForInstance ──
console.log('authHeadersForInstance tests');
headers = authHeadersForInstance({ url: 'http://localhost:9090', token: 'inst-token' });
assertEq(headers.Authorization, 'Bearer inst-token', 'instance token used');

headers = authHeadersForInstance({ url: 'http://localhost:9090', token: '' });
// Should fall back to global authToken
state.authToken = 'global-token';
headers = authHeadersForInstance({ url: 'http://localhost:9090', token: '' });
assertEq(headers.Authorization, 'Bearer global-token', 'falls back to global token');
state.authToken = '';

// ── apiUrl ──
console.log('apiUrl tests');
assertEq(apiUrl('/api/commands', { url: 'http://localhost:9090' }), 'http://localhost:9090/api/commands', 'with instance URL');
assertEq(apiUrl('/api/commands', { url: 'http://localhost:9090' }), 'http://localhost:9090/api/commands', 'relative path');
assertEq(apiUrl('/api/commands'), 'http://localhost:9090/api/commands', 'no instance → uses base');

// ── parseSpawnArgs ──
console.log('parseSpawnArgs tests');
let args = parseSpawnArgs('hello world');
assertEq(args.length, 2, 'simple args count');
assertEq(args[0], 'hello', 'first arg');
assertEq(args[1], 'world', 'second arg');

args = parseSpawnArgs('');
assertEq(args.length, 0, 'empty string');

args = parseSpawnArgs('"quoted arg"');
assertEq(args.length, 1, 'quoted single arg');
assertEq(args[0], 'quoted arg', 'quoted arg unquoted');

args = parseSpawnArgs('--flag "value with spaces"');
assertEq(args.length, 2, 'mixed quoted and unquoted');
assertEq(args[1], 'value with spaces', 'quoted value');

args = parseSpawnArgs("-c 'echo hello'");
assertEq(args.length, 2, 'single quoted');
assertEq(args[1], 'echo hello', 'single quoted value');

args = parseSpawnArgs('cmd');
assertEq(args.length, 1, 'single arg');
assertEq(args[0], 'cmd', 'single arg value');

args = parseSpawnArgs('--name "my value" --other thing');
assertEq(args.length, 4, 'multiple mixed args');

// ── parseSpawnEnvVars ──
console.log('parseSpawnEnvVars tests');
let env = parseSpawnEnvVars('');
assert(typeof env === 'object' && !Array.isArray(env), 'empty string returns object');

env = parseSpawnEnvVars('KEY=value');
assertEq(env.KEY, 'value', 'single var');

env = parseSpawnEnvVars('KEY=value\nPATH=/usr/bin');
assertEq(env.PATH, '/usr/bin', 'second var value');

env = parseSpawnEnvVars('KEY="value with spaces"');
assert(typeof env.KEY === 'string', 'quoted env value returns string');
assert(env.KEY.includes('value'), 'quoted env value contains content');

// ── _hex ──
console.log('_hex tests');
if (typeof _hex === 'function') {
    assertEq(_hex(0), '00', 'zero');
    assertEq(_hex(15), '0f', '15');
    assertEq(_hex(255), 'ff', '255');
    assertEq(_hex(16), '10', '16');
}

// ── _htmlEscapeChar ──
console.log('_htmlEscapeChar tests');
if (typeof _htmlEscapeChar === 'function') {
    assertEq(_htmlEscapeChar('<'), '&lt;', 'less than');
    assertEq(_htmlEscapeChar('>'), '&gt;', 'greater than');
    assertEq(_htmlEscapeChar('&'), '&amp;', 'ampersand');
    assertEq(_htmlEscapeChar('"'), '&quot;', 'double quote');
    assertEq(_htmlEscapeChar('a'), 'a', 'normal char unchanged');
}

// ── updateThemeButton — distinct icons per theme ──
console.log('updateThemeButton icon tests');
if (typeof updateThemeButton === 'function') {
    const btn = document.getElementById('themeToggle');
    // Auto (no data-theme) — depends on prefers-color-scheme; mock it
    document.documentElement.removeAttribute('data-theme');
    globalThis.matchMedia = function(q) { return { matches: false, addListener: function(){} }; };
    updateThemeButton();
    assertEq(btn.textContent, '\u2600', 'auto-dark shows sun icon');

    globalThis.matchMedia = function(q) { return { matches: true, addListener: function(){} }; };
    updateThemeButton();
    assertEq(btn.textContent, '\u263E', 'auto-light shows moon icon');

    document.documentElement.setAttribute('data-theme', 'light');
    updateThemeButton();
    assertEq(btn.textContent, '\u2600', 'light theme shows sun icon');

    document.documentElement.setAttribute('data-theme', 'dark');
    updateThemeButton();
    assertEq(btn.textContent, '\u263E', 'dark theme shows moon icon (not sun)');

    document.documentElement.setAttribute('data-theme', 'grey');
    updateThemeButton();
    assertEq(btn.textContent, '\u25FC', 'grey theme shows square icon');

    // Cleanup
    document.documentElement.removeAttribute('data-theme');
}

console.log('\n[utils.js] ' + _testPassed + ' passed so far');
