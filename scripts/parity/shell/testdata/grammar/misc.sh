#!/bin/bash
for i do
  echo $i
done
exec 3<>"$1"
echo $(( b[0] + 1 ))
