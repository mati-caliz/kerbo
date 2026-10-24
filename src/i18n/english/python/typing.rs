use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: anotaciones, tipos y stubs.
pub const TYPING_TEXTS: [(PythonRule, RuleText); 20] = [
    (
        PythonRule::UP037,
        RuleText::new(
            "Quoted annotation that is never evaluated",
            "remove the quotes",
            "Python does not evaluate this annotation at runtime (from __future__ import annotations, a stub or a local variable), so the quotes only add noise.",
            "def f(x: \"Node\"): with the future import becomes def f(x: Node):.",
        ),
    ),
    (
        PythonRule::UP044,
        RuleText::new(
            "Unpack[] where * is enough",
            "write *X instead of Unpack[X]",
            "Since Python 3.11 (PEP 646) a variadic type is unpacked with *, which is shorter and reads like the rest of the language.",
            "def f(*args: Unpack[Ts]): becomes def f(*args: *Ts):.",
        ),
    ),
    (
        PythonRule::RUF013,
        RuleText::new(
            "Implicit Optional",
            "add None to the type: X | None",
            "A parameter that defaults to None must say so in its type; PEP 484 forbids the implicit Optional.",
            "def f(x: int = None) becomes def f(x: int | None = None).",
        ),
    ),
    (
        PythonRule::PYI016,
        RuleText::new(
            "Repeated member in a union",
            "remove the repeated type",
            "A union keeps each type once, so the repetition adds nothing and usually hides a typo.",
            "int | str | int becomes int | str.",
        ),
    ),
    (
        PythonRule::PYI030,
        RuleText::new(
            "Several Literal types in a union",
            "merge them into a single Literal",
            "Literal accepts several values, so one Literal says the same and reads better.",
            "Literal[1] | Literal[2] becomes Literal[1, 2].",
        ),
    ),
    (
        PythonRule::PYI061,
        RuleText::new(
            "Redundant Literal[None]",
            "use None instead of Literal[None]",
            "None is already a type with a single value, so wrapping it in Literal only adds noise.",
            "Literal[1, None] becomes Literal[1] | None.",
        ),
    ),
    (
        PythonRule::PYI062,
        RuleText::new(
            "Repeated member in a Literal",
            "remove the repeated value",
            "A Literal keeps each value once, so the repetition adds nothing and usually hides a typo.",
            "Literal[1, 2, 1] becomes Literal[1, 2].",
        ),
    ),
    (
        PythonRule::PYI064,
        RuleText::new(
            "Final[Literal[...]] where Final is enough",
            "write only Final",
            "A Final variable assigned a literal already has that literal as its type.",
            "x: Final[Literal[1]] = 1 becomes x: Final = 1.",
        ),
    ),
    (
        PythonRule::PYI057,
        RuleText::new(
            "Use of ByteString",
            "use collections.abc.Buffer or bytes | bytearray",
            "ByteString is deprecated since Python 3.9 and is removed in 3.14; its meaning was never clear.",
            "def f(data: ByteString) becomes def f(data: Buffer).",
        ),
    ),
    (
        PythonRule::PYI059,
        RuleText::new(
            "Generic[] is not the last base class",
            "move Generic[...] to the end of the bases",
            "Generic must be the last base so the method resolution order stays consistent; otherwise class creation can fail.",
            "class A(Generic[T], Base) becomes class A(Base, Generic[T]).",
        ),
    ),
    (
        PythonRule::PYI050,
        RuleText::new(
            "NoReturn in a parameter annotation",
            "use Never for the parameter",
            "NoReturn is meant for return types; for a parameter that can never receive a value, Never says it clearly.",
            "def f(x: NoReturn) becomes def f(x: Never).",
        ),
    ),
    (
        PythonRule::PYI041,
        RuleText::new(
            "Redundant numeric union",
            "keep only the widest numeric type",
            "For type checkers an int is accepted where a float is expected and a float where a complex is, so the narrower type adds nothing.",
            "def f(x: int | float) becomes def f(x: float).",
        ),
    ),
    (
        PythonRule::PYI032,
        RuleText::new(
            "Any as the other operand of __eq__ or __ne__",
            "annotate the parameter as object",
            "object accepts anything just like Any but keeps the comparison type checked.",
            "def __eq__(self, other: Any) becomes def __eq__(self, other: object).",
        ),
    ),
    (
        PythonRule::PYI063,
        RuleText::new(
            "PEP 484 style positional-only parameter",
            "use / instead of the double underscore",
            "Since Python 3.8 positional-only parameters are declared with /, which the language enforces.",
            "def f(__x: int) becomes def f(x: int, /).",
        ),
    ),
    (
        PythonRule::PYI058,
        RuleText::new(
            "__iter__ annotated as returning Generator",
            "annotate it as returning Iterator",
            "Callers only iterate the result, so Iterator states the contract without promising send() or a return value.",
            "def __iter__(self) -> Generator[int, None, None] becomes def __iter__(self) -> Iterator[int].",
        ),
    ),
    (
        PythonRule::PYI029,
        RuleText::new(
            "__str__ or __repr__ repeated in a stub",
            "remove the method from the stub",
            "The signature is the same as the one inherited from object, so declaring it again adds nothing.",
            "def __repr__(self) -> str: ... is removed.",
        ),
    ),
    (
        PythonRule::PYI018,
        RuleText::new(
            "Unused private TypeVar",
            "remove the declaration or use it",
            "Nothing in the module uses this private TypeVar, ParamSpec or TypeVarTuple, so it is leftover code.",
            "_T = TypeVar(\"_T\") without uses is removed.",
        ),
    ),
    (
        PythonRule::PYI046,
        RuleText::new(
            "Unused private Protocol",
            "remove the protocol or use it",
            "Nothing in the module uses this private protocol, so it is leftover code.",
            "class _Runner(Protocol): ... without uses is removed.",
        ),
    ),
    (
        PythonRule::PYI042,
        RuleText::new(
            "Type alias in snake_case",
            "name the alias in CamelCase",
            "Type aliases name types, and types are written in CamelCase so they are not mistaken for values.",
            "int_list: TypeAlias = list[int] becomes IntList: TypeAlias = list[int].",
        ),
    ),
    (
        PythonRule::PYI025,
        RuleText::new(
            "collections.abc.Set imported without the AbstractSet alias",
            "import it as AbstractSet",
            "Set is easily confused with the builtin set; the AbstractSet alias makes it clear.",
            "from collections.abc import Set becomes from collections.abc import Set as AbstractSet.",
        ),
    ),
];
