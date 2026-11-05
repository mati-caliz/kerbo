#!C:/Program Files/Git/usr/bin/bash
cat /etc/passwd | while read line; do let ++n; done
echo $n
