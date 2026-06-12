# Refactoring Worklog

## Phase 0a: Remove diagnostic logging
- Removed all `[DBG]` console.log statements from `websocket.js` (8 lines) and `panels.js` (3 lines)
- Tests: 698→698 (no change, as expected)

## Phase 1a: Create api.js centralized API layer
- Created `modules/api.js` — single module with 29 methods covering all 24 unique HTTP endpoints + 2 WS endpoints
- All methods are async, use `apiUrl()` and `authHeadersForInstance()` from utils.js internally
- `connectVtty()` returns `{ ws, close(), readyState }` handle — replaces 3 duplicate WS setup functions (675 lines) with one 50-line implementation
- `connectLogWs()` returns same handle pattern
- `killAll()` handles both filtered (array of IDs) and unfiltered (kill-all endpoint) cases
- Added `test/test_api.js` — 37 tests verifying all methods exist, URL construction, handle objects
- Added `api.js` to `index.html` script load order (after utils.js)
- Added `api.js` to `test/setup.js` module load order
- Tests: 698→735 (37 new, all pass). Pre-existing 2 CSS failures unchanged.
- No existing modules migrated yet — api.js is available but not yet used by callers