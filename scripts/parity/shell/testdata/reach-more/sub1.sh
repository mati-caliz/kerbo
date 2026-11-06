#!/bin/bash
(
  f() { echo a; }
  rm "$(f)"
)
exit
