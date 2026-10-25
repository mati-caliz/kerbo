"""Casos del importer en los arreglos."""

# comentario después del docstring

import datetime as dt
import re as regex
from collections import (
    OrderedDict,
    abc,
)
from datetime import timezone
from typing import TYPE_CHECKING, DefaultDict, Deque, List

if TYPE_CHECKING:
    import logging

stamp = dt.timezone.utc
other = timezone.utc
flags = regex.I | regex.M
queue: Deque[int] = []
table: DefaultDict[str, int] = {}
items: List[int] = []
ordered = OrderedDict()
print(abc)


def uses_late():
    return os.path.join("a", "b"), late.error


def logs():
    return logging.WARN


import os
import os.path
import socket as late

try:
    pass
except (late.error, ValueError):
    pass
except (OSError, late.error):
    pass
