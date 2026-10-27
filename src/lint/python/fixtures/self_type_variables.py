"""Bordes del arreglo de PYI019."""

import typing
from typing import TypeVar, cast

_S = TypeVar("_S", bound="Shape")
_T = TypeVar("_T")
_Q = TypeVar("_Q")


class Shape:
    def copy(self: _S, other: _S) -> _S:
        clone = cast(_S, other)
        return clone

    @classmethod
    def build(cls: type[_S], count: int) -> list[_S]:
        return [cls() for _ in range(count)]

    def quoted(self: "_S") -> "_S":
        return self

    def first[S, A, B](self: S, value: A) -> S:
        return self

    def middle[A, S, B](self: S, value: A, other: B) -> S:
        return self

    def last[A, S](self: S, value: A) -> S:
        return self

    def only[S](self: S) -> S:
        return self

    def bounded[S: Shape](self: S) -> S:
        return self

    def commented[
        S,  # la variable
    ](self: S) -> S:
        return self

    def escaped(self: _Q) -> "\x5fQ":
        return self


def outside(value: _S) -> _S:
    return value


class Other:
    def method(self: _T) -> _T:
        return self


def shadowed():
    Self = 1

    class Inner:
        def method(self: _T) -> _T:
            return self

    return Self, Inner, typing
