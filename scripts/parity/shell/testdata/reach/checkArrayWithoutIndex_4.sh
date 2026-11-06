#!/bin/bash
coproc tail -f log; echo $COPROC
