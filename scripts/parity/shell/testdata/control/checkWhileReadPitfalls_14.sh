#!/bin/bash
while read foo; do echo $(ssh host cmd) < /dev/null; done
