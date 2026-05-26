#!/usr/bin/env bash
# Focused VTTY diagnostic: capture buffer dimensions and content,
# compare with expected output.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VRUNNER_BIN="${VRUNNER_BIN:-./target/release/vrunner}"
PORT=19991
RESULTS=""

run_test() {
    local name="$1" cols="$2" rows="$3" script="$4"

    # Start vrunner
    "$VRUNNER_BIN" --port "$PORT" --vtty-cols "$cols" --vtty-rows "$rows" 2>/dev/null &
    local vr_pid=$!
    sleep 1

    # Spawn command
    local result
    result=$(curl -sf -X POST "http://127.0.0.1:$PORT/api/commands" \
        -H "Content-Type: application/json" \
        -d "{\"cmd\": \"bash\", \"args\": [\"$script\"]}" 2>/dev/null) || {
        echo "SKIP: $name (vrunner not reachable)"
        kill $vr_pid 2>/dev/null; wait $vr_pid 2>/dev/null
        return
    }

    local cmd_id
    cmd_id=$(echo "$result" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['id'])")

    # Wait for command to finish
    for i in $(seq 1 20); do
        alive=$(curl -sf "http://127.0.0.1:$PORT/api/commands" 2>/dev/null \
            | python3 -c "
import sys, json, os
data = json.load(sys.stdin)
for c in data['data']:
    if c['id'] == '$cmd_id':
        try:
            os.kill(c['pid'], 0)
            print('alive')
        except ProcessLookupError:
            print('dead')
        break
" 2>/dev/null || echo "dead")
        [ "$alive" = "dead" ] && break
        sleep 0.3
    done
    sleep 0.3

    # Fetch VTTY HTML endpoint (has dimensions info)
    local info
    info=$(curl -sf "http://127.0.0.1:$PORT/api/commands/$cmd_id/vtty/html" 2>/dev/null)

    # Fetch plain VTTY content
    local content
    content=$(curl -sf "http://127.0.0.1:$PORT/api/commands/$cmd_id/vtty" 2>/dev/null)

    # Shutdown
    curl -sf -X POST "http://127.0.0.1:$PORT/api/shutdown" >/dev/null 2>&1
    sleep 0.5
    kill $vr_pid 2>/dev/null; wait $vr_pid 2>/dev/null

    # Parse dimensions from HTML endpoint
    local emu_rows emu_cols
    emu_rows=$(echo "$info" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']['dimensions']; print(d['rows'])" 2>/dev/null || echo "?")
    emu_cols=$(echo "$info" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']['dimensions']; print(d['cols'])" 2>/dev/null || echo "?")

    # Parse content - get line count and max line width
    local line_count max_width
    eval $(echo "$content" | python3 -c "
import sys, json, re
data = json.load(sys.stdin)
raw = data['data']['content']
# Strip all ANSI escapes
plain = re.sub(r'\x1b[^a-zA-Z]*[a-zA-Z]', '', raw)
plain = re.sub(r'\x1b', '', plain)
lines = plain.split('\n')
lines = [l for l in lines]  # keep empty lines
lc = len(lines)
mw = max((len(l) for l in lines), default=0)
print(f'line_count={lc}')
print(f'max_width={mw}')
# Show first 3 line lengths
for i, l in enumerate(lines[:3]):
    print(f'line_{i}_len={len(l)}')
# Show last 3 line lengths  
for i in range(max(0, len(lines)-3), len(lines)):
    print(f'line_{i}_len={len(lines[i])}')
" 2>/dev/null || echo "line_count=0 max_width=0")

    echo "=== $name (requested ${cols}x${rows}) ==="
    echo "  Emulator dimensions: ${emu_rows}x${emu_cols}"
    echo "  Content lines: $line_count"
    echo "  Max line width: $max_width"

    local pass=true
    if [ "$emu_cols" != "$cols" ] || [ "$emu_rows" != "$rows" ]; then
        echo "  FAIL: emulator dimensions mismatch (got ${emu_rows}x${emu_cols}, expected ${rows}x${cols})"
        pass=false
    fi
    # Allow +1 for trailing newline
    if [ "$line_count" -lt "$rows" ] || [ "$line_count" -gt $((rows + 2)) ]; then
        echo "  WARN: line count $line_count outside expected range [$rows, $((rows+2))]"
    fi
    if [ "$max_width" -lt "$cols" ]; then
        echo "  FAIL: max line width $max_width < requested cols $cols"
        pass=false
    fi
    if $pass; then
        echo "  PASS"
    fi
    echo ""
}

# Test 1: 80x24 simple fill
cat > "$SCRIPT_DIR/t1.sh" << 'EOF'
#!/bin/bash
COLS=$(tput cols)
for i in $(seq 1 23); do
    printf "R%02d" "$i"
    head -c "$((COLS - 4))" < /dev/zero | tr '\0' '=' 2>/dev/null || printf '%*s' "$((COLS - 4))" '' | tr ' ' '='
    printf '\r\n'
done
printf "END "
head -c "$((COLS - 4))" < /dev/zero | tr '\0' '.' 2>/dev/null || printf '%*s' "$((COLS - 4))" '' | tr ' ' '.'
printf '\n'
EOF

# Test 2: 180x40 wide fill
cat > "$SCRIPT_DIR/t2.sh" << 'EOF'
#!/bin/bash
COLS=$(tput cols)
for i in $(seq 1 39); do
    printf "R%02d" "$i"
    head -c "$((COLS - 4))" < /dev/zero | tr '\0' '#' 2>/dev/null || printf '%*s' "$((COLS - 4))" '' | tr ' ' '#'
    printf '\r\n'
done
printf "END"
head -c "$((COLS - 3))" < /dev/zero | tr '\0' '.' 2>/dev/null || printf '%*s' "$((COLS - 3))" '' | tr ' ' '.'
printf '\n'
EOF

# Test 3: ncurses-style positioning (simulates htop pattern)
cat > "$SCRIPT_DIR/t3.sh" << 'EOF'
#!/bin/bash
COLS=$(tput cols)
LINES=$(tput lines)
# Clear screen
printf '\033[2J'
# Draw header at row 1
printf '\033[1;1H'
printf '%*s' "$COLS" '' | tr ' ' '='
# Draw footer at last row
printf "\033[${LINES};1H"
printf '%*s' "$COLS" '' | tr ' ' '-'
# Fill middle rows
for i in $(seq 2 $((LINES - 1))); do
    printf "\033[${i};1H"
    printf "| Row %02d %$((COLS - 12))s |" "$i" ""
done
# Put cursor at row 2
printf '\033[2;1H'
EOF

# Test 4: CSI cursor movement test
cat > "$SCRIPT_DIR/t4.sh" << 'EOF'
#!/bin/bash
COLS=$(tput cols)
LINES=$(tput lines)
printf '\033[2J\033[H'  # clear + home
# Write at specific positions
printf '\033[1;1HA'        # top-left
printf "\033[${LINES};${COLS}HZ"  # bottom-right
printf '\033[1;'"$COLS"'HB'  # top-right
printf "\033[${LINES};1HC"  # bottom-left
printf '\033[2;2HD'        # row 2, col 2
printf '\033[3;3HE'        # row 3, col 3
EOF

echo "============================================"
echo " VTTY Diagnostic Suite"
echo "============================================"
echo ""

run_test "80x24_fill" 80 24 "$SCRIPT_DIR/t1.sh"
run_test "180x40_fill" 180 40 "$SCRIPT_DIR/t2.sh"
run_test "120x30_csi" 120 30 "$SCRIPT_DIR/t3.sh"
run_test "80x24_csi_pos" 80 24 "$SCRIPT_DIR/t4.sh"
run_test "40x15_small" 40 15 "$SCRIPT_DIR/t1.sh"
run_test "200x50_large" 200 50 "$SCRIPT_DIR/t2.sh"

echo "Done."
