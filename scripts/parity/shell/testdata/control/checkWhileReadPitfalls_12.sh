#!/bin/bash
while read foo
do
mplayer foo.ogv << EOF
q
EOF
done
