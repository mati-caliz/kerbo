#!C:/Program Files/Git/usr/bin/bash
for f in *; do cd $f || continue; git pull; cd ..; done
