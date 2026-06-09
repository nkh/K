//! Interactive selection UI for CLI commands.
//!
//! When `--interactive` is passed to commands like list, freeze, thaw, stop,
//! stop-command, resize, cat, and screenshot, the user is presented with a
//! numbered list of applicable items and can select one or more by typing
//! space-separated numbers.

use std::io::{self, Write};

/// An item that can be presented in the interactive selection list.
#[derive(Debug, Clone)]
pub struct SelectItem {
    /// Display label shown in the list (e.g., "htop (PID 5678)")
    pub label: String,
    /// Internal identifier (command ID, instance PID, etc.)
    pub id: String,
}

/// Present a numbered list of items and let the user select one or more.
///
/// Prints each item with a number prefix, then reads a line of input.
/// The user types space-separated numbers (e.g., "1 3 5") to select items.
/// Pressing Enter without any selection aborts with an error.
///
/// Returns the selected items in order. Returns an error if:
/// - stdin is not a TTY (interactive mode requires a terminal)
/// - the list is empty
/// - the user provides invalid numbers
/// - the user enters nothing
pub fn select_items(items: &[SelectItem], prompt: &str) -> anyhow::Result<Vec<SelectItem>> {
    if items.is_empty() {
        anyhow::bail!("No items to select from.");
    }

    if !atty::is(atty::Stream::Stdin) {
        anyhow::bail!(
            "Interactive mode requires a terminal (stdin is not a TTY). \
             Provide explicit arguments instead."
        );
    }

    // Print numbered list
    eprintln!();
    for (i, item) in items.iter().enumerate() {
        eprintln!("  {}) {}", i + 1, item.label);
    }
    eprintln!();

    // Read user selection
    eprint!("{}: ", prompt);
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim();
    if input.is_empty() {
        anyhow::bail!("No selection made. Aborting.");
    }

    // Parse space-separated numbers
    let mut selected_indices: Vec<usize> = Vec::new();
    for token in input.split_whitespace() {
        match token.parse::<usize>() {
            Ok(n) => {
                if n == 0 || n > items.len() {
                    anyhow::bail!(
                        "Invalid selection '{}'. Valid range: 1-{}.",
                        token,
                        items.len()
                    );
                }
                selected_indices.push(n - 1); // Convert to 0-based
            }
            Err(_) => {
                anyhow::bail!(
                    "Invalid selection '{}'. Enter space-separated numbers (e.g., '1 3 5').",
                    token
                );
            }
        }
    }

    if selected_indices.is_empty() {
        anyhow::bail!("No valid selections. Aborting.");
    }

    Ok(selected_indices
        .into_iter()
        .map(|i| items[i].clone())
        .collect())
}

/// Filter items based on a predicate (e.g., only frozen, only running).
pub fn filter_items<F>(items: &[SelectItem], predicate: F) -> Vec<SelectItem>
where
    F: Fn(&SelectItem) -> bool,
{
    items.iter().filter(|i| predicate(i)).cloned().collect()
}

/// Print a list of SelectItems without requiring selection (informational).
pub fn print_items(items: &[SelectItem]) {
    for (i, item) in items.iter().enumerate() {
        eprintln!("  {}) {}", i + 1, item.label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_items() {
        let items = vec![
            SelectItem {
                label: "running".to_string(),
                id: "1".to_string(),
            },
            SelectItem {
                label: "frozen".to_string(),
                id: "2".to_string(),
            },
            SelectItem {
                label: "running2".to_string(),
                id: "3".to_string(),
            },
        ];
        let frozen = filter_items(&items, |i| i.label == "frozen");
        assert_eq!(frozen.len(), 1);
        assert_eq!(frozen[0].id, "2");
    }

    #[test]
    fn test_empty_items() {
        let items: Vec<SelectItem> = vec![];
        let result = select_items(&items, "Select");
        assert!(result.is_err());
    }

    #[test]
    fn test_select_items_non_tty_errors() {
        // In test environment, stdin is not a TTY
        let items = vec![
            SelectItem { label: "item1".to_string(), id: "1".to_string() },
            SelectItem { label: "item2".to_string(), id: "2".to_string() },
        ];
        let result = select_items(&items, "Select");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("stdin is not a TTY") || msg.contains("No selection made"));
    }

    #[test]
    fn test_filter_items_all_match() {
        let items = vec![
            SelectItem { label: "cmd-a".to_string(), id: "1".to_string() },
            SelectItem { label: "cmd-b".to_string(), id: "2".to_string() },
            SelectItem { label: "cmd-c".to_string(), id: "3".to_string() },
        ];
        let result = filter_items(&items, |_| true);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_items_none_match() {
        let items = vec![
            SelectItem { label: "cmd-a".to_string(), id: "1".to_string() },
        ];
        let result = filter_items(&items, |_| false);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_items_by_id() {
        let items = vec![
            SelectItem { label: "first".to_string(), id: "aaa".to_string() },
            SelectItem { label: "second".to_string(), id: "bbb".to_string() },
        ];
        let result = filter_items(&items, |i| i.id == "aaa");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "aaa");
    }

    #[test]
    fn test_print_items_does_not_panic() {
        let items = vec![
            SelectItem { label: "test-item".to_string(), id: "id-1".to_string() },
        ];
        // Should not panic
        print_items(&items);
    }

    #[test]
    fn test_print_items_empty_does_not_panic() {
        let items: Vec<SelectItem> = vec![];
        print_items(&items);
    }

    #[test]
    fn test_select_item_clone() {
        let item = SelectItem { label: "test".to_string(), id: "123".to_string() };
        let cloned = item.clone();
        assert_eq!(cloned.label, item.label);
        assert_eq!(cloned.id, item.id);
    }

    #[test]
    fn test_select_item_debug() {
        let item = SelectItem { label: "test".to_string(), id: "123".to_string() };
        let debug = format!("{:?}", item);
        assert!(debug.contains("test"));
        assert!(debug.contains("123"));
    }
}
