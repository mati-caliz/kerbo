import collections
import dataclasses
import datetime
from typing import NewType, Optional, Sequence, Union

UserId = NewType("UserId", int)


def mutable(a=[], b={}, c=set(), d=collections.deque(), e=list[int](), f=(), g=None):
    return a, b, c, d, e, f, g


def annotated(a: Sequence[int] = [], b: Optional[int] = int(), c: Union[int, str] = len("x"), d: int | None = len("x")):
    return a, b, c, d


def calls(a=len("x"), b=datetime.datetime.now(), c=datetime.timedelta(1), d=UserId(1), e=lambda: len("x")):
    return a, b, c, d, e


def nested(a=[len(x) for x in "ab"]):
    return a


handler = lambda a=len("x"), b=[]: (a, b)


class Base:
    pass


class Child(Base):
    def __init__(self):
        super(Child, self).__init__()
        super().__init__()
        super(Base, self).__init__()

    def other(this):
        super(Child, this).__init__()
        super(Child, self).__init__()

    def shadowed(self):
        Child = Base
        super(Child, self).__init__()

    def cell(self):
        super(__class__, self).__init__()

    class Inner(Base):
        def method(self):
            super(Child.Inner, self).__init__()
            super(Inner, self).__init__()

    def comprehension(self):
        return [super(Child, self) for _ in range(1)]


@dataclasses.dataclass(slots=True)
class Slotted(Base):
    def method(self):
        super(Slotted, self).__init__()


def ranges(values):
    for index in range(0, 10):
        print(index)
    for index in range(0, 10, 2):
        print(index)
    for index in range(1, 10):
        print(index)
    return range(0x0, len(values))


def attributes(target, name):
    first = getattr(target, "value")
    second = getattr(target, name)
    third = getattr(target, "not valid")
    fourth = getattr(target, "__private")
    fifth = getattr(target, "__dunder__")
    sixth = getattr(target, "value", None)
    setattr(target, "value", 1)
    result = setattr(target, "value", 1)
    setattr(target, "__debug__", 1)
    setattr(target, "class", 1)
    return first, second, third, fourth, fifth, sixth, result


def first_elements(values):
    return [
        list(values)[0],
        tuple(values)[0],
        list(values)[1],
        [value for value in values][0],
        [*values][0],
        list(values).pop(0),
        list(value for value in values)[0],
        sorted(values)[0],
        list(values, )[0],
    ]
