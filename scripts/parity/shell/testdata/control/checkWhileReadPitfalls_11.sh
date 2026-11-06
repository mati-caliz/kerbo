#!/bin/bash
while read foo; do mplayer foo.ogv <<< q; done
