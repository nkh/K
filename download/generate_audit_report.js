const { Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell,
  Header, Footer, AlignmentType, HeadingLevel, PageNumber, WidthType,
  BorderStyle, ShadingType, TableOfContents, PageBreak, SectionType,
  NumberFormat } = require("docx");
const fs = require("fs");

// ── Tech palette ──
const P = {
  primary: "0A1628", body: "1A2B40", secondary: "6878A0",
  accent: "5B8DB8", surface: "F4F8FC",
  table: { headerBg: "1B6B7A", headerText: "FFFFFF", accentLine: "5B8DB8", innerLine: "C8DDE2", surface: "EDF3F5" }
};
const c = (hex) => hex.replace("#", "");
const NB = { style: BorderStyle.NONE, size: 0, color: "FFFFFF" };
const noBorders = { top: NB, bottom: NB, left: NB, right: NB };
const allNoBorders = { top: NB, bottom: NB, left: NB, right: NB, insideHorizontal: NB, insideVertical: NB };

// ── Helper functions ──
function h1(text) {
  return new Paragraph({
    heading: HeadingLevel.HEADING_1,
    spacing: { before: 480, after: 200 },
    children: [new TextRun({ text, bold: true, color: c(P.primary), font: { name: "Times New Roman" }, size: 32 })]
  });
}

function h2(text) {
  return new Paragraph({
    heading: HeadingLevel.HEADING_2,
    spacing: { before: 360, after: 160 },
    children: [new TextRun({ text, bold: true, color: c(P.primary), font: { name: "Times New Roman" }, size: 28 })]
  });
}

function h3(text) {
  return new Paragraph({
    heading: HeadingLevel.HEADING_3,
    spacing: { before: 280, after: 120 },
    children: [new TextRun({ text, bold: true, color: c(P.primary), font: { name: "Times New Roman" }, size: 26 })]
  });
}

function body(text) {
  return new Paragraph({
    alignment: AlignmentType.JUSTIFIED,
    spacing: { line: 312, after: 120 },
    children: [new TextRun({ text, size: 22, color: c(P.body), font: { name: "Calibri" } })]
  });
}

function bodyBold(label, text) {
  return new Paragraph({
    alignment: AlignmentType.JUSTIFIED,
    spacing: { line: 312, after: 120 },
    children: [
      new TextRun({ text: label, bold: true, size: 22, color: c(P.primary), font: { name: "Calibri" } }),
      new TextRun({ text, size: 22, color: c(P.body), font: { name: "Calibri" } })
    ]
  });
}

function bullet(text) {
  return new Paragraph({
    bullet: { level: 0 },
    spacing: { line: 312, after: 60 },
    children: [new TextRun({ text, size: 22, color: c(P.body), font: { name: "Calibri" } })]
  });
}

function makeHeaderCell(text) {
  return new TableCell({
    children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text, bold: true, size: 20, color: c(P.table.headerText), font: { name: "Calibri" } })] })],
    shading: { type: ShadingType.CLEAR, fill: c(P.table.headerBg) },
    margins: { top: 60, bottom: 60, left: 100, right: 100 },
    tableHeader: true,
  });
}

function makeCell(text, shade = false) {
  return new TableCell({
    children: [new Paragraph({ spacing: { line: 280 }, children: [new TextRun({ text, size: 20, color: c(P.body), font: { name: "Calibri" } })] })],
    shading: shade ? { type: ShadingType.CLEAR, fill: c(P.table.surface) } : undefined,
    margins: { top: 50, bottom: 50, left: 100, right: 100 },
  });
}

function makeCellBold(text, shade = false) {
  return new TableCell({
    children: [new Paragraph({ spacing: { line: 280 }, children: [new TextRun({ text, bold: true, size: 20, color: c(P.primary), font: { name: "Calibri" } })] })],
    shading: shade ? { type: ShadingType.CLEAR, fill: c(P.table.surface) } : undefined,
    margins: { top: 50, bottom: 50, left: 100, right: 100 },
  });
}

function statusCell(text, shade) {
  const color = text === "PASS" ? "2E7D32" : text === "FAIL" ? "C62828" : text === "WARN" ? "E65100" : c(P.body);
  return new TableCell({
    children: [new Paragraph({ alignment: AlignmentType.CENTER, spacing: { line: 280 }, children: [new TextRun({ text, bold: true, size: 20, color, font: { name: "Calibri" } })] })],
    shading: shade ? { type: ShadingType.CLEAR, fill: c(P.table.surface) } : undefined,
    margins: { top: 50, bottom: 50, left: 100, right: 100 },
  });
}

function simpleTable(headers, rows) {
  const headerRow = new TableRow({ children: headers.map(h => makeHeaderCell(h)), tableHeader: true });
  const dataRows = rows.map((row, i) => new TableRow({ children: row.map(cell => makeCell(cell, i % 2 === 0)), cantSplit: true }));
  return new Table({
    width: { size: 100, type: WidthType.PERCENTAGE },
    borders: {
      top: { style: BorderStyle.SINGLE, size: 2, color: c(P.table.accentLine) },
      bottom: { style: BorderStyle.SINGLE, size: 2, color: c(P.table.accentLine) },
      left: { style: BorderStyle.NONE }, right: { style: BorderStyle.NONE },
      insideHorizontal: { style: BorderStyle.SINGLE, size: 1, color: c(P.table.innerLine) },
      insideVertical: { style: BorderStyle.NONE },
    },
    rows: [headerRow, ...dataRows],
  });
}

