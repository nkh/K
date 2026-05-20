#!/bin/bash
echo "Line 1: Hello World, this is a test"
echo "Line 2: 1234567890"
echo "Line 3: $(printf '%0.s=' $(seq 1 120))"
echo "Line 4: END"
