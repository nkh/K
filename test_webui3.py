"""Test web UI with longer wait to see if async operations complete."""
import json
import time
from playwright.sync_api import sync_playwright

BASE = "http://127.0.0.1:9090"

def main():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        
        console_logs = []
        page.on("console", lambda msg: console_logs.append(f"[{msg.type}] {msg.text}"))
        page_errors = []
        page.on("pageerror", lambda err: page_errors.append(str(err)))
        
        page.goto(BASE, wait_until="networkidle", timeout=15000)
        
        # Wait longer for async operations
        time.sleep(6)
        
        state = page.evaluate("""() => ({
            selectedCmdId: state.selectedCmdId,
            selectedInstUrl: state.selectedInstUrl,
            serverReachable: state.serverReachable,
            connections: state.connections ? state.connections.map(c => ({url: c.url, label: c.label, reachable: c.reachable, _commands_count: c._commands ? c._commands.length : 'none'})) : null,
            panels: state.panels ? state.panels.map(p => ({id: p.id, selectedCmdId: p.selectedCmdId, selectedInstUrl: p.selectedInstUrl})) : null,
            _snapshotLoaded: state._snapshotLoaded,
        })""")
        print(f"=== State after 6s wait ===")
        print(json.dumps(state, indent=2))
        
        # Check sidebar content
        sidebar_cmds = page.evaluate("""() => {
            const items = document.querySelectorAll('.cmd-item');
            return Array.from(items).map(el => ({
                text: el.textContent.trim().substring(0, 50),
                selected: el.classList.contains('selected'),
                cmdId: el.dataset.cmdId
            }));
        }""")
        print(f"\n=== Sidebar commands ({len(sidebar_cmds)}) ===")
        for cmd in sidebar_cmds:
            print(f"  {json.dumps(cmd)}")
        
        # Check panels content
        panels = page.evaluate("""() => {
            const ps = document.querySelectorAll('.panel');
            return Array.from(ps).map(el => ({
                id: el.id,
                visible: el.offsetParent !== null,
                hasVtty: el.querySelectorAll('.c.w1').length > 0,
                hasPre: !!el.querySelector('pre'),
                cmdName: (el.querySelector('.cmd-fullname') || {}).textContent || '',
                text: el.textContent.substring(0, 200)
            }));
        }""")
        print(f"\n=== Panels ===")
        for p in panels:
            print(f"  {json.dumps(p)}")
        
        # Check for VTTY cells globally
        vtty_count = page.evaluate("() => document.querySelectorAll('.c.w1').length")
        print(f"\nVTTY cells globally: {vtty_count}")
        
        # Check page errors
        print(f"\n=== Page errors ({len(page_errors)}) ===")
        for err in page_errors:
            print(f"  {err}")
        
        # Check console warnings/errors
        for log in console_logs:
            if '[error]' in log or '[warning]' in log:
                print(f"  LOG: {log}")
        
        browser.close()

if __name__ == "__main__":
    main()
