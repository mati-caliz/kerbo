#!/bin/bash
{ a; b; } >> f1
c >> f1
d >> f1
g >> f1 &
h >> f1
i >> f1
j >> f1
k && l >> f1
