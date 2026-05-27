# Special Keys Reference

The special keys help modal provides a comprehensive reference for typing non-printable characters and control sequences in the send keys input field.

![Special keys help](screenshots/13-special-keys-help.png)

## Opening the Reference

Click the **?** button next to the send keys input field in any panel header.

## Control Characters

Control characters are typed using `\xNN` notation where `NN` is the hexadecimal ASCII code:

| Key | Code | Notation |
|-----|------|----------|
| Ctrl+A (SOH) | 0x01 | `\x01` |
| Ctrl+B (STX) | 0x02 | `\x02` |
| Ctrl+C (SIGINT) | 0x03 | `\x03` |
| Ctrl+D (EOF) | 0x04 | `\x04` |
| Ctrl+E (ENQ) | 0x05 | `\x05` |
| Ctrl+Z (SIGTSTP) | 0x1a | `\x1a` |
| Ctrl+\ (SIGQUIT) | 0x1c | `\x1c` |

## Common Escape Sequences

| Sequence | Character |
|----------|-----------|
| `\n` | Line feed (newline) |
| `\r` | Carriage return |
| `\t` | Horizontal tab |
| `\b` | Backspace |
| `\e` | Escape (0x1b) |
| `\x00` | NUL |
| `\x7f` | Delete (DEL) |

## Cursor Keys

| Sequence | Key |
|----------|-----|
| `\x1b[A` | Up |
| `\x1b[B` | Down |
| `\x1b[C` | Right |
| `\x1b[D` | Left |
| `\x1b[H` | Home |
| `\x1b[F` | End |
| `\x1b[5~` | Page Up |
| `\x1b[6~` | Page Down |

## Function Keys

| Sequence | Key |
|----------|-----|
| `\x1bOP` | F1 |
| `\x1bOQ` | F2 |
| `\x1bOR` | F3 |
| `\x1bOS` | F4 |
| `\x1b[15~` | F5 |
| `\x1b[17~` | F6 |
| `\x1b[18~` | F7 |
| `\x1b[19~` | F8 |

## Usage Tips

- Multiple special keys can be combined in a single send. For example, `\x1b[A\x1b[A` sends two up-arrow presses.
- Mix text with special keys: `hello\n` sends "hello" followed by a newline.
- The `\e` shorthand is equivalent to `\x1b` (escape character).
