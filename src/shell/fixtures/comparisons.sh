#!/bin/bash
cow=0; foo=0; if [[ $lol -eq cow && $lol -eq foo ]]; then echo foo; fi
(( a == 1 && a == 2 ))
[ $x -eq 1 ] && [ $x -eq 2 ]
[ 1 -eq $x ] && [ $x -eq 2 ]
[ 1 -eq $x ] && [ $x -eq 1 ]
[ $a = $b ] && [ $a = $c ]
lol=0 foo=0; (( a==lol && a==foo ))
[ "$a" = lol && "$a" = foo ]
[ a = $cow && b = $foo ]
[ $a = a ] && [ $a = b ]
[ $a = a ] && [ $a = b ] || true
[[ $a == x && $a == x ]]
[ 0 -eq $FOO ] && [ 0 -eq $BAR ]
(( 0192 ))
(( 0x192 ))
(( 1 ^ 0777 ))
i=i + 1
foo=bar * 2
foo + opts
[ x ] & [ y ]
test -e foo & [ y ]
[ x ] | [ y ]
case foo in lol$n) foo;; esac
case foo in $(foo)) foo;; esac
case foo in *$bar*) foo;; esac
a=(1, 2)
a+=(1,2,3)
cow=(1 "foo,bar" 3)
cow=('one,' 'two')
a=([a]=b,[c]=d,[e]=f)
a=(1,2)
[ foo && bar ]
[[ foo -o bar ]]
[[ foo || bar ]]
[ foo -a bar ]
[ -z 3 -o a = b ]
if [ ]; then ..; fi
[ foo -o bar ]
[[ $foo =~ *foo* ]]
[[ $foo =~ f* ]]
[[ $foo =~ $foo ]]
[[ $foo =~ ^c.* ]]
[[ $foo =~ \* ]]
[[ $foo =~ (o*) ]]
[[ $foo =~ \*foo ]]
[[ $foo =~ x\* ]]
ls > $((i++))
cat < "foo$((i++)).txt"
while true; do true; done > $((i++))
cat <<< $((i++))
ls > $((i=i+1))
[[ $foo < 3 ]]
[ $foo -eq 'N' ]
[ x$foo -gt x${N} ]
[ $foo > $bar ]
[[ foo < bar ]]
[ foo -eq 'y' ]
[[ 'foo' -eq 2 ]]
[[ foo -eq 2 ]]
foo=1; [[ foo -eq 2 ]]
[[ 0 >= $(cmd) ]]
[[ 2 -eq / ]]
[[ foo -eq foo ]]
x=10; [[ $x > $z ]]
x=0; if [[ -n $def ]]; then x=$def; fi; while [ $x > $z ]; do lol; done
x=$RANDOM; [ $x > $z ]
[[ $((n++)) > $x ]]
[[ $foo ]] > 3
[[ $foo > 2.72 ]]
[[ $foo -le 2.72 ]]
[[ 3.14 -eq $foo ]]
[[ 3.14 == $foo ]]
[ foo <= bar ]
if [[ $lol -ne cow || $lol -ne foo ]]; then echo foo; fi
(( a!=lol || a!=foo ))
[ "$a" != lol || "$a" != foo ]
[ a != $cow || b != $foo ]
[[ $a != /home || $a != */public_html/* ]]
[ $a != a ] || [ $a != b ]
[ $a != a ] || [ $a != b ] || true
[[ $a != x || $a != x ]]
[ 0 -ne $FOO ] || [ 0 -ne $BAR ]
x+=1
x+=42
(( x += 1 ))
declare -i x=0; x+=1
x+='1'
n=foo; x+=n
n=4; x+=n
n=4; x+=$n
declare -ia var; var[x]+=1
foo = $bar
var +=$(foo)
$foo = $bar
'var' =$bar
foo ='$bar'
$foo =$bar
'var' =$bar
var += $(foo)
[ test =~ foo ]
( -e file )
( 1 -gt 2 )
( grep -c foo bar )
[ 1 -gt 2 ]
( -e file && -x file )
( -e file || -x file && -t 1 )
( ! -d file )
test 3 > 1
test 3 \> 1
/usr/bin/test $var > $foo
test 1 -eq 2 2> file
if -z n ]]; then true; fi 
if [[ -z n ]]; then true; fi 
a || b ] && thing
run [ foo ]
run bar ']'
[ -a foo ]
[ ! -a foo ]
[ foo -a bar ]
echo $(( ((1+1)) ))
x[(1+1)]=1
$(( (x) ))
(( (x) ))
x[(1+1)+1]=1
(( (1*1)+1 ))
(( (1)+1 ))
