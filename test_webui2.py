"""More detailed web UI test to find exactly where JS execution fails."""
import json
import time
from playwright.sync_api import sync_playwright

BASE = "http://127.0.0.1:9090"

def main():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        
        # Collect console logs with timing
        console_logs = []
        page.on("console", lambda msg: console_logs.append(f"[{msg.type}] {msg.text}"))
        
        # Collect page errors
        page_errors = []
        page.on("pageerror", lambda err: page_errors.append(str(err)))
        
        # Intercept network requests to see what's fetched and what fails
        failed_requests = []
        page.on("requestfailed", lambda req: failed_requests.append(f"FAILED: {req.url} - {req.failure}"))
        
        print("=== Loading web UI ===")
        page.goto(BASE, wait_until="domcontentloaded", timeout=15000)
        print(f"Page title: {page.title()}")
        
        # Wait for all JS to initialize
        time.sleep(4)
        
        # Check for each critical function
        critical_funcs = [
            'initBottombar', 'getSelectedPanel', 'loadSnapshot', 
            'addConnection', 'addPanelDirect', 'startRefresh',
            'fetchServerConfig', 'renderPanels', 'loadCommands',
            'selectCommand', 'connectPanelWs', '_buildSidebar'
        ]
        
        print("\n=== Critical function availability ===")
        for fn in critical_funcs:
            available = page.evaluate(f"() => typeof {fn} === 'function'")
            print(f"  {fn}: {'OK' if available else 'MISSING!'}")
        
        # Check state
        state_checks = page.evaluate("""() => ({
            selectedCmdId: state.selectedCmdId,
            selectedInstUrl: state.selectedInstUrl,
            serverReachable: state.serverReachable,
            connections: state.connections ? state.connections.map(c => ({url: c.url, label: c.label, reachable: c.reachable})) : null,
            panels: state.panels ? state.panels.map(p => ({id: p.id, selectedCmdId: p.selectedCmdId, selectedInstUrl: p.selectedInstUrl})) : null,
            instanceUrls: state.instanceUrls,
            _snapshotLoaded: state._snapshotLoaded,
        })""")
        print(f"\n=== State ===")
        print(json.dumps(state_checks, indent=2))
        
        # Check console errors
        print(f"\n=== Console errors ===")
        for log in console_logs:
            if log.startswith("[error]") or log.startswith("[warning]"):
                print(f"  {log}")
        
        print(f"\n=== Page errors ===")
        for err in page_errors:
            print(f"  {err}")
        
        print(f"\n=== Failed requests ===")
        for req in failed_requests:
            print(f"  {req}")
        
        # Check sidebar content
        sidebar_cmds = page.evaluate("""() => {
            const items = document.querySelectorAll('.cmd-item');
            return Array.from(items).map(el => el.textContent.trim());
        }""")
        print(f"\n=== Sidebar commands: {sidebar_cmds} ===")
        
        # Check panels
        panel_content = page.evaluate("""() => {
            const panels = document.querySelectorAll('.panel');
            return Array.from(panels).map(el => ({
                id: el.id,
                visible: el.offsetParent !== null,
                text: el.textContent.substring(0, 100)
            }));
        }""")
        print(f"\n=== Panels: ===")
        for p in panel_content:
            print(f"  {json.dumps(p)}")
        
        browser.close()

if __name__ == "__main__":
    main()
