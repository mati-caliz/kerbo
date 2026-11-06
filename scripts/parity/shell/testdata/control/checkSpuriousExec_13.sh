#!/bin/dash
shopt -s execfail; exec foo; exec bar; echo 'Error'; exit 1;
