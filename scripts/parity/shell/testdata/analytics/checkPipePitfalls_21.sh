#!/bin/bash
foo | grep --after-context 999 bar | wc -l
