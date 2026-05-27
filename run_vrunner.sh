#!/bin/bash
cd /home/z/my-project
./target/release/vrunner --port 19876 --bind 127.0.0.1 -- sleep 99999 &
echo $! > /tmp/vrunner_pid.txt
sleep 2
echo "Server started with PID $(cat /tmp/vrunner_pid.txt)"
curl -s http://127.0.0.1:19876/api/info
