#!/bin/bash
while read foo; do ssh -n $foo uptime; done < file
