#!/bin/bash
ps -o pid,args -p $(pgrep java) | grep -F net.shellcheck.Test
