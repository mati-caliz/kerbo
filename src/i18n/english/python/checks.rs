use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: chequeos de excepciones, loops y llamadas.
pub const CHECK_TEXTS: [(PythonRule, RuleText); 36] = [
    (
        PythonRule::TRY203,
        RuleText::new(
            "try whose except clauses only re-raise",
            "remove the try, or handle the error in the except",
            "An except that only raises the same exception again does nothing but add indentation.",
            "try:\n    f()\nexcept ValueError:\n    raise becomes f().",
        ),
    ),
    (
        PythonRule::PLE0704,
        RuleText::new(
            "Bare raise outside an except clause",
            "raise a specific exception",
            "A bare raise outside an except has no active exception to re-raise and fails with RuntimeError.",
            "raise becomes raise ValueError(\"...\").",
        ),
    ),
    (
        PythonRule::SIM210,
        RuleText::new(
            "True if condition else False",
            "use bool(condition), or the condition itself if it is a comparison",
            "The ternary only converts the condition to a boolean, which bool() says directly.",
            "True if x else False becomes bool(x).",
        ),
    ),
    (
        PythonRule::B020,
        RuleText::new(
            "Loop variable that overrides what is being iterated",
            "give the loop variable a different name",
            "Once the loop starts, the name no longer points to the collection, so later uses see the last element.",
            "for items in items: becomes for item in items:.",
        ),
    ),
    (
        PythonRule::UP024,
        RuleText::new(
            "Old alias of OSError",
            "use OSError",
            "IOError, EnvironmentError, socket.error and the like are all OSError since Python 3.3.",
            "except IOError: becomes except OSError:.",
        ),
    ),
    (
        PythonRule::UP041,
        RuleText::new(
            "Old alias of TimeoutError",
            "use TimeoutError",
            "socket.timeout and asyncio.TimeoutError are TimeoutError since Python 3.10 and 3.11.",
            "except socket.timeout: becomes except TimeoutError:.",
        ),
    ),
    (
        PythonRule::UP003,
        RuleText::new(
            "type() of a literal instead of the type itself",
            "write the type",
            "type(\"\") is just a roundabout way to write str.",
            "type(\"\") becomes str.",
        ),
    ),
    (
        PythonRule::UP022,
        RuleText::new(
            "stdout and stderr set to PIPE in subprocess.run",
            "use capture_output=True",
            "capture_output=True says the same in one argument.",
            "run(cmd, stdout=PIPE, stderr=PIPE) becomes run(cmd, capture_output=True).",
        ),
    ),
    (
        PythonRule::B004,
        RuleText::new(
            "hasattr(x, \"__call__\") to check if something is callable",
            "use callable(x)",
            "hasattr misses objects whose class defines __call__ through __getattr__, and callable() says it directly.",
            "hasattr(x, \"__call__\") becomes callable(x).",
        ),
    ),
    (
        PythonRule::UP011,
        RuleText::new(
            "@lru_cache() without arguments",
            "write @lru_cache without parentheses",
            "Since Python 3.8 lru_cache can be used directly as a decorator.",
            "@functools.lru_cache() becomes @functools.lru_cache.",
        ),
    ),
    (
        PythonRule::SIM202,
        RuleText::new(
            "not a != b instead of a == b",
            "write a == b",
            "Negating an inequality is a double negative that says a == b the long way.",
            "not a != b becomes a == b.",
        ),
    ),
    (
        PythonRule::PIE794,
        RuleText::new(
            "Class attribute defined twice",
            "remove the first definition",
            "Only the last assignment survives, so the first one is dead code or a copy-paste mistake.",
            "the second name = 1 in the class is removed.",
        ),
    ),
    (
        PythonRule::PIE807,
        RuleText::new(
            "lambda: [] or lambda: {} instead of list or dict",
            "pass list or dict directly",
            "The builtin already builds an empty container when called, without wrapping it in a lambda.",
            "default_factory=lambda: [] becomes default_factory=list.",
        ),
    ),
    (
        PythonRule::PLW0128,
        RuleText::new(
            "Same name assigned twice in one statement",
            "rename one of the targets",
            "Only the last value survives, so the first target is almost always a typo.",
            "a, a = 1, 2 becomes a, b = 1, 2.",
        ),
    ),
    (
        PythonRule::PLW0642,
        RuleText::new(
            "Assignment to self or cls in a method",
            "use another local name",
            "Reassigning self or cls only changes the local variable, not the object or the class.",
            "self = other becomes current = other.",
        ),
    ),
    (
        PythonRule::PLW0211,
        RuleText::new(
            "Static method whose first parameter is self or cls",
            "rename the parameter, or make it a regular or class method",
            "A static method receives no instance or class, so calling the first parameter self or cls misleads the reader.",
            "@staticmethod def f(self): becomes @staticmethod def f(value):.",
        ),
    ),
    (
        PythonRule::PLC3002,
        RuleText::new(
            "Lambda called right where it is defined",
            "write the expression directly",
            "(lambda: x)() is just x with an extra function call.",
            "(lambda: compute())() becomes compute().",
        ),
    ),
    (
        PythonRule::DTZ002,
        RuleText::new(
            "datetime.today() without a time zone",
            "use datetime.now(tz=...)",
            "today() returns a naive datetime in local time, which breaks comparisons and arithmetic with aware datetimes.",
            "datetime.today() becomes datetime.now(tz=timezone.utc).",
        ),
    ),
    (
        PythonRule::S112,
        RuleText::new(
            "except that only continues the loop",
            "handle or log the error before continuing",
            "Silently skipping an exception hides failures that should at least be logged.",
            "except Exception:\n    continue gets a log call before continue.",
        ),
    ),
    (
        PythonRule::C410,
        RuleText::new(
            "list() around a list or tuple literal",
            "write the list literal",
            "The literal already builds the list without an extra call.",
            "list([1, 2]) becomes [1, 2].",
        ),
    ),
    (
        PythonRule::C406,
        RuleText::new(
            "dict() around a list or tuple of pairs",
            "write the dict literal",
            "A dict literal says the same without building an intermediate sequence of pairs.",
            "dict([(\"a\", 1)]) becomes {\"a\": 1}.",
        ),
    ),
    (
        PythonRule::DTZ003,
        RuleText::new(
            "datetime.utcnow() returns a naive datetime",
            "use datetime.now(tz=timezone.utc)",
            "utcnow() returns a datetime without a time zone that many functions treat as local time.",
            "datetime.utcnow() becomes datetime.now(tz=timezone.utc).",
        ),
    ),
    (
        PythonRule::PLR1722,
        RuleText::new(
            "exit() or quit() instead of sys.exit()",
            "use sys.exit()",
            "exit and quit are meant for the interactive shell and may not exist when Python runs without the site module.",
            "exit(1) becomes sys.exit(1).",
        ),
    ),
    (
        PythonRule::UP021,
        RuleText::new(
            "universal_newlines instead of text in subprocess.run",
            "use text=True",
            "universal_newlines is the old name of text since Python 3.7.",
            "run(cmd, universal_newlines=True) becomes run(cmd, text=True).",
        ),
    ),
    (
        PythonRule::PLW1509,
        RuleText::new(
            "preexec_fn in subprocess.Popen",
            "avoid preexec_fn, for example with start_new_session or process_group",
            "preexec_fn is not safe in programs with threads and can deadlock the child process.",
            "Popen(cmd, preexec_fn=os.setsid) becomes Popen(cmd, start_new_session=True).",
        ),
    ),
    (
        PythonRule::UP033,
        RuleText::new(
            "@lru_cache(maxsize=None) instead of @cache",
            "use functools.cache",
            "Since Python 3.9 functools.cache is an unbounded lru_cache.",
            "@lru_cache(maxsize=None) becomes @cache.",
        ),
    ),
    (
        PythonRule::B003,
        RuleText::new(
            "Assignment to os.environ",
            "change the variables with os.environ[...] or os.environ.update",
            "Replacing os.environ only rebinds the name; the process environment and child processes keep the old variables.",
            "os.environ = {} becomes os.environ.clear().",
        ),
    ),
    (
        PythonRule::PLC0208,
        RuleText::new(
            "Iteration over a set literal",
            "iterate over a tuple or list",
            "A set has no defined order and is built only to be traversed, so a tuple says the same and keeps the order.",
            "for x in {1, 2}: becomes for x in (1, 2):.",
        ),
    ),
    (
        PythonRule::UP010,
        RuleText::new(
            "Unnecessary __future__ import",
            "remove the feature from the import",
            "Every feature in this import is already mandatory in Python 3, so the import changes nothing.",
            "from __future__ import print_function is removed.",
        ),
    ),
    (
        PythonRule::RUF017,
        RuleText::new(
            "Lists flattened with sum()",
            "use itertools.chain.from_iterable or a comprehension",
            "sum() with an empty list as the start copies the accumulated list at every step, which takes quadratic time.",
            "sum(groups, []) becomes list(itertools.chain.from_iterable(groups)).",
        ),
    ),
    (
        PythonRule::B033,
        RuleText::new(
            "Repeated item in a set literal",
            "remove the repeated item",
            "A set keeps each value once, so the repeated item is dropped silently and usually hides a typo.",
            "{\"a\", \"b\", \"a\"} becomes {\"a\", \"b\"}.",
        ),
    ),
    (
        PythonRule::SIM101,
        RuleText::new(
            "Repeated isinstance calls on the same value",
            "merge them into one isinstance with a tuple",
            "isinstance accepts a tuple of types, so one call says the same with less repetition.",
            "isinstance(x, int) or isinstance(x, str) becomes isinstance(x, (int, str)).",
        ),
    ),
    (
        PythonRule::B019,
        RuleText::new(
            "Cache decorator on a method",
            "cache a module-level function or use cached_property",
            "functools.cache and lru_cache keep self in the cache, so the instances are never freed.",
            "@lru_cache on def total(self) becomes @cached_property.",
        ),
    ),
    (
        PythonRule::PLR1704,
        RuleText::new(
            "Parameter redefined by a loop, with or except",
            "use another name for the new variable",
            "The for, with or except variable overwrites the parameter, and the rest of the function no longer sees the value it received.",
            "for item in items: inside def f(item) becomes for element in items:.",
        ),
    ),
    (
        PythonRule::RUF046,
        RuleText::new(
            "int() of a value that is already an integer",
            "remove the int() call",
            "The value is already an int (an integer literal, len(), round() without digits, math.floor() and the like), so the conversion does nothing.",
            "int(len(items)) becomes len(items).",
        ),
    ),
    (
        PythonRule::RUF100,
        RuleText::new(
            "Unused noqa directive",
            "remove the directive or the codes it no longer needs",
            "A noqa that silences nothing hides future problems on that line and misleads whoever reads it.",
            "x = 1  # noqa: F401 on a line without F401 can go away.",
        ),
    ),
];
