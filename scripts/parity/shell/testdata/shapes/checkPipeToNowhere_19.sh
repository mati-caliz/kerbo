#!/bin/bash
find . -print0 | du --files0-from=/dev/stdin