function statusTable(headers, rows, statusColIdx) {
  const headerRow = new TableRow({ children: headers.map(h => makeHeaderCell(h)), tableHeader: true });
  const dataRows = rows.map((row, i) => new TableRow({
    children: row.map((cell, ci) => ci === statusColIdx ? statusCell(cell, i % 2 === 0) : makeCell(cell, i % 2 === 0)),
    cantSplit: true,
  }));
  return new Table({
    width: { size: 100, type: WidthType.PERCENTAGE },
    borders: {
      top: { style: BorderStyle.SINGLE, size: 2, color: c(P.table.accentLine) },
      bottom: { style: BorderStyle.SINGLE, size: 2, color: c(P.table.accentLine) },
      left: { style: BorderStyle.NONE }, right: { style: BorderStyle.NONE },
      insideHorizontal: { style: BorderStyle.SINGLE, size: 1, color: c(P.table.innerLine) },
      insideVertical: { style: BorderStyle.NONE },
    },
    rows: [headerRow, ...dataRows],
  });
}

function spacer() {
  return new Paragraph({ spacing: { after: 80 }, children: [] });
}

// ── Cover page ──
function buildCover() {
  const titleText = "K Project Comprehensive Audit Report";
  const { titlePt, titleLines } = calcTitleLayout(titleText, 14000, 38, 24);
  const spacing = calcCoverSpacing({ titleLineCount: titleLines.length, titlePt, metaLineCount: 3, fixedHeight: 600 });

  return [
    // Top accent line
    new Paragraph({ spacing: { before: 0, after: 0 }, border: { top: { style: BorderStyle.SINGLE, size: 24, color: c(P.accent) } }, children: [] }),
    // Spacer to push content down
    new Paragraph({ spacing: { before: spacing.topSpacing, after: 0 }, children: [] }),
    // Title
    ...titleLines.map(line => new Paragraph({
      alignment: AlignmentType.LEFT,
      spacing: { line: Math.ceil(titlePt * 23), lineRule: "atLeast", after: 80 },
      children: [new TextRun({ text: line, bold: true, size: titlePt * 2, color: c(P.primary), font: { name: "Times New Roman" } })]
    })),
    // Subtitle
    new Paragraph({ spacing: { before: spacing.midSpacing, after: 120 }, children: [
      new TextRun({ text: "Documentation, Tests, Man Pages, Shell Completions & Feature Completeness", size: 24, color: c(P.secondary), font: { name: "Calibri" } })
    ] }),
    // Meta info
    new Paragraph({ spacing: { before: 200, after: 60 }, children: [
      new TextRun({ text: "Date: ", bold: true, size: 22, color: c(P.secondary), font: { name: "Calibri" } }),
      new TextRun({ text: "June 9, 2026", size: 22, color: c(P.body), font: { name: "Calibri" } })
    ] }),
    new Paragraph({ spacing: { after: 60 }, children: [
      new TextRun({ text: "Branch: ", bold: true, size: 22, color: c(P.secondary), font: { name: "Calibri" } }),
      new TextRun({ text: "web_ui_fix2 (based on main)", size: 22, color: c(P.body), font: { name: "Calibri" } })
    ] }),
    new Paragraph({ spacing: { after: 60 }, children: [
      new TextRun({ text: "Commit: ", bold: true, size: 22, color: c(P.secondary), font: { name: "Calibri" } }),
      new TextRun({ text: "5970851", size: 22, color: c(P.body), font: { name: "Calibri" } })
    ] }),
    // Bottom accent line
    new Paragraph({ spacing: { before: spacing.bottomSpacing, after: 0 }, border: { bottom: { style: BorderStyle.SINGLE, size: 12, color: c(P.accent) } }, children: [] }),
  ];
}

function calcTitleLayout(title, maxWidth, preferredPt = 38, minPt = 24) {
  let pt = preferredPt;
  let lines;
  const charW = (p) => p * 20;
  const cpl = (p) => Math.floor(maxWidth / charW(p));
  while (pt >= minPt) {
    const c = cpl(pt);
    if (c < 2) { pt -= 2; continue; }
    lines = splitTitleLines(title, c);
    if (lines.length <= 3) break;
    pt -= 2;
  }
  if (!lines || lines.length > 3) { lines = splitTitleLines(title, cpl(minPt)); pt = minPt; }
  return { titlePt: pt, titleLines: lines };
}

function splitTitleLines(title, cpl) {
  if (title.length <= cpl) return [title];
  const breakAfter = new Set([' ', '-', '_']);
  const lines = [];
  let rem = title;
  while (rem.length > cpl) {
    let br = -1;
    for (let i = cpl; i >= Math.floor(cpl * 0.6); i--) { if (i < rem.length && breakAfter.has(rem[i - 1])) { br = i; break; } }
    if (br === -1) br = cpl;
    lines.push(rem.slice(0, br).trim());
    rem = rem.slice(br).trim();
  }
  if (rem) lines.push(rem);
  return lines;
}

