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

## Phase 1b: Migrate all fetch() calls to api.js
- Migrated **37 fetch() calls** across **16 files** to use `api.*` methods
- Modules migrated: server-connections (7), spawn (11), panels (4), keyboard (2), commands-core (2), templates (3), notifications (1), websocket (2), workspaces (3), logs (1), search (6), snapshot (2), misc (1), vtty (5), sidebar (1)
- Added `api.getSnapshot()`, `api.getPeers()`, `api.getVttyText()`, `api.getJson()` to cover all endpoints
- **Zero fetch() calls remain outside api.js**
- Net result: **-156 lines** (259 removed, 103 added)
- Tests: 735→735, 0 regressions