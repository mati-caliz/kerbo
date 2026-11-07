#!/bin/bash
x=$1
if [ "$x" \< 3 ]; then echo a; fi
if [[ $x \> 3 ]]; then echo b; fi
if [ a '<' b ]; then echo c; fi
[ "$x" \>= 3 ] && echo d
