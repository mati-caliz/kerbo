#!/bin/bash
while read foo; do ssh $foo hostname < /dev/null; done