function calcCoverSpacing({ titleLineCount = 1, titlePt = 36, metaLineCount = 0, fixedHeight = 600 }) {
  const titleHeight = titlePt * 23 + 80 * (titleLineCount - 1);
  const metaHeight = 22 * 11 * metaLineCount + 60 * metaLineCount;
  const totalContent = titleHeight + metaHeight + 400 + fixedHeight;
  const remaining = 16838 - 200 - totalContent;
  const topSpacing = Math.max(2000, Math.min(4000, remaining * 0.5));
  const midSpacing = Math.max(400, Math.min(1200, remaining * 0.2));
  const bottomSpacing = Math.max(600, remaining * 0.3);
  return { topSpacing, midSpacing, bottomSpacing };
}

// ── Build document content ──
const coverChildren = buildCover();

const bodyChildren = [
  // ── TABLE OF CONTENTS ──
  new TableOfContents("Table of Contents", { hyperlink: true, headingStyleRange: "1-3" }),
  new Paragraph({ spacing: { after: 120 }, children: [
    new TextRun({ text: "Note: Right-click the TOC and select \"Update Field\" to refresh page numbers after opening.", italics: true, size: 20, color: c(P.secondary), font: { name: "Calibri" } })
  ] }),
  new Paragraph({ children: [new PageBreak()] }),

  // ── 1. EXECUTIVE SUMMARY ──
  h1("1. Executive Summary"),
  body("This report presents a comprehensive audit of the K project (vrc/vrw), covering five critical areas: Rust test coverage, JavaScript web UI test coverage, man page completeness, shell completion scripts, and documentation completeness. The audit was performed on the web_ui_fix2 branch (commit 5970851) which includes all Phase 2 and Phase 3 web UI fixes, the kill-all/stop-all feature, per-server header colors, and state persistence."),
  body("The K project is a Rust-based terminal multiplexer and process manager with two binaries: vrc (CLI-focused, Unix Domain Socket IPC) and vrw (web dashboard with WebSocket-based terminal emulation). The project features a virtual terminal emulator (vtty), interactive display with split panes, a web admin interface with modular JavaScript, daemon mode, TLS support, and per-instance process management."),
  body("Key findings include a significant test coverage gap (73.3% of Rust source files have no unit tests), critical man page defects (VRL typo in 10 pages, missing pages for keep/unkeep commands), zero shell completion documentation in user-facing docs, and several stale values in the requirements document. The kill-all and stop-all commands have been implemented on the web_ui_fix2 branch with basic tests."),

  h2("1.1 Key Metrics at a Glance"),
  simpleTable(
    ["Metric", "Value", "Assessment"],
    [
      ["Rust source files", "101", "Large codebase"],
      ["Files with unit tests", "27 (26.7%)", "FAIL - below 50%"],
      ["Total Rust unit tests", "~431", "Good quantity"],
      ["Total Rust integration tests", "276", "Good quantity"],
      ["Web UI JS modules", "26", "Moderate"],
      ["JS modules with tests", "24 (92%)", "PASS"],
      ["JS trivial assertion ratio", "~36%", "WARN - needs improvement"],
      ["Man pages total", "34", "Comprehensive"],
      ["Missing man pages", "2 (keep, unkeep)", "FAIL"],
      ["Man pages with typos", "10 (VRL instead of VRC)", "FAIL"],
      ["Shell completions", "5 shells (bash/zsh/fish/elvish/powershell)", "PASS"],
      ["Completion documentation", "Only in man pages", "FAIL - absent from README/MANUAL/FAQ"],
      ["Doc pages (mdBook)", "~57", "Very comprehensive"],
      ["Stale requirement values", "2 (port, config path)", "WARN"],
    ]
  ),
  spacer(),

  // ── 2. RUST TEST COVERAGE AUDIT ──
  h1("2. Rust Test Coverage Audit"),
  body("The Rust backend consists of 101 source files across 13 modules. Unit tests are concentrated in the vtty and cli modules, which together account for the majority of the ~431 unit tests. Integration tests in the tests/ directory add 276 more tests, bringing the grand total to approximately 707 tests. However, the distribution is highly uneven: entire modules such as ipc, daemon, logging, and handles have zero unit test coverage."),

  h2("2.1 Coverage by Module"),
  statusTable(
    ["Module", "Files", "Tested", "Coverage %", "Status"],
    [
      ["vtty/", "14", "9", "64%", "PASS"],
      ["process/", "7", "4", "57%", "WARN"],
      ["cli/", "13", "5", "38%", "WARN"],
      ["cli/commands/", "14", "5", "36%", "WARN"],
      ["config/", "16", "5", "31%", "FAIL"],
      ["interactive/", "7", "2", "29%", "FAIL"],
      ["web/", "17", "2", "12%", "FAIL"],
      ["web/handlers/", "14", "1", "7%", "FAIL"],
      ["ipc/", "4", "0", "0%", "FAIL"],
      ["daemon/", "3", "0", "0%", "FAIL"],
      ["logging/", "2", "0", "0%", "FAIL"],
      ["instance/", "3", "0", "0%", "FAIL"],
      ["handles/", "6", "0", "0%", "FAIL"],
    ], 4
  ),
  spacer(),

  h2("2.2 Well-Tested Files (Top 10 by Test Count)"),
  simpleTable(
    ["File", "Tests", "Focus Areas"],
    [
      ["src/vtty/emulator.rs", "83", "Text rendering, colors, cursor, scroll, OSC, sixel, CJK"],
      ["src/cli/args.rs", "68", "CLI flag parsing, conflicts, implicit spawn, subcommands"],
      ["src/vtty/parser.rs", "51", "CSI/OSC/DCS parsing, UTF-8, escape sequences"],
      ["src/vtty/renderer.rs", "31", "Plain/ANSI/HTML output, RLE, wide chars, diff"],
      ["src/vtty/buffer.rs", "21", "Scroll, resize, insert/delete, diff, generation tracking"],
      ["src/vtty/sink.rs", "19", "Broadcast, in-memory, log sinks, lifecycle"],
      ["src/vtty/rate_limiter.rs", "15", "Burst, throttle, refill, config, disabled state"],
      ["src/vtty/cell.rs", "14", "Character width (ASCII, CJK, emoji, combining)"],
      ["src/process/error.rs", "14", "Error display, IO error mapping, Send/Sync"],
      ["src/cli/commands/common.rs", "10", "Target resolution, display string, auto-select"],
    ]
  ),
  spacer(),

  h2("2.3 Critical Untested Modules"),
  body("The following modules have zero unit tests and contain security-critical or core infrastructure code that should be prioritized for test coverage:"),

  h3("2.3.1 IPC Module (Zero Tests)"),
  body("The inter-process communication module (src/ipc/) provides the Unix Domain Socket protocol used by vrc to communicate with running instances. Functions such as encode_frame(), decode_frame(), and spawn_control_server() are tested only through integration tests. Frame encoding/decoding bugs could cause silent data corruption or command injection vulnerabilities. The server's socket lifecycle and client reconnection logic also lack unit-level validation."),

  h3("2.3.2 Web Handler Modules (7% Coverage)"),
  body("All 14 web handler files in src/web/handlers/ have essentially zero unit tests. These handlers implement the REST API and WebSocket endpoints for the vrw web dashboard. Critical handlers such as ws.rs (WebSocket terminal), commands.rs (process lifecycle), auth.rs (authentication), and certificates.rs (TLS certificate management) are security-sensitive and handle untrusted network input. The only handler with any test is commands.rs, which has 2 tests for the kill-all response structure added on the web_ui_fix2 branch."),

  h3("2.3.3 Instance Registry (Zero Tests)"),
  body("The instance module (src/instance/) provides the InstanceRegistry which is the foundation for multi-instance management. Functions for registering, discovering, and tracking running instances across the system have no unit tests. This is particularly important for the recently added stop-all command, which iterates over all registered instances."),

  h3("2.3.4 Configuration Loader (Zero Tests)"),
  body("The config loader (src/config/loader.rs) reads and parses YAML configuration files, applying environment variable overrides and profile selection. Despite being a critical code path that every invocation depends on, it has no unit tests. Configuration loading bugs could manifest as silent misconfigurations or panics at startup."),

  h2("2.4 Integration Test Summary"),
  body("The five integration test files provide good coverage of the core process lifecycle: spawn, list, kill, VTTY output, key encoding, snapshot/diff, freeze/thaw, and resize. The regression tests (64 tests) cover real-world scenarios including process exit detection, concurrent spawns, and display rendering. However, integration tests do not cover web API endpoints, WebSocket communication, or TLS certificate workflows."),

  h2("2.5 Test Infrastructure Gaps"),
  bullet("Only one dev-dependency: tokio-test = \"0.4\". Missing: tempfile, assert_cmd, mockall, tower-testutil, axum-test."),
  bullet("No clippy configuration (clippy.toml or .clippy.toml). All lints use default settings."),
  bullet("No code coverage tool configured (no cargo-tarpaulin, cargo-llvm-cov, or codecov)."),
  bullet("No [profile.test] configuration for optimized test builds."),
  bullet("No [features] section for test-specific features."),
  bullet("No [[test]] configuration for integration test harness customization."),

  h2("2.6 kill-all / stop-all Command Status"),
  body("Both kill-all and stop-all commands exist and are implemented on the web_ui_fix2 branch. The handle_kill_all_commands() function in src/cli/commands/ipc.rs kills all commands in a single running vrc instance by PID. The handle_stop_all_commands() function in src/cli/commands/stop.rs stops all commands across all running vrw instances. Basic unit tests exist in commands.rs (2 tests for kill-all response structure) and stop.rs (3 tests including stop-all)."),
  body("The web API endpoint POST /api/commands/kill-all is also implemented in src/web/handlers/commands.rs with a handler that calls the kill-all function and returns structured JSON. The route is registered in src/web/router.rs."),

  // ── 3. WEB UI TEST COVERAGE AUDIT ──
  h1("3. Web UI Test Coverage Audit"),
  body("The web frontend consists of 26 JavaScript modules in static/admin/modules/. Test coverage is generally good, with 22 modules having direct test files and 3 having indirect coverage through related test files. The test suite uses a custom zero-dependency test framework with mock DOM, providing approximately 500+ assertions across 22 test files."),

  h2("3.1 Module-to-Test Mapping"),
  statusTable(
    ["Module", "Test File", "Status"],
    [
      ["app.js", "NONE", "FAIL"],
      ["eventbus.js", "test_eventbus.js", "PASS"],
      ["state.js", "test_state.js", "PASS"],
      ["utils.js", "test_utils.js", "PASS"],
      ["focus.js", "test_focus.js", "PASS"],
      ["theme.js", "test_theme.js", "PASS"],
      ["sidebar.js", "test_sidebar.js", "PASS"],
      ["panels.js", "test_panels.js", "PASS"],
      ["commands.js", "test_commands.js", "PASS"],
      ["websocket.js", "test_websocket.js", "PASS"],
      ["vtty.js", "test_vtty.js", "PASS"],
      ["spawn.js", "test_spawn.js", "PASS"],
      ["logs.js", "test_logs.js", "PASS"],
      ["keyboard.js", "test_keyboard.js", "PASS"],
      ["search.js", "test_search.js", "PASS"],
      ["notifications.js", "test_notifications.js", "PASS"],
      ["onboarding.js", "test_onboarding.js", "PASS"],
      ["templates.js", "test_templates.js", "PASS"],
      ["dragdrop.js", "test_dragdrop.js", "PASS"],
      ["workspaces.js", "test_workspaces.js", "PASS"],
      ["misc.js", "test_misc.js", "PASS"],
      ["snapshot.js", "NONE (indirect only)", "FAIL"],
      ["command-selection.js", "test_commands.js (indirect)", "WARN"],
      ["command-ui.js", "test_commands.js (indirect)", "WARN"],
      ["commands-core.js", "test_commands.js (indirect)", "WARN"],
      ["server-connections.js", "test_commands.js (indirect)", "WARN"],
    ], 2
  ),
  spacer(),

  h2("3.2 Critical Gaps"),
  body("The two modules with the highest risk are app.js and snapshot.js. The app.js module is the main orchestrator controlling the entire application lifecycle, including initialization, auto-connect logic, refresh scheduling, and startup sequencing. No part of this initialization flow is tested. The snapshot.js module handles state persistence (loadSnapshot, fetchServerConfig), which was recently implemented on the web_ui_fix2 branch and is completely untested in isolation."),

  h2("3.3 Test Quality Assessment"),
  body("Test quality varies significantly across files. High-quality tests (test_utils.js, test_eventbus.js, test_regression.js) use specific expected-value assertions and behavioral verification. However, approximately 36% of all assertions are trivial \"does not throw\" smoke tests that would pass even if the function body were empty. Six test files have over 70% trivial assertions: test_sidebar.js, test_search.js, test_notifications.js, test_logs.js, test_onboarding.js, and test_spawn.js."),
  body("The test infrastructure uses a custom mock DOM that only supports basic operations (#id, .class, tag selectors). No innerHTML parsing means DOM rendering tests are severely limited. Module loading via eval() shares global scope, so stub functions from setup.js (140+ pre-declared no-ops) can silently replace real implementations, causing tests to exercise no-ops rather than actual code. This is particularly dangerous for renderPanels(), loadCommands(), and other core functions that are stubbed but never overridden in test files."),

  h2("3.4 Regression Test Suite"),
  body("The regression suite is excellent: 51 named regression cases in test_regression.js plus 14 bug-fix cases in test_regression_bugs.js cover cross-module scenarios such as welcome panel dismissal, generation skip optimization, theme persistence, XSS prevention, drag-drop data transfer, and multiple UI state transitions. These tests provide the strongest behavioral guarantees in the suite."),

  // ── 4. MAN PAGE AUDIT ──
  h1("4. Man Page Completeness Audit"),
  body("The project includes 34 man pages covering both vrc and vrw binaries and their subcommands. Coverage is excellent overall, with only two subcommands missing man pages entirely. However, several formatting bugs and content gaps were identified that affect usability and discoverability."),

  h2("4.1 Missing Man Pages"),
  statusTable(
    ["Command", "Expected File", "Priority", "Status"],
    [
      ["vrw keep", "vrw-keep.1", "High", "FAIL"],
      ["vrw unkeep", "vrw-unkeep.1", "High", "FAIL"],
    ], 3
  ),
  spacer(),
  body("Both keep and unkeep commands are defined in src/cli/args.rs (lines 467-484) and have handler implementations in src/cli/commands/keep.rs. However, no man pages exist for either command. The vrw.1 SEE ALSO section also does not reference these commands."),

  h2("4.2 Formatting Bugs: VRL Typo"),
  body("A critical typographic bug affects 10 of the 12 vrc subcommand man pages. The .TH (title header) macro contains \"VRL\" instead of \"VRC\", which causes apropos and man -k to index these pages under the wrong name. Users searching for vrc-list, vrc-stop, etc. via apropos will not find them. The affected pages are: vrc-list.1, vrc-stop.1, vrc-config-check.1, vrc-completions.1, vrc-keys.1, vrc-cat.1, vrc-spawn-in.1, vrc-freeze.1, vrc-thaw.1, and vrc-resize.1. Only vrc.1, vrc-kill.1, and vrc-stop-command.1 have the correct \"VRC\" spelling."),

  h2("4.3 Other Man Page Issues"),
  bullet("vrw-completions.1 has year 2026 instead of 2025 (all other pages use 2025)."),
  bullet("vrw-screenshot.1 uses unescaped hyphens in .TH (VRW-SCREENSHOT instead of VRW\\-SCREENSHOT), inconsistent with other pages."),
  bullet("vrw-purge.1 is missing the -i/--interactive option in its synopsis and options section, despite the CLI defining this flag."),
  bullet("vrc.1 has no EXAMPLES section, despite being the main entry point for the vrc binary."),
  bullet("vrc.1 SEE ALSO is missing vrc-config-check(1) and vrc-completions(1) references."),
  bullet("vrw.1 SEE ALSO is missing vrw-keep(1) and vrw-unkeep(1) references (secondary, as pages do not yet exist)."),
  bullet("8 pages have no OPTIONS section (acceptable for commands with no flags, but vrw-purge.1 is a genuine gap)."),

  h2("4.4 Man Page Quality Summary"),
  statusTable(
    ["Aspect", "Rating", "Notes"],
    [
      ["Coverage (32/34 subcommands)", "4/5", "Only keep/unkeep missing"],
      ["Descriptions", "5/5", "Thorough, include UDS/HTTP mechanics and curl examples"],
      ["Examples", "4/5", "Most pages have 3-5 examples; vrc.1 has none"],
      ["Options documentation", "4/5", "vrw-purge missing -i flag"],
      ["Cross-references", "4/5", "vrc.1 missing 2 refs"],
      ["Formatting consistency", "3/5", "VRL typo in 10 pages is significant"],
    ], 1
  ),
  spacer(),

  // ── 5. SHELL COMPLETION AUDIT ──
  h1("5. Shell Completion Scripts Audit"),
  body("Shell completions are implemented via the clap_complete v4 crate, supporting five shells: Bash, Zsh, Fish, Elvish, and PowerShell. Completions are generated on demand at runtime through the \"vrw completions <SHELL>\" and \"vrc completions <SHELL>\" subcommands. No pre-generated completion files exist in the repository; the build.rs file does not generate completions at build time."),

  h2("5.1 Completion Generation Architecture"),
  body("When the vrw binary is compiled with both features (vrc and vrw), the completion tree for vrc is manually constructed by starting from the full vrw command tree, hiding vrw-only subcommands and flags, and adding vrc-only subcommands (keys, spawn-in). The code comment in src/cli/args.rs acknowledges this is \"close enough for shell completion purposes.\" The binary name is resolved from argv[0] via runtime_binary_name(), satisfying requirement FR-91 for renamed binary support."),
  body("This manual construction approach introduces a maintenance risk: as new subcommands are added, the vrc completion command builder must be manually updated to hide/show the appropriate commands. If a developer adds a new vrw-only subcommand but forgets to hide it in build_vrc_completions_command(), it will incorrectly appear in vrc tab completions."),

  h2("5.2 Documentation Gap"),
  body("Shell completions are essentially undocumented in user-facing documentation. The README.md, MANUAL.md, and FAQ.md contain no mention of the completions subcommand, no installation instructions, and no examples of how to enable tab completion. The only documentation exists in two man pages (vrw-completions.1 and vrc-completions.1) and a brief listing in docs/reference/cli.md. This means most users will never discover this feature."),
  body("The vrc-completions.1 man page also has the VRL typo (same as other vrc subcommand pages) and is less detailed than the vrw-completions.1 counterpart. It lacks persistence examples for bashrc and the session activation pattern that vrw-completions.1 includes."),

  h2("5.3 Completion Assessment"),
  statusTable(
    ["Aspect", "Status", "Notes"],
    [
      ["Shells supported", "PASS", "5 shells via clap_complete v4"],
      ["Pre-generated files", "WARN", "None shipped; on-demand only"],
      ["build.rs generation", "FAIL", "Not configured; needed for packaging"],
      ["vrc accuracy (dual-compile)", "WARN", "Manually constructed; may drift"],
      ["Man pages", "WARN", "vrw-completions good; vrc-completions has typo"],
      ["User-facing docs", "FAIL", "Absent from README, MANUAL, FAQ"],
      ["Renamed binary support", "PASS", "FR-91 satisfied via runtime_binary_name()"],
    ], 1
  ),
  spacer(),

  // ── 6. DOCUMENTATION COMPLETENESS AUDIT ──
  h1("6. Documentation Completeness Audit"),
  body("The project has extensive documentation organized using the Diataxis framework, with approximately 57 pages in the mdBook documentation plus a comprehensive MANUAL.md (2600+ lines). Documentation quality is generally high, but several significant gaps exist."),

  h2("6.1 README.md Assessment"),
  body("The README (228 lines) is well-structured with a binary comparison table, feature lists, quick start instructions, usage examples, and architecture overview. Missing elements include: no mention of shell completions, no minimum Rust version (FAQ says 1.75+), no cargo install one-liner for quick installation, no build status badges, and no contributor guidelines link."),

  h2("6.2 MANUAL.md Assessment"),
  body("The MANUAL (2600+ lines) is comprehensive, organized into six parts plus appendices. It covers getting started, everyday use, advanced topics (split-pane, retain/purge, TLS, daemon mode), API reference, security model, and contributor guidelines. Notable gaps: no shell completions section, no performance/benchmarks section, and the --display-all flag deprecation messaging is inconsistent between sections."),

  h2("6.3 Configuration Documentation Gap"),
  body("The docs/configuration.md file (560 lines) documents all shared configuration sections thoroughly with CLI flag mapping tables. However, vrw-only configuration sections (server, security, tls, web) are not documented in this reference file despite being referenced in MANUAL.md. The recently added web.panel_colors configuration for per-server header colors (on web_ui_fix2 branch) is documented through code but not in the configuration reference documentation."),

  h2("6.4 CLI Reference Documentation Gap"),
  body("The docs/reference/cli.md file (666 lines) provides detailed CLI documentation but is missing several flags that exist in args.rs: --profile, --working-directory, --server-name, --pid, --register-with, --certificate, and --token-file. These flags are functional but undocumented."),

  h2("6.5 FAQ Assessment"),
  body("The FAQ (322 lines) is comprehensive for vrc usage but entirely vrc-focused. No questions address vrw-specific topics such as the web dashboard, API usage, TLS configuration, remote access, WebSocket connections, or environment presets. There is also no FAQ entry about configuration file locations or precedence."),

  h2("6.6 Stale Requirement Values"),
  body("The requirements document (docs/requirements.md) contains two stale values that no longer match the codebase. Requirement FR-38 references the config path as ~/.config/vrw/config.yaml, but the actual code uses ~/.config/vrc/config.yaml. Requirement FR-41 lists the default port as 8080, but the actual default is 9090. These discrepancies would confuse anyone using the requirements document as a reference."),

  h2("6.7 Testing Documentation Gap"),
  body("The docs/testing.md file (271 lines) documents Rust unit tests, comprehensive tests, integration tests, regression tests, and benchmarks. However, it completely omits three test categories that exist in the repository: the VTTY integration tests in tests/vtty/ (6+ shell scripts testing terminal rendering), the web UI JavaScript tests in static/admin/test/ (22 test files with 500+ assertions), and the cookbook test scripts in docs/cookbook/scripts/ (5 test scripts). This means contributors may not be aware of these test suites."),

  h2("6.8 Documentation Summary"),
  statusTable(
    ["Document", "Lines", "Completeness", "Key Gaps"],
    [
      ["README.md", "228", "Good", "No completions, no minimum Rust version"],
      ["MANUAL.md", "2600+", "Excellent", "No completions section, --display-all inconsistency"],
      ["docs/configuration.md", "560", "Good", "Missing vrw-only sections (server, security, tls, web)"],
      ["docs/reference/cli.md", "666", "Good", "Missing 7 CLI flags"],
      ["docs/faq.md", "322", "Moderate", "vrc-only; no vrw-specific questions"],
      ["docs/requirements.md", "235", "Moderate", "2 stale values (port, config path)"],
      ["docs/testing.md", "271", "Moderate", "Missing 3 test categories"],
      ["Man pages (34 total)", "-", "Good", "VRL typo (10 pages), missing keep/unkeep"],
    ], 2
  ),
  spacer(),

  // ── 7. PRIORITIZED ACTION ITEMS ──
  h1("7. Prioritized Action Items"),
  body("The following action items are ordered by impact and effort. Each item includes the affected files, estimated effort, and a rationale for prioritization."),

  h2("7.1 Critical Priority"),

  h3("7.1.1 Fix VRL Typo in 10 Man Pages"),
  body("Impact: All 10 vrc subcommand man pages have \"VRL\" instead of \"VRC\" in their .TH macro, making them invisible to apropos/man -k searches. This is a trivial text replacement but has high discoverability impact. Files: vrc-list.1, vrc-stop.1, vrc-config-check.1, vrc-completions.1, vrc-keys.1, vrc-cat.1, vrc-spawn-in.1, vrc-freeze.1, vrc-thaw.1, vrc-resize.1. Effort: 5 minutes."),

  h3("7.1.2 Add Web Handler Unit Tests"),
  body("Impact: All 14 web handler files have zero unit tests, representing the largest untested surface area. These handlers process untrusted network input and implement security-critical operations (auth, TLS certs, WebSocket). Priority handlers: ws.rs, commands.rs, auth.rs, certificates.rs. Use axum::test or tower test utilities. Effort: 2-3 days for critical handlers."),

  h3("7.1.3 Add Missing Man Pages (keep, unkeep)"),
  body("Impact: The keep and unkeep commands have no documentation. Users must read source code to discover these features. Create vrw-keep.1 and vrw-unkeep.1 following the pattern of existing man pages, and add SEE ALSO references in vrw.1. Effort: 30 minutes."),

  h2("7.2 High Priority"),

  h3("7.2.1 Add Shell Completion Documentation"),
  body("Impact: Shell completions are a valuable usability feature that is completely undiscoverable. Add a section to README.md with one-liner installation examples for each shell. Add a section to MANUAL.md Part I or Part II. Add a FAQ entry: \"How do I install shell completions?\" Effort: 1 hour."),

  h3("7.2.2 Fix Stale Requirements"),
  body("Impact: The requirements document is a formal reference that should match the codebase. Fix FR-38 to use ~/.config/vrc/ path. Fix FR-41 to use default port 9090. Effort: 5 minutes."),

  h3("7.2.3 Fix vrw-purge.1 Missing -i Option"),
  body("Impact: Users will not know about the --interactive flag for purge, which provides a safety confirmation prompt. Add -i/--interactive to the synopsis and options section of vrw-purge.1. Effort: 10 minutes."),

  h3("7.2.4 Add app.js and snapshot.js Tests"),
  body("Impact: These are the two most critical untested web UI modules. app.js controls the entire initialization flow; snapshot.js handles state persistence (recently implemented). Add test_app.js covering init(), auto-connect, and startup sequencing. Add test_snapshot.js covering loadSnapshot(), fetchServerConfig(), and error handling with mocked fetch. Effort: 1-2 days."),

  h3("7.2.5 Add IPC Unit Tests"),
  body("Impact: Frame encoding/decoding is the foundation of all inter-process communication. Bugs here could cause silent data corruption. Add unit tests for encode_frame(), decode_frame(), and boundary conditions (empty frames, oversized frames, malformed data). Effort: 4 hours."),

  h2("7.3 Medium Priority"),

  h3("7.3.1 Upgrade Trivial JS Tests to Behavioral Tests"),
  body("Impact: Approximately 180 assertions (36%) in the JS test suite are \"does not throw\" smoke tests. Upgrade the 6 worst-offending test files (test_sidebar.js, test_search.js, test_notifications.js, test_logs.js, test_onboarding.js, test_spawn.js) to include at least one state-mutation or DOM-change assertion per function. Effort: 1 day."),

  h3("7.3.2 Add Configuration Loader Tests"),
  body("Impact: load_config() is called on every invocation and parses YAML with variable overrides. Add unit tests for config file parsing, environment variable overrides, profile selection, error handling for malformed YAML, and default value application. Effort: 4 hours."),

  h3("7.3.3 Document Missing CLI Flags"),
  body("Impact: Seven functional CLI flags are undocumented in docs/reference/cli.md. Add documentation for --profile, --working-directory, --server-name, --pid, --register-with, --certificate, and --token-file. Effort: 1 hour."),

  h3("7.3.4 Document vrw-Only Config Sections"),
  body("Impact: The server, security, tls, and web configuration sections are used by vrw but not documented in docs/configuration.md. Add reference tables following the existing pattern, including the new web.panel_colors feature. Effort: 2 hours."),

  h3("7.3.5 Update Testing Documentation"),
  body("Impact: docs/testing.md is missing three test categories that exist in the repository. Add sections for VTTY integration tests (tests/vtty/), web UI JavaScript tests (static/admin/test/), and cookbook test scripts (docs/cookbook/scripts/). Effort: 1 hour."),

  h3("7.3.6 Add vrw-Specific FAQ Entries"),
  body("Impact: The FAQ is entirely vrc-focused. Add at least 5-10 questions covering: web dashboard access, API authentication, TLS setup, remote access, WebSocket connections, environment presets, and configuration file locations. Effort: 1-2 hours."),

  h2("7.4 Low Priority (Infrastructure)"),

  h3("7.4.1 Add Clippy Configuration"),
  body("Create a clippy.toml or .clippy.toml file to enforce lint standards beyond defaults. This improves code quality consistency across contributors. Effort: 30 minutes."),

  h3("7.4.2 Add Code Coverage Tooling"),
  body("Integrate cargo-tarpaulin or cargo-llvm-cov to measure line/branch coverage. Add a CI step with a coverage threshold (e.g., 50%). This provides objective metrics for tracking test improvement. Effort: 2 hours."),

  h3("7.4.3 Add Dev-Dependencies for Better Testing"),
  body("Add tempfile, assert_cmd, mockall, and tower-testutil to dev-dependencies. These enable file-system testing, CLI integration testing, mocking, and HTTP handler testing respectively. Effort: 1 hour."),

  h3("7.4.4 Fix Minor Man Page Issues"),
  body("Fix year 2026 to 2025 in vrw-completions.1. Fix unescaped hyphens in vrw-screenshot.1 .TH. Add EXAMPLES section to vrc.1. Add missing SEE ALSO references. Effort: 30 minutes."),

  h3("7.4.5 Generate Completion Files at Build Time"),
  body("Add completion script generation to build.rs for packaging support (deb/rpm/homebrew). This enables shipping pre-generated completion files instead of requiring on-demand generation. Effort: 2 hours."),
];

