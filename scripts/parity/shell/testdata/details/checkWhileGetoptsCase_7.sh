#!/bin/bash
while getopts 'a:b' x; do case x$x in xa) foo;; xb) foo;; esac; done
