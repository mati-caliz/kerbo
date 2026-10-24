use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas de stubs de flake8-pyi del conjunto por defecto de ruff 0.17.
pub const STUB_RULE_TEXTS: [(PythonRule, RuleText); 24] = [
    (
        PythonRule::PYI001,
        RuleText::new(
            "TypeVar in a stub without a leading underscore",
            "prefix the name with _",
            "A TypeVar in a stub is an implementation detail; without the underscore it looks like part of the public API.",
            "T = TypeVar(\"T\") becomes _T = TypeVar(\"_T\").",
        ),
    ),
    (
        PythonRule::PYI002,
        RuleText::new(
            "Complex if condition in a stub",
            "compare sys.version_info or sys.platform directly",
            "Type checkers only understand simple comparisons against sys.version_info and sys.platform in stubs.",
            "if sys.maxsize > 2**32: is not understood; if sys.platform == \"linux\": is.",
        ),
    ),
    (
        PythonRule::PYI003,
        RuleText::new(
            "Unrecognized sys.version_info check",
            "compare sys.version_info with a tuple of integers",
            "Type checkers only understand version checks against a tuple of integers, sys.version_info[:n] or sys.version_info[0].",
            "sys.version_info[1] >= 9 becomes sys.version_info >= (3, 9).",
        ),
    ),
    (
        PythonRule::PYI004,
        RuleText::new(
            "Version comparison against a patch version",
            "compare only the major and minor version",
            "Type checkers ignore the patch number, so the check does not do what it says.",
            "sys.version_info >= (3, 8, 3) becomes sys.version_info >= (3, 8).",
        ),
    ),
    (
        PythonRule::PYI005,
        RuleText::new(
            "Version comparison against a tuple of the wrong length",
            "compare against a tuple as long as the slice",
            "sys.version_info[:2] == (3,) is never true, because the two tuples always have different lengths.",
            "sys.version_info[:2] == (3,) becomes sys.version_info[:1] == (3,).",
        ),
    ),
    (
        PythonRule::PYI007,
        RuleText::new(
            "Unrecognized sys.platform check",
            "compare sys.platform with == or != against a string",
            "Type checkers only understand sys.platform compared by equality with a string literal.",
            "sys.platform.startswith(\"linux\") becomes sys.platform == \"linux\".",
        ),
    ),
    (
        PythonRule::PYI008,
        RuleText::new(
            "Unknown platform name in a sys.platform check",
            "use linux, win32, cygwin or darwin",
            "A misspelled platform name makes the branch dead without any warning from the type checker.",
            "sys.platform == \"linus\" becomes sys.platform == \"linux\".",
        ),
    ),
    (
        PythonRule::PYI009,
        RuleText::new(
            "pass as the whole body in a stub",
            "write ... instead",
            "The convention for an empty body in a stub is ..., which reads as left out on purpose.",
            "def f() -> None: pass becomes def f() -> None: ...",
        ),
    ),
    (
        PythonRule::PYI010,
        RuleText::new(
            "Function in a stub with a body",
            "replace the body with ...",
            "A stub only describes signatures; code in the body never runs and misleads the reader.",
            "def f() -> None: print(\"x\") becomes def f() -> None: ...",
        ),
    ),
    (
        PythonRule::PYI012,
        RuleText::new(
            "pass in a class body that has other statements",
            "delete the pass",
            "pass only makes sense when the class would otherwise be empty.",
            "class A: pass followed by x: int keeps only x: int.",
        ),
    ),
    (
        PythonRule::PYI013,
        RuleText::new(
            "... in a class body that has other statements",
            "delete the ...",
            "... marks an empty body; with other statements it adds nothing.",
            "class A: ... followed by x: int keeps only x: int.",
        ),
    ),
    (
        PythonRule::PYI015,
        RuleText::new(
            "Assignment in a stub with a value that is not simple",
            "replace the value with ...",
            "Stubs only keep simple values (numbers, strings, short containers); anything else is noise that may go stale.",
            "x: int = compute() becomes x: int = ...",
        ),
    ),
    (
        PythonRule::PYI017,
        RuleText::new(
            "Assignment in a stub with several targets or an attribute",
            "assign one name per statement",
            "Type checkers expect one simple name per assignment in a stub.",
            "a, b = 1, 2 becomes a: int and b: int.",
        ),
    ),
    (
        PythonRule::PYI026,
        RuleText::new(
            "Type alias in a stub without TypeAlias",
            "annotate it with TypeAlias",
            "Without the annotation the alias reads like a variable, and type checkers may treat it as one.",
            "Alias = int | None becomes Alias: TypeAlias = int | None.",
        ),
    ),
    (
        PythonRule::PYI033,
        RuleText::new(
            "Type comment in a stub",
            "use an annotation instead",
            "Type comments are the old syntax; in a stub annotations always work and are what tools expect.",
            "x = 1  # type: int becomes x: int.",
        ),
    ),
    (
        PythonRule::PYI035,
        RuleText::new(
            "__all__, __slots__ or __match_args__ without a value in a stub",
            "give it its value",
            "These names mean the same in a stub as at runtime, so tools need the actual value.",
            "__all__: list[str] becomes __all__ = [\"name\"].",
        ),
    ),
    (
        PythonRule::PYI043,
        RuleText::new(
            "Private type alias ending in T",
            "rename it without the T suffix",
            "The T suffix suggests a TypeVar, so readers mistake the alias for a type variable.",
            "_FooT: TypeAlias = int becomes _FooType: TypeAlias = int.",
        ),
    ),
    (
        PythonRule::PYI044,
        RuleText::new(
            "from __future__ import annotations in a stub",
            "delete the import",
            "Annotations in a stub are never evaluated, so the import changes nothing.",
            "from __future__ import annotations at the top of a .pyi can be removed.",
        ),
    ),
    (
        PythonRule::PYI045,
        RuleText::new(
            "__iter__ that returns Iterable instead of Iterator",
            "annotate the return as Iterator",
            "iter() needs an object with __next__; an Iterable return type tells the type checker that may be missing.",
            "def __iter__(self) -> Iterable[int] becomes def __iter__(self) -> Iterator[int].",
        ),
    ),
    (
        PythonRule::PYI047,
        RuleText::new(
            "Private type alias that is never used",
            "use it or delete it",
            "A private alias nobody refers to is dead code.",
            "_Unused: TypeAlias = int with no reference can be removed.",
        ),
    ),
    (
        PythonRule::PYI048,
        RuleText::new(
            "Function in a stub with several statements",
            "leave a single ... as the body",
            "A stub only describes signatures; the body should be one statement.",
            "def f(): x = 1; return x becomes def f(): ...",
        ),
    ),
    (
        PythonRule::PYI049,
        RuleText::new(
            "Private TypedDict that is never used",
            "use it or delete it",
            "A private TypedDict nobody refers to is dead code.",
            "class _Unused(TypedDict): key: int with no reference can be removed.",
        ),
    ),
    (
        PythonRule::PYI052,
        RuleText::new(
            "Assignment in a stub without a type annotation",
            "annotate the type",
            "In a stub the value does not matter, the type does; without an annotation the type checker has to guess it.",
            "x = \"text\" becomes x: str.",
        ),
    ),
    (
        PythonRule::PYI066,
        RuleText::new(
            "Branch for an older Python version before the newer one",
            "test sys.version_info >= ... first",
            "Putting the newest version first keeps the stub readable as old branches get removed.",
            "if sys.version_info < (3, 9): ... else: ... becomes if sys.version_info >= (3, 9): ... else: ...",
        ),
    ),
];
