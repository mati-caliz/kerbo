#!/bin/bash
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
