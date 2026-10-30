#!/bin/bash
foo() { echo ${!var*}; }; foo;
