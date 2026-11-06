#!/bin/bash
while getopts 'a:b' x; do case $x in a) foo;; esac; done
