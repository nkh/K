//! Web UI static asset integrity tests.
//!
//! These tests verify that the embedded HTML, CSS, and JS files are
//! internally consistent.  They catch regressions where JavaScript
//! references DOM elements (via getElementById) that don't exist in the
//! HTML, or where the HTML references scripts/styles that aren't embedded.
//!
//! All tests operate on the compile-time embedded assets via `rust-embed`,
//! so they run without starting a server.

use rust_embed::RustEmbed;

/// Mirror the same folder as the production handler.
#[derive(RustEmbed)]
#[folder = "static/admin/"]
struct TestAssets;

/// Extract the embedded file content as a UTF-8 string, panicking if
/// the file is missing or not valid UTF-8.
fn asset(name: &str) -> String {
    let content = TestAssets::get(name)
        .unwrap_or_else(|| panic!("embedded asset '{}' not found — is static/admin/ populated?", name));
    String::from_utf8_lossy(&content.data).into_owned()
}

// ─── Helper: extract all `id="..."` values from HTML ────────────────────────

/// Return a set of all element IDs declared in the HTML source.
/// Handles both `id="foo"` and `id='foo'`.
fn html_ids(html: &str) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    // Match id="..." or id='...'
    for cap in regex_lite(html, r#"id\s*=\s*"([^"]+)""#) {
        ids.insert(cap);
    }
    for cap in regex_lite(html, r#"id\s*=\s*'([^']+)'"#) {
        ids.insert(cap);
    }
    ids
}

/// Naive regex captures — returns all first-group matches.
fn regex_lite(haystack: &str, pattern: &str) -> Vec<String> {
    let re = regex::Regex::new(pattern).expect("invalid regex pattern");
    re.captures_iter(haystack)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// 1. EMBEDDED ASSET EXISTENCE
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn web_index_html_exists() {
    let html = asset("index.html");
    assert!(html.len() > 100, "index.html suspiciously small");
}

#[test]
fn web_app_js_exists() {
    let js = asset("app.js");
    assert!(js.len() > 1000, "app.js suspiciously small");
}

#[test]
fn web_style_css_exists() {
    let css = asset("style.css");
    assert!(css.len() > 100, "style.css suspiciously small");
}

#[test]
fn web_favicon_exists() {
    assert!(TestAssets::get("favicon.ico").is_some(), "favicon.ico missing");
}

// ═══════════════════════════════════════════════════════════════════════
// 2. HTML REFERENCES VALID ASSETS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn web_html_links_existing_css() {
    let html = asset("index.html");
    // The HTML must reference style.css
    assert!(
        html.contains("href=\"style.css\""),
        "index.html must link style.css"
    );
    // And the CSS must exist
    assert!(TestAssets::get("style.css").is_some());
}

#[test]
fn web_html_links_existing_js() {
    let html = asset("index.html");
    assert!(
        html.contains("src=\"app.js\""),
        "index.html must include app.js"
    );
    assert!(TestAssets::get("app.js").is_some());
}

