#!/bin/bash
while getopts 'a:123' x; do case $x in a) foo;; [0-9]) bar;; esac; done
