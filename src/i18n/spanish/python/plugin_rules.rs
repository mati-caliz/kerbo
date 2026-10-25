use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas de plugins de flake8 y de pyflakes del conjunto por defecto de ruff 0.17 que no aparecen en el corpus.
pub const PLUGIN_RULE_TEXTS: [(PythonRule, RuleText); 18] = [
    (
        PythonRule::EXE001,
        RuleText::new(
            "Shebang en un archivo que no es ejecutable",
            "dale permiso de ejecución o sacá el shebang",
            "Un shebang dice que el archivo se corre como script, pero sin el permiso de ejecución el sistema no lo corre.",
            "chmod +x script.py, o borrá la línea #!/usr/bin/env python.",
        ),
    ),
    (
        PythonRule::EXE002,
        RuleText::new(
            "Archivo ejecutable sin shebang",
            "agregá un shebang o sacale el permiso de ejecución",
            "Sin shebang el sistema no sabe qué intérprete usar, así que el script falla o corre con el que no es.",
            "Agregá #!/usr/bin/env python como primera línea, o chmod -x modulo.py.",
        ),
    ),
    (
        PythonRule::EXE004,
        RuleText::new(
            "Espacios antes del shebang",
            "sacá los espacios antes de #!",
            "El sistema sólo lee #! si son los dos primeros caracteres del archivo, así que cualquier espacio antes lo vuelve un comentario.",
            "  #!/usr/bin/env python pasa a #!/usr/bin/env python.",
        ),
    ),
    (
        PythonRule::INT001,
        RuleText::new(
            "f-string adentro de una llamada a gettext",
            "traducí la plantilla y formateá el resultado",
            "El f-string se completa antes de buscarlo, así que el catálogo busca el texto ya armado y no encuentra nada.",
            "_(f\"Hola {nombre}\") pasa a _(\"Hola %s\") % nombre.",
        ),
    ),
    (
        PythonRule::INT003,
        RuleText::new(
            "Formato con % adentro de una llamada a gettext",
            "formateá el texto ya traducido",
            "El % se aplica antes de buscarlo, así que el catálogo busca el texto ya completo y no encuentra nada.",
            "_(\"Hola %s\" % nombre) pasa a _(\"Hola %s\") % nombre.",
        ),
    ),
    (
        PythonRule::LOG002,
        RuleText::new(
            "getLogger con __file__ o __cached__",
            "usá __name__",
            "El nombre de un logger es el del módulo con puntos; una ruta de archivo rompe la jerarquía de loggers y su configuración.",
            "logging.getLogger(__file__) pasa a logging.getLogger(__name__).",
        ),
    ),
    (
        PythonRule::LOG009,
        RuleText::new(
            "logging.WARN sin documentar",
            "usá logging.WARNING",
            "logging.WARN es un alias sin documentar; el nombre documentado es logging.WARNING.",
            "level=logging.WARN pasa a level=logging.WARNING.",
        ),
    ),
    (
        PythonRule::LOG014,
        RuleText::new(
            "exc_info fuera de un except",
            "sacá exc_info o mové la llamada al except",
            "Fuera de un except no hay excepción que adjuntar, así que el log muestra NoneType: None.",
            "logging.warning(\"falló\", exc_info=True) fuera de un except pasa a logging.warning(\"falló\").",
        ),
    ),
    (
        PythonRule::G010,
        RuleText::new(
            "warn de logging, que está deprecado",
            "usá warning",
            "warn es un alias deprecado de warning y Python avisa cada vez que se llama.",
            "logger.warn(\"x\") pasa a logger.warning(\"x\").",
        ),
    ),
    (
        PythonRule::G101,
        RuleText::new(
            "Clave de extra que choca con un atributo de LogRecord",
            "cambiale el nombre a la clave",
            "LogRecord tira KeyError cuando extra quiere pisar uno de sus atributos, así que la llamada al log falla.",
            "extra={\"name\": usuario} pasa a extra={\"user_name\": usuario}.",
        ),
    ),
    (
        PythonRule::G202,
        RuleText::new(
            "exc_info de más en logging.exception",
            "sacá exc_info",
            "logging.exception ya registra la excepción actual, así que exc_info=True no suma nada.",
            "logger.exception(\"falló\", exc_info=True) pasa a logger.exception(\"falló\").",
        ),
    ),
    (
        PythonRule::PT010,
        RuleText::new(
            "pytest.raises sin la excepción esperada",
            "pasale la excepción esperada",
            "Sin el tipo de excepción pytest.raises falla cuando corre el test.",
            "with pytest.raises(): pasa a with pytest.raises(ValueError):.",
        ),
    ),
    (
        PythonRule::PT020,
        RuleText::new(
            "pytest.yield_fixture, que está deprecado",
            "usá pytest.fixture",
            "pytest.fixture acepta yield desde pytest 3.0; yield_fixture es un alias deprecado que pytest 10 saca.",
            "@pytest.yield_fixture pasa a @pytest.fixture.",
        ),
    ),
    (
        PythonRule::PT025,
        RuleText::new(
            "pytest.mark.usefixtures sobre un fixture",
            "sacá la marca y pedí el fixture como parámetro",
            "Las marcas no hacen nada sobre un fixture, así que los fixtures de la lista nunca se preparan.",
            "@pytest.mark.usefixtures(\"db\") sobre un fixture pasa a un parámetro db.",
        ),
    ),
    (
        PythonRule::PT026,
        RuleText::new(
            "pytest.mark.usefixtures sin argumentos",
            "sacá la marca o listá los fixtures",
            "Una marca usefixtures sin nombres de fixtures no hace nada.",
            "@pytest.mark.usefixtures() se puede borrar.",
        ),
    ),
    (
        PythonRule::TC010,
        RuleText::new(
            "String adentro de una unión X | Y que se evalúa",
            "poné entre comillas la unión entera o sacá las comillas",
            "Python evalúa la anotación y \"Foo\" | None tira TypeError, porque un string no admite |.",
            "x: \"Foo\" | None pasa a x: \"Foo | None\".",
        ),
    ),
    (
        PythonRule::F407,
        RuleText::new(
            "Característica de __future__ que no existe",
            "sacá el import",
            "Importar algo que __future__ no define es un SyntaxError.",
            "from __future__ import braces no anda nunca.",
        ),
    ),
    (
        PythonRule::F622,
        RuleText::new(
            "Dos destinos con * en la misma asignación",
            "dejá un solo destino con *",
            "Python no puede saber cuál de los nombres con * se lleva los valores que sobran, así que es un SyntaxError.",
            "*a, *b = items pasa a primero, *resto = items.",
        ),
    ),
];
