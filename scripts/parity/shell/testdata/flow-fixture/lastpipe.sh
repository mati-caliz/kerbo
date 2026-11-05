#!/bin/bash
shopt -s lastpipe
echo x | read -r last_stage
echo "$last_stage"
read -r middle_stage | cat
echo "$middle_stage"
