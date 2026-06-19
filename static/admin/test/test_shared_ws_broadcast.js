/// test/test_shared_ws_broadcast.js — Tests that a single shared WS
/// broadcasts vtty_dirty to ALL panels subscribed to the same command.
///
/// KEY INSIGHT: fetchVttyDiffForPanel is referenced by closure inside the
/// websocket.js IIFE, so replacing it on globalThis has NO effect on the
/// onmessage handler. Instead we spy on api.getVttyDiff which IS called
/// through the global api object.
require('./setup');

const { runAsyncTests } = require('./helpers');

// Clear shared WS pool and diff timers (persists across tests via IIFE closures)
function _clearSharedWsState() {
    const subs = globalThis._getSharedSubs();
    for (const key of Object.keys(subs)) {
        const sub = subs[key];
        if (sub.reconnectTimer) { clearTimeout(sub.reconnectTimer); sub.reconnectTimer = null; }
        if (sub.pingInterval) { clearInterval(sub.pingInterval); sub.pingInterval = null; }
        if (sub.ws) { sub.ws.onclose = null; sub.ws.close(); sub.ws = null; }
    }
    // Can't delete the const, but can clear all entries
    for (const key of Object.keys(subs)) delete subs[key];
    // Clear diff timers
    // _diffTimers is a const inside websocket.js IIFE — not directly accessible.
    // But since each test creates new panels with unique IDs, old timers won't interfere.
}

