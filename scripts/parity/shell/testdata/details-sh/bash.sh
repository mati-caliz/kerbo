#!/bin/bash
PS1='\e[0m'
PS1='\[\e[0m\]'
PS1='\033[0m\x1b'
echo a    b
echo a    >/dev/null b
printf "%s %s" a
printf "%(%F)T %d" 1
printf "%-5.2f %5s" 1 2
printf "%*d" 3 4
printf "%llu" 2
printf "%" x
printf "a%%b" x
printf -v x "%s" y
printf -- "%s" y z
printf "%(" y
b=([1][2]=3)
eval echo {1..$n}
echo {1..$n}
declare x=1 y=$x
local z=1 w=$((z + 1))
let a= 1
alias c =d
