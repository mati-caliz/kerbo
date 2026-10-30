#!/bin/bash
alias foo="echo $PWD"
alias -p
alias foo='echo {1..10}'
alias a='cp $1 /a'
alias $1='foo'
alias a="echo \${@}"
rm -r "${DIR}"/{.gitignore,.gitattributes,ci}
rm -r /{bin,sbin}/$exec
rm -r /{{usr,},{bin,sbin}}/$exec
rm -r /{{a,b},{c,d}}/$exec
rm -r /home/$foo
rm -r /home/${USER:?}/*
rm -fr /home/$(whoami)/*
rm -r /home/${USER:-thing}/*
rm --recursive /etc/*$config*
rm -rf /home
rm -rf /usr /lib/nvidia-current/xorg/xorg
rm -rf "$STEAMROOT/"*
chmod -r 0755 dir
chmod -R 0755 dir
chmod a-r dir
egrep '.+'
fgrep '*' files
var=$(tempfile)
tempfile=$(mktemp)
exit
exit 1
exit $var
exit $((a|b))
exit -1
exit 1000
exit 'hello world'
export $foo
export "$foo"
export foo
export ${foo?}
foo=$(expr 3 + 2)
expr length "$var"
expr foo > bar
expr 1 | 2
expr 1 * 2
foo=$(expr foo : regex)
foo=$(expr foo \< regex)
expr match foo bar
expr foo : fo*
expr 5 -3
expr "$@"
expr 5 $rest
find . -name '*.wav' -o -name '*.au' -exec rm {} +
find . -name '*.wav' -o \( -name '*.au' -exec rm {} + \)
find . -name '*.wav' -o -name '*.au'
find . -exec 'cat {} | wc -l' \;
find . -execdir 'cat {} | wc -l' +
find . -exec wc -l {} \;
find / -name *.php
find * -name '*.php'
find . -exec echo {} \; > file
find . -execdir sh -c 'foo > file' \;
find -type f
find
find . -type f
find -H -L "$path" -print
find -O3 .
find -D exec .
find --help
find -Hx . -print
cat foo | grep *.mp3
grep '^aa*' file
grep --include=*.png foo
grep -F 'Foo*' file
grep -- -foo bar*
grep -e -foo bar*
grep --regex -foo bar*
grep --include 'Foo*' file
grep --exclude 'Foo*' file
grep --exclude-dir 'Foo*' file
grep -- 'Foo*' file
grep -Ev cow*test *.mp3
grep --fixed-strings 'Foo*' file
grep -o 'x*' file
grep --only-matching 'x*' file
grep '.*' file
grep --regex=*.mp3 file
grep foo *.mp3
grep-v  --regex=moo *
grep foo \*.mp3
grep *foo* file
ls | grep foo*.jpg
grep '[0-9]*' file
find . -exec sh -c 'echo {}' \;
find . -execdir bash -c 'rm "{}"' ';'
find . -exec sh -c 'rm "$@"' _ {} \;
su; rm file; su $USER
su foo; something; exit
echo rm | su foo
su root < script
let a=1
(( a=1 ))
local foo=3
f() { local foo=3; }
mkdir -p -m 0755 a/b
mkdir -pm 0755 $dir
mkdir a/b
mkdir --parents a/b
mkdir --mode=0755 a/b
mkdir_func -pm 0755 a/b
mkdir -p -m 0755 singlelevel
mkdir -p -m 0755 ../bin
mkdir -p -m 0755 ../bin/laden
mkdir -p -m 0755 ./bin
mkdir -p -m 0755 ./bin/laden
mkdir -p -m 0755 ./../bin
mkdir -vpm 0755 a/b
mkdir -p -m 0755 .././bin
mkdir -p -m 0755 ../../bin
mkdir -pm 0755 -v a/b
mkdir --parents --mode=0755 a/b
mkdir --parents --mode 0755 a/b
mkdir -p --mode=0755 a/b
mkdir --parents -m 0755 a/b
mkdir -p a/b
mkdir -m 0755 a/b
mv foo bar
mv 'foo bar'{,bak}
mv "$@"
mv -t foo bar
mv --target-directory=foo bar
mv --target-direc=foo bar
mv --version
mv "${!var}"
trap f 8
trap f 0
trap f 14
trap f SIGKILL
trap f 9
trap f stop
trap 'stop' int
read $var
read -r $var
read -p $var
read -rd $delim name
read "$var"
read -a $var
read $1
read ${var?}
read arr[val]
return
return 1
return $var
return $((a|b))
return -1
return 1000
return 'hello world'
set foo 42
set foo = 42
set foo=42
set -- if=/dev/null
set 'a=5'
set
sudo cd /root
sudo export x=3
sudo ls /usr/local/protected
sudo ls && export x=3
sudo echo ls
sudo -n -u export ls
sudo docker export foo
sudo echo 3 > /proc/file
sudo cmd < input
sudo cmd >> file
sudo cmd &> file
sudo cmd 2>&1
sudo cmd 2> log
sudo cmd > /dev/null 2>&1
time -f lol sleep 10
time sleep 10
time -p foo
command time -f lol sleep 10
tr [a-f] [A-F]
tr --squeeze-repeats rl lr
tr abc '[d*]'
tr '[=e=]' 'e'
tr 'a-z' 'A-Z'
tr '[a-z]' '[A-Z]'
tr -d '[:lower:]'
tr -d '[:upper:]'
tr -d '|/_[:upper:]'
ls [a-z]
tr foo bar
tr 'hello' 'world'
tr aeiou _____
a-z n-za-m
unset foo[1]
unset foo
unset foo[$i]
unset foo[x${i}y]
unset foo][
echo 'foo\nbar\n'
echo '\x4a'
echo '\xat'
echo '\xth'
echo -e 'foi\nbar'
echo "n:\t42"
echo lol
echo -n -e '
'
echo '\506'
echo '\5a'
echo '\8a'
echo '\d5a'
xargs -i{} echo {}
xargs -I{} echo {}
xargs sed -i -e foo
xargs -e sed -i foo
xargs -x sed -i foo
