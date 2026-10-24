use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas de pylint del conjunto por defecto de ruff 0.17.
pub const PYLINT_ERROR_TEXTS: [(PythonRule, RuleText); 20] = [
    (
        PythonRule::PLE0100,
        RuleText::new(
            "__init__ that is a generator",
            "move the yield to another method",
            "A yield turns __init__ into a generator, and Python raises TypeError when the class is instantiated.",
            "def __init__(self): yield 1 fails at Foo().",
        ),
    ),
    (
        PythonRule::PLE0115,
        RuleText::new(
            "Name declared both nonlocal and global",
            "keep only one of the declarations",
            "A name cannot be nonlocal and global in the same scope; Python rejects the function.",
            "global x followed by nonlocal x is a SyntaxError.",
        ),
    ),
    (
        PythonRule::PLE0118,
        RuleText::new(
            "Name used before its global declaration",
            "move the global statement before the first use",
            "Python requires the global declaration to come before any use of the name in that scope.",
            "print(x) followed by global x is a SyntaxError.",
        ),
    ),
    (
        PythonRule::PLE1132,
        RuleText::new(
            "Keyword argument repeated through a ** dictionary",
            "drop one of the repeated keys",
            "Python raises TypeError when the same keyword arrives twice in a call.",
            "f(a=1, **{\"a\": 2}) fails with multiple values for a.",
        ),
    ),
    (
        PythonRule::PLE1205,
        RuleText::new(
            "Logging call with more arguments than placeholders",
            "add the missing placeholders or drop the extra arguments",
            "The logging module reports a formatting error instead of the message when there are arguments left over.",
            "logging.info(\"%s\", a, b) has one argument too many.",
        ),
    ),
    (
        PythonRule::PLE1206,
        RuleText::new(
            "Logging call with fewer arguments than placeholders",
            "pass one argument per placeholder",
            "The logging module reports a formatting error instead of the message when arguments are missing.",
            "logging.info(\"%s %s\", a) is missing an argument.",
        ),
    ),
    (
        PythonRule::PLE1310,
        RuleText::new(
            "strip called with repeated characters",
            "remove the repeated characters, or use removeprefix or removesuffix",
            "strip takes a set of characters, not a prefix, so repeating one changes nothing and usually hides a mistake.",
            "name.lstrip(\"abca\") strips a, b and c in any order; name.removeprefix(\"abca\") strips the text.",
        ),
    ),
    (
        PythonRule::PLE1507,
        RuleText::new(
            "os.getenv with a key that is not a string",
            "pass the variable name as a string",
            "os.getenv only accepts strings as the name; anything else raises TypeError.",
            "os.getenv(1) becomes os.getenv(\"1\").",
        ),
    ),
    (
        PythonRule::PLE1519,
        RuleText::new(
            "singledispatch on a method",
            "use functools.singledispatchmethod",
            "singledispatch dispatches on the first argument, which in a method is self, so it never looks at the real argument.",
            "@singledispatch on def method(self, arg) becomes @singledispatchmethod.",
        ),
    ),
    (
        PythonRule::PLE1520,
        RuleText::new(
            "singledispatchmethod on a plain function",
            "use functools.singledispatch",
            "singledispatchmethod skips the first argument as if it were self, so on a function it dispatches on the wrong one.",
            "@singledispatchmethod on def handle(arg) becomes @singledispatch.",
        ),
    ),
    (
        PythonRule::PLE1700,
        RuleText::new(
            "yield from inside an async function",
            "iterate with a for loop and yield each value",
            "Python does not allow yield from in an async function; the module does not even compile.",
            "async def f(): yield from items becomes for item in items: yield item.",
        ),
    ),
    (
        PythonRule::PLE2510,
        RuleText::new(
            "Literal backspace character in a string",
            "write it as \\b",
            "An invisible control character is easy to miss when reading the code and can break terminals.",
            "\"a<BS>b\" becomes \"a\\bb\".",
        ),
    ),
    (
        PythonRule::PLE2512,
        RuleText::new(
            "Literal SUB character in a string",
            "write it as \\x1a",
            "An invisible control character is easy to miss when reading the code, and some tools treat SUB as end of file.",
            "\"a<SUB>b\" becomes \"a\\x1ab\".",
        ),
    ),
    (
        PythonRule::PLE2513,
        RuleText::new(
            "Literal ESC character in a string",
            "write it as \\x1b",
            "An invisible escape character can change what a terminal shows when the code is printed.",
            "\"a<ESC>b\" becomes \"a\\x1bb\".",
        ),
    ),
    (
        PythonRule::PLE2514,
        RuleText::new(
            "Literal NUL character in a string",
            "write it as \\0",
            "A raw NUL byte in the source breaks many tools that read the file as text.",
            "\"a<NUL>b\" becomes \"a\\0b\".",
        ),
    ),
    (
        PythonRule::PLE2515,
        RuleText::new(
            "Literal zero-width space in a string",
            "write it as \\u200b",
            "A zero-width space is invisible, so two strings that look equal can differ.",
            "\"a<ZWSP>b\" becomes \"a\\u200bb\".",
        ),
    ),
    (
        PythonRule::PLW0604,
        RuleText::new(
            "global statement at module level",
            "delete it",
            "At module level every name is already global, so the statement does nothing.",
            "global counter at the top of a module can be removed.",
        ),
    ),
    (
        PythonRule::PLW1501,
        RuleText::new(
            "Invalid mode for open",
            "use a valid combination of r, w, x, a, b, t and +",
            "open raises ValueError for a mode with repeated letters, text and binary together, or more than one of r, w, x and a.",
            "open(path, \"rw\") becomes open(path, \"r+\").",
        ),
    ),
    (
        PythonRule::PLW1507,
        RuleText::new(
            "Shallow copy of os.environ",
            "use os.environ.copy()",
            "copy.copy(os.environ) keeps writing to the real environment, so changes to the copy leak into the process.",
            "env = copy.copy(os.environ) becomes env = os.environ.copy().",
        ),
    ),
    (
        PythonRule::PLW2101,
        RuleText::new(
            "New lock used as a with context",
            "create the lock once and share it",
            "A lock created right in the with statement is not shared with anything else, so it never blocks anyone.",
            "with threading.Lock(): becomes with self.lock: with the lock created in __init__.",
        ),
    ),
];
