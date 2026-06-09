---
Task ID: 1
Agent: main
Task: Fix all pre-existing test failures and add regression tests for web UI bugs

Work Log:
- Compiled Rust project on web_ui_fix branch — compiles cleanly
- Ran JS regression tests — found 6 pre-existing failures: REG-04, REG-10, REG-17, REG-21, REG-28, REG-30
- Fixed REG-04: test set refreshMs=100 then called changeRefreshMs(100) which adds 100 → 200; changed initial value to 0
- Fixed REG-10: mock DOM querySelector didn't search children; fixed MockElement className setter to sync with _classList; fixed querySelector to search children first
- Fixed REG-17: test saved workspaces as array but code expects name-keyed object
- Fixed REG-21: test called togglePinCmd(instUrl, cmdId) but function takes (cmdName)
- Fixed REG-28: _onboardingSteps not exported; exported from onboarding.js
- Fixed REG-30: _hex not exported to window; added window._hex export in utils.js
- Fixed setup.js classList.add/remove/toggle to sync back to className
- Investigated 3 remaining bugs (drag-drop, sidebar dedup, spawn 9090): all already fixed in code, tests confirm
- Added REG-BUG-013 through REG-BUG-016 tests
- All 194 tests pass, 0 failures
- Pushed to origin/web_ui_fix (commits 49fdc8f, 908cb5f, 6fa3c7e)

Stage Summary:
- 6 pre-existing test failures fixed by correcting test code and adding missing exports
- 3 web UI bugs (drag-drop, sidebar dedup, spawn 9090) confirmed already fixed
- Mock DOM improvements: className/_classList sync, querySelector child search
- Missing global exports added: _hex, _onboardingSteps
- Total: 194 tests pass, 0 failures
- Rust compiles cleanly
