#!/bin/bash
set -e; ! true; rest
set -e
shellcheck disable=SC0000
! true
rest
! true; rest
set -e; while true; do ! true; done
set -e; if ! true; then true; fi
set -e; ( ! true )
set -e; { ! true; }
set -e; x() { ! [ x ]; }
set -e; if { ! true; }; then true; fi
set -e; while ! true; do true; done
a= b
c=
IFS= read -r d
e=  # x
f=; g
local h= i
export j= k
echo 'l\'
echo 'm\\'
echo 'n\o'
echo 'p\' 'q\'
set -e
! true; rest
while true; do ! true; done
if ! true; then true; fi
( ! true )
{ ! true; }
x() { ! [ x ]; }
if { ! true; }; then true; fi
while ! true; do true; done
if a; then ! b; c; else ! d; fi
for r in s; do ! t; done
! u
