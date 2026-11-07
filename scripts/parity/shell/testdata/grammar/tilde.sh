#!/bin/bash
f=$1
if [[ ~/a == /b ]]; then echo a; fi
! { [ -f "$f" ] && echo x; }
