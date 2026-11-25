try:
    pass
except EOFError, TypeError: pass
except (A, B) as e: pass
except VeryLongExceptionNameNumberOne, VeryLongExceptionNameNumberTwo, VeryLongExceptionNameThree: pass
match x:
    case Interpolation(value, _, conversion, format_spec):
        pass
    case Foo(_):
        pass
    case Foo(a=_):
        pass
