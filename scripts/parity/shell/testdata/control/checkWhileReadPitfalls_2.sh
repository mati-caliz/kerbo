#!/bin/bash
while read -u 3 foo; do ssh $foo uptime; done 3< file
