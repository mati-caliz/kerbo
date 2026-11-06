#!/bin/bash
for i in ${arr[@]}; do echo ${arr[i]}; done
for i in ${arr[@]}; do echo ${arr[$i]}; done
for i in ${arr[@]}; do echo $((arr[i])); done
for i in ${arr1[@]} ${arr2[@]}; do echo ${arr1[$i]}; done
for i in ${arr1[@]} ${arr2[@]}; do echo ${arr2[$i]}; done
for i in ${arr[@]}; do echo ${arr[K]}; done
for i in ${arr[@]}; do i=42; echo ${arr[i]}; done
for i in ${arr[@]}; do echo ${arr2[i]}; done
true | ! true
true | ( ! true )
! ! true | true
echo {1..$n}
echo {1,3,$n}
eval echo DSC{0001..$n}.jpg
echo {$i..100}
foo[a]=3
foo=( [a][b]=c )
foo=( [a]=c )
echo ${foo[bar]}
! ! true
! true
[ ! -o braceexpand ]
[ -o braceexpand ]
[[ ! -o braceexpand ]]
! [ -o braceexpand ]
[ ! -a file ]
printf "Lol: $s"
printf '%s %s %s' foo bar baz
printf '%(%s%s)T' -1
printf '%s %s\n' 1 2 3
printf '%s %s\n' 1 2 3 4
printf '%*s\n' 1
printf '%*s\n' 1 2
printf $'string'
printf '%-*s\n' 1
printf '%-*s\n' 1 2
printf '%(%s)T'
printf 'Lol: $s'
printf '%d %(%s)T' 42
printf '%d %(%s)T'
printf '%s
%s' foo
printf -v cow $(cmd)
printf "%${count}s" var
printf '%s %s %s' foo bar
printf foo bar baz
printf -- foo bar baz
printf '%s %s %s' "${var[@]}"
printf '%s %s %s\n' *.png
time -p foo | bar
time ( foo; bar; )
time sleep 1
echo foo         bar
echo       foo
echo foo  bar
echo 'foo          bar'
echo a > myfile.txt b
        echo foo\
        bar
while getopts 'a:b' x; do case $x in a) foo;; esac; done
while getopts 'a:' x; do case $x in a) foo;; b) bar;; esac; done
while getopts 'a:b' x; do case $x in a) foo;; b) bar;; *) :;esac; done
while getopts 'a:123' x; do case $x in a) foo;; [0-9]) bar;; esac; done
while getopts 'a:' x; do case $x in a) foo;; \?) bar;; *) baz;; esac; done
while getopts 'a:b' x; do case $y in a) foo;; esac; done
while getopts 'a:b' x; do case x$x in xa) foo;; xb) foo;; esac; done
while getopts 'a:b' x; do x=a; case $x in a) foo;; esac; done
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
