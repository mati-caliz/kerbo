#!/bin/bash
b=${a%$c} d=${a#"$c"} e=${a##$c/} f=${a%%`c`} g=${a#$(c)x} h=${a:-$c} i=${#a}
