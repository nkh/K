/// test_bug_toolbar_focused_leaf.js — Fix 5.1
/// Toolbar buttons for restart/copy/export should pass the focused leaf ID
/// in split panes, not the parent panel ID (matching keyboard shortcut behavior).
'use strict';

const { createMockEvent } = require('./helpers');

console.log('\n=== Fix 5.1: Toolbar resolves focused leaf in split panes ===\n');

// TFL-001: RestartCommand resolves to focused leaf in split pane
{
    let receivedId = null;
    const savedHandler = window.restartCommand;
    const savedGetActivePanelId = window.getActivePanelId;
    const savedState = window.state;

    window.restartCommand = function(id) { receivedId = id; };
    window.getActivePanelId = function() { return 'panel-A'; };
    window.state = {
        _focusedPanelId: 'panel-A',
        panels: [{
            id: 'panel-A',
            split: { branch: { id: 'leaf-B', cmdId: 'cmd-1', instUrl: 'http://localhost:9090' } },
            _focusedLeafId: 'leaf-B'
        }]
    };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'RestartCommand');
    const ev = createMockEvent({ target: btn });
    window._dispatchAction(ev);

    assertEq(receivedId, 'leaf-B',
        'TFL-001: RestartCommand receives focused leaf ID in split pane');

    window.restartCommand = savedHandler;
    window.getActivePanelId = savedGetActivePanelId;
    window.state = savedState;
}

// TFL-002: CopyTerminalSelection resolves to focused leaf in split pane
{
    let receivedId = null;
    const savedHandler = window.copyTerminalSelection;
    const savedGetActivePanelId = window.getActivePanelId;
    const savedState = window.state;

    window.copyTerminalSelection = function(id) { receivedId = id; };
    window.getActivePanelId = function() { return 'panel-X'; };
    window.state = {
        _focusedPanelId: 'panel-X',
        panels: [{
            id: 'panel-X',
            _rootSplit: { branch: { id: 'leaf-Y', cmdId: 'cmd-2', instUrl: 'http://localhost:9090' } },
            _focusedLeafId: 'leaf-Y'
        }]
    };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'CopyTerminalSelection');
    const ev = createMockEvent({ target: btn });
    window._dispatchAction(ev);

    assertEq(receivedId, 'leaf-Y',
        'TFL-002: CopyTerminalSelection receives focused leaf ID in _rootSplit pane');

    window.copyTerminalSelection = savedHandler;
    window.getActivePanelId = savedGetActivePanelId;
    window.state = savedState;
}

// TFL-003: ExportTerminal resolves to focused leaf in split pane
{
    let receivedId = null;
    const savedHandler = window.exportTerminal;
    const savedGetActivePanelId = window.getActivePanelId;
    const savedState = window.state;

    window.exportTerminal = function(id) { receivedId = id; };
    window.getActivePanelId = function() { return 'panel-M'; };
    window.state = {
        _focusedPanelId: 'panel-M',
        panels: [{
            id: 'panel-M',
            split: { branch: { id: 'leaf-N', cmdId: 'cmd-3', instUrl: 'http://localhost:9090' } },
            _focusedLeafId: 'leaf-N'
        }]
    };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ExportTerminal');
    const ev = createMockEvent({ target: btn });
    window._dispatchAction(ev);

    assertEq(receivedId, 'leaf-N',
        'TFL-003: ExportTerminal receives focused leaf ID in split pane');

    window.exportTerminal = savedHandler;
    window.getActivePanelId = savedGetActivePanelId;
    window.state = savedState;
}

// TFL-004: Non-split pane passes panel ID (no leaf resolution)
{
    let receivedId = null;
    const savedHandler = window.restartCommand;
    const savedGetActivePanelId = window.getActivePanelId;
    const savedState = window.state;

    window.restartCommand = function(id) { receivedId = id; };
    window.getActivePanelId = function() { return 'panel-Z'; };
    window.state = {
        _focusedPanelId: 'panel-Z',
        panels: [{ id: 'panel-Z', split: null, _rootSplit: null, _focusedLeafId: null }]
    };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'RestartCommand');
    const ev = createMockEvent({ target: btn });
    window._dispatchAction(ev);

    assertEq(receivedId, 'panel-Z',
        'TFL-004: Non-split pane passes panel ID unchanged');

    window.restartCommand = savedHandler;
    window.getActivePanelId = savedGetActivePanelId;
    window.state = savedState;
}

// TFL-005: panelId sig (e.g. ChangePanelFontSize) still passes panel ID, not leaf
{
    let receivedId = null;
    const savedHandler = window.changePanelFontSize;
    const savedGetActivePanelId = window.getActivePanelId;
    const savedState = window.state;

    window.changePanelFontSize = function(id) { receivedId = id; };
    window.getActivePanelId = function() { return 'panel-F'; };
    window.state = {
        _focusedPanelId: 'panel-F',
        panels: [{
            id: 'panel-F',
            split: { branch: { id: 'leaf-G' } },
            _focusedLeafId: 'leaf-G'
        }]
    };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'ChangePanelFontSize');
    btn.setAttribute('data-delta', '1');
    const ev = createMockEvent({ target: btn });
    window._dispatchAction(ev);

    // panelId-delta sig should NOT resolve to leaf — font size is per-panel
    assertEq(receivedId, 'panel-F',
        'TFL-005: panelId-delta sig still passes panel ID (font size is per-panel)');

    window.changePanelFontSize = savedHandler;
    window.getActivePanelId = savedGetActivePanelId;
    window.state = savedState;
}

// TFL-006: focusedLeaf sig falls back to panel ID when _focusedLeafId is null
{
    let receivedId = null;
    const savedHandler = window.restartCommand;
    const savedGetActivePanelId = window.getActivePanelId;
    const savedState = window.state;

    window.restartCommand = function(id) { receivedId = id; };
    window.getActivePanelId = function() { return 'panel-Q'; };
    window.state = {
        _focusedPanelId: 'panel-Q',
        panels: [{
            id: 'panel-Q',
            split: { branch: { id: 'leaf-R' } },
            _focusedLeafId: null  // not set yet
        }]
    };

    const btn = document.createElement('button');
    btn.setAttribute('data-action', 'RestartCommand');
    const ev = createMockEvent({ target: btn });
    window._dispatchAction(ev);

    assertEq(receivedId, 'panel-Q',
        'TFL-006: Falls back to panel ID when _focusedLeafId is null');

    window.restartCommand = savedHandler;
    window.getActivePanelId = savedGetActivePanelId;
    window.state = savedState;
}