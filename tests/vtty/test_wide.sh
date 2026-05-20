#!/bin/bash
for i in $(seq 1 40); do
    printf "R%02d " "$i"
    remaining=$((180 - 5))
    head -c "$remaining" < /dev/zero | tr '\0' 'X'
    printf '\r\n'
done
