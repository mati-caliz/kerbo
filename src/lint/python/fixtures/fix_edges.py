from __future__ import annotations, generators, print_function
import io, os; import sys
from typing import Text
import typing

x = ""u""
y = u"a" u"b"


class Base(object, metaclass=type):
    pass


class Commented(  # comentario
    object
):
    pass


class Empty():
    pass


class Spaced(
    # comentario
):
    pass


class Child(Base):
    def method(self):
        super(Child, self).method()
        super(
            Child,
            self,
        ).method()


value = getattr(1, "real")
other = getattr(sys, "path")
chained = getattr(os.environ, "copy")()
spanning = getattr(
    sys,
    "argv",
)
padded = 1 if getattr(sys, "path")else 2
kind = type("")or type(1)
complex_kind = type(1j)
encoded = "texto".encode("utf-8")
unicode_encoded = "año".encode("utf-8")
keyword_encoded = "año".encode(encoding="utf-8")
joined = ("a" "b").encode()
prefixed = u"a".encode()
formatted = f"{x}".encode("utf-8")
print((x))
print(((x)))
data = {"a": 1}
if "a" in data.keys():
    pass
if "a" in data.keys()and x:
    pass
if "a" in sys.modules.keys():
    pass
for key in range(0, 3):
    pass
for key in range(0, 3, 1):
    pass


def placeholders():
    """Doc."""
    pass  # comentario


def before_docstring():
    pass
    "no es un docstring"


def ellipsis_body():
    x = 1
    ...


def unused_values():
    a = b = 1
    c = sys.exit(0)
    d: int = 1
    e: int = sys.exit(0)
    with open("f") as handle, open("g") as other_handle:
        print(handle)
    first, second = 1, 2
    print(first)


def annotated(argument: "int", other: """
    int
"""):
    local: "list[int]" = []
    multiline: """
        int
    """ = 1
    commented: """int  # comentario""" = 1
    return local, multiline, commented


factory = lambda: []
mapping = lambda: {}
text_alias: Text = ""
qualified: typing.Text = ""
stream = io.open("f")
