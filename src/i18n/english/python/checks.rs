use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: chequeos de excepciones, loops y llamadas.
pub const CHECK_TEXTS: [(PythonRule, RuleText); 18] = [
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
];
