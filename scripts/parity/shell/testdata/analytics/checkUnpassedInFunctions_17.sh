#!/bin/bash
foo() { mycommand ${1+--verbose}; }; foo
