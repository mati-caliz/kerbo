#!C:/Program Files/Git/usr/bin/bash
for f in `find / -name '*.mp3'`; do mplayer "$f"; done
