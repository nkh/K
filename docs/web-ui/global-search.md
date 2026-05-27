# Global Search

The global search overlay allows you to search for text across all command output buffers simultaneously. This is useful for finding specific output, error messages, or patterns across multiple running commands.

![Global search](screenshots/14-global-search.png)

## Opening Global Search

- Click the **🔍** button in the top bar
- Or press **Ctrl+Shift+F**

## Search Input
Type your search query and press **Enter** or click the **Search** button. The search is case-sensitive and matches against the plain text content of all command terminal buffers.

## Results

Search results are grouped by command. Each group shows:

- **Command header**: The command name and a preview line. Click to select that command and jump to its terminal view.
- **Result lines**: Individual lines from the terminal buffer that contain the match. Matching text is highlighted. Hover over a result line to highlight it further.

## Closing

Click the **✕** button or press **Escape** to close the search overlay.
