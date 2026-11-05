#!/bin/sh
echo "$a" | sed s/a/b/
[ -d foo* ]
IFS="
"
for f in *; do cd "$f"; x; cd ..; done
