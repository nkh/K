#!/usr/bin/env bash
# VTTY buffer fidelity test suite
# Runs test scripts, captures their raw terminal output to reference files,
# then runs the same commands inside vrw and captures the VTTY buffer
# for comparison.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REF_DIR="$SCRIPT_DIR/reference"
VRW_DIR="$SCRIPT_DIR/vrw_output"
VRW_BIN="${VRW_BIN:-./target/release/vrw}"
PORT=19990
PASS=0
FAIL=0
SKIP=0

mkdir -p "$REF_DIR" "$VRW_DIR"

# We need to run tests inside a real PTY to get proper ANSI output.
# Use `script` to capture terminal output with escape sequences.
capture_ref() {
    local name="$1" cols="$2" rows="$3" script="$4"
    local ref_file="$REF_DIR/${name}.raw"
    # Use script with a pipe to force unbuffered PTY output
    # -q: quiet, -c: command, -e: return exit code
    script -qec "stty cols $cols rows $rows; bash '$script'" /dev/null 2>/dev/null \
        | head -c $((cols * rows * 200)) > "$ref_file" || true
    echo "$ref_file"
}

run_in_vrw() {
    local name="$1" cols="$2" rows="$3" script="$4"
    local out_file="$VRW_DIR/${name}.plain"
    local out_raw="$VRW_DIR/${name}.raw"

    # Start vrw with the specified dimensions
    "$VRW_BIN" --port "$PORT" --vtty-cols "$cols" --vtty-rows "$rows" &
    local vr_pid=$!
    sleep 1

    # Spawn the test command via API
    local result
    result=$(curl -sf -X POST "http://127.0.0.1:$PORT/api/commands" \
        -H "Content-Type: application/json" \
        -d "{\"cmd\": \"bash\", \"args\": [\"$script\"]}" 2>/dev/null) || {
        echo "FAIL: could not reach vrw on port $PORT"
        kill $vr_pid 2>/dev/null; wait $vr_pid 2>/dev/null
        return 1
    }

    local cmd_id
    cmd_id=$(echo "$result" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['id'])")

    # Wait for the command to finish (check process liveness)
    local tries=0
    while [ $tries -lt 30 ]; do
        local info
        info=$(curl -sf "http://127.0.0.1:$PORT/api/commands" 2>/dev/null)
        if echo "$info" | python3 -c "
import sys, json
data = json.load(sys.stdin)
ids = [c['id'] for c in data['data']]
sys.exit(0 if '$cmd_id' in ids else 1)
" 2>/dev/null; then
            # Check if process is still alive via pid
            local alive
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
" 2>/dev/null)
            if [ "$alive" = "dead" ]; then
                break
            fi
        else
            break
        fi
        sleep 0.5
        tries=$((tries + 1))
    done

    sleep 0.5

    # Capture VTTY plain text output
    curl -sf "http://127.0.0.1:$PORT/api/commands/$cmd_id/vtty" 2>/dev/null \
        | python3 -c "
import sys, json
data = json.load(sys.stdin)
content = data['data']['content']
# Strip ANSI escapes for plain comparison
import re
plain = re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', content)
plain = re.sub(r'\x1b[()][A-Za-z0-9]', '', plain)
plain = re.sub(r'\x1b[\[\]?][^\x07]*?(\x07|\x1b\\)', '', plain)
# Remove trailing whitespace per line
lines = plain.split('\n')
lines = [l.rstrip() for l in lines]
print('\n'.join(lines))
" > "$out_file"

    # Also capture the raw content with metadata for debugging
    curl -sf "http://127.0.0.1:$PORT/api/commands/$cmd_id/vtty" 2>/dev/null \
        | python3 -c "
import sys, json
data = json.load(sys.stdin)
content = data['data']['content']
print(f'CONTENT_LENGTH={len(content)}')
print(f'FIRST_200={repr(content[:200])}')
print(f'LAST_200={repr(content[-200:])}')
# Count actual newlines
lines = content.split('\n')
print(f'LINE_COUNT={len(lines)}')
# Report max line length (stripping ANSI)
import re
plain = re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', content)
plain = re.sub(r'\x1b[()][A-Za-z0-9]', '', plain)
plain_lines = plain.split('\n')
max_len = max(len(l) for l in plain_lines) if plain_lines else 0
min_len = min(len(l) for l in plain_lines if l.strip()) if any(l.strip() for l in plain_lines) else 0
print(f'MAX_LINE_LEN={max_len}')
print(f'MIN_LINE_LEN={min_len}')
# Show each line length
for i, l in enumerate(plain_lines[:10]):
    print(f'LINE_{i}_LEN={len(l)}')
" > "$out_raw"

    # Shutdown vrw
    curl -sf -X POST "http://127.0.0.1:$PORT/api/shutdown" >/dev/null 2>&1
    sleep 0.5
    kill $vr_pid 2>/dev/null; wait $vr_pid 2>/dev/null

    echo "$out_file"
}

