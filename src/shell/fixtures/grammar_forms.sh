#!/bin/bash
# Formas que la gramática de arriba no leía y por las que kerbo salteaba el script entero.
for arg do
  echo $arg
done
exec 3<>"$1"
V=3 kube::log::status $V
a=${1:$2:$3}
echo $a "${@:$#}"
for ((i = 1; i <= b[0]; i++)); do
  echo $i
done
if echo "$a" | grep -q 404$; then
  echo $a
fi
c="${1%%]*}"
d="${1//$2\/$3\//}"
e="${1//'https://a/'/gs://}"
f=${1:-x<1,y<2%}
echo $c $d $e $f
case $1 in
  *"a"*"b"*"c"*)
    echo $1
    ;;
esac
if [[ "$1" = *".pb" || "$2" = *".pb" ]]; then
  echo $c
fi
if [[ $1 = -* && $2 != exec ]]; then
  echo $d
fi
g="${1:+${1};}c"
h=${1/)*}
echo $g $h
git ls-files 'a' `# nota` \
  'b' `# otra` \
  "$c"
case $1 in a) echo $e ;& esac
[ $1 \< $2 ]
[[ $1 \< $2 ]]
[ $1 '<' $2 ]
echo $f
if [ "$a" \< 3 ]; then echo a; fi
if [[ $a \> 3 ]]; then echo b; fi
if [ a '<' b ]; then echo c; fi
[ "$a" \>= 3 ] && echo d
((3.14 * a))
echo $((b[0] + 1.5))
echo $((10#${a##*.}))
body="{\"rdata\":"[${a}]"}"
list=[$a]
echo "$body" "$list"
