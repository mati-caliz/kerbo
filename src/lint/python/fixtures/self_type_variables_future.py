"""PYI019 con las anotaciones diferidas."""

from __future__ import annotations

from typing import TypeVar


class Shape:
    def copy(self: _S, other: _S) -> _S:
        return other

    @classmethod
    def build(cls: type[_S]) -> _S:
        return cls()

    def generic[S](self: S, value: "S") -> S:
        return value


_S = TypeVar("_S", bound=Shape)
