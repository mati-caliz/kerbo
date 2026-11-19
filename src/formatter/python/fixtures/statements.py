"""Docstring del módulo.

    Con sangría
  y líneas."""
import   os
import os.path as  path , sys
from   .  import  x
from .. pkg . sub import (a)
from a import (b,)
from a import b as c, d
from typing import *
from __future__ import  annotations
from a.very.long.module.name.that.goes.on.and_on import first_name, second_name, third_name_x
from a.very.long.module.name.that.goes.on.and_on import first_name, second_name, third_name_xy
from a.very.long.module.name import (first_name as one, second_name as two, third_name as three, fourth)
x=1
y  =  2;z=3
def f(a,b=1,*args,c,d:int=2,**kwargs)->int:
    '''   Docstring de función.   '''



    return a+b
class A(B,metaclass=M):

    x:int=1
    def method(self): pass
    def other(self):
        return self.method( )
class Empty(): ...
class Stub:
    def a(self): ...
    def b(self): ...

    def c(self): ...
if x:
    pass
elif y:
    pass
else:
    pass
for i in range(10):
    continue
else:
    break
while True:
    x+=1
try:
    pass
except ValueError as e:
    raise
except (TypeError,KeyError):
    raise ValueError("x") from None
else:
    pass
finally:
    pass
with open("f") as f, open("g") as g:
    pass
with open("a_very_long_file_name_that_goes_on.txt") as first_file, open("another_very_long_file_name.txt") as second_file:
    pass
async def g():
    async with a as b:
        await c
    async for i in d:
        yield i
    return
def long_function_name(argument_one, argument_two, argument_three, argument_four, argument_five):
    global first_global_name, second_global_name, third_global_name, fourth_global_name, fifth
    nonlocal a,b
    del a
    del a,b
    del (a,b)
    assert x,"mensaje"
    assert some_very_long_condition_name_here and another_very_long_condition_name_there, "un mensaje bastante largo"
    lambda: 1
    return (yield)
@decorator
@other.decorator(1,2)
def decorated():
    pass



x = 1
if True:

    def nested():
        pass
    y = 2
lazy import q
