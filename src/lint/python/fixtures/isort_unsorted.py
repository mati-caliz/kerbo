import sys
import os, json
from typing import Any, TYPE_CHECKING
import requests
from . import sibling
from collections import (
    OrderedDict,
    defaultdict,
)
from collections import abc  # comentario
from very_long_module_name_for_testing.submodule import first_name, second_name, third_name
from other import name  # noqa: F401 este comentario no cuenta para el ancho de la línea
x = 1


def function():
    import sys
    import os

    return os, sys


try:
    import json
except ImportError:
    json = None

# isort: off
import zlib
import abc
# isort: on
import bz2
import array
import re  # isort: skip
import ast


class Thing:
    pass
