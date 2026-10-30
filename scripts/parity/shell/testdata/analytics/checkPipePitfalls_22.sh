#!/bin/bash
foo | grep -B 1 --after-context 999 bar | wc -l
