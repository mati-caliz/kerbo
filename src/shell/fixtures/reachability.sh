#!/bin/bash
foo=(a b); echo $foo
read -ra arr <<< 'foo bar'; echo "$arr"
read -rpfoobar r; r=42
foo='bar baz'; foo=($foo); echo ${foo[0]}
coproc foo while true; do echo cow; done; echo $foo
coproc tail -f log; echo $COPROC
a[0]=foo; echo $a
echo $PIPESTATUS
a=(a b); a+=c
declare -a foo; foo=bar;
read -r -a arr <<< 'foo bar'; echo "$arr"
x; [ $? -eq 1 ] || [ $? -eq 2 ]
x; [ $? -eq 1 ]
x; echo "Exit is $?"; [ $? -eq 0 ]
x; [ $? -eq 0 ] && echo Success
x; if [ $? -eq 0 ]; then var=$?; fi
x; [ $? -gt 0 ] && fail=$?
[ 1 -eq 2 ]; status=$?
[ 1 -eq 2 ]; exit $?
param='--foo="bar"'; app $param
param="--foo='lolbar'"; app $param
param='--foo="bar"'; app "$param"
param="don't bother with this one"; app $param
param="--foo='lolbar'"; eval app $param
param='my\ file'; cmd="rm $param"; $cmd
param='my\ file'; cmd="rm ${#param}"; $cmd
param='my\ file'; rm $param
param="/foo/'bar baz'/etc"; rm $param
param="/foo/'bar baz'/etc"; rm ${#param}
f; f() { true; }
f() { true; }; f
if ! mycmd --version; then mycmd() { true; }; fi
mycmd || mycmd() { f; }
false || mycmd; mycmd() { f; }
f() { one; }; f; f() { two; }; f
x; a=$?
y && echo b; c=$?
z | echo d; e=$?
echo f; [ $? = 0 ]
command printf g; h=$?
printf i; j=$?
if [ 1 = 1 ]; then k=$?; fi
[ 1 = 1 ] && l=$?
[ 1 = 1 ] || [ $? = 1 ]
[ 1 = 1 ]; exit $?
foo; bar; exit; baz
die() { exit; }; foo; bar; die; baz
foo; bar || exit; baz
f() { foo; };    # Maybe sourced
f() { foo; }; exit  # Not sourced
foo
case $a in *"$b"*) exit;; esac
bar
case $a in *) exit;; esac
baz
qux
ender() { exit; }
ender
g() { :; }
g
b
g() { h() { :; }; }
exit
(
  f() { echo a; }
  rm "$(f)"
)
exit
(
  f() { echo a; }
  f
)
exit
(
  f() { echo a; }
  g
)
exit
