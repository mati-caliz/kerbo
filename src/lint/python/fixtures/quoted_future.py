from __future__ import annotations

from typing import Annotated, List, Literal, TypeVar, cast

T = TypeVar("T")


class Node:
    parent: "Node"
    children: List["Node"] = []
    meta: Annotated["Node", "metadata"]
    kind: Literal["leaf", "branch"]

    def walk(self, other: "Node") -> "Node | None":
        local: "Node" = other
        print(cast("Node", local))
        return None


def outer(value: list["Node"], *args: "int", **kwargs: "str") -> "dict[str, Node]":
    return {}


value: "Node"
broken: "Node[" = None
fstring: f"Node" = None
bytes_value: b"Node" = None
