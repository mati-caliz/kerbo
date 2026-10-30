#!/bin/busybox sh
f() { local a; echo -ne x; echo -E y; [[ a == b ]]; source x; echo $'a' ${x:0:1} ${x/a/b}; cmd &> /dev/null; trap x SIGINT; }
f
