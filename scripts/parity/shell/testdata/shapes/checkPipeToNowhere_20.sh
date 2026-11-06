#!/bin/bash
find . | du --exclude-from=/dev/fd/0
