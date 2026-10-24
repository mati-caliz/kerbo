use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de la tanda de reglas sueltas de ruff 0.17: formatos, archivos, versiones y __all__.
pub const ASSORTED_CHECK_TEXTS: [(PythonRule, RuleText); 27] = [
    (
        PythonRule::FURB188,
        RuleText::new(
            "Slice that removes a prefix or suffix after checking it",
            "use removeprefix or removesuffix",
            "str.removeprefix and str.removesuffix do the check and the slice in one call, without repeating the length.",
            "s = s[4:] if s.startswith(\"pre_\") else s becomes s = s.removeprefix(\"pre_\").",
        ),
    ),
    (
        PythonRule::SIM401,
        RuleText::new(
            "if/else block that looks up a dict key with a default",
            "use dict.get with the default",
            "dict.get returns the default when the key is missing, so the whole block is one assignment.",
            "if k in d: x = d[k] else: x = 0 becomes x = d.get(k, 0).",
        ),
    ),
    (
        PythonRule::FURB122,
        RuleText::new(
            "File write inside a for loop",
            "use writelines",
            "writelines writes every element in one call, without a Python-level loop.",
            "for line in lines: f.write(line) becomes f.writelines(lines).",
        ),
    ),
    (
        PythonRule::PLE1300,
        RuleText::new(
            "Unsupported format character",
            "use a format type that exists",
            "Python raises ValueError at runtime when it meets a format type it does not know.",
            "\"{:z}\".format(x) fails; \"{:d}\".format(x) works.",
        ),
    ),
    (
        PythonRule::PLE1307,
        RuleText::new(
            "Format type does not match the argument type",
            "match the conversion to the value",
            "A %d with a string argument raises TypeError when the line runs.",
            "\"%d\" % \"1\" fails; \"%d\" % 1 or \"%s\" % \"1\" work.",
        ),
    ),
    (
        PythonRule::RUF063,
        RuleText::new(
            "Annotations read from the class __dict__",
            "use inspect.get_annotations",
            "Since Python 3.14 __dict__ only has __annotations__ when the class uses the annotations future import; the inspect function works in every case.",
            "cls.__dict__.get(\"__annotations__\") becomes inspect.get_annotations(cls).",
        ),
    ),
    (
        PythonRule::RUF068,
        RuleText::new(
            "Duplicate entry in __all__",
            "remove the repeated name",
            "A repeated name adds nothing and usually comes from a copy and paste or a half-done refactor.",
            "__all__ = [\"a\", \"b\", \"a\"] becomes __all__ = [\"a\", \"b\"].",
        ),
    ),
    (
        PythonRule::RUF009,
        RuleText::new(
            "Function call in a dataclass default",
            "use field(default_factory=...)",
            "The call runs once when the class is defined, so every instance shares the same object.",
            "items: list = make_list() becomes items: list = field(default_factory=make_list).",
        ),
    ),
    (
        PythonRule::ASYNC100,
        RuleText::new(
            "Timeout context without any await",
            "add a checkpoint or drop the timeout",
            "A timeout can only fire at an await, so a block without one never gets cancelled.",
            "with asyncio.timeout(1): do_work() needs an await inside to be useful.",
        ),
    ),
    (
        PythonRule::PLE2502,
        RuleText::new(
            "Bidirectional control character",
            "remove the bidirectional character",
            "Right-to-left marks can make code look different from what runs, which hides bugs or attacks.",
            "A RIGHT-TO-LEFT MARK inside a string can make part of the line look like a comment.",
        ),
    ),
    (
        PythonRule::PTH124,
        RuleText::new(
            "py.path is in maintenance mode",
            "use pathlib",
            "py.path no longer gets new features; pathlib is in the standard library.",
            "py.path.local(\"/foo\") becomes pathlib.Path(\"/foo\").",
        ),
    ),
    (
        PythonRule::FURB129,
        RuleText::new(
            "readlines() in a for loop",
            "iterate over the file directly",
            "Iterating the file reads one line at a time instead of loading them all in a list first.",
            "for line in f.readlines(): becomes for line in f:.",
        ),
    ),
    (
        PythonRule::ASYNC222,
        RuleText::new(
            "Blocking wait for a process in an async function",
            "wait for the process asynchronously",
            "os.waitpid and friends block the whole event loop until the process ends.",
            "os.waitpid(pid, 0) inside async def blocks every task; use asyncio subprocesses or a thread.",
        ),
    ),
    (
        PythonRule::W605,
        RuleText::new(
            "Invalid escape sequence",
            "use a raw string or double the backslash",
            "Python warns about unknown escapes and will turn them into errors.",
            "\"\\.png\" becomes r\"\\.png\".",
        ),
    ),
    (
        PythonRule::UP049,
        RuleText::new(
            "Private type parameter name",
            "drop the leading underscore",
            "PEP 695 type parameters only exist inside their class or function, so the underscore adds no privacy.",
            "class Box[_T]: becomes class Box[T]:.",
        ),
    ),
    (
        PythonRule::PYI006,
        RuleText::new(
            "sys.version_info compared with an operator other than < or >=",
            "compare with < or >=",
            "Other operators behave in surprising ways with micro versions: (3, 8, 10) <= (3, 8) is false.",
            "sys.version_info > (3, 8) becomes sys.version_info >= (3, 9).",
        ),
    ),
    (
        PythonRule::UP029,
        RuleText::new(
            "Unnecessary import of a builtin",
            "remove the import",
            "Imports from builtins, six or io only made sense for Python 2 compatibility; the builtin is already there.",
            "from builtins import zip can be removed.",
        ),
    ),
    (
        PythonRule::PLE0643,
        RuleText::new(
            "Index out of range on a literal sequence",
            "use an index inside the sequence",
            "The literal has fewer elements than the index, so the line raises IndexError.",
            "[0, 1, 2][3] always fails.",
        ),
    ),
    (
        PythonRule::PLW0406,
        RuleText::new(
            "Module imports itself",
            "remove the self import",
            "Importing the current module is a circular import and can fail with ImportError.",
            "import pkg.mod inside pkg/mod.py.",
        ),
    ),
    (
        PythonRule::INT002,
        RuleText::new(
            "format() inside a gettext call",
            "format the translated string instead",
            "The string is formatted before the lookup, so the catalog searches for the filled-in text and finds nothing.",
            "_(\"Hi {}\".format(name)) becomes _(\"Hi {}\").format(name).",
        ),
    ),
    (
        PythonRule::EXE005,
        RuleText::new(
            "Shebang that is not on the first line",
            "move the shebang to the top of the file",
            "The system only reads #! when they are the first two characters of the file; anywhere else it is a plain comment.",
            "x = 1 followed by #!/usr/bin/env python does nothing.",
        ),
    ),
    (
        PythonRule::PLE0604,
        RuleText::new(
            "Invalid object in __all__",
            "list only strings in __all__",
            "__all__ must contain names as strings; anything else breaks from module import *.",
            "__all__ = [Foo, 1] becomes __all__ = [\"Foo\"].",
        ),
    ),
    (
        PythonRule::PYI020,
        RuleText::new(
            "Quoted annotation in a stub",
            "remove the quotes",
            "Stubs never run, so forward references work without quotes.",
            "def f() -> \"int\": ... becomes def f() -> int: ....",
        ),
    ),
    (
        PythonRule::PLE1142,
        RuleText::new(
            "await outside an async function",
            "make the function async",
            "await is only valid inside async def; anywhere else it is a syntax error.",
            "def f(): await g() becomes async def f(): await g().",
        ),
    ),
    (
        PythonRule::TC005,
        RuleText::new(
            "Empty TYPE_CHECKING block",
            "remove the block",
            "An if TYPE_CHECKING: pass does nothing and only adds noise.",
            "if TYPE_CHECKING: pass can be deleted.",
        ),
    ),
    (
        PythonRule::UP036,
        RuleText::new(
            "Version block for an unsupported Python version",
            "remove the outdated branch",
            "The branch depends on a Python version older than the minimum supported one, so it is dead code or always taken.",
            "if sys.version_info < (3, 8): ... can go away.",
        ),
    ),
    (
        PythonRule::SIM113,
        RuleText::new(
            "Manual index counter in a for loop",
            "use enumerate",
            "enumerate keeps the index next to each element without a separate counter to maintain.",
            "i = 0; for x in xs: ...; i += 1 becomes for i, x in enumerate(xs):.",
        ),
    ),
];
