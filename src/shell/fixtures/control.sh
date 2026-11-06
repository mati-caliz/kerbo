#!/bin/bash
ls *[:digit:].jpg
ls *[[:digit:]].jpg
ls [10-15]
ls [a-zA-Z]
tr -d [aa]
[[ $x == [!!]* ]]
[[ -v arr[keykey] ]]
[[ arr[keykey] -gt 1 ]]
read arr[keykey]
n=$(echo $foo | wc -c)
find / -name '*.php' -exec rm {};
find / -exec touch {} && ls {} \;
find / -execdir cat {} | grep lol +
find / -name '*.php' -exec foo {} +
find / -execdir bash -c 'a && b' \;
find / -type d -execdir rm *.jpg \;
for f in "$(ls)"; do echo foo; done
for f in "$@"; do echo foo; done
for f in *.mp3; do echo foo; done
for f in "*.mp3"; do echo foo; done
for f in 'find /'; do true; done
for f in 1,2,3; do true; done
for f in foo{1,2,3}; do true; done
for f in ls; do true; done
for f in "${!arr}"; do true; done
for f in ls, grep, mv; do true; done
for f in 'ls', 'grep', 'mv'; do true; done
for f in 'ls,' 'grep,' 'mv'; do true; done
function foo() { command foo --lol "$@"; }
function foo { lol; }
foo() { echo bar; }
./ file
./file
continue 2
for f; do ( break; ); done
if true; then continue; fi
while true; do break; done
if true; then break; fi
while true; do true | { break; }; done
while true; do true | { break; }; done
for i in *; do for i in *.bar; do true; done; done
for i in *; do for((i=0; i<3; i++)); do true; done; done
for i in *; do for j in *.bar; do true; done; done
for _ in *; do for _ in *.bar; do true; done; done
PATH="$var/$foo"
PATH="mydir"
PATH=/cow/foo
PATH=/cow/foo/bin
PATH='/bin:/sbin'
PATH="$var/$foo" cmd
PATH=$OLDPATH
PATH=$PATH:/stuff
A=ls | grep foo
A=foo cmd | grep foo
A=foo
exec foo; true
exec file; r=$?; printf >&2 'failed
'; return $r
exec file; :
shopt -s execfail; exec foo; exec bar; echo 'Error'; exit 1;
shopt -s execfail; exec foo; exec bar; echo 'Error'; exit 1;
if a; then exec b; exec c; fi
echo cow; exec foo
if a; then exec b; fi
exec > file; cmd
exec foo > file; cmd
exec file; echo failed; exit 3
exec {origout}>&1- >tmp.log 2>&1; bar
for file in rc.d/*; do exec "$file"; done
foo |& bar
foo |& bar
PATH="$PATH:~/bin"
PATH='~foo/bin'
PATH=~/bin
while read foo; do ssh $foo uptime; done < file
while read foo; do mplayer foo.ogv > file; done
while read foo; do mplayer foo.ogv <<< q; done
while read foo
do
mplayer foo.ogv << EOF
q
EOF
done
while read foo; do x=$(ssh host cmd); done
while read foo; do echo $(ssh host cmd) < /dev/null; done
while read foo; do ssh $foo cmd & done
while read -u 3 foo; do ssh $foo uptime; done 3< file
while true; do ssh host uptime; done
while read foo; do ssh $foo hostname < /dev/null; done
while read foo; do echo ls | ssh $foo; done
while read foo <&3; do ssh $foo; done 3< foo
while read foo; do if true; then ssh $foo uptime; fi; done < file
while read foo; do ssh -n $foo uptime; done < file
while read foo; do ffmpeg -i foo.mkv bar.mkv -an; done
