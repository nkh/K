"""Test the vrw web UI by loading it in a headless browser and checking behavior."""
import json
import time
from playwright.sync_api import sync_playwright

BASE = "http://127.0.0.1:9090"

def main():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        
        # Collect console logs
        console_logs = []
        page.on("console", lambda msg: console_logs.append(f"[{msg.type}] {msg.text}"))
        
        # Collect page errors
        page_errors = []
        page.on("pageerror", lambda err: page_errors.append(str(err)))
        
        print("=== Loading web UI ===")
        page.goto(BASE, wait_until="networkidle", timeout=15000)
        print(f"Page title: {page.title()}")
        print(f"URL: {page.url}")
        
        # Wait for JS to run
        time.sleep(3)
        
        # Check console logs
        print(f"\n=== Console logs ({len(console_logs)}) ===")
        for log in console_logs[:80]:
            print(f"  {log}")
        if len(console_logs) > 80:
            print(f"  ... and {len(console_logs)-80} more")
        
        # Check page errors
        print(f"\n=== Page errors ({len(page_errors)}) ===")
        for err in page_errors[:20]:
            print(f"  {err}")
        
        # Check what's visible
        print("\n=== Page content checks ===")
        
        # Check sidebar
        sidebar = page.query_selector(".sidebar")
        if sidebar:
            print(f"Sidebar found: visible={sidebar.is_visible()}")
            sidebar_text = sidebar.inner_text()
            print(f"Sidebar text: {repr(sidebar_text[:300])}")
        else:
            print("NO SIDEBAR FOUND!")
        
        # Check panels
        panels = page.query_selector_all(".panel")
        print(f"Panels found: {len(panels)}")
        for i, panel in enumerate(panels):
            vis = panel.is_visible()
            text = panel.inner_text()[:200]
            print(f"  Panel {i}: visible={vis}, text={repr(text)}")
        
        # Check for "No command selected" message
        body_text = page.inner_text("body")
        if "No command selected" in body_text:
            print("\n!!! 'No command selected' message found - AUTO-SELECT FAILED !!!")
        else:
            print("\nNo 'No command selected' message found")
        
        # Check for the sleep command in sidebar
        if "sleep" in body_text:
            print("'sleep' command found in page - good")
        else:
            print("!!! 'sleep' command NOT found in page !!!")
        
        # Check if VTTY is being displayed
        vtty_cells = page.query_selector_all(".c.w1")
        print(f"\nVTTY cells found: {len(vtty_cells)}")
        if len(vtty_cells) > 0:
            print("VTTY rendering is active - auto-select likely worked!")
        
        # Check the selectedCmdId state
        selected_state = page.evaluate("() => typeof state !== 'undefined' && state.selectedCmdId")
        print(f"\nstate.selectedCmdId: {selected_state}")
        
        server_reachable = page.evaluate("() => typeof state !== 'undefined' && state.serverReachable")
        print(f"state.serverReachable: {server_reachable}")
        
        snapshot_loaded = page.evaluate("() => typeof state !== 'undefined' && state._snapshotLoaded")
        print(f"state._snapshotLoaded: {snapshot_loaded}")
        
        panels_state = page.evaluate("() => typeof state !== 'undefined' ? state.panels : 'undefined'")
        print(f"state.panels: {panels_state}")
        
        # Check if connections list has anything
        connections = page.evaluate("() => typeof connections !== 'undefined' ? connections : 'undefined'")
        print(f"connections global: {connections}")
        
        # Check state.instUrl
        inst_url = page.evaluate("() => typeof state !== 'undefined' ? state.instUrl : 'undefined'")
        print(f"state.instUrl: {inst_url}")
        
        browser.close()

if __name__ == "__main__":
    main()
