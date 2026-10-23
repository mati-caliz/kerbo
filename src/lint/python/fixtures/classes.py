import ctypes
import dataclasses
from typing import ClassVar, Final, Sequence

import pydantic


class Plain:
    items = []
    mapping = {}
    flags = set()
    numbers = (1, 2)
    __slots__ = []
    first = second = []
    typed: list = []
    sequence: Sequence[int] = []
    shared: ClassVar[list] = []
    constant: Final = []
    bare: ClassVar = []
    shared = []
    nothing: list


@dataclasses.dataclass
class Data:
    items: list = []
    other = []


class Model(pydantic.BaseModel):
    items: list = []


class Derived(Model):
    items = []


class Point(ctypes.Structure):
    before = []
    _fields_ = [("x", ctypes.c_int)]
    after = []


def factory():
    class Local:
        items = []

    return Local
