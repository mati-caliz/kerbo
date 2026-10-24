use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: loops, llamadas, async y versiones.
pub const TAIL_CHECK_TEXTS: [(PythonRule, RuleText); 27] = [
    (
        PythonRule::UP028,
        RuleText::new(
            "for loop that only yields its items",
            "use yield from",
            "yield from says the same in one line and also forwards send() and throw() to the inner iterator.",
            "for x in items: yield x becomes yield from items.",
        ),
    ),
    (
        PythonRule::PIE810,
        RuleText::new(
            "Several startswith or endswith calls on the same value",
            "pass a tuple to a single call",
            "startswith and endswith accept a tuple of prefixes, so one call says the same.",
            "s.startswith(\"a\") or s.startswith(\"b\") becomes s.startswith((\"a\", \"b\")).",
        ),
    ),
    (
        PythonRule::C417,
        RuleText::new(
            "map() with a lambda",
            "write a comprehension or generator",
            "A comprehension reads more directly than map with a lambda and avoids a function call per item.",
            "map(lambda x: x + 1, items) becomes (x + 1 for x in items).",
        ),
    ),
    (
        PythonRule::PLC0132,
        RuleText::new(
            "TypeVar name that does not match its variable",
            "use the same name in both places",
            "Type checkers and error messages show the name passed to TypeVar, so a different one is confusing.",
            "T = TypeVar(\"U\") becomes T = TypeVar(\"T\").",
        ),
    ),
    (
        PythonRule::YTT203,
        RuleText::new(
            "sys.version_info[1] compared to an integer",
            "compare sys.version_info with a tuple",
            "Comparing only the minor version breaks on Python 4, where the minor number starts over.",
            "sys.version_info[1] < 7 becomes sys.version_info < (3, 7).",
        ),
    ),
    (
        PythonRule::YTT201,
        RuleText::new(
            "sys.version_info[0] == 3",
            "compare with >= instead",
            "On Python 4 the comparison becomes false and the code takes the Python 2 branch.",
            "sys.version_info[0] == 3 becomes sys.version_info >= (3,).",
        ),
    ),
    (
        PythonRule::RUF040,
        RuleText::new(
            "assert message that is not a string",
            "write the message as a string, or compare inside the assert",
            "The second part of an assert is the message; a number there usually means a comparison was intended.",
            "assert len(items), 2 becomes assert len(items) == 2.",
        ),
    ),
    (
        PythonRule::PLR1733,
        RuleText::new(
            "Dict lookup of a value the loop already has",
            "use the value variable",
            "Iterating d.items() already gives the value, so d[key] looks it up again for nothing.",
            "for k, v in d.items(): print(d[k]) becomes print(v).",
        ),
    ),
    (
        PythonRule::PLR1736,
        RuleText::new(
            "List lookup of an item the loop already has",
            "use the item variable",
            "Iterating enumerate(items) already gives the item, so items[index] looks it up again for nothing.",
            "for i, x in enumerate(items): print(items[i]) becomes print(x).",
        ),
    ),
    (
        PythonRule::FURB136,
        RuleText::new(
            "Ternary that is min() or max()",
            "use min() or max()",
            "A ternary that picks the lesser or greater of two values says the same as min() or max().",
            "a if a > b else b becomes max(a, b).",
        ),
    ),
    (
        PythonRule::PLR1708,
        RuleText::new(
            "raise StopIteration in a generator",
            "use return",
            "Since PEP 479 a StopIteration raised inside a generator becomes a RuntimeError.",
            "raise StopIteration becomes return.",
        ),
    ),
    (
        PythonRule::FURB132,
        RuleText::new(
            "Check before removing from a set",
            "use set.discard",
            "discard removes the item if present without raising, so the check is not needed.",
            "if x in s: s.remove(x) becomes s.discard(x).",
        ),
    ),
    (
        PythonRule::UP050,
        RuleText::new(
            "metaclass=type in a class",
            "remove the metaclass argument",
            "type is already the default metaclass.",
            "class A(metaclass=type): becomes class A:.",
        ),
    ),
    (
        PythonRule::RUF020,
        RuleText::new(
            "Never or NoReturn in a union",
            "remove it from the union",
            "Never has no values, so it adds nothing to a union.",
            "Never | int becomes int.",
        ),
    ),
    (
        PythonRule::TRY401,
        RuleText::new(
            "Exception passed again to logger.exception",
            "remove it from the message",
            "logger.exception already logs the current exception with its traceback.",
            "logger.exception(f\"failed: {e}\") becomes logger.exception(\"failed\").",
        ),
    ),
    (
        PythonRule::FURB161,
        RuleText::new(
            "bin(x).count(\"1\") to count bits",
            "use int.bit_count()",
            "Since Python 3.10 integers count their set bits directly, without building a string.",
            "bin(x).count(\"1\") becomes x.bit_count().",
        ),
    ),
    (
        PythonRule::FURB168,
        RuleText::new(
            "isinstance(x, type(None))",
            "write x is None",
            "None has a single instance, so comparing identity says the same more directly.",
            "isinstance(x, type(None)) becomes x is None.",
        ),
    ),
    (
        PythonRule::PLE0117,
        RuleText::new(
            "nonlocal without a binding in an enclosing function",
            "bind the name in the enclosing function or use global",
            "Python rejects a nonlocal name that no enclosing function defines.",
            "nonlocal count with no count in the outer function needs count = 0 there.",
        ),
    ),
    (
        PythonRule::LOG001,
        RuleText::new(
            "logging.Logger created directly",
            "use logging.getLogger",
            "A Logger created directly is not registered, so it ignores the logging configuration.",
            "logging.Logger(__name__) becomes logging.getLogger(__name__).",
        ),
    ),
    (
        PythonRule::FURB163,
        RuleText::new(
            "math.log with base 2, 10 or e",
            "use math.log2, math.log10 or math.log",
            "The dedicated functions are more precise and say the base in their name.",
            "math.log(x, 2) becomes math.log2(x).",
        ),
    ),
    (
        PythonRule::DTZ004,
        RuleText::new(
            "datetime.utcfromtimestamp()",
            "use datetime.fromtimestamp(ts, tz=timezone.utc)",
            "utcfromtimestamp returns a naive datetime that many functions take as local time; it is deprecated since 3.12.",
            "datetime.utcfromtimestamp(ts) becomes datetime.fromtimestamp(ts, tz=timezone.utc).",
        ),
    ),
    (
        PythonRule::PERF101,
        RuleText::new(
            "list() of something that can already be iterated",
            "iterate it directly",
            "Copying into a list before the loop only allocates memory.",
            "for x in list((1, 2)): becomes for x in (1, 2):.",
        ),
    ),
    (
        PythonRule::B032,
        RuleText::new(
            "Annotation where an assignment was meant",
            "use = instead of :",
            "a[\"b\"]: 1 only annotates the expression and assigns nothing.",
            "a[\"b\"]: 1 becomes a[\"b\"] = 1.",
        ),
    ),
    (
        PythonRule::PLW0711,
        RuleText::new(
            "except with a boolean operation",
            "catch a tuple of exceptions",
            "A or B evaluates to A, so except A or B only catches A.",
            "except A or B: becomes except (A, B):.",
        ),
    ),
    (
        PythonRule::ASYNC251,
        RuleText::new(
            "time.sleep in an async function",
            "await asyncio.sleep",
            "time.sleep blocks the event loop and freezes every other task while it waits.",
            "time.sleep(1) becomes await asyncio.sleep(1).",
        ),
    ),
    (
        PythonRule::ASYNC230,
        RuleText::new(
            "Blocking open() in an async function",
            "use an async file API like anyio.open_file",
            "Reading or writing a file with open() blocks the event loop.",
            "with open(path) becomes async with await anyio.open_file(path).",
        ),
    ),
    (
        PythonRule::ASYNC115,
        RuleText::new(
            "sleep(0) to yield to the event loop",
            "use the checkpoint function of the library",
            "trio and anyio have a dedicated checkpoint for this, which states the intent.",
            "await trio.sleep(0) becomes await trio.lowlevel.checkpoint().",
        ),
    ),
];
