import abc
import functools
from enum import property as enum_property

value = 1
items = [1, 2]

print((value))
print(((value)))
x = ((value))
y = ((value, value))
z = (())
w = ((yield_value for yield_value in items))
v = ((
    value
))
u = (  # comentario
    (value)
)
t = ((value) + 1)
s = [(value)]
r = f"{((value))}"
q = ((value)[0])
p = (value)


class Single:
    __slots__ = "name"


class Annotated:
    __slots__: str = "name"


class Many:
    __slots__ = ("name",)


class Grouped:
    __slots__ = ("name")


class Formatted:
    __slots__ = f"{value}"


class Chained:
    other = __slots__ = "name"


class Raw:
    __slots__ = b"name"


class Nested:
    if value:
        __slots__ = "name"


for item in items:
    print(item)
else:
    print("fin")

for item in items:
    if item:
        break
else:
    print("fin")

while value:
    with open("f") as handle:
        break
else:
    print("fin")

for item in items:
    for inner in items:
        break
else:
    print("fin")

for item in items:
    for inner in items:
        pass
    else:
        break
else:
    print("fin")

for item in items:
    try:
        pass
    except ValueError:
        break
else:
    print("fin")

for item in items:
    match item:
        case 1:
            break
else:
    print("fin")

for item in items:
    def helper():
        pass
else:
    print("fin")


def none_only(flag):
    if flag:
        return None
    return None


def mixed(flag):
    if flag:
        return None
    return 1


def bare(flag):
    if flag:
        return
    return None


def single():
    return None


def generator(flag):
    if flag:
        return None
    yield 1


def annotated(flag) -> int:
    if flag:
        return None
    return None


def annotated_none(flag) -> None:
    if flag:
        return (None)
    return None


class Holder:
    @property
    def attribute(self):
        print(self)
        return None

    @functools.cached_property
    def cached(self):
        print(self)
        return None

    @abc.abstractproperty
    def abstract(self):
        print(self)
        return None

    @enum_property
    def member(self):
        print(self)
        return None

    @staticmethod
    def static():
        print(1)
        return None


def outer(flag):
    def inner():
        return 1

    if flag:
        return None
    return None


def nested_generator(flag):
    def inner():
        yield 1

    if flag:
        return None
    return None


def lambda_yield(flag):
    callback = lambda: (yield)
    if flag:
        return None
    return None