async function main() {
    await runAsyncTests('Shared WS Broadcast — multi-panel same-command', [
        async function testTwoPanelsSameCommandBothReceiveDirty() {
            resetTestState();
            _clearSharedWsState();

            globalThis.updateVttyDisplayForPanel = function() {};
            globalThis.applyVttyDiffForPanel = function() {};
            globalThis.updateVttyMetadataForPanel = function() {};
            globalThis.scheduleVttyHttpForPanel = function() {};
            globalThis.handlePeerEvent = function() {};
            globalThis.updateWsQualityIndicator = function() {};
            globalThis.notifyCommandEnded = function() {};

            // Set up state
            state.panels = [];
            state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
            state.updateMode = 'push';
            state._diffBaselines = {};
            state._lastGeneration = {};
            state.bufferView = 'current';

            // Create two panels (addPanelDirect already pushes to state.panels)
            const panel1 = addPanelDirect();
            panel1.id = 'panel-AAA';
            panel1.selectedInstUrl = 'http://localhost:9090';
            panel1.selectedCmdId = 'cmd-42';

            const panel2 = addPanelDirect();
            panel2.id = 'panel-BBB';
            panel2.selectedInstUrl = 'http://localhost:9090';
            panel2.selectedCmdId = 'cmd-42';

            // Connect panel1 to the command
            connectPanelWs(panel1.id);

            // Wait for MockWebSocket to auto-open (setTimeout 0)
            await new Promise(r => setTimeout(r, 10));

            // Verify shared sub pool has one entry with panel1
            const subs = globalThis._getSharedSubs();
            const key = 'http://localhost:9090/cmd-42';
            assertOk(subs[key], 'shared sub exists after panel1 connects');
            assertEq(subs[key].panels.size, 1, 'one panel in shared sub');
            assertOk(subs[key].panels.has('panel-AAA'), 'panel-AAA is in shared sub');

            // Get the WS ref before connecting panel2
            const wsBeforePanel2 = subs[key].ws;

            // Connect panel2 to the SAME command
            connectPanelWs(panel2.id);

            // Wait for any async activity
            await new Promise(r => setTimeout(r, 10));

            // Both panels should be in the same shared sub
            assertEq(subs[key].panels.size, 2, 'two panels in shared sub after panel2 connects');
            assertOk(subs[key].panels.has('panel-AAA'), 'panel-AAA still in shared sub');
            assertOk(subs[key].panels.has('panel-BBB'), 'panel-BBB is in shared sub');

            // The SAME WebSocket should be reused (not replaced)
            assert(wsBeforePanel2 === subs[key].ws,
                'SAME WebSocket reused for panel2 (not replaced)');

            // Now set up a fetch response and clear fetch calls
            // fetchVttyDiffForPanel is called by the onmessage handler via closure.
            // It calls api.getVttyDiff which calls fetch. We track fetch calls.
            _resetFetch();
            _setFetchJson({
                status: 'ok',
                data: { baseline: 'bl-001', generation: 1, html: '<pre>hello</pre>' }
            });

            // Simulate a vtty_dirty message arriving on the shared WS
            const ws = subs[key].ws;
            assert(ws !== null, 'ws is not null');
            assertEq(ws.readyState, 1, 'ws is OPEN');

            ws.onmessage({
                data: JSON.stringify({ type: 'vtty_dirty' })
            });

            // The vtty_dirty handler calls fetchVttyDiffForPanel for each panel.
            // fetchVttyDiffForPanel uses setTimeout(0) before calling _doFetchVttyDiff,
            // which calls api.getVttyDiff (which calls fetch).
            // Wait for the debounced fetches to fire.
            await new Promise(r => setTimeout(r, 50));

            // We expect 2 fetch calls (one per panel), both to the diff endpoint
            const diffUrls = _fetchCalls.filter(c =>
                c.url.includes('/api/commands/cmd-42/vtty/diff')
            );

            assertEq(diffUrls.length, 2,
                '2 fetch calls to diff endpoint after vtty_dirty (got ' + diffUrls.length + ')');

            // Both should be GET requests to the same endpoint
            for (const call of diffUrls) {
                assertEq(call.method, 'GET', 'diff fetch is GET');
                assert(call.url.includes('cmd-42'), 'diff URL includes cmd-42');
            }

            console.log('  diff fetch URLs: ' + JSON.stringify(diffUrls.map(c => c.url)));
        },

        async function testThreePanelsSameCommandAllReceiveDirty() {
            resetTestState();
            _clearSharedWsState();

            globalThis.updateVttyDisplayForPanel = function() {};
            globalThis.applyVttyDiffForPanel = function() {};
            globalThis.updateVttyMetadataForPanel = function() {};
            globalThis.scheduleVttyHttpForPanel = function() {};
            globalThis.handlePeerEvent = function() {};
            globalThis.updateWsQualityIndicator = function() {};
            globalThis.notifyCommandEnded = function() {};

            state.panels = [];
            state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
            state.updateMode = 'push';
            state._diffBaselines = {};
            state._lastGeneration = {};
            state.bufferView = 'current';

            const ids = ['panel-X1', 'panel-X2', 'panel-X3'];
            for (const id of ids) {
                const p = addPanelDirect();
                p.id = id;
                p.selectedInstUrl = 'http://localhost:9090';
                p.selectedCmdId = 'cmd-99';
            }

            // Connect all three panels
            for (const id of ids) connectPanelWs(id);
            await new Promise(r => setTimeout(r, 30));

            const subs = globalThis._getSharedSubs();
            const key = 'http://localhost:9090/cmd-99';
            assertEq(subs[key].panels.size, 3, 'three panels in shared sub');

            // Set up fetch response for diff
            _resetFetch();
            _setFetchJson({
                status: 'ok',
                data: { baseline: 'bl-002', generation: 1, html: '<pre>hello</pre>' }
            });

            // Simulate vtty_dirty
            subs[key].ws.onmessage({
                data: JSON.stringify({ type: 'vtty_dirty' })
            });

            await new Promise(r => setTimeout(r, 50));

            const diffUrls = _fetchCalls.filter(c =>
                c.url.includes('/api/commands/cmd-99/vtty/diff')
            );

            assertEq(diffUrls.length, 3,
                '3 fetch calls to diff endpoint for 3 panels (got ' + diffUrls.length + ')');

            console.log('  3-panel diff fetch URLs: ' + JSON.stringify(diffUrls.map(c => c.url)));
        },

        async function testUnsubscribeRemovesPanelFromBroadcast() {
            resetTestState();
            _clearSharedWsState();

            globalThis.updateVttyDisplayForPanel = function() {};
            globalThis.applyVttyDiffForPanel = function() {};
            globalThis.updateVttyMetadataForPanel = function() {};
            globalThis.scheduleVttyHttpForPanel = function() {};
            globalThis.handlePeerEvent = function() {};
            globalThis.updateWsQualityIndicator = function() {};
            globalThis.notifyCommandEnded = function() {};

            state.panels = [];
            state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
            state.updateMode = 'push';
            state._diffBaselines = {};
            state._lastGeneration = {};
            state.bufferView = 'current';

            const p1 = addPanelDirect();
            p1.id = 'panel-UN1';
            p1.selectedInstUrl = 'http://localhost:9090';
            p1.selectedCmdId = 'cmd-UN';

            const p2 = addPanelDirect();
            p2.id = 'panel-UN2';
            p2.selectedInstUrl = 'http://localhost:9090';
            p2.selectedCmdId = 'cmd-UN';

            connectPanelWs(p1.id);
            connectPanelWs(p2.id);
            await new Promise(r => setTimeout(r, 30));

            // Disconnect panel1
            disconnectPanelWs(p1.id);
            await new Promise(r => setTimeout(r, 10));

            const subs = globalThis._getSharedSubs();
            const key = 'http://localhost:9090/cmd-UN';

            // Shared sub still exists with just panel2
            assertOk(subs[key], 'shared sub still exists');
            assertEq(subs[key].panels.size, 1, 'one panel remains');
            assertOk(subs[key].panels.has('panel-UN2'), 'panel-UN2 remains');
            assert(!subs[key].panels.has('panel-UN1'), 'panel-UN1 removed');

            // Set up fetch and send dirty
            _resetFetch();
            _setFetchJson({
                status: 'ok',
                data: { baseline: 'bl-003', generation: 1, html: '<pre>hello</pre>' }
            });

            subs[key].ws.onmessage({
                data: JSON.stringify({ type: 'vtty_dirty' })
            });

            await new Promise(r => setTimeout(r, 50));

            const diffUrls = _fetchCalls.filter(c =>
                c.url.includes('/api/commands/cmd-UN/vtty/diff')
            );

            assertEq(diffUrls.length, 1,
                'only 1 fetch call after unsubscribe (got ' + diffUrls.length + ')');
        },

        async function testSecondPanelDoesNotReconnectWs() {
            // Critical test: connecting a second panel to the same command
            // must NOT create a new WebSocket (which would cause the server
            // to drop the old one, killing updates for the first panel).
            resetTestState();
            _clearSharedWsState();

            globalThis.updateVttyDisplayForPanel = function() {};
            globalThis.applyVttyDiffForPanel = function() {};
            globalThis.updateVttyMetadataForPanel = function() {};
            globalThis.scheduleVttyHttpForPanel = function() {};
            globalThis.handlePeerEvent = function() {};
            globalThis.updateWsQualityIndicator = function() {};
            globalThis.notifyCommandEnded = function() {};

            state.panels = [];
            state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
            state.updateMode = 'push';
            state._diffBaselines = {};
            state._lastGeneration = {};
            state.bufferView = 'current';

            const p1 = addPanelDirect();
            p1.id = 'panel-NO-RECONNECT-1';
            p1.selectedInstUrl = 'http://localhost:9090';
            p1.selectedCmdId = 'cmd-NR';

            connectPanelWs(p1.id);
            await new Promise(r => setTimeout(r, 10));

            const subs = globalThis._getSharedSubs();
            const key = 'http://localhost:9090/cmd-NR';
            const wsRef = subs[key].ws;

            // Track how many MockWebSockets are created
            const origWS = globalThis.WebSocket;
            let wsCreated = 0;
            globalThis.WebSocket = function(url) {
                wsCreated++;
                return new origWS(url);
            };
            globalThis.WebSocket.OPEN = origWS.OPEN;
            globalThis.WebSocket.CLOSED = origWS.CLOSED;
            globalThis.WebSocket.CONNECTING = origWS.CONNECTING;

            const p2 = addPanelDirect();
            p2.id = 'panel-NO-RECONNECT-2';
            p2.selectedInstUrl = 'http://localhost:9090';
            p2.selectedCmdId = 'cmd-NR';

            connectPanelWs(p2.id);
            await new Promise(r => setTimeout(r, 10));

            assertEq(wsCreated, 0,
                'NO new WebSocket created when second panel subscribes (wsCreated=' + wsCreated + ')');

            // Same WS object
            assert(wsRef === subs[key].ws,
                'same WS object still in shared sub');

            // Both panels in the set
            assertEq(subs[key].panels.size, 2, 'both panels in shared sub');

            globalThis.WebSocket = origWS;
        },

        async function testCommandEndedBroadcastsToAllPanels() {
            resetTestState();
            _clearSharedWsState();

            globalThis.updateVttyDisplayForPanel = function() {};
            globalThis.applyVttyDiffForPanel = function() {};
            globalThis.updateVttyMetadataForPanel = function() {};
            globalThis.scheduleVttyHttpForPanel = function() {};
            globalThis.handlePeerEvent = function() {};
            globalThis.updateWsQualityIndicator = function() {};

            let endNotifCount = 0;
            globalThis.notifyCommandEnded = function(cmdId) {
                if (cmdId === 'cmd-END') endNotifCount++;
            };

            state.panels = [];
            state.connections = [{ url: 'http://localhost:9090', label: 'Local', token: '' }];
            state.updateMode = 'push';
            state._diffBaselines = {};
            state._lastGeneration = {};
            state.bufferView = 'current';

            const p1 = addPanelDirect();
            p1.id = 'panel-END1';
            p1.selectedInstUrl = 'http://localhost:9090';
            p1.selectedCmdId = 'cmd-END';

            const p2 = addPanelDirect();
            p2.id = 'panel-END2';
            p2.selectedInstUrl = 'http://localhost:9090';
            p2.selectedCmdId = 'cmd-END';

            connectPanelWs(p1.id);
            connectPanelWs(p2.id);
            await new Promise(r => setTimeout(r, 30));

            const subs = globalThis._getSharedSubs();
            const key = 'http://localhost:9090/cmd-END';
            assertEq(subs[key].panels.size, 2, 'two panels before command_ended');

            // Set baselines so we can verify they get deleted
            state._diffBaselines['panel-END1/cmd-END'] = 'bl-end-1';
            state._diffBaselines['panel-END2/cmd-END'] = 'bl-end-2';

            // Simulate command_ended
            subs[key].ws.onmessage({
                data: JSON.stringify({ type: 'command_ended', cmdId: 'cmd-END' })
            });

            // Both baselines should be cleared
            assert(state._diffBaselines['panel-END1/cmd-END'] === undefined,
                'baseline for panel-END1 cleared on command_ended');
            assert(state._diffBaselines['panel-END2/cmd-END'] === undefined,
                'baseline for panel-END2 cleared on command_ended');

            // notifyCommandEnded called once (not per panel)
            assertEq(endNotifCount, 1, 'notifyCommandEnded called once');
        },
    ]);

    // Print summary
    console.log('\n[shared_ws_broadcast] Total: ' +
        globalThis._testPassed + ' passed, ' +
        globalThis._testFailed + ' failed\n');
}

main().catch(e => {
    console.error('Test runner error:', e);
    process.exit(1);
});