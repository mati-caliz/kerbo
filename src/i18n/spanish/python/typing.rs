use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: anotaciones, tipos y stubs.
pub const TYPING_TEXTS: [(PythonRule, RuleText); 20] = [
    (
        PythonRule::UP037,
        RuleText::new(
            "Anotación entre comillas que nunca se evalúa",
            "sacá las comillas",
            "Python no evalúa esta anotación al ejecutar (con from __future__ import annotations, en un stub o en una variable local), así que las comillas sólo agregan ruido.",
            "def f(x: \"Node\"): con el import del futuro pasa a def f(x: Node):.",
        ),
    ),
    (
        PythonRule::UP044,
        RuleText::new(
            "Unpack[] donde alcanza con *",
            "escribí *X en vez de Unpack[X]",
            "Desde Python 3.11 (PEP 646) un tipo variádico se desempaqueta con *, que es más corto y se lee como el resto del lenguaje.",
            "def f(*args: Unpack[Ts]): pasa a def f(*args: *Ts):.",
        ),
    ),
    (
        PythonRule::RUF013,
        RuleText::new(
            "Optional implícito",
            "sumá None al tipo: X | None",
            "Un parámetro que por defecto vale None tiene que decirlo en el tipo; PEP 484 prohíbe el Optional implícito.",
            "def f(x: int = None) pasa a def f(x: int | None = None).",
        ),
    ),
    (
        PythonRule::PYI016,
        RuleText::new(
            "Miembro repetido en una unión",
            "sacá el tipo repetido",
            "Una unión guarda cada tipo una sola vez, así que la repetición no suma nada y suele esconder un error de tipeo.",
            "int | str | int pasa a int | str.",
        ),
    ),
    (
        PythonRule::PYI030,
        RuleText::new(
            "Varios Literal en una unión",
            "juntalos en un solo Literal",
            "Literal acepta varios valores, así que un solo Literal dice lo mismo y se lee mejor.",
            "Literal[1] | Literal[2] pasa a Literal[1, 2].",
        ),
    ),
    (
        PythonRule::PYI061,
        RuleText::new(
            "Literal[None] de más",
            "usá None en vez de Literal[None]",
            "None ya es un tipo con un solo valor, así que envolverlo en Literal sólo agrega ruido.",
            "Literal[1, None] pasa a Literal[1] | None.",
        ),
    ),
    (
        PythonRule::PYI062,
        RuleText::new(
            "Miembro repetido en un Literal",
            "sacá el valor repetido",
            "Un Literal guarda cada valor una sola vez, así que la repetición no suma nada y suele esconder un error de tipeo.",
            "Literal[1, 2, 1] pasa a Literal[1, 2].",
        ),
    ),
    (
        PythonRule::PYI064,
        RuleText::new(
            "Final[Literal[...]] donde alcanza con Final",
            "escribí sólo Final",
            "Una variable Final asignada con un literal ya tiene ese literal como tipo.",
            "x: Final[Literal[1]] = 1 pasa a x: Final = 1.",
        ),
    ),
    (
        PythonRule::PYI057,
        RuleText::new(
            "Uso de ByteString",
            "usá collections.abc.Buffer o bytes | bytearray",
            "ByteString está obsoleto desde Python 3.9 y desaparece en 3.14; nunca quedó claro qué significaba.",
            "def f(data: ByteString) pasa a def f(data: Buffer).",
        ),
    ),
    (
        PythonRule::PYI059,
        RuleText::new(
            "Generic[] no es la última base",
            "pasá Generic[...] al final de las bases",
            "Generic tiene que ser la última base para que el orden de resolución de métodos sea consistente; si no, crear la clase puede fallar.",
            "class A(Generic[T], Base) pasa a class A(Base, Generic[T]).",
        ),
    ),
    (
        PythonRule::PYI050,
        RuleText::new(
            "NoReturn en la anotación de un parámetro",
            "usá Never para el parámetro",
            "NoReturn es para tipos de retorno; para un parámetro que nunca puede recibir un valor, Never lo dice claro.",
            "def f(x: NoReturn) pasa a def f(x: Never).",
        ),
    ),
    (
        PythonRule::PYI041,
        RuleText::new(
            "Unión numérica redundante",
            "dejá sólo el tipo numérico más amplio",
            "Para un chequeador de tipos un int vale donde se pide un float y un float donde se pide un complex, así que el tipo más chico no suma nada.",
            "def f(x: int | float) pasa a def f(x: float).",
        ),
    ),
    (
        PythonRule::PYI032,
        RuleText::new(
            "Any como el otro operando de __eq__ o __ne__",
            "anotá el parámetro como object",
            "object acepta cualquier cosa igual que Any pero mantiene chequeada la comparación.",
            "def __eq__(self, other: Any) pasa a def __eq__(self, other: object).",
        ),
    ),
    (
        PythonRule::PYI063,
        RuleText::new(
            "Parámetro sólo posicional al estilo de PEP 484",
            "usá / en vez del doble guion bajo",
            "Desde Python 3.8 los parámetros sólo posicionales se declaran con /, y el lenguaje lo hace cumplir.",
            "def f(__x: int) pasa a def f(x: int, /).",
        ),
    ),
    (
        PythonRule::PYI058,
        RuleText::new(
            "__iter__ anotado como si devolviera Generator",
            "anotalo como si devolviera Iterator",
            "Quien lo llama sólo recorre el resultado, así que Iterator describe el contrato sin prometer send() ni un valor de retorno.",
            "def __iter__(self) -> Generator[int, None, None] pasa a def __iter__(self) -> Iterator[int].",
        ),
    ),
    (
        PythonRule::PYI029,
        RuleText::new(
            "__str__ o __repr__ repetido en un stub",
            "sacá el método del stub",
            "La firma es la misma que se hereda de object, así que declararla de nuevo no suma nada.",
            "def __repr__(self) -> str: ... se borra.",
        ),
    ),
    (
        PythonRule::PYI018,
        RuleText::new(
            "TypeVar privado sin usar",
            "borrá la declaración o usala",
            "Nada en el módulo usa este TypeVar, ParamSpec o TypeVarTuple privado, así que es código que quedó.",
            "_T = TypeVar(\"_T\") sin usos se borra.",
        ),
    ),
    (
        PythonRule::PYI046,
        RuleText::new(
            "Protocolo privado sin usar",
            "borrá el protocolo o usalo",
            "Nada en el módulo usa este protocolo privado, así que es código que quedó.",
            "class _Runner(Protocol): ... sin usos se borra.",
        ),
    ),
    (
        PythonRule::PYI042,
        RuleText::new(
            "Alias de tipo en snake_case",
            "nombrá el alias en CamelCase",
            "Los alias de tipo nombran tipos, y los tipos se escriben en CamelCase para no confundirlos con valores.",
            "int_list: TypeAlias = list[int] pasa a IntList: TypeAlias = list[int].",
        ),
    ),
    (
        PythonRule::PYI025,
        RuleText::new(
            "collections.abc.Set importado sin el alias AbstractSet",
            "importalo como AbstractSet",
            "Set se confunde fácil con el builtin set; el alias AbstractSet lo deja claro.",
            "from collections.abc import Set pasa a from collections.abc import Set as AbstractSet.",
        ),
    ),
];
