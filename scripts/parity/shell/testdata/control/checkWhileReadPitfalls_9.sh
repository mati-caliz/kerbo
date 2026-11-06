#!/bin/bash
while read foo; do ffmpeg -i foo.mkv bar.mkv -an; done
