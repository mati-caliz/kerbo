#!/bin/bash
while getopts 'a:' x; do case $x in a) foo;; \?) bar;; *) baz;; esac; done
