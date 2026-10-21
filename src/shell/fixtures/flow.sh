#!/bin/bash
set -o pipefail
clean=value
dirty="two words"
declare -i counter=0
counter=$(date +%s)
echo $clean $dirty $counter
read -r line rest < /dev/null
echo $line $rest
mapfile -t lines < /dev/null
echo ${lines[0]}
printf -v formatted '%s' x
echo $formatted
wait -p waited
echo $waited
unset clean
echo $clean
for item in a b c; do echo $item; done
for item in a "b c"; do echo $item; done
for ((index = 0; index < 3; index++)); do echo $index; done
while read -r entry; do echo $entry; done < /dev/null
case $1 in
  start) mode=fast ;;
  stop) mode="very slow" ;&
  *) mode=other ;;
esac
echo $mode
if [ -n "$1" ]; then
  picked=yes
elif [ -n "$2" ]; then
  picked="no way"
else
  picked=maybe
fi
echo $picked
helper() {
  local inner=ok
  echo $inner $outer
  return 0
}
outer=fine
helper
( sub=1; echo $sub )
echo $sub
total=$(( counter + 1 ))
echo $total
echo ${unknown:=default} $unknown
: ${PREFIX:=/usr}
name=$base/$file.txt
echo $name
cat <<EOF
$heredoc
EOF
array=( $(ls) $clean )
echo "${array[@]}" ${array[@]}
x=1 helper
let counted=counted+1
echo $counted
declare -A table
table[key]=1
echo ${table[key]}
command echo $clean
builtin read -r later
echo $later
exit 0
echo $after
