#!/bin/bash
alias x=y; x
alias x=y
x
{ alias x=y
x
}
alias x=y; 'x';
:
{
alias x=y
x
}
:
{
alias x=y
alias x=z
x
}
declare -A foo; foo=(bar)
declare -A arr; arr=( var=value )
arr=( 1=value )
arr=( $a=value )
arr=( $((1+1))=value )
declare -a foo; foo=(bar)
declare -A foo; foo=([i]=bar)
typeset -A foo; foo+=(bar)
arr=( [foo]= bar )
arr=( [foo] = bar )
arr=( var=value )
arr=( [foo]=bar )
arr=( [foo]="" )
f() { f; }
f() { command f; }
cd() { cd "$lol/$1" || exit; }
cd() { [ -z "$1" ] || cd "$1"; }
cd() { something; cd $1; }
cd() { builtin cd $1; }
/
/foo/ bar/baz
/
/*
$foo/$bar
foo, bar
. foo.sh
: foo
/usr/bin/python[23] file.py
$"(foo)"
$"{foo}"
"$(foo)"
$".."
-e file
foo
  --bar=baz
'--myexec--' args
var=cmd --arg
foo*
echo foo*
foo | echo bar
ls > file | grep foo
ls | grep foo < file
ls > foo > bar
ls > foo 2> bar > baz
ls > foo &> bar
ls > foo 2> bar |& grep 'No space left'
echo World | cat << EOF
hello $(cat)
EOF

echo World | cat << 'EOF'
hello $(cat)
EOF

ls 1>&3 3>&1 3>&- | wc -l
find . -print0 | du --files0-from=/dev/stdin
basename < file.txt
find . | du --exclude-from=/dev/fd/0
yes | cp -ri foo/* bar
yes | rm --interactive *
printf 'Lol' <<< str
printf 'Lol' << eof
lol
eof

echo foo | xargs du
ls | echo $(cat)
echo foo | var=$(cat) ls
mv -i f . < /dev/stdin
> file
> file | grep foo
grep foo | > bar
grep foo > bar
foo | grep bar > baz
var=$(< file)
var=`< file`
ls > rm
ls > 'rm'
ls > myfile
( 1 > 2 )
foo 1>2
echo foo > '2'
foo 1>&2
foo_bar2=val; $"foo_bar2"
$"foo_bar2"
$".."
var=val; $"$var"
foo=var; bar=val2; $"foo bar"
