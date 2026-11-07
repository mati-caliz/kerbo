#!/bin/bash
var=(1 2)
x=1
if [ var[x] -eq 2 ]; then echo a; fi
echo $(var[x])
var[x]
