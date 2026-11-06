#!/bin/bash
while getopts 'a:' x; do case $x in a) foo;; b) bar;; esac; done
