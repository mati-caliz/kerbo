#!/bin/bash
while getopts 'a:b' x; do x=a; case $x in a) foo;; esac; done
