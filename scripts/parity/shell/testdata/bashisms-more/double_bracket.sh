#!/bin/sh
x=1
if [[ $x = 1 ]]; then echo y; fi
[[ -n $x && $x == 2 ]] && echo z
[[ $x < 3 ]]
