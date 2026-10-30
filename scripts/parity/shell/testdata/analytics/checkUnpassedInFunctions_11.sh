#!/bin/bash
foo() { bar() { echo $1; }; bar baz; }; foo;
