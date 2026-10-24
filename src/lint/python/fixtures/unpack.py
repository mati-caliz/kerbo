import typing
from typing import Generic, TypedDict, TypeVarTuple, Unpack

import typing_extensions

Shape = TypeVarTuple("Shape")


class Movie(TypedDict):
    name: str


class Array(Generic[Unpack[Shape]]):
    pass


class Pair(Generic[int, Unpack[Shape]]):
    pass


def variadic(*args: Unpack[Shape]) -> tuple[Unpack[Shape]]:
    return args


def qualified(*args: typing.Unpack[tuple[int, ...]]) -> None:
    pass


def extension(*args: typing_extensions.Unpack[Shape]) -> None:
    pass


def keywords(**kwargs: Unpack[Movie]) -> None:
    pass


def plain(value: Unpack[Shape]) -> None:
    pass


def union(*args: Unpack[int | str]) -> None:
    pass


Alias = tuple[int, Unpack[Shape]]
Bare = Unpack[Shape]
Nested = dict[str, tuple[Unpack[Shape]]]
Parenthesized = tuple[(int, Unpack[Shape])]
quoted: "tuple[Unpack[Shape]]" = ()
