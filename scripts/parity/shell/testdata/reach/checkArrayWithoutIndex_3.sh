#!/bin/bash
coproc foo while true; do echo cow; done; echo $foo
