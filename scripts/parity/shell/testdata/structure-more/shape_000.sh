#!/bin/bash
! [[ $(diff -q "${K}"/a "${K}"/b > /dev/null) ]] || exit 1
