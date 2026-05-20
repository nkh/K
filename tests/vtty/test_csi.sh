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
