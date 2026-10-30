#!/bin/bash
find . -execdir sh -c 'foo > file' \;
