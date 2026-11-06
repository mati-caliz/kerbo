#!/bin/bash
while read foo; do echo ls | ssh $foo; done
