#!/bin/bash
while read foo; do ssh $foo cmd & done
