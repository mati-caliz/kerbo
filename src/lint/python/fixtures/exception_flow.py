items = [[1], [2]]
value = 1


def handlers():
    try:
        pass
    except ValueError:
        raise
    try:
        pass
    except ValueError as error:
        raise error
    except TypeError:
        raise
    try:
        pass
    except ValueError:
        raise
    except TypeError:
        print(1)
    try:
        pass
    except ValueError as error:
        raise error from None
    try:
        pass
    except ValueError:
        # comentario
        raise
    try:
        pass
    finally:
        pass
    try:
        pass
    except* ValueError:
        raise


def bare():
    raise


def in_handler():
    try:
        pass
    except ValueError:
        def inner():
            raise
        raise


class Context:
    def __exit__(self, *args):
        raise

    @staticmethod
    def __exit__(*args):
        raise


def __exit__(*args):
    raise


def ternaries(flag):
    a = True if flag else False
    b = False if flag else True
    c = True if flag > 1 else False
    d = (True) if flag else (False)
    return a, b, c, d


for items in items:
    pass

for value in [value, 2]:
    pass

for name in (name for name in items):
    pass

for key, other in {key: 1 for key in items}.items():
    pass

for element in map(lambda element: element, items):
    pass

for item in sorted(items, key=lambda item: item):
    pass

for item in items:
    pass

for item.attribute in item.values:
    pass
