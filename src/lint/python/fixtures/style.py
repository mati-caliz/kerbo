a = b = c = 1
if a == None: pass
if None != a:
    pass
if a == None == b:
    pass
if a == True:
    pass
if False != a:
    pass
if a == True == b:
    pass
if 1 == None:
    pass
if a == 1 == None:
    pass
if a==None:
    pass
if not a in b:
    pass
if not a is b:
    pass
if not (a in b):
    pass
try:
    pass
except:
    pass
except ValueError:
    pass
try:
    pass
except:
    raise
try:
    pass
except :  # comment
    if a:
        raise
f = lambda: 0
f.x = lambda: 0
g: object = lambda: 0
h, k = lambda: 0, 1
o = p = lambda: 0
r = (lambda: 0)


class K:
    m = lambda self: 0
    n: object = lambda: 0
    def method(self): pass
    def stub(self): ...


def d():
    n = lambda: 0
    return n


l = 1
O = 2
I = 3
for l in []:
    pass
def l(): pass
class I: pass
def fn(l, O=1, *I, **kwargs): pass
lambda l: 0
with open() as l: pass
global l
[l for l in range(3)]
l += 1
(l := 3)
try:
    pass
except ValueError as l:
    pass
z = {"a": 1, "a": 2, a: 1, a: 2, 1: 0, 1.0: 1, True: 2, 0x1: 3, (1, 2): 0, (1, 2): 1, -1: 0, -1: 1}
y = {"\n": 1, "\x0a": 2, b"a": 1, "a" "b": 2, "ab": 3, None: 0, None: 1, a.b: 0, a.b: 1}
if a is 1 or a is "s" or a is not () or a is ... or a is [] or a is None:
    pass
x = 1; y = 2
x = 1;
x = 1 ;  # comment
while x: x -= 1
for item in []: continue
with open() as handle: pass
if x: ...
else: pass
class Stub: ...
assert (x, 1)
assert ()
if (x, 1):
    pass
elif (1,):
    pass
raise NotImplemented
raise NotImplemented("x")
print >> sys.stderr
break
continue
return 1
yield 1
await x
for item in []:
    pass
else:
    break
while True:
    def inner():
        break
    try:
        pass
    finally:
        continue


class Body:
    return 2


def nested():
    from os import *
