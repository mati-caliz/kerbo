#!/bin/bash
if [ -z "$(egrep a b | /usr/bin/fgrep c)" ]; then :; fi
