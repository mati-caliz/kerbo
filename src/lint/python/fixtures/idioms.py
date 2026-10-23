import logging
import re
import subprocess
from re import I

#
# bloque con texto
#

value = 1  #
#


def comparisons(first, second):
    return [
        first == first,
        first < second < second,
        len(first) == len(first),
        len(first) == len(second),
        1 == 1,
        "a" != "b",
        None is None,
        -1 == -1,
        f"{first}" == "x",
    ]


def flags(text):
    return re.match("x", text, re.I | re.M), re.IGNORECASE, I


def runs(command):
    subprocess.run(command)
    subprocess.run(command, check=True)
    logging.info("raíz")
    logging.getLogger(__name__).info("propio")
    return (
        set(x for x in command),
        set((x for x in command)),
        set(x for x in command if x),
        dict((x, x) for x in command),
        dict(((x, x) for x in command)),
        dict((x, *x) for x in command),
        dict(x for x in command),
    )


def negations(first, second):
    if not first == second:
        raise ValueError("x")
    if not first == second:
        print(first)
    print(not (first == second), not first != second, not first == second == 1)


class Comparable:
    def __eq__(self, other):
        return not self == other

    def __ne__(self, other):
        return not self == other
