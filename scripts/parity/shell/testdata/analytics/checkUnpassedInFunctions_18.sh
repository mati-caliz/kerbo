#!/bin/bash
foo() { if mycheck; then foo ${1?Missing}; fi; }; foo
