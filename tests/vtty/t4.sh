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
