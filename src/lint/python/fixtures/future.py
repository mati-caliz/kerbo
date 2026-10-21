from __future__ import annotations as _annotations

from typing import List


def forward(a: Later, b: List[AlsoLater]) -> Later:
    c: Missing = 1
    return c


x: Later = 1
y: NeverDefined = 2


class Later:
    def method(self) -> Later:
        return self


class AlsoLater:
    pass
