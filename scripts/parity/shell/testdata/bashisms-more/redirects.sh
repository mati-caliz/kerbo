#!/bin/sh
cmd &>/dev/null
cmd &>>log
cmd >&log
cmd 2>&1
cmd 10>file
cmd {fd}>file
cat < /dev/udp/h/1
cmd >& 2
