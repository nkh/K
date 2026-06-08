// ─── Utilities ───
// Pure utility functions used across all modules.
(function() {
    'use strict';

/// Format a runtime duration in seconds to a human-readable string.
/// Handles null/undefined inputs gracefully.
function formatRuntime(secs) {
    if (!secs || secs < 0) return '';
    if (secs < 60) return Math.floor(secs) + 's';
    if (secs < 3600) return Math.floor(secs / 60) + 'm ' + Math.floor(secs % 60) + 's';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return h + 'h ' + m + 'm';
}

function getBaseUrl() {
    return state.connections.length > 0 ? state.connections[0].url : window.location.origin;
}

function authHeaders(token) {
    const t = token || state.authToken;
    const headers = { 'Content-Type': 'application/json' };
    if (t) headers['Authorization'] = 'Bearer ' + t;
    return headers;
}

function authHeadersForInstance(inst) {
    return authHeaders(inst.token || state.authToken);
}

function apiUrl(path, inst) {
    const base = inst ? inst.url : getBaseUrl();
    return base + path;
}

// ─── Utilities ───
function escHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

/// Convert a byte (0-255) to a 2-digit lowercase hex string.
function _hex(b) {
    return (b < 16 ? '0' : '') + b.toString(16);
}

// HTML-escape a character, matching the server's html_escape() function.
function _htmlEscapeChar(ch) {
    switch (ch) {
        case '&': return '&amp;';
        case '<': return '&lt;';
        case '>': return '&gt;';
        case "'": return '&#39;';
        case '"': return '&quot;';
        default: return ch;
    }
}

// Apply an incremental diff from the server directly to the DOM.
// This updates only the changed cells, avoiding a full innerHTML replacement.
//
// The diff data has the format:
//   { generation, cursor, dimensions, changed_count, cells: [...] }
// Each cell: { row, col, ch, fg: [r,g,b], bg: [r,g,b], bold, italic, ... }

// ─── Spawn argument parser ───
// Splits a string into arguments respecting quoted strings.
// Supports double-quoted and single-quoted strings.
// Examples:
//   '-c "echo hello; echo world"' -> ['-c', 'echo hello; echo world']
//   "--flag 'arg with spaces'"      -> ['--flag', 'arg with spaces']
//   'plain args'                    -> ['plain', 'args']
function parseSpawnArgs(str) {
    if (!str) return [];
    const args = [];
    let current = '';
    let inQuote = null; // '"' or "'"
    let escaped = false;
    for (let i = 0; i < str.length; i++) {
        const ch = str[i];
        if (escaped) {
            current += ch;
            escaped = false;
            continue;
        }
        if (ch === '\\') {
            escaped = true;
            continue;
        }
        if (inQuote) {
            if (ch === inQuote) {
                inQuote = null;
            } else {
                current += ch;
            }
            continue;
        }
        if (ch === '"' || ch === "'") {
            inQuote = ch;
            continue;
        }
        if (ch === ' ' || ch === '\t') {
            if (current) {
                args.push(current);
                current = '';
            }
            continue;
        }
        current += ch;
    }
    if (current) args.push(current);
    return args;
}

/// Parse the environment variables textarea into a {key: value} object.
/// Each line should be KEY=VALUE. Lines not containing '=' are skipped.
/// Whitespace around key and value is trimmed. Empty lines are ignored.
function parseSpawnEnvVars(text) {
    const env = {};
    if (!text) return env;
    const lines = text.split('\n');
    for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith('#')) continue;  // skip empty/comment lines
        const eqIdx = trimmed.indexOf('=');
        if (eqIdx < 1) continue;  // skip lines without '=' or with '=' at start
        const key = trimmed.substring(0, eqIdx).trim();
        const value = trimmed.substring(eqIdx + 1).trim();
        if (key) env[key] = value;
    }
    return env;
}


    // Expose to global scope
    window.formatRuntime = formatRuntime;
    window.getBaseUrl = getBaseUrl;
    window.authHeaders = authHeaders;
    window.authHeadersForInstance = authHeadersForInstance;
    window.apiUrl = apiUrl;
    window.escHtml = escHtml;
    window.parseSpawnArgs = parseSpawnArgs;
    window.parseSpawnEnvVars = parseSpawnEnvVars;
    window._hex = _hex;
})();