// ── Assemble document ──
const doc = new Document({
  styles: {
    default: {
      document: {
        run: { font: { name: "Calibri" }, size: 22, color: c(P.body) },
        paragraph: { spacing: { line: 312 } },
      },
      heading1: {
        run: { font: { name: "Times New Roman" }, size: 32, bold: true, color: c(P.primary) },
        paragraph: { spacing: { before: 480, after: 200 } },
      },
      heading2: {
        run: { font: { name: "Times New Roman" }, size: 28, bold: true, color: c(P.primary) },
        paragraph: { spacing: { before: 360, after: 160 } },
      },
      heading3: {
        run: { font: { name: "Times New Roman" }, size: 26, bold: true, color: c(P.primary) },
        paragraph: { spacing: { before: 280, after: 120 } },
      },
    },
  },
  sections: [
    // Cover section
    {
      properties: {
        page: {
          size: { width: 11906, height: 16838 },
          margin: { top: 1440, bottom: 1440, left: 1701, right: 1417 },
        },
      },
      children: coverChildren,
    },
    // TOC + Body section
    {
      properties: {
        page: {
          size: { width: 11906, height: 16838 },
          margin: { top: 1440, bottom: 1440, left: 1701, right: 1417 },
          pageNumbers: { start: 1, formatType: NumberFormat.DECIMAL },
        },
      },
      footers: {
        default: new Footer({
          children: [new Paragraph({
            alignment: AlignmentType.CENTER,
            children: [new TextRun({ children: [PageNumber.CURRENT], size: 18, color: c(P.secondary), font: { name: "Calibri" } })],
          })],
        }),
      },
      children: bodyChildren,
    },
  ],
});

Packer.toBuffer(doc).then(buf => {
  fs.writeFileSync("/home/z/my-project/K/download/K_Project_Audit_Report.docx", buf);
  console.log("Audit report generated: /home/z/my-project/K/download/K_Project_Audit_Report.docx");
});
