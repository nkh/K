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
