#!/bin/ksh
cat foo | while read bar; do a=$bar; done
echo "$a"
