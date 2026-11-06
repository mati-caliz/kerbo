#!/bin/bash
exec file; r=$?; printf >&2 'failed
'; return $r
