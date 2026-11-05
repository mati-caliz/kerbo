#!/bin/bash
A=ls -l
A=cat foo | grep bar
A=foo ls -l
PAGER=cat grep bar
PAGER="cat" grep bar
here=pwd
x=$x
x=${x}
x="$x"
x=$x mycmd
${foo//$n/lol}
${$#}
${bar}
$# ${#} $! ${!} ${!#} ${#!}
for f in *; do cd $f; git pull; cd ..; done
for f in *; do cd $f || continue; git pull; cd ..; done
while [[ $PWD != / ]]; do cd ..; done
cd $tmp; foo; cd -
cd ..; foo; cd ..
for dir in */; do cd "$dir"; some_cmd; cd ..; done
set -e; for dir in */; do cd "$dir"; some_cmd; cd ..; done
cd tmp
foo
cd ..

[ x$foo = xlol ]
test x$foo = xlol
[ $foo = xbar ]
test $foo = xbar
[ "x$foo" = 'xlol' ]
[ x"$foo" = x'lol' ]
[ X$foo != Xbar ]
[[ foo != bar ]]
[[ '~' == x ]]
[ n -le 4 ]
[[ n -le 4 ]]
[[ $n -le 4 && n != 2 ]]
[[ $n -le 3 ]]
[[ $n -le $n ]]
[[ a -ot b ]]
[ a -nt b ]
[[ ~foo == '~foo' ]]
[[ *.png == [a-z] ]]
[[ '$(foo)' ]]
[ "-f lol" ]
[[ cmd ]]
[[ ! cmd ]]
[[ true ]]
[ 1 ]
[ false ]
FOO=$(echo "$cow" | sed 's/foo/bar/g')
FOO=$(sed 's/foo/bar/g' <<< "$cow")
rm $(echo $cow | sed -e 's,foo,bar,')
rm $(sed -e 's,foo,bar,' <<< $cow)
var=$(foo > bar)
var=`foo 1> bar`
var=$(foo | bar > baz)
stderr=$(foo 2>&1 > /dev/null)
var=$(foo; bar > baz)
var=$(foo > bar; baz)
var=$(cat <&3)
foo[1.2]=bar
declare -A foo; foo[1.2]=bar
for f in $(ls *.mp3); do mplayer "$f"; done
for f in `ls *.mp3`; do mplayer "$f"; done
for f in `find / -name '*.mp3'`; do mplayer "$f"; done
for i in $var/*.txt; do true; done
for i in "$var"/*.txt; do true; done
for i in $var; do true; done
rm *.txt
ls ??.*
rm -- *.txt
*.txt
echo 'Files:' *.txt
printf '%s\n' *
[[ a==$foo ]]
[ -z foo ]
[ $foo=3 ]
[ $foo!=3 ]
[ "$(ls) " ]
[ -n "$(true) " ]
[ -z $(true)z ]
[ -z $(true) ]
[ $(true)$(true) ]
foo >> file; bar >> file; baz >> file;
foo >> file; bar | grep f >> file; baz >> file;
foo < file; bar < file; baz < file;
[[ $foo =~ "bar.*" ]]
[[ $foo =~ '(cow|bar)' ]]
[[ $foo =~ $foo ]]
[[ $foo =~ "bar" ]]
[[ $foo =~ 'cow bar' ]]
[[ $foo =~ 'cow|bar' ]]
cat foo > foo
mapfile -t foo <foo
cat lol | sed -e 's/a/b/g' > lol
cat lol | sed -e 's/a/b/g' > foo.bar && mv foo.bar lol
foo /dev/null > /dev/null
foo > bar 2> bar
echo foo > foo
sed 's/foo/bar/g' file | sponge file
while read -r line; do _="$fname"; done <"$fname"
while read -r line; do cat < "$fname"; done <"$fname"
a && ( [ b ] || ! [ c ] )
( [ a ] )
( [ a ] && [ b ] || test c )
( [ a ] && { [ b ] && [ c ]; } )
( [[ ${var:=x} = y ]] )
( [[ $((i++)) = 10 ]] )
( [[ $((i+=1)) = 10 ]] )
f() ( [[ x ]] )
IFS="\n"
IFS=$'\t'
IFS=' \t\n'
[ -e *.mp3 ]
[ "$@" ]
[[ "$@" ]]
[ *.png ]
[ "$@" == "" ]
[[ "$@" == "" ]]
[[ "$*" == "" ]]
[[ -v foo[123] ]]
[ -e foo* ]
[ -d foo* ]
[[ var[x] -eq 2*3 ]]
[[ $a == *b* ]]
[ 6 -eq 2*3 ]
[[ *.png == '' ]]
[[ foo == f{o,oo,ooo} ]]
[[ $@ ]]
[ -e $@ ]
[ $@ == $@ ]
[[ $@ = $@ ]]
[[ foo =~ bar{1,2} ]]
case foo in bar) true; esac
case foo-$bar in ??|*) true; esac
case foo in foo) true; esac
case foo-$bar in foo*|*bar|*baz*) true; esac
case $f in *.txt) true;; f??.txt) false;; esac
case $f in ?*) true;; *) false;; esac
case $f in $(x)) true;; asdf) false;; esac
case $f in cow) true;; bar|cow) false;; esac
case $f in x) true;;& x) false;; esac
a[$((1+1))]=n
a[1+1]=n
a[$(echo $((1+1)))]=n
declare -A a; a[$((1+1))]=val
! [[ $(diff -q "${K}"/a "${K}"/b > /dev/null) ]] || exit 1
x=$(cmd 2>&1 > out)
y=$(cmd > out 2>&1)
z=$(cmd 2> err)
w=$(cmd >&2)
v=$(cmd 1>&2)
u=$(cmd && other > out)
test -r "$1" && read -r line <"$1"
cat "$1" >> "$1"
echo "$f" > "$f"
n=$f cmd < "$f"
case "$a" in
  *"name$b"*) one ;;
  *"name$b"*) two ;;
  a|a) three ;;
  'x'*) four ;;
  x1) five ;;
esac
case $b in
  foo) ;&
  foo) ;;
esac
case x$c in
  y*) ;;
esac
case "lit" in
  lit) ;;
esac
if [ -n "$(grep a b)" ]; then :; fi
if [ -z "$(egrep a b | /usr/bin/fgrep c)" ]; then :; fi
if [ "$(grep a b)" ]; then :; fi
if [ -n "`grep a b`" ]; then :; fi
if [[ -n $(zgrep a b) ]]; then :; fi
while ( [ a = "$1" ] ); do ( [ b = "$2" ] ); done
until ( test -f "$a" ); do :; done
for q in 1; do ( [ "$q" ] ); done
( [ "$a" ] && x=$((y++)) )
( [ "${a:=b}" ] )
( [ "$a" ] ) > out
! ( [ "$a" ] )
if ! ( [ "$a" ] ); then :; fi
f1() ( [ "$1" ] )
f2() { ( [ "$1" ] ); }
for r in "$d"/* $d/* $(ls)/*.txt `pwd`/?; do :; done
for s in $(find . -name x | grep y); do :; done
for t in $(ls -l); do :; done
for t in `ls`; do :; done
select t in $(ls); do :; done
cp ?a b*
cp -- *.txt
parallel echo ::: *.txt
printf '%s' *.txt
/bin/echo *.txt
ls [a-z]*
A=*.txt ls
A=b *.txt
A=b -x
A="cat" foo
A=c\at
B=cat C=ls
export D=ls
IFS='\'
IFS=tn
IFS=$'\n'
IFS=abc
local_var=${local_var}
e="$e$e"
e+=$e
a[$((1))]=2
declare -A assoc
assoc[$((1))]=2
a[1.5]=c
a[x.y]=c
{ a; b; } >> f1
c >> f1
d >> f1
g >> f1 &
h >> f1
i >> f1
j >> f1
k && l >> f1
cd a
foo
cd ..
cd b; cd -; cd c
[[ a == "$b"c ]]
[[ a$b == c$d ]]
[[ a == b ]]
[[ 1 -eq 2 ]]
[ 1 -eq 2 ]
[ a -nt b ]
[ "$a" = *.txt ]
[ *.txt = "$a" ]
[[ *.txt = "$a" ]]
[[ "$a" = *.txt ]]
[ "$a" = {b,c} ]
[[ "${b[@]}" = x ]]
[ "${b[@]}" = x ]
[ -v a* ]
[[ -v a* ]]
[ -z "$@" ]
[[ -n {a,b} ]]
[ "$a=b" ]
[ "a=$a" ]
[ 'a='$a ]
[ "" ]
[ '' ]
[ "$a" ]
[ false ] && [ 0 ] && [[ true ]] && [[ 1 ]]
[ "false" ]
[ -n a ]
[ -z "" ]
[ -n "" ]
[ ! -z "b$a" ]
[ -f "a$b" ]
[ x$a = 'x' ]
[ "x" = "x$a" ]
[ X"$a" == xb ]
[ y$a = xb ]
test x$a != xb
test x$a -eq xb
[[ x$a == xb && y$a == xb ]]
[[ $a =~ ^a$ ]]
[[ $a =~ "^a" ]]
[[ $a =~ "ab" ]]
[[ $a =~ 'a.' ]]
[[ $a =~ "$b" ]]
[[ $a =~ "a"* ]]
b=${a%$c} d=${a#"$c"} e=${a##$c/} f=${a%%`c`} g=${a#$(c)x} h=${a:-$c} i=${#a}
j="${a%$c}"
k=${a%${c}}
l=${a%x$c}
m=${a%'$c'}
sed 's/a/b/' <<< "$a"
sed -e 's,a,b,g' <<< "$a"
sed 's/a/b/;s/c/d/' <<< "$a"
echo "$a" | sed 's/a/b/g'
echo $a | sed -e s/a/b/
echo "$a" "$b" | sed 's/a/b/g'
echo "a" | sed 's/a/b/g'
printf "$a" | sed 's/a/b/g'
echo "$a" | sed 's/a/b/g' | cat
