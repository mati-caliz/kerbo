#!C:/Program Files/Git/usr/bin/bash
test1() { echo foo | if [[ $var ]]; then echo $var; fi; }; test2() { echo $var; }
