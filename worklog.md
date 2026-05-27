---
Task ID: 1
Agent: main
Task: Fix completely broken web UI

Work Log:
- Analyzed user's screenshot showing broken layout (single-column instead of sidebar+content, empty terminal, misaligned elements)
- Identified that previous commits (1d7e662, d85f186, 4fed331) reorganized HTML/CSS/JS and broke the UI
- Initially restored to 24f0fc3 (oldest of 13 web UI commits) - too old, missing theme support
- Correctly identified 111ae5e as the last known-good state (parent of first broken commit)
- Verified 111ae5e already has all 3 features the user wanted: theme toggle, compact buttons, direct keyboard input
- Restored all 5 changed files: index.html, style.css, app.js, MANUAL.md, docs/usage.md
- Confirmed 0 diff between working tree and 111ae5e for all web UI and doc files
- All 67 tests pass (65 unit + 2 doc-tests)
- Cargo check succeeds with 0 errors

Stage Summary:
- Root cause: Previous modification attempts reorganized HTML structure, added new CSS classes (btn-xxs, panel-actions, panel-keys-bar), and restructured JS renderPanels() — breaking the layout entirely
- Fix: Full revert to 111ae5e (known-good state with all features already working)
- Key insight: The 3 requested features (theme toggle, compact buttons, direct keyboard input) were ALREADY present in the known-good state — no changes were needed
- The files are embedded via rust_embed — user must recompile to see the fix
