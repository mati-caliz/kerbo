#!/bin/bash
a=$@
a="${arr[@]}"
a=*.png
a={1..10}
a='*.gif'
a=$*
a=( $@ )
echo "foo$@"
echo ${arr[@]}lol
echo $a$@
echo $@
echo "${arr[@]}"
echo $[1+2]
echo $((1+2))
for f in $*; do ..; done
ls ${#arr[*]}
a=$*
[[ $* = 'a b' ]]
for f in ${var[*]}; do ..; done
ls ${*//foo/bar}
ls ${var[*]%%.*}
ls ${*}
ls ${#*}
ls ${arr[*]}
for f in $(cat foo); do stuff; done
for f in `cat foo`; do stuff; done
for f in $(cat foo | grep lol); do stuff; done
for f in `cat foo | grep lol`; do stuff; done
for f in $(cat foo | grep bar | wc -l); do stuff; done
echo 'var='value';'
cmd ${x+--name="$x" --output="$x.out"}
echo "foo"/"bar"
declare "foo"="bar"
'foo'*
wget --user-agent='something'
echo "VALUES ("id")"
"$dir"/"$file"
"$dir"some_stuff"$file"
${dir/"foo"/"bar"}
  'foo'\
  'bar'
[[ $x =~ "foo"("bar"|"baz") ]]
foo | grep -o bar | wc
foo | grep bar | wc
foo | grep -o bar | wc -c
foo | grep bar | wc -c
foo | grep -o bar | wc -cmwL
foo | grep bar | wc -cmwL
foo | grep -r bar | wc -l
foo | grep -l bar | wc -l
foo | grep -L bar | wc -l
foo | grep -A2 bar | wc -l
foo | grep -B999 bar | wc -l
foo | grep --after-context 999 bar | wc -l
foo | grep -B 1 --after-context 999 bar | wc -l
ps -o pid,args -p $(pgrep java) | grep -F net.shellcheck.Test
ls | grep -v mp3
find . -print0 | xargs -0 foo
ls -N | foo
find . | xargs foo
find . -printf '%s\n' | xargs foo
foo | grep bar | wc -l
foo | grep -o bar | wc -l
if $(true); then true; fi
$(cmd) --flag1 --flag2
$((i++))
var="~/out.txt"
foo > '~/dir'
~/file
echo '/~foo/cow'
awk '$0 ~ /foo/'
foo() { echo $1; }; foo
foo() { echo $!; }; foo;
foo() { bar() { echo $1; }; bar baz; }; foo;
foo() { echo ${!var*}; }; foo;
foo() { echo $1; }
foo

foo() { echo $#; }; foo
foo() { echo ${1-x}; }; foo
foo() { echo ${1:-x}; }; foo
foo() { mycommand ${1+--verbose}; }; foo
foo() { if mycheck; then foo ${1?Missing}; fi; }; foo
foo() { echo $1; };
foo() { echo $lol; }; foo
foo() { echo $0; }; foo
foo() { echo $1; }; foo 'lol'; foo
foo() { set -- *; echo $1; }; foo
foo() { echo $1; }; foo; foo;
foo() { echo $((1)); }; foo;
foo() { echo $(($b)); }; foo;
if [ -n $foo ]; then echo cow; fi
[ -n $cow ]
[[ -n $foo ]] && echo cow
[ -n $cow -o -t 1 ]
[ -n "$@" ]
source "$1"
. "$dir/lib.sh"
. "$(dirname "$0")/lib.sh"
source ~/foo
source "${HOME}/x"
source -- lib.sh
source -p path lib.sh
source
. 
f() {
  . ./lib.sh
}
if true; then source x.sh; fi
source /dev/null
source "$x"
source "$y"
source lib.sh
. ./lib.sh
\. ../common.sh
"source" lib.sh
'.' lib.sh
builtin source lib.sh
