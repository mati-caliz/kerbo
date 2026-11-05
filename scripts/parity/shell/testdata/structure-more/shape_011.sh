#!/bin/bash
case "$a" in
  *"name$b"*) one ;;
  *"name$b"*) two ;;
  a|a) three ;;
  'x'*) four ;;
  x1) five ;;
esac
