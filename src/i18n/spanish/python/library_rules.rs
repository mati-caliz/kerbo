use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas de llamadas a bibliotecas y excepciones del conjunto por defecto de ruff 0.17 que no aparecen en el corpus.
pub const LIBRARY_RULE_TEXTS: [(PythonRule, RuleText); 22] = [
    (
        PythonRule::ASYNC105,
        RuleText::new(
            "Función async de trio llamada sin await",
            "agregale el await",
            "Sin await la corrutina se crea y se descarta, así que la función nunca corre.",
            "trio.sleep(1) pasa a await trio.sleep(1).",
        ),
    ),
    (
        PythonRule::ASYNC116,
        RuleText::new(
            "sleep de más de un día en trio o anyio",
            "usá sleep_forever",
            "Un sleep tan largo casi siempre quiere esperar para siempre, y sleep_forever lo dice.",
            "await trio.sleep(10**10) pasa a await trio.sleep_forever().",
        ),
    ),
    (
        PythonRule::ASYNC210,
        RuleText::new(
            "Llamada HTTP que bloquea adentro de una función async",
            "usá un cliente HTTP async",
            "Un pedido que bloquea frena todo el loop de eventos hasta que llega la respuesta.",
            "requests.get(url) pasa a await client.get(url) con httpx.AsyncClient.",
        ),
    ),
    (
        PythonRule::ASYNC220,
        RuleText::new(
            "Creación de un subproceso que bloquea adentro de una función async",
            "usá asyncio.create_subprocess_exec",
            "Crear el proceso con la API que bloquea frena el loop de eventos mientras arranca.",
            "subprocess.Popen(cmd) pasa a await asyncio.create_subprocess_exec(*cmd).",
        ),
    ),
    (
        PythonRule::ASYNC221,
        RuleText::new(
            "Proceso que bloquea corrido adentro de una función async",
            "corré el proceso con la API async de subprocesos",
            "Esperar el proceso con la API que bloquea frena el loop de eventos hasta que termina.",
            "subprocess.run(cmd) pasa a await (await asyncio.create_subprocess_exec(*cmd)).wait().",
        ),
    ),
    (
        PythonRule::B014,
        RuleText::new(
            "Excepción repetida en la tupla de un except",
            "nombrá cada excepción una sola vez",
            "El nombre repetido no suma nada y suele esconder un error de tipeo de otra excepción.",
            "except (ValueError, ValueError): pasa a except ValueError:.",
        ),
    ),
    (
        PythonRule::B022,
        RuleText::new(
            "contextlib.suppress sin excepciones",
            "pasale las excepciones a silenciar, o sacalo",
            "Sin argumentos suppress no atrapa nada, así que el with sólo agrega ruido.",
            "with contextlib.suppress(): pasa a with contextlib.suppress(FileNotFoundError):.",
        ),
    ),
    (
        PythonRule::B025,
        RuleText::new(
            "Excepción que ya atrapó un except anterior",
            "sacala de este except",
            "Gana el primer except que coincide, así que éste nunca corre para esa excepción.",
            "except ValueError: ... except ValueError: deja sólo el primero.",
        ),
    ),
    (
        PythonRule::B029,
        RuleText::new(
            "except con una tupla vacía",
            "nombrá las excepciones a atrapar",
            "except (): no atrapa nada, así que el except nunca corre.",
            "except (): pasa a except (ValueError, TypeError):.",
        ),
    ),
    (
        PythonRule::B039,
        RuleText::new(
            "Valor por defecto mutable en un ContextVar",
            "usá None y creá el valor cuando falte",
            "El objeto por defecto lo comparten todos los contextos, así que cambiarlo se filtra entre ellos.",
            "ContextVar(\"items\", default=[]) pasa a ContextVar(\"items\", default=None).",
        ),
    ),
    (
        PythonRule::C415,
        RuleText::new(
            "Inversión de un iterable que después se ordena, se invierte o se pasa a conjunto",
            "sacá el [::-1]",
            "sorted, set y reversed no ganan nada con invertir antes el orden.",
            "sorted(values[::-1]) pasa a sorted(values).",
        ),
    ),
    (
        PythonRule::SIM107,
        RuleText::new(
            "return en el finally que pisa un return anterior",
            "sacá el return del finally",
            "Un return en el finally siempre gana, así que el valor del try o del except se pierde.",
            "try: return 1 finally: return 2 siempre devuelve 2.",
        ),
    ),
    (
        PythonRule::SIM220,
        RuleText::new(
            "and entre un nombre y su negación",
            "reemplazá la expresión por False",
            "x and not x siempre es falso, lo que suele indicar que la condición está mal.",
            "if a and not a: nunca corre.",
        ),
    ),
    (
        PythonRule::SIM221,
        RuleText::new(
            "or entre un nombre y su negación",
            "reemplazá la expresión por True",
            "x or not x siempre es verdadero, lo que suele indicar que la condición está mal.",
            "if a or not a: siempre corre.",
        ),
    ),
    (
        PythonRule::SIM911,
        RuleText::new(
            "zip de las claves y los valores de un diccionario",
            "usá dict.items()",
            "items() ya da los pares, sin armar dos vistas y juntarlas con zip.",
            "zip(d.keys(), d.values()) pasa a d.items().",
        ),
    ),
    (
        PythonRule::FURB162,
        RuleText::new(
            "Z reemplazada a mano antes de datetime.fromisoformat",
            "pasá el texto directamente",
            "Desde Python 3.11 fromisoformat entiende el sufijo Z.",
            "datetime.fromisoformat(s.replace(\"Z\", \"+00:00\")) pasa a datetime.fromisoformat(s).",
        ),
    ),
    (
        PythonRule::FURB169,
        RuleText::new(
            "type(x) comparado con type(None)",
            "usá x is None",
            "Comparar tipos para saber si algo es None es una vuelta larga para un chequeo de identidad.",
            "type(x) == type(None) pasa a x is None.",
        ),
    ),
    (
        PythonRule::FURB177,
        RuleText::new(
            "Path().resolve() para obtener el directorio actual",
            "usá Path.cwd()",
            "Path.cwd() dice lo que hace y no depende de cómo se resuelve un camino vacío.",
            "Path().resolve() pasa a Path.cwd().",
        ),
    ),
    (
        PythonRule::FURB181,
        RuleText::new(
            "digest().hex() de hashlib",
            "usá hexdigest()",
            "hexdigest() devuelve el mismo texto directamente, sin armar antes los bytes.",
            "hashlib.sha256(data).digest().hex() pasa a hashlib.sha256(data).hexdigest().",
        ),
    ),
    (
        PythonRule::PTH210,
        RuleText::new(
            "with_suffix con un sufijo que no empieza con punto",
            "agregale el punto adelante",
            "with_suffix tira ValueError con un sufijo sin punto, o con un punto solo.",
            "path.with_suffix(\"txt\") pasa a path.with_suffix(\".txt\").",
        ),
    ),
    (
        PythonRule::PGH005,
        RuleText::new(
            "Método de un mock sin llamar, o que no existe",
            "llamá al método de assert, o usá el nombre correcto",
            "mock.assert_called es verdadero sin llamarlo, y mock.called_once se crea en el momento, así que el chequeo siempre pasa.",
            "mock.assert_called_once pasa a mock.assert_called_once().",
        ),
    ),
    (
        PythonRule::PLC0131,
        RuleText::new(
            "TypeVar covariante y contravariante a la vez",
            "dejá una sola de las varianzas",
            "Una variable de tipo no puede ser las dos cosas; Python tira ValueError al crearla.",
            "TypeVar(\"T\", covariant=True, contravariant=True) deja sólo covariant=True.",
        ),
    ),
];
