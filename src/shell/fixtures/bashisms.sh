#!/bin/sh
echo $((16#ff)) $((8#7)) $((x + 2#1))
a=(1 2 3)
b+=(4)
echo "${a[@]}" "${#a[@]}" "${a[0]}"
echo {1..3} {a,b} x{y,z}w "{a,b}" {a} {}
cp file{,.bak}
case $x in [^a]*) echo ;; esac
echo -e "a"
echo -E x
echo -n
echo -s x
echo -en x
echo -- -n
exec -a name cmd
exec cmd
let x=1
source ./a
. ./a
read
read -r
read -p prompt x
read -s x
printf "%q
" a
printf -v v x
printf %s q
trap "" err debug RETURN sigterm Term
trap - EXIT
wait -n
wait
hash -d
jobs -x
unset -v x
ulimit -n 1
umask -S
type -t ls
cd -L /
cd -@ /
export -n x
readonly -f g
x=1
if [[ $x = 1 ]]; then echo y; fi
[[ -n $x && $x == 2 ]] && echo z
[[ $x < 3 ]]
echo ${x/a/b} ${x//a} ${x:1} ${x: -1} ${x:-d} ${x:=d} ${x:+d} ${x:?d} ${#x} ${x#a} ${x%%b}
echo ${!p} ${x^} ${x,,} ${@:2} ${*%a} ${#@} ${#*}
echo ${a[0]} ${a[*]} ${!a[*]} ${!pre@}
echo $BASH_SOURCE $PIPESTATUS $EUID $UID $HOSTNAME $SECONDS $BASH_VERSION ${FUNCNAME[0]}
echo $((RANDOM + SECONDS))
x=$(< file)
y=`< file`
z=$(<file cat)
cat > out*.log
cat < in?.txt
foo.bar() { :; }
function baz { :; }
function qux() { :; }
a:b() { :; }
_ok() { :; }
ls [^a]* [!b]* *[^0-9]
f() {
  local a=1 b
  local
  echo "$a$b"
}
f
a |& b
coproc foo
select x in a b; do echo $x; done
for ((i=0;i<3;i++)); do :; done
((x++))
echo $[1+2]
echo $((2**3)) $((x--)) $((++x))
cmd &>/dev/null
cmd &>>log
cmd >&log
cmd 2>&1
cmd 10>file
cmd {fd}>file
cat < /dev/udp/h/1
cmd >& 2
set -o pipefail -o posix
set +o errexit
set -euo pipefail
set -eE
set --foo
set -h
set -k
set -- -o pipefail
set -o
set +H
echo $"hola"
echo $'tab	'
echo "$'no'"
[ -v x ]
[ -R x ]
[ -N f ]
[ -G f ]
[ -O f ]
test a "<" b
[ a = b -a c = d ]
[ ! -a f ]
test -N f
test a =~ b
declare -a x
typeset y
shopt -s nullglob
pushd /tmp
popd
mapfile -t lines < f
compgen -W a
complete -F f g
builtin echo hi
caller
readarray z
disown
suspend
enable -n echo
dirs