run_test() {
    local name="$1" cols="$2" rows="$3" script="$4" desc="$5"
    echo ""
    echo "=========================================="
    echo "TEST: $name ($desc)"
    echo "  Dimensions: ${cols}x${rows}"
    echo "=========================================="

    # Run in vrw
    local vr_out
    vr_out=$(run_in_vrw "$name" "$cols" "$rows" "$script")

    if [ ! -f "$vr_out" ]; then
        echo "  FAIL: vrw output file not found"
        FAIL=$((FAIL + 1))
        return
    fi

    # Show diagnostics
    local diag_file="$VRW_DIR/${name}.raw"
    if [ -f "$diag_file" ]; then
        echo "  --- Buffer diagnostics ---"
        cat "$diag_file"
        echo ""
    fi

    # Check: buffer line count should match requested rows
    local vr_linecount
    vr_linecount=$(wc -l < "$vr_out" | tr -d ' ')
    echo "  Buffer lines: $vr_linecount (expected: $rows)"

    # Check: content line widths
    echo "  --- First 5 lines (showing width) ---"
    head -5 "$vr_out" | while IFS= read -r line; do
        printf "  [%3d chars] %s\n" "${#line}" "${line:0:80}"
    done

    # Check: buffer should be at least 'cols' characters wide on non-empty lines
    local max_width=0
    while IFS= read -r line; do
        if [ "${#line}" -gt "$max_width" ]; then
            max_width="${#line}"
        fi
    done < "$vr_out"
    echo "  Max line width: $max_width (expected: $cols)"

    # Verdict
    if [ "$vr_linecount" -ge "$rows" ] && [ "$max_width" -ge "$cols" ]; then
        echo "  PASS"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: buffer dimensions don't match requested ${cols}x${rows}"
        echo "        Got ${max_width}x${vr_linecount}"
        FAIL=$((FAIL + 1))
    fi
}

echo "============================================="
echo " VTTY Buffer Dimension Fidelity Test Suite"
echo " Binary: $VRW_BIN"
echo "============================================="

# Test 1: Simple grid pattern at 80x24
cat > "$SCRIPT_DIR/test_grid.sh" << 'TESTEOF'
#!/bin/bash
# Fill each line with a marker showing its line number, padded to fill cols
for i in $(seq 1 24); do
    printf "R%02d " "$i"
    # Fill rest of line with dots
    remaining=$((80 - 5))
    printf '%0.s.' $(seq 1 $remaining 2>/dev/null) || printf '%*s' "$remaining" '' | tr ' ' '.'
    printf '\r\n'
done
TESTEOF
chmod +x "$SCRIPT_DIR/test_grid.sh"

run_test "grid_80x24" 80 24 "$SCRIPT_DIR/test_grid.sh" "80x24 grid pattern"

# Test 2: Wide grid at 180x40
cat > "$SCRIPT_DIR/test_wide.sh" << 'TESTEOF'
#!/bin/bash
for i in $(seq 1 40); do
    printf "R%02d " "$i"
    remaining=$((180 - 5))
    head -c "$remaining" < /dev/zero | tr '\0' 'X'
    printf '\r\n'
done
TESTEOF
chmod +x "$SCRIPT_DIR/test_wide.sh"

run_test "wide_180x40" 180 40 "$SCRIPT_DIR/test_wide.sh" "180x40 wide grid"

# Test 3: Simple echo at 120x30
cat > "$SCRIPT_DIR/test_simple.sh" << 'TESTEOF'
#!/bin/bash
echo "Line 1: Hello World, this is a test"
echo "Line 2: 1234567890"
echo "Line 3: $(printf '%0.s=' $(seq 1 120))"
echo "Line 4: END"
TESTEOF
chmod +x "$SCRIPT_DIR/test_simple.sh"

run_test "simple_120x30" 120 30 "$SCRIPT_DIR/test_simple.sh" "120x30 simple text"

