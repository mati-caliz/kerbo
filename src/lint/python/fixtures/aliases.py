import asyncio
import functools
import os
import socket
import subprocess
from functools import lru_cache
from subprocess import PIPE, run


def errors():
    try:
        pass
    except IOError:
        pass
    try:
        pass
    except (EnvironmentError, ValueError):
        pass
    try:
        pass
    except (ValueError, KeyError):
        pass
    try:
        pass
    except os.error:
        pass
    try:
        pass
    except socket.timeout:
        pass
    try:
        pass
    except (asyncio.TimeoutError, socket.error):
        pass
    try:
        pass
    except IOError as error:
        raise error
    raise IOError
    raise IOError("x")
    raise socket.timeout from None
    raise (EnvironmentError)


IOError("x")
socket.error()
asyncio.TimeoutError()

type("")
type(b"")
type(1)
type(1.5)
type(1j)
type(True)
type(None)
type(-1)
type(f"{type}")
type("", key=1)
type(x)

subprocess.run(["ls"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
run(["ls"], stdout=PIPE, stderr=PIPE)
subprocess.run(["ls"], stdout=subprocess.PIPE)
subprocess.run(["ls"], stdout=PIPE, stderr=subprocess.STDOUT)
subprocess.run(["ls"], stdout=PIPE, stderr=PIPE, capture_output=False)

hasattr(len, "__call__")
getattr(len, "__call__")
getattr(len, "__call__", None)
hasattr(len, "__call__", 1)
hasattr(len, "__len__")
hasattr(len, b"__call__")


@functools.lru_cache()
def cached():
    return 1


@lru_cache()
def cached_too():
    return 1


@lru_cache(maxsize=None)
def bounded():
    return 1


@lru_cache
def plain():
    return 1


class Holder:
    @functools.lru_cache( )
    def method(self):
        return 1
