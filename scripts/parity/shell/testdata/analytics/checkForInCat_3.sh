#!/bin/bash
for f in $(cat foo | grep bar | wc -l); do stuff; done
