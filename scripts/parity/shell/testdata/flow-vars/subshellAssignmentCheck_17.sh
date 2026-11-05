#!C:/Program Files/Git/usr/bin/bash
foo=${ { bar=$(baz); } 2>&1; }; echo $foo $bar
