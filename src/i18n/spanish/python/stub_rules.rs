use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas de stubs de flake8-pyi del conjunto por defecto de ruff 0.17.
pub const STUB_RULE_TEXTS: [(PythonRule, RuleText); 24] = [
    (
        PythonRule::PYI001,
        RuleText::new(
            "TypeVar de un stub sin guion bajo adelante",
            "agregale un _ al nombre",
            "Un TypeVar en un stub es un detalle interno; sin el guion bajo parece parte de la API pública.",
            "T = TypeVar(\"T\") pasa a _T = TypeVar(\"_T\").",
        ),
    ),
    (
        PythonRule::PYI002,
        RuleText::new(
            "Condición compleja en un if de un stub",
            "compará directamente sys.version_info o sys.platform",
            "En un stub los chequeadores de tipos sólo entienden comparaciones simples con sys.version_info y sys.platform.",
            "if sys.maxsize > 2**32: no se entiende; if sys.platform == \"linux\": sí.",
        ),
    ),
    (
        PythonRule::PYI003,
        RuleText::new(
            "Chequeo de sys.version_info que no se reconoce",
            "compará sys.version_info con una tupla de enteros",
            "Los chequeadores de tipos sólo entienden versiones comparadas con una tupla de enteros, sys.version_info[:n] o sys.version_info[0].",
            "sys.version_info[1] >= 9 pasa a sys.version_info >= (3, 9).",
        ),
    ),
    (
        PythonRule::PYI004,
        RuleText::new(
            "Comparación de versión contra un número de parche",
            "compará sólo la versión mayor y menor",
            "Los chequeadores de tipos ignoran el número de parche, así que el chequeo no hace lo que dice.",
            "sys.version_info >= (3, 8, 3) pasa a sys.version_info >= (3, 8).",
        ),
    ),
    (
        PythonRule::PYI005,
        RuleText::new(
            "Comparación de versión contra una tupla del largo equivocado",
            "compará contra una tupla del mismo largo que el corte",
            "sys.version_info[:2] == (3,) nunca es verdadero, porque las dos tuplas siempre tienen largos distintos.",
            "sys.version_info[:2] == (3,) pasa a sys.version_info[:1] == (3,).",
        ),
    ),
    (
        PythonRule::PYI007,
        RuleText::new(
            "Chequeo de sys.platform que no se reconoce",
            "compará sys.platform con == o != contra un texto",
            "Los chequeadores de tipos sólo entienden sys.platform comparado por igualdad con un texto literal.",
            "sys.platform.startswith(\"linux\") pasa a sys.platform == \"linux\".",
        ),
    ),
    (
        PythonRule::PYI008,
        RuleText::new(
            "Nombre de plataforma desconocido en un chequeo de sys.platform",
            "usá linux, win32, cygwin o darwin",
            "Un nombre de plataforma mal escrito deja la rama muerta sin que el chequeador de tipos avise.",
            "sys.platform == \"linus\" pasa a sys.platform == \"linux\".",
        ),
    ),
    (
        PythonRule::PYI009,
        RuleText::new(
            "pass como cuerpo entero en un stub",
            "escribí ... en su lugar",
            "La convención para un cuerpo vacío en un stub es ..., que se lee como omitido a propósito.",
            "def f() -> None: pass pasa a def f() -> None: ...",
        ),
    ),
    (
        PythonRule::PYI010,
        RuleText::new(
            "Función de un stub con cuerpo",
            "reemplazá el cuerpo por ...",
            "Un stub sólo describe firmas; el código del cuerpo nunca corre y confunde al que lee.",
            "def f() -> None: print(\"x\") pasa a def f() -> None: ...",
        ),
    ),
    (
        PythonRule::PYI012,
        RuleText::new(
            "pass en el cuerpo de una clase que tiene otras sentencias",
            "borrá el pass",
            "pass sólo tiene sentido cuando la clase quedaría vacía.",
            "class A: pass seguido de x: int deja sólo x: int.",
        ),
    ),
    (
        PythonRule::PYI013,
        RuleText::new(
            "... en el cuerpo de una clase que tiene otras sentencias",
            "borrá el ...",
            "... marca un cuerpo vacío; con otras sentencias no suma nada.",
            "class A: ... seguido de x: int deja sólo x: int.",
        ),
    ),
    (
        PythonRule::PYI015,
        RuleText::new(
            "Asignación de un stub con un valor que no es simple",
            "reemplazá el valor por ...",
            "Los stubs sólo guardan valores simples (números, textos, contenedores cortos); cualquier otra cosa es ruido que puede quedar viejo.",
            "x: int = compute() pasa a x: int = ...",
        ),
    ),
    (
        PythonRule::PYI017,
        RuleText::new(
            "Asignación de un stub con varios destinos o un atributo",
            "asigná un solo nombre por sentencia",
            "En un stub los chequeadores de tipos esperan un solo nombre simple por asignación.",
            "a, b = 1, 2 pasa a a: int y b: int.",
        ),
    ),
    (
        PythonRule::PYI026,
        RuleText::new(
            "Alias de tipo en un stub sin TypeAlias",
            "anotalo con TypeAlias",
            "Sin la anotación el alias se lee como una variable, y los chequeadores de tipos pueden tratarlo como tal.",
            "Alias = int | None pasa a Alias: TypeAlias = int | None.",
        ),
    ),
    (
        PythonRule::PYI033,
        RuleText::new(
            "Comentario de tipo en un stub",
            "usá una anotación",
            "Los comentarios de tipo son la sintaxis vieja; en un stub las anotaciones siempre funcionan y son lo que esperan las herramientas.",
            "x = 1  # type: int pasa a x: int.",
        ),
    ),
    (
        PythonRule::PYI035,
        RuleText::new(
            "__all__, __slots__ o __match_args__ sin valor en un stub",
            "dale su valor",
            "Estos nombres significan lo mismo en un stub que en ejecución, así que las herramientas necesitan el valor real.",
            "__all__: list[str] pasa a __all__ = [\"name\"].",
        ),
    ),
    (
        PythonRule::PYI043,
        RuleText::new(
            "Alias de tipo privado terminado en T",
            "renombralo sin la T del final",
            "La T del final hace pensar en un TypeVar, así que el alias se confunde con una variable de tipo.",
            "_FooT: TypeAlias = int pasa a _FooType: TypeAlias = int.",
        ),
    ),
    (
        PythonRule::PYI044,
        RuleText::new(
            "from __future__ import annotations en un stub",
            "borrá el import",
            "Las anotaciones de un stub nunca se evalúan, así que el import no cambia nada.",
            "from __future__ import annotations arriba de un .pyi se puede borrar.",
        ),
    ),
    (
        PythonRule::PYI045,
        RuleText::new(
            "__iter__ que devuelve Iterable en vez de Iterator",
            "anotá el retorno como Iterator",
            "iter() necesita un objeto con __next__; devolver Iterable le dice al chequeador que puede faltar.",
            "def __iter__(self) -> Iterable[int] pasa a def __iter__(self) -> Iterator[int].",
        ),
    ),
    (
        PythonRule::PYI047,
        RuleText::new(
            "Alias de tipo privado que no se usa",
            "usalo o borralo",
            "Un alias privado al que nadie hace referencia es código muerto.",
            "_Unused: TypeAlias = int sin ninguna referencia se puede borrar.",
        ),
    ),
    (
        PythonRule::PYI048,
        RuleText::new(
            "Función de un stub con varias sentencias",
            "dejá un solo ... como cuerpo",
            "Un stub sólo describe firmas; el cuerpo tiene que ser una sola sentencia.",
            "def f(): x = 1; return x pasa a def f(): ...",
        ),
    ),
    (
        PythonRule::PYI049,
        RuleText::new(
            "TypedDict privado que no se usa",
            "usalo o borralo",
            "Un TypedDict privado al que nadie hace referencia es código muerto.",
            "class _Unused(TypedDict): key: int sin ninguna referencia se puede borrar.",
        ),
    ),
    (
        PythonRule::PYI052,
        RuleText::new(
            "Asignación de un stub sin anotación de tipo",
            "anotá el tipo",
            "En un stub el valor no importa, el tipo sí; sin anotación el chequeador de tipos lo tiene que adivinar.",
            "x = \"text\" pasa a x: str.",
        ),
    ),
    (
        PythonRule::PYI066,
        RuleText::new(
            "Rama para una versión vieja de Python antes que la nueva",
            "chequeá primero sys.version_info >= ...",
            "Poner primero la versión más nueva deja el stub legible a medida que se borran las ramas viejas.",
            "if sys.version_info < (3, 9): ... else: ... pasa a if sys.version_info >= (3, 9): ... else: ...",
        ),
    ),
];
