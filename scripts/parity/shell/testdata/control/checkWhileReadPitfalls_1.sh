#!/bin/bash
while read foo; do ssh $foo uptime; done < file
