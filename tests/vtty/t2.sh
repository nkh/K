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
