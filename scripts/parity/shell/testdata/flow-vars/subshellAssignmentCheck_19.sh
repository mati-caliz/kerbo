#!/bin/bash
shopt -s lastpipe; echo a | read -r b; echo "$b"