#[test]
fn web_html_links_existing_favicons() {
    let html = asset("index.html");
    // Extract all favicon file references
    let hrefs = regex_lite(&html, r#"href="([^"]*\.(ico|png))""#);
    for href in &hrefs {
        // Strip path prefix if any (our assets are flat in static/admin/)
        let filename = href.rsplit('/').next().unwrap_or(href);
        assert!(
            TestAssets::get(filename).is_some(),
            "favicon referenced in HTML but not embedded: {}",
            href
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. JS getElementById REFERENCES MATCH HTML IDs
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn web_js_get_element_by_id_refs_exist_in_html() {
    // This is the critical test that would have caught the crash bug where
    // JS called getElementById('cursorPos'), getElementById('resizeRows'),
    // getElementById('resizeCols') but those IDs were removed from HTML.
    let html = asset("index.html");
    let js = asset("app.js");

    let html_ids = html_ids(&html);

    // Extract all getElementById('...') and getElementById("...") calls
    let js_refs: Vec<String> = regex_lite(&js, r#"getElementById\s*\(\s*['"]([^'"]+)['"]\s*\)"#);

    // IDs that are generated dynamically (appended with panel.id, etc.)
    // should not be checked against static HTML. We skip any ID that
    // contains a variable interpolation pattern.
    // Build a set of dynamically-assigned IDs (created via createElement + .id)
    let dynamic_ids: std::collections::HashSet<String> = regex_lite(&js, r#"\.id\s*=\s*'([^']+)'"#)
        .into_iter()
        .collect();

    // Patterns that indicate a getElementById call is dynamic (not a static ref)
    let dynamic_ctx_patterns: &[&str] = &[
        "'${", "\"${",    // template literal: getElementById('${panel.id}')
        "+ '", "+ \"",    // concatenation: getElementById('keyInput-' + panelId)
    ];

    let mut missing = Vec::new();
    for id_ref in &js_refs {
        // Skip dynamically-constructed IDs (template literals, concatenation)
        let is_dynamic = dynamic_ctx_patterns.iter().any(|p| {
            if let Some(pos) = js.find(id_ref.as_str()) {
                let start = pos.saturating_sub(20);
                let end = (pos + id_ref.len() + 20).min(js.len());
                let window = &js[start..end];
                window.contains(p)
            } else {
                false
            }
        });
        if is_dynamic {
            continue;
        }
        // Skip IDs that are created dynamically via createElement + .id assignment
        if dynamic_ids.contains(id_ref) {
            continue;
        }

        if !html_ids.contains(id_ref) {
            missing.push(id_ref.clone());
        }
    }

    if !missing.is_empty() {
        panic!(
            "JavaScript references {} HTML element ID(s) that do not exist in index.html:\n  {}\n\
             This will cause a runtime crash when the JS tries to access these elements.",
            missing.len(),
            missing.iter().map(|s| format!("getElementById('{}')", s)).collect::<Vec<_>>().join("\n  "),
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. HTML STRUCTURAL INTEGRITY
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn web_html_has_doctype() {
    let html = asset("index.html");
    assert!(
        html.trim_start().starts_with("<!DOCTYPE html>"),
        "index.html must start with <!DOCTYPE html>"
    );
}

#[test]
fn web_html_has_closing_tags() {
    let html = asset("index.html");
    // Basic structural sanity: </html>, </body>, </head> must be present
    assert!(html.contains("</html>"), "missing </html>");
    assert!(html.contains("</body>"), "missing </body>");
    assert!(html.contains("</head>"), "missing </head>");
}

#[test]
fn web_html_title_is_set() {
    let html = asset("index.html");
    assert!(
        html.contains("<title>") && html.contains("</title>"),
        "index.html must have a <title> element"
    );
}

#[test]
fn web_topbar_has_theme_toggle() {
    // Regression: the theme toggle button was documented but missing from HTML.
    let html = asset("index.html");
    assert!(
        html.contains("themeToggle"),
        "index.html must have a theme toggle button with id='themeToggle'"
    );
}

#[test]
fn web_bottombar_has_cursor_and_resize() {
    // Regression: cursorPos, resizeRows, resizeCols were removed from HTML
    // but JS still referenced them, causing crashes.
    let html = asset("index.html");
    let ids = html_ids(&html);
    assert!(
        ids.contains("cursorPos"),
        "index.html must have an element with id='cursorPos'"
    );
    assert!(
        ids.contains("resizeRows"),
        "index.html must have an element with id='resizeRows'"
    );
    assert!(
        ids.contains("resizeCols"),
        "index.html must have an element with id='resizeCols'"
    );
    assert!(
        ids.contains("termDims"),
        "index.html must have an element with id='termDims'"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 5. CSS THEME VARIABLES
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn web_css_has_dark_theme_variables() {
    let css = asset("style.css");
    assert!(css.contains(":root"), "CSS must define :root theme variables");
    assert!(css.contains("--bg-primary"), ":root must define --bg-primary");
    assert!(css.contains("--text-primary"), ":root must define --text-primary");
    assert!(css.contains("--accent"), ":root must define --accent");
}

#[test]
fn web_css_has_light_theme_variables() {
    let css = asset("style.css");
    assert!(
        css.contains("[data-theme=\"light\"]"),
        "CSS must define [data-theme=\"light\"] override variables"
    );
}

#[test]
fn web_css_has_button_size_variants() {
    let css = asset("style.css");
    // All button size variants must exist
    for variant in &[".btn-xs", ".btn-sm", ".btn-xxs"] {
        assert!(css.contains(variant), "CSS must define {} button class", variant);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. JS FUNCTION DEFINITIONS
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn web_js_has_core_functions() {
    let js = asset("app.js");
    // Every onclick handler referenced in HTML must be defined in JS
    let html = asset("index.html");
    let handlers = [
        "toggleSidebar",
        "addPanel",
        "togglePauseRun",
        "killAllCommands",
        "changeFontSize",
        "switchBuffer",
        "toggleTheme",
        "showShortcuts",
        "loadCommands",
        "spawnCommand",
        "saveToken",
        "toggleBottombar",
        "switchViewTab",
        "searchLogs",
        "clearLogSearch",
        "loadLog",
        "closePanelModal",
        "confirmAddPanel",
        "switchUpdateMode",
        "applyPollInterval",
    ];
    for handler in &handlers {
        // Check that the HTML references it (at least one does)
        let html_uses = html.contains(handler);
        // Check that JS defines it as a function
        let js_defines = js.contains(&format!("function {}(", handler)) || js.contains(&format!("function {} (", handler));
        if html_uses {
            assert!(
                js_defines,
                "HTML calls {}() but app.js does not define it",
                handler
            );
        }
    }
}

#[test]
fn web_js_has_direct_keyboard_input() {
    // Regression: direct keyboard input feature was added then reverted.
    let js = asset("app.js");
    assert!(
        js.contains("sendDirectKey"),
        "app.js must define sendDirectKey() for direct keyboard input"
    );
    assert!(
        js.contains("autoFocusTerminal"),
        "app.js must define autoFocusTerminal() to auto-focus terminal on command select"
    );
}

#[test]
fn web_js_has_send_keys_function() {
    // The send-keys input bar must still work alongside direct keyboard input.
    let js = asset("app.js");
    assert!(
        js.contains("sendKeysToPanel"),
        "app.js must define sendKeysToPanel() for the send-keys input bar"
    );
}
