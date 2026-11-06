#!/bin/bash
while read foo <&3; do ssh $foo; done 3< foo
