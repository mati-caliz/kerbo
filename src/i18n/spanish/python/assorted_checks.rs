use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de la tanda de reglas sueltas de ruff 0.17: formatos, archivos, versiones y __all__.
pub const ASSORTED_CHECK_TEXTS: [(PythonRule, RuleText); 27] = [
    (
        PythonRule::FURB188,
        RuleText::new(
            "Corte que saca un prefijo o sufijo después de chequearlo",
            "usá removeprefix o removesuffix",
            "str.removeprefix y str.removesuffix hacen el chequeo y el corte en una llamada, sin repetir el largo.",
            "s = s[4:] if s.startswith(\"pre_\") else s pasa a s = s.removeprefix(\"pre_\").",
        ),
    ),
    (
        PythonRule::SIM401,
        RuleText::new(
            "Bloque if/else que busca una clave de un diccionario con un default",
            "usá dict.get con el default",
            "dict.get devuelve el default si la clave no está, así que el bloque entero es una asignación.",
            "if k in d: x = d[k] else: x = 0 pasa a x = d.get(k, 0).",
        ),
    ),
    (
        PythonRule::FURB122,
        RuleText::new(
            "Escritura en un archivo adentro de un loop for",
            "usá writelines",
            "writelines escribe todos los elementos en una llamada, sin un loop en Python.",
            "for line in lines: f.write(line) pasa a f.writelines(lines).",
        ),
    ),
    (
        PythonRule::PLE1300,
        RuleText::new(
            "Carácter de formato que no existe",
            "usá un tipo de formato válido",
            "Python tira ValueError en tiempo de ejecución cuando encuentra un tipo de formato que no conoce.",
            "\"{:z}\".format(x) falla; \"{:d}\".format(x) anda.",
        ),
    ),
    (
        PythonRule::PLE1307,
        RuleText::new(
            "El tipo del formato no coincide con el del argumento",
            "usá la conversión que corresponde al valor",
            "Un %d con un string como argumento tira TypeError cuando se ejecuta la línea.",
            "\"%d\" % \"1\" falla; \"%d\" % 1 o \"%s\" % \"1\" andan.",
        ),
    ),
    (
        PythonRule::RUF063,
        RuleText::new(
            "Anotaciones leídas del __dict__ de la clase",
            "usá inspect.get_annotations",
            "Desde Python 3.14 el __dict__ sólo tiene __annotations__ si la clase usa el import del futuro; la función de inspect anda en todos los casos.",
            "cls.__dict__.get(\"__annotations__\") pasa a inspect.get_annotations(cls).",
        ),
    ),
    (
        PythonRule::RUF068,
        RuleText::new(
            "Nombre repetido en __all__",
            "sacá el nombre repetido",
            "Un nombre repetido no suma nada y suele venir de copiar y pegar o de un cambio a medias.",
            "__all__ = [\"a\", \"b\", \"a\"] pasa a __all__ = [\"a\", \"b\"].",
        ),
    ),
    (
        PythonRule::RUF009,
        RuleText::new(
            "Llamada a función en el default de una dataclass",
            "usá field(default_factory=...)",
            "La llamada corre una sola vez al definir la clase, así que todas las instancias comparten el mismo objeto.",
            "items: list = make_list() pasa a items: list = field(default_factory=make_list).",
        ),
    ),
    (
        PythonRule::ASYNC100,
        RuleText::new(
            "Contexto de timeout sin ningún await",
            "agregá un punto de espera o sacá el timeout",
            "Un timeout sólo puede cortar en un await, así que un bloque sin ninguno nunca se cancela.",
            "with asyncio.timeout(1): trabajar() necesita un await adentro para servir.",
        ),
    ),
    (
        PythonRule::PLE2502,
        RuleText::new(
            "Carácter de control bidireccional",
            "sacá el carácter bidireccional",
            "Las marcas de derecha a izquierda hacen que el código se vea distinto de lo que corre, y eso esconde errores o ataques.",
            "Una RIGHT-TO-LEFT MARK en un string puede hacer que parte de la línea parezca un comentario.",
        ),
    ),
    (
        PythonRule::PTH124,
        RuleText::new(
            "py.path está en mantenimiento",
            "usá pathlib",
            "py.path ya no suma funciones; pathlib viene en la biblioteca estándar.",
            "py.path.local(\"/foo\") pasa a pathlib.Path(\"/foo\").",
        ),
    ),
    (
        PythonRule::FURB129,
        RuleText::new(
            "readlines() en un loop for",
            "recorré el archivo directamente",
            "Recorrer el archivo lee de a una línea en vez de cargarlas todas en una lista antes.",
            "for line in f.readlines(): pasa a for line in f:.",
        ),
    ),
    (
        PythonRule::ASYNC222,
        RuleText::new(
            "Espera bloqueante de un proceso en una función async",
            "esperá el proceso de forma asíncrona",
            "os.waitpid y compañía bloquean todo el event loop hasta que el proceso termina.",
            "os.waitpid(pid, 0) adentro de async def frena todas las tareas; usá subprocesos de asyncio o un hilo.",
        ),
    ),
    (
        PythonRule::W605,
        RuleText::new(
            "Secuencia de escape inválida",
            "usá un string crudo o duplicá la barra",
            "Python avisa por los escapes que no conoce y los va a pasar a error.",
            "\"\\.png\" pasa a r\"\\.png\".",
        ),
    ),
    (
        PythonRule::UP049,
        RuleText::new(
            "Parámetro de tipo con nombre privado",
            "sacá el guion bajo del principio",
            "Los parámetros de tipo de PEP 695 sólo existen adentro de su clase o función, así que el guion bajo no agrega privacidad.",
            "class Box[_T]: pasa a class Box[T]:.",
        ),
    ),
    (
        PythonRule::PYI006,
        RuleText::new(
            "sys.version_info comparado con algo que no es < ni >=",
            "compará con < o >=",
            "Los otros operadores se portan raro con las versiones micro: (3, 8, 10) <= (3, 8) da falso.",
            "sys.version_info > (3, 8) pasa a sys.version_info >= (3, 9).",
        ),
    ),
    (
        PythonRule::UP029,
        RuleText::new(
            "Import innecesario de un builtin",
            "sacá el import",
            "Importar de builtins, six o io sólo servía para ser compatible con Python 2; el builtin ya está.",
            "from builtins import zip se puede borrar.",
        ),
    ),
    (
        PythonRule::PLE0643,
        RuleText::new(
            "Índice fuera de rango sobre una secuencia escrita en el código",
            "usá un índice que exista",
            "La secuencia tiene menos elementos que el índice, así que la línea tira IndexError.",
            "[0, 1, 2][3] falla siempre.",
        ),
    ),
    (
        PythonRule::PLW0406,
        RuleText::new(
            "El módulo se importa a sí mismo",
            "sacá el import de sí mismo",
            "Importar el propio módulo es un import circular y puede fallar con ImportError.",
            "import pkg.mod adentro de pkg/mod.py.",
        ),
    ),
    (
        PythonRule::INT002,
        RuleText::new(
            "format() adentro de una llamada a gettext",
            "formateá el texto ya traducido",
            "El texto se formatea antes de buscarlo, así que el catálogo busca el texto ya completo y no encuentra nada.",
            "_(\"Hola {}\".format(nombre)) pasa a _(\"Hola {}\").format(nombre).",
        ),
    ),
    (
        PythonRule::EXE005,
        RuleText::new(
            "Shebang que no está en la primera línea",
            "mové el shebang al principio del archivo",
            "El sistema sólo lee #! si son los dos primeros caracteres del archivo; en otro lado es un comentario común.",
            "x = 1 seguido de #!/usr/bin/env python no hace nada.",
        ),
    ),
    (
        PythonRule::PLE0604,
        RuleText::new(
            "Objeto inválido en __all__",
            "poné sólo strings en __all__",
            "__all__ tiene que tener nombres como strings; cualquier otra cosa rompe from modulo import *.",
            "__all__ = [Foo, 1] pasa a __all__ = [\"Foo\"].",
        ),
    ),
    (
        PythonRule::PYI020,
        RuleText::new(
            "Anotación entre comillas en un stub",
            "sacá las comillas",
            "Los stubs nunca se ejecutan, así que las referencias hacia adelante andan sin comillas.",
            "def f() -> \"int\": ... pasa a def f() -> int: ....",
        ),
    ),
    (
        PythonRule::PLE1142,
        RuleText::new(
            "await fuera de una función async",
            "hacé la función async",
            "await sólo vale adentro de async def; en otro lado es un error de sintaxis.",
            "def f(): await g() pasa a async def f(): await g().",
        ),
    ),
    (
        PythonRule::TC005,
        RuleText::new(
            "Bloque TYPE_CHECKING vacío",
            "borrá el bloque",
            "Un if TYPE_CHECKING: pass no hace nada y sólo agrega ruido.",
            "if TYPE_CHECKING: pass se puede borrar.",
        ),
    ),
    (
        PythonRule::UP036,
        RuleText::new(
            "Bloque para una versión de Python que ya no se soporta",
            "sacá la rama vieja",
            "La rama depende de una versión de Python más vieja que la mínima, así que es código muerto o se toma siempre.",
            "if sys.version_info < (3, 8): ... se puede sacar.",
        ),
    ),
    (
        PythonRule::SIM113,
        RuleText::new(
            "Contador de índice a mano en un loop for",
            "usá enumerate",
            "enumerate lleva el índice junto a cada elemento sin un contador aparte que mantener.",
            "i = 0; for x in xs: ...; i += 1 pasa a for i, x in enumerate(xs):.",
        ),
    ),
];
