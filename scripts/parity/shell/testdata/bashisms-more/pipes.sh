#!/bin/sh
a |& b
coproc foo
select x in a b; do echo $x; done
for ((i=0;i<3;i++)); do :; done
((x++))
echo $[1+2]
echo $((2**3)) $((x--)) $((++x))
