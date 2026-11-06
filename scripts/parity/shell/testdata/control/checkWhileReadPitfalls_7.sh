#!/bin/bash
while read foo; do if true; then ssh $foo uptime; fi; done < file
