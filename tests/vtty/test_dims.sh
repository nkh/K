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
