#!/bin/sh
[ -v x ]
[ -R x ]
[ -N f ]
[ -G f ]
[ -O f ]
test a "<" b
[ a = b -a c = d ]
[ ! -a f ]
test -N f
test a =~ b
