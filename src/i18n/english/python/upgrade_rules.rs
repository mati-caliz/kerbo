use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas UP, RUF y YTT del conjunto por defecto de ruff 0.17 que no aparecen en el corpus.
pub const UPGRADE_RULE_TEXTS: [(PythonRule, RuleText); 22] = [
    (
        PythonRule::UP001,
        RuleText::new(
            "__metaclass__ = type that does nothing",
            "delete the assignment",
            "In Python 3 every class already uses type as its metaclass, and __metaclass__ is not read anymore.",
            "class A: __metaclass__ = type becomes class A: pass.",
        ),
    ),
    (
        PythonRule::UP023,
        RuleText::new(
            "Import of the deprecated xml.etree.cElementTree",
            "import xml.etree.ElementTree instead",
            "cElementTree was an alias of ElementTree and was removed in Python 3.9.",
            "import xml.etree.cElementTree as ET becomes import xml.etree.ElementTree as ET.",
        ),
    ),
    (
        PythonRule::UP026,
        RuleText::new(
            "Use of the mock package instead of unittest.mock",
            "import mock from unittest",
            "unittest.mock is part of the standard library, so the extra mock dependency is not needed.",
            "import mock becomes from unittest import mock.",
        ),
    ),
    (
        PythonRule::RUF008,
        RuleText::new(
            "Mutable default value in a dataclass field",
            "use field(default_factory=...)",
            "The default is created once and shared by every instance, so a change through one instance shows up in all of them.",
            "items: list[int] = [] becomes items: list[int] = field(default_factory=list).",
        ),
    ),
    (
        PythonRule::RUF016,
        RuleText::new(
            "Index of the wrong type on a literal list, tuple or string",
            "index with an integer or a slice",
            "Indexing a list, tuple or string with anything else raises TypeError.",
            "[1, 2][\"a\"] fails; [1, 2][0] works.",
        ),
    ),
    (
        PythonRule::RUF018,
        RuleText::new(
            "Assignment expression inside an assert",
            "assign the value before the assert",
            "Asserts are removed with python -O, so the variable is never assigned and later uses fail.",
            "assert (x := f()) followed by print(x) becomes x = f(); assert x.",
        ),
    ),
    (
        PythonRule::RUF024,
        RuleText::new(
            "Mutable value passed to dict.fromkeys",
            "use a dict comprehension",
            "dict.fromkeys uses the same object for every key, so changing one value changes all of them.",
            "dict.fromkeys(keys, []) becomes {key: [] for key in keys}.",
        ),
    ),
    (
        PythonRule::RUF026,
        RuleText::new(
            "default_factory passed by name to defaultdict",
            "pass the factory as the first positional argument",
            "defaultdict takes the factory only as a positional argument; by name it becomes a key of the dict.",
            "defaultdict(default_factory=list) becomes defaultdict(list).",
        ),
    ),
    (
        PythonRule::RUF030,
        RuleText::new(
            "print() as the message of an assert",
            "pass the message itself",
            "print runs and returns None, so the assertion error has no message.",
            "assert x, print(\"bad\") becomes assert x, \"bad\".",
        ),
    ),
    (
        PythonRule::RUF033,
        RuleText::new(
            "__post_init__ parameter with a default value",
            "declare it as an InitVar field with the default",
            "The dataclass always passes every InitVar to __post_init__, so the default is never used.",
            "def __post_init__(self, extra=1) becomes extra: InitVar[int] = 1 in the class.",
        ),
    ),
    (
        PythonRule::RUF048,
        RuleText::new(
            "__version__ parsed with map(int, ...)",
            "use packaging.version or compare strings carefully",
            "A version like 1.2.3rc1 has parts that are not integers, and int() raises ValueError on them.",
            "map(int, __version__.split(\".\")) becomes packaging.version.parse(__version__).",
        ),
    ),
    (
        PythonRule::RUF049,
        RuleText::new(
            "Enum decorated with @dataclass",
            "remove the decorator",
            "Enum members are not instances built from fields, so the dataclass machinery breaks the enum.",
            "@dataclass on class Color(Enum) can be removed.",
        ),
    ),
    (
        PythonRule::RUF058,
        RuleText::new(
            "itertools.starmap over a zip",
            "use map with the iterables directly",
            "map already takes several iterables in parallel, so zipping and unpacking them again is extra work.",
            "starmap(pow, zip(a, b)) becomes map(pow, a, b).",
        ),
    ),
    (
        PythonRule::RUF101,
        RuleText::new(
            "Old rule code in a noqa",
            "use the current code of the rule",
            "The old code still works through a redirect, but it hides which rule is really silenced.",
            "# noqa: TCH001 becomes # noqa: TC001.",
        ),
    ),
    (
        PythonRule::YTT101,
        RuleText::new(
            "sys.version[:3] used as the version",
            "use sys.version_info",
            "From Python 3.10 on, the first three characters are 3.1, not the real version.",
            "sys.version[:3] becomes \"{}.{}\".format(*sys.version_info[:2]).",
        ),
    ),
    (
        PythonRule::YTT102,
        RuleText::new(
            "sys.version[2] used as the minor version",
            "use sys.version_info.minor",
            "From Python 3.10 on, the third character is only the first digit of the minor version.",
            "sys.version[2] becomes sys.version_info.minor.",
        ),
    ),
    (
        PythonRule::YTT103,
        RuleText::new(
            "sys.version compared with a string",
            "compare sys.version_info with a tuple",
            "Comparing strings compares letter by letter, so \"3.10\" is smaller than \"3.9\".",
            "sys.version >= \"3.5\" becomes sys.version_info >= (3, 5).",
        ),
    ),
    (
        PythonRule::YTT202,
        RuleText::new(
            "six.PY3 used as a version check",
            "use not six.PY2",
            "six.PY3 is only true for Python 3, so it breaks on a future Python 4.",
            "if six.PY3: becomes if not six.PY2:.",
        ),
    ),
    (
        PythonRule::YTT204,
        RuleText::new(
            "sys.version_info.minor compared with an integer",
            "compare sys.version_info with a tuple",
            "Comparing only the minor version breaks when the major version changes.",
            "sys.version_info.minor > 7 becomes sys.version_info > (3, 7).",
        ),
    ),
    (
        PythonRule::YTT301,
        RuleText::new(
            "sys.version[0] used as the major version",
            "use sys.version_info.major",
            "With a two-digit major version, the first character is no longer the whole major version.",
            "sys.version[0] becomes sys.version_info.major.",
        ),
    ),
    (
        PythonRule::YTT302,
        RuleText::new(
            "sys.version compared with a one-character string",
            "compare sys.version_info with a tuple",
            "Comparing strings compares letter by letter, so the check breaks with a two-digit major version.",
            "sys.version >= \"3\" becomes sys.version_info >= (3,).",
        ),
    ),
    (
        PythonRule::YTT303,
        RuleText::new(
            "sys.version[:1] used as the major version",
            "use sys.version_info.major",
            "With a two-digit major version, the first character is no longer the whole major version.",
            "sys.version[:1] becomes str(sys.version_info.major).",
        ),
    ),
];
