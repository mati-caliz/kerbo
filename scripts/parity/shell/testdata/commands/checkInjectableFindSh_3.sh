#!/bin/bash
find . -exec sh -c 'rm "$@"' _ {} \;
