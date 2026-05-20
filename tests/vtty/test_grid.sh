#!/bin/bash
# Fill each line with a marker showing its line number, padded to fill cols
for i in $(seq 1 24); do
    printf "R%02d " "$i"
    # Fill rest of line with dots
    remaining=$((80 - 5))
    printf '%0.s.' $(seq 1 $remaining 2>/dev/null) || printf '%*s' "$remaining" '' | tr ' ' '.'
    printf '\r\n'
done
