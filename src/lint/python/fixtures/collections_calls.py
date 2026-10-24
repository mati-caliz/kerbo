import builtins

items = [3, 1, 2]


def handlers():
    try:
        pass
    except ValueError as error:
        raise error
    try:
        pass
    except ValueError as error:
        raise error from None
    try:
        pass
    except ValueError as error:
        raise (error)
    try:
        pass
    except ValueError as error:
        if items:
            raise error
    try:
        pass
    except ValueError as error:
        try:
            raise error
        except TypeError:
            raise error
        finally:
            raise error
    try:
        pass
    except ValueError as error:

        def inner():
            raise error

        raise ValueError from error
    try:
        pass
    except ValueError:
        raise
    try:
        pass
    except* ValueError as group:
        raise group


"abc".encode("utf-8")
"abc".encode()
"abc".encode("UTF8")
"abc".encode(encoding="utf-8")
"abc".encode("latin-1")
"ñ".encode()
"ñ".encode("utf-8")
"ñ".encode(encoding="utf-8")
"\N{BULLET}".encode()
"é".encode("utf-8")
"\777".encode()
"\101".encode()
r"é".encode()
f"{items}".encode()
f"{items}".encode("utf-8")
("a" "b").encode()
b"abc".decode()
"abc".encode("utf-8", "strict")
"abc".encode(errors="strict")

list(x for x in items)
list((x * 2 for x in items))
list([x for x in items])
list(x for x in items if x)
tuple([1, 2])
tuple((1, 2))
tuple(())
tuple((items))
tuple([1], )
tuple(x for x in items)
list(sorted(items))
reversed(sorted(items))
list(sorted(items), )
builtins.list(sorted(items))
sorted(list(items))
any([x for x in items])
all({x for x in items})
any([x for x in items], )
sum([x for x in items])
any(x for x in items)


async def asynchronous():
    any([await x for x in items])
    any([x async for x in items])


def shadowed(list, tuple, any):
    list(x for x in items)
    tuple([1])
    any([x for x in items])