# Test 4: Colors at 80x10
cat > "$SCRIPT_DIR/test_colors.sh" << 'TESTEOF'
#!/bin/bash
RED='\033[31m'
GREEN='\033[32m'
YELLOW='\033[33m'
BLUE='\033[34m'
MAGENTA='\033[35m'
CYAN='\033[36m'
RESET='\033[0m'
echo -e "${RED}Red line:    $(printf '%0.s#' $(seq 1 80 2>/dev/null) || printf '%*s' 80 '' | tr ' ' '#')${RESET}"
echo -e "${GREEN}Green line:  $(printf '%0.s#' $(seq 1 80 2>/dev/null) || printf '%*s' 80 '' | tr ' ' '#')${RESET}"
echo -e "${YELLOW}Yellow line: $(printf '%0.s#' $(seq 1 80 2>/dev/null) || printf '%*s' 80 '' | tr ' ' '#')${RESET}"
echo -e "${BLUE}Blue line:   $(printf '%0.s#' $(seq 1 80 2>/dev/null) || printf '%*s' 80 '' | tr ' ' '#')${RESET}"
echo -e "${MAGENTA}Magenta line:$(printf '%0.s#' $(seq 1 80 2>/dev/null) || printf '%*s' 80 '' | tr ' ' '#')${RESET}"
echo -e "${CYAN}Cyan line:   $(printf '%0.s#' $(seq 1 80 2>/dev/null) || printf '%*s' 80 '' | tr ' ' '#')${RESET}"
echo "Plain line:  $(printf '%0.s=' $(seq 1 80 2>/dev/null) || printf '%*s' 80 '' | tr ' ' '=')"
TESTEOF
chmod +x "$SCRIPT_DIR/test_colors.sh"

run_test "colors_80x10" 80 10 "$SCRIPT_DIR/test_colors.sh" "80x10 colored lines"

# Test 5: tput / stty size query
cat > "$SCRIPT_DIR/test_dims.sh" << 'TESTEOF'
#!/bin/bash
echo "=== Terminal Size Query ==="
echo "stty size: $(stty size)"
echo "COLUMNS=$COLUMNS"
echo "LINES=$LINES"
echo "tput cols: $(tput cols)"
echo "tput lines: $(tput lines)"
echo "=== Fill test ==="
COLS=$(tput cols)
for i in $(seq 1 10); do
    printf "Row %02d: " "$i"
    head -c "$COLS" < /dev/zero | tr '\0' '*' 2>/dev/null || printf '%*s' "$COLS" '' | tr ' ' '*'
    printf '\r\n'
done
TESTEOF
chmod +x "$SCRIPT_DIR/test_dims.sh"

run_test "dims_80x24" 80 24 "$SCRIPT_DIR/test_dims.sh" "80x24 dimension query"
run_test "dims_120x30" 120 30 "$SCRIPT_DIR/test_dims.sh" "120x30 dimension query"
run_test "dims_180x40" 180 40 "$SCRIPT_DIR/test_dims.sh" "180x40 dimension query"

# Test 6: ncurses-style positioning (simulate what htop does)
cat > "$SCRIPT_DIR/test_csi.sh" << 'TESTEOF'
#!/bin/bash
# Simulate ncurses-style full redraw using CSI sequences
clear
# Draw a box border
COLS=$(tput cols)
LINES=$(tput lines)

# Top border
printf '\033[1;1H'
printf "+%s+" "$(printf '%0.s-' $(seq 1 $((COLS-2)) 2>/dev/null) || printf '%*s' $((COLS-2)) '' | tr ' ' '-')"

# Content lines
for i in $(seq 2 $((LINES-1))); do
    printf "\033[${i};1H"
    printf "| Row %02d %$((COLS-10))s |" "$i" ""
done

# Bottom border
printf "\033[${LINES};1H"
printf "+%s+" "$(printf '%0.s-' $(seq 1 $((COLS-2)) 2>/dev/null) || printf '%*s' $((COLS-2)) '' | tr ' ' '-')"

# Move cursor to a known position
printf "\033[2;3H"
TESTEOF
chmod +x "$SCRIPT_DIR/test_csi.sh"

run_test "csi_80x24" 80 24 "$SCRIPT_DIR/test_csi.sh" "80x24 CSI positioning"
run_test "csi_120x30" 120 30 "$SCRIPT_DIR/test_csi.sh" "120x30 CSI positioning"

echo ""
echo "============================================="
echo " RESULTS: $PASS passed, $FAIL failed, $SKIP skipped"
echo "============================================="
echo ""
echo "Reference files: $REF_DIR/"
echo "Vrunner output:   $VRW_DIR/"

exit $FAIL
