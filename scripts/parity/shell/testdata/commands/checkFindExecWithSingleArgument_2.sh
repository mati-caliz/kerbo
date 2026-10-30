#!/bin/bash
find . -execdir 'cat {} | wc -l' +
