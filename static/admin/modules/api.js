// ─── API Layer ───
// Centralized server communication. Every HTTP fetch and WebSocket
// connection goes through here. No other module should call fetch()
// or new WebSocket() directly.
//
// All functions return promises (except connectVtty which returns a handle).
// instUrl can be omitted to use the default (first) connection.
(function() {
    'use strict';

    // ── Internal helpers (delegates to utils.js) ──

    function _url(path, instUrl) {
        return apiUrl(path, instUrl ? { url: instUrl } : undefined);
    }

    function _headers(instUrl) {
        // Find the connection for auth
        const inst = instUrl
            ? state.connections.find(c => c.url === instUrl)
            : state.connections[0];
        return authHeadersForInstance(inst || {});
    }

    function _jsonGet(path, instUrl) {
        return fetch(_url(path, instUrl), { headers: _headers(instUrl) })
            .then(r => r.ok ? r.json() : r.json().then(d => Promise.reject(d)));
    }

    function _jsonPost(path, instUrl, body) {
        return fetch(_url(path, instUrl), {
            method: 'POST',
            headers: _headers(instUrl),
            body: body ? JSON.stringify(body) : undefined,
        }).then(r => r.ok ? r.json().catch(() => ({})) : r.json().then(d => Promise.reject(d)));
    }

    function _jsonDelete(path, instUrl) {
        return fetch(_url(path, instUrl), {
            method: 'DELETE',
            headers: _headers(instUrl),
        }).then(r => r.ok ? r.json().catch(() => ({})) : r.json().then(d => Promise.reject(d)));
    }

    function _textGet(path, instUrl) {
        return fetch(_url(path, instUrl), { headers: _headers(instUrl) })
            .then(r => r.ok ? r.text() : r.text().then(t => Promise.reject(t)));
    }

    function _blobGet(path, instUrl) {
        return fetch(_url(path, instUrl), { headers: _headers(instUrl) })
            .then(r => r.ok ? r.blob() : r.blob().then(b => Promise.reject(b)));
    }

    // ── Public API ──

    const api = {

        // ── Server info ──
        getInfo(instUrl) {
            return _jsonGet('/api/info', instUrl);
        },

        getCertificates(instUrl) {
            return _jsonGet('/api/certificates', instUrl);
        },

        // ── Commands ──
        getCommands(instUrl) {
            return _jsonGet('/api/commands', instUrl);
        },

        lookupCommand(name, instUrl) {
            return _jsonGet('/api/commands/lookup/' + encodeURIComponent(name), instUrl);
        },

        spawnCommand(instUrl, body) {
            return _jsonPost('/api/commands', instUrl, body);
        },

        getCommandResources(instUrl, cmdId) {
            return _jsonGet('/api/commands/' + cmdId + '/resources', instUrl);
        },

        // ── Command actions ──
        freeze(instUrl, cmdId) {
            return _jsonPost('/api/commands/' + cmdId + '/freeze', instUrl);
        },

        thaw(instUrl, cmdId) {
            return _jsonPost('/api/commands/' + cmdId + '/thaw', instUrl);
        },

        kill(instUrl, cmdId) {
            return _jsonPost('/api/commands/' + cmdId + '/kill', instUrl);
        },

        killAll(instUrl, cmdIds) {
            if (cmdIds && cmdIds.length > 0) {
                return Promise.all(cmdIds.map(id => api.kill(instUrl, id)));
            }
            return _jsonPost('/api/commands/kill-all', instUrl);
        },

        restart(instUrl, cmdId) {
            return _jsonPost('/api/commands/' + cmdId + '/restart', instUrl);
        },

        keep(instUrl, cmdId) {
            return _jsonPost('/api/commands/' + cmdId + '/keep', instUrl);
        },

        unkeep(instUrl, cmdId) {
            return _jsonPost('/api/commands/' + cmdId + '/unkeep', instUrl);
        },

        purge(instUrl, cmdId) {
            return _jsonDelete('/api/commands/' + cmdId, instUrl);
        },

        // ── Command I/O ──
        sendKeys(instUrl, cmdId, body) {
            return _jsonPost('/api/commands/' + cmdId + '/keys', instUrl, body);
        },

        sendMouse(instUrl, cmdId, body) {
            return _jsonPost('/api/commands/' + cmdId + '/mouse', instUrl, body);
        },

        resize(instUrl, cmdId, body) {
            return _jsonPost('/api/commands/' + cmdId + '/resize', instUrl, body);
        },

        // ── VTTY ──
        getVttyChanged(instUrl, cmdId) {
            return _jsonGet('/api/commands/' + cmdId + '/vtty/changed', instUrl);
        },

        getVttyHtml(instUrl, cmdId) {
            return _jsonGet('/api/commands/' + cmdId + '/vtty/html', instUrl);
        },

        getVttyDiff(instUrl, cmdId, baseline) {
            const qs = baseline ? '?baseline=' + encodeURIComponent(baseline) : '';
            return _jsonGet('/api/commands/' + cmdId + '/vtty/diff' + qs, instUrl);
        },

        getVttyPng(instUrl, cmdId, params) {
            const qs = params ? '?' + new URLSearchParams(params).toString() : '';
            return _blobGet('/api/commands/' + cmdId + '/vtty/png' + qs, instUrl);
        },

        // ── WebSocket: VTTY ──
        connectVtty(instUrl, cmdId, { onMessage, onClose, onMetadata } = {}) {
            const base = instUrl || getBaseUrl();
            const wsBase = base.replace(/^http/, 'ws');
            const wsUrl = wsBase + '/api/commands/' + cmdId + '/ws';

            const ws = new WebSocket(wsUrl);
            let pingTimer;
            let closed = false;

            ws.onopen = () => {
                pingTimer = setInterval(() => {
                    if (ws.readyState === WebSocket.OPEN) {
                        ws.send(JSON.stringify({ type: 'ping' }));
                    }
                }, 15000);
            };

            ws.onmessage = (event) => {
                let data;
                try { data = JSON.parse(event.data); } catch(e) { return; }

                if (data.type === 'pong') return;
                if (data.type === 'metadata' && onMetadata) { onMetadata(data); return; }
                if (onMessage) onMessage(data);
            };

            ws.onclose = (event) => {
                if (closed) return;
                closed = true;
                clearInterval(pingTimer);
                if (onClose) onClose(event);
            };

            ws.onerror = () => {};

            return {
                ws,
                close() {
                    if (closed) return;
                    closed = true;
                    clearInterval(pingTimer);
                    if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
                        ws.close();
                    }
                },
                get readyState() { return ws.readyState; },
            };
        },

        // ── WebSocket: Logs ──
        connectLogWs(instUrl, { onMessage, onClose } = {}) {
            const base = instUrl || getBaseUrl();
            const wsBase = base.replace(/^http/, 'ws');
            const wsUrl = wsBase + '/api/ws/logs';

            const ws = new WebSocket(wsUrl);
            let closed = false;

            ws.onopen = () => {};
            ws.onmessage = (event) => {
                if (onMessage) onMessage(event.data);
            };
            ws.onclose = (event) => {
                if (closed) return;
                closed = true;
                if (onClose) onClose(event);
            };
            ws.onerror = () => {};

            return {
                ws,
                close() {
                    if (closed) return;
                    closed = true;
                    ws.close();
                },
            };
        },

        // ── Spawn completions ──
        getCompletions(instUrl, prefix) {
            return _jsonGet('/api/completions?prefix=' + encodeURIComponent(prefix), instUrl);
        },

        // ── Templates ──
        getTemplates(instUrl) {
            return _jsonGet('/api/templates', instUrl);
        },

        // ── Logs ──
        getLog(instUrl, params) {
            const qs = params ? '?' + new URLSearchParams(params).toString() : '';
            return _jsonGet('/api/log' + qs, instUrl);
        },

        // ── Environments ──
        getEnvironments(instUrl) {
            return _jsonGet('/api/environments', instUrl);
        },

        activateEnvironment(instUrl, body) {
            return _jsonPost('/api/commands', instUrl, body);
        },

        // ── Static docs ──
        getDocs() {
            const base = getBaseUrl();
            return fetch(base + '/admin/docs.md')
                .then(r => r.ok ? r.text() : Promise.reject(r.status));
        },

        // ── Snapshots ──
        getSnapshot(instUrl) {
            return _jsonGet('/api/snapshot', instUrl);
        },

        // ── Peers ──
        getPeers() {
            return _jsonGet('/api/peers');
        },

        // ── Search ──
        getVttyText(instUrl, cmdId) {
            return _jsonGet('/api/commands/' + cmdId + '/vtty/text', instUrl);
        },

        // ── Generic JSON GET (for variant VTTY endpoints like scrollback/buffer) ──
        getJson(path, instUrl) {
            return _jsonGet(path, instUrl);
        },
    };

    // Expose
    window.api = api;
})();