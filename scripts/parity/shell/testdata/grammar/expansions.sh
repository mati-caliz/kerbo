#!/bin/bash
x=$1
echo "${x%%]*}" "${x//$x\/$x\//}" "${x:-a<b}" "${@:$#}"
grep -q 404$ "$x"
