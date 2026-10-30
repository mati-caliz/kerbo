#!/bin/dash
f() { local a; echo -n x; echo -e y; read -p p -r v; echo $_ ${x:0:1} $'a'; [[ a ]]; }
f
