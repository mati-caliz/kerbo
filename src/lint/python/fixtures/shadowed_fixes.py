def bool(value):
    return value


def list():
    return []


def max(*values):
    return values


def min(*values):
    return values


def uses_bool(a):
    if a:
        return True
    else:
        return False


def uses_ternary(a):
    return True if a else False


def uses_comparison(a, b):
    return True if a == b else False


def uses_or(a, b):
    return bool(a) or not a or b


def uses_min_max(a, b, c):
    if a > b:
        b = a
    if c < b:
        b = c
    return a if a > b else b


class Holder:
    items = dataclass_field(default_factory=lambda: [])
    other = dataclass_field(default_factory=lambda: {})
    used = (bool(1), list(), max(1), min(1))
