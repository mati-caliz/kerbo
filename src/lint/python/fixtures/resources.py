import contextlib
import pathlib
import tempfile
import unittest


def opening(path):
    handle = open(path)
    with open(path) as managed:
        print(managed)
    open(path).close()
    data = open(path).read()
    temporary = tempfile.NamedTemporaryFile()
    other = pathlib.Path(path).open()
    with contextlib.ExitStack() as stack:
        stack.enter_context(open(path))
    stack = contextlib.ExitStack()
    stack.enter_context(open(path))
    return handle, data, temporary, other


def returning(path):
    return open(path)


async def asynchronous(path):
    async with contextlib.AsyncExitStack() as stack:
        await stack.enter_async_context(open(path))


class Wrapper:
    def __enter__(self):
        self.handle = open("x")
        return self


class Case(unittest.TestCase):
    def test(self):
        self.enterContext(open("x"))


def validate(value, items):
    if isinstance(value, int):
        raise ValueError("no")
    if not isinstance(value, str):
        raise ValueError("no")
    if isinstance(value, int) and callable(value):
        raise RuntimeError
    elif issubclass(type(value), int):
        raise Exception("x")
    else:
        raise ValueError("y")
    if isinstance(value, int):
        raise TypeError("ok")
    if value > 1:
        raise ValueError("no es de tipo")
    if isinstance(value, int):
        if items:
            return None
        raise ValueError("despues de un return")
    if isinstance(value, int):
        for item in items:
            if isinstance(item, int):
                raise ValueError("adentro")
    if value:
        if isinstance(value, int):
            raise ValueError("el if de afuera no chequea tipos")
    if isinstance(value, int):
        if isinstance(value, bool):
            raise ValueError("los dos chequean tipos")
    if isinstance(value, int):
        pass
    elif value:
        raise ValueError("elif sin chequeo")
