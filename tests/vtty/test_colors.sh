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
