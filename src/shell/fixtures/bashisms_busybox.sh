#!/bin/busybox sh
f() { local a; echo -ne x; echo -E y; [[ a == b ]]; source x; echo $'a' ${x:0:1} ${x/a/b}; cmd &> /dev/null; trap x SIGINT; }
f
x='test'
${!x}
x='test'
${x[1]}
x='test'
${!x[@]}
x='test'
${x^^[t]}
type -p test
read -p foo -r bar
