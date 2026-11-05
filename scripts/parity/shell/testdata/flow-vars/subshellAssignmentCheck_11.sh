#!C:/Program Files/Git/usr/bin/bash
cat /etc/passwd | while read line; do let n=n+1; done
echo $n
