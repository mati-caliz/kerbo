#!/bin/bash
foo | grep -B999 bar | wc -l
