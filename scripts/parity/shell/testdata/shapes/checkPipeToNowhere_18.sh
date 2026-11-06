#!/bin/bash
ls 1>&3 3>&1 3>&- | wc -l
