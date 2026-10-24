use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: chequeos de excepciones, loops y llamadas.
pub const CHECK_TEXTS: [(PythonRule, RuleText); 38] = [
    (
        PythonRule::TRY203,
        RuleText::new(
            "try cuyos except sólo vuelven a lanzar",
            "sacá el try, o manejá el error en el except",
            "Un except que sólo vuelve a lanzar la misma excepción no hace nada más que sumar indentación.",
            "try:\n    f()\nexcept ValueError:\n    raise pasa a f().",
        ),
    ),
    (
        PythonRule::PLE0704,
        RuleText::new(
            "raise a secas fuera de un except",
            "lanzá una excepción concreta",
            "Un raise a secas fuera de un except no tiene una excepción activa para relanzar y falla con RuntimeError.",
            "raise pasa a raise ValueError(\"...\").",
        ),
    ),
    (
        PythonRule::SIM210,
        RuleText::new(
            "True if condición else False",
            "usá bool(condición), o la condición sola si es una comparación",
            "El ternario sólo convierte la condición a booleano, que es lo que bool() dice directamente.",
            "True if x else False pasa a bool(x).",
        ),
    ),
    (
        PythonRule::B020,
        RuleText::new(
            "Variable del loop que pisa lo que se recorre",
            "dale otro nombre a la variable del loop",
            "Apenas arranca el loop, el nombre deja de apuntar a la colección, así que los usos posteriores ven el último elemento.",
            "for items in items: pasa a for item in items:.",
        ),
    ),
    (
        PythonRule::UP024,
        RuleText::new(
            "Alias viejo de OSError",
            "usá OSError",
            "IOError, EnvironmentError, socket.error y compañía son todos OSError desde Python 3.3.",
            "except IOError: pasa a except OSError:.",
        ),
    ),
    (
        PythonRule::UP041,
        RuleText::new(
            "Alias viejo de TimeoutError",
            "usá TimeoutError",
            "socket.timeout y asyncio.TimeoutError son TimeoutError desde Python 3.10 y 3.11.",
            "except socket.timeout: pasa a except TimeoutError:.",
        ),
    ),
    (
        PythonRule::UP003,
        RuleText::new(
            "type() de un literal en vez del tipo directamente",
            "escribí el tipo",
            "type(\"\") es una vuelta larga para escribir str.",
            "type(\"\") pasa a str.",
        ),
    ),
    (
        PythonRule::UP022,
        RuleText::new(
            "stdout y stderr en PIPE en subprocess.run",
            "usá capture_output=True",
            "capture_output=True dice lo mismo en un solo argumento.",
            "run(cmd, stdout=PIPE, stderr=PIPE) pasa a run(cmd, capture_output=True).",
        ),
    ),
    (
        PythonRule::B004,
        RuleText::new(
            "hasattr(x, \"__call__\") para ver si algo se puede llamar",
            "usá callable(x)",
            "hasattr se pierde los objetos cuya clase define __call__ vía __getattr__, y callable() lo dice directamente.",
            "hasattr(x, \"__call__\") pasa a callable(x).",
        ),
    ),
    (
        PythonRule::UP011,
        RuleText::new(
            "@lru_cache() sin argumentos",
            "escribí @lru_cache sin paréntesis",
            "Desde Python 3.8 lru_cache se puede usar directamente como decorador.",
            "@functools.lru_cache() pasa a @functools.lru_cache.",
        ),
    ),
    (
        PythonRule::SIM202,
        RuleText::new(
            "not a != b en vez de a == b",
            "escribí a == b",
            "Negar una desigualdad es una doble negación que dice a == b por el camino largo.",
            "not a != b pasa a a == b.",
        ),
    ),
    (
        PythonRule::PIE794,
        RuleText::new(
            "Atributo de clase definido dos veces",
            "sacá la primera definición",
            "Sólo sobrevive la última asignación, así que la primera es código muerto o un error de copiar y pegar.",
            "el segundo nombre = 1 de la clase se borra.",
        ),
    ),
    (
        PythonRule::PIE807,
        RuleText::new(
            "lambda: [] o lambda: {} en vez de list o dict",
            "pasá list o dict directamente",
            "El builtin ya arma un contenedor vacío al llamarlo, sin envolverlo en una lambda.",
            "default_factory=lambda: [] pasa a default_factory=list.",
        ),
    ),
    (
        PythonRule::PLW0128,
        RuleText::new(
            "El mismo nombre asignado dos veces en una sentencia",
            "renombrá uno de los destinos",
            "Sólo sobrevive el último valor, así que el primer destino casi siempre es un error de tipeo.",
            "a, a = 1, 2 pasa a a, b = 1, 2.",
        ),
    ),
    (
        PythonRule::PLW0642,
        RuleText::new(
            "Asignación a self o cls en un método",
            "usá otro nombre local",
            "Reasignar self o cls sólo cambia la variable local, no el objeto ni la clase.",
            "self = otro pasa a actual = otro.",
        ),
    ),
    (
        PythonRule::PLW0211,
        RuleText::new(
            "Método estático cuyo primer parámetro se llama self o cls",
            "renombrá el parámetro, o hacelo un método común o de clase",
            "Un método estático no recibe ni la instancia ni la clase, así que llamar self o cls al primer parámetro confunde.",
            "@staticmethod def f(self): pasa a @staticmethod def f(valor):.",
        ),
    ),
    (
        PythonRule::PLC3002,
        RuleText::new(
            "Lambda que se llama apenas se define",
            "escribí la expresión directamente",
            "(lambda: x)() es x con una llamada de más.",
            "(lambda: calcular())() pasa a calcular().",
        ),
    ),
    (
        PythonRule::DTZ002,
        RuleText::new(
            "datetime.today() sin zona horaria",
            "usá datetime.now(tz=...)",
            "today() devuelve un datetime sin zona en la hora local, que rompe comparaciones y cuentas con datetimes con zona.",
            "datetime.today() pasa a datetime.now(tz=timezone.utc).",
        ),
    ),
    (
        PythonRule::S112,
        RuleText::new(
            "except que sólo sigue con el loop",
            "manejá o registrá el error antes de seguir",
            "Saltear una excepción en silencio esconde fallas que al menos deberían quedar registradas.",
            "except Exception:\n    continue suma un log antes del continue.",
        ),
    ),
    (
        PythonRule::C410,
        RuleText::new(
            "list() alrededor de una lista o tupla escrita",
            "escribí la lista directamente",
            "El literal ya arma la lista sin una llamada de más.",
            "list([1, 2]) pasa a [1, 2].",
        ),
    ),
    (
        PythonRule::C406,
        RuleText::new(
            "dict() alrededor de una lista o tupla de pares",
            "escribí el diccionario directamente",
            "Un diccionario escrito dice lo mismo sin armar una secuencia intermedia de pares.",
            "dict([(\"a\", 1)]) pasa a {\"a\": 1}.",
        ),
    ),
    (
        PythonRule::DTZ003,
        RuleText::new(
            "datetime.utcnow() devuelve un datetime sin zona",
            "usá datetime.now(tz=timezone.utc)",
            "utcnow() devuelve un datetime sin zona horaria que muchas funciones toman como hora local.",
            "datetime.utcnow() pasa a datetime.now(tz=timezone.utc).",
        ),
    ),
    (
        PythonRule::PLR1722,
        RuleText::new(
            "exit() o quit() en vez de sys.exit()",
            "usá sys.exit()",
            "exit y quit son para la consola interactiva y pueden no existir si Python corre sin el módulo site.",
            "exit(1) pasa a sys.exit(1).",
        ),
    ),
    (
        PythonRule::UP021,
        RuleText::new(
            "universal_newlines en vez de text en subprocess.run",
            "usá text=True",
            "universal_newlines es el nombre viejo de text desde Python 3.7.",
            "run(cmd, universal_newlines=True) pasa a run(cmd, text=True).",
        ),
    ),
    (
        PythonRule::PLW1509,
        RuleText::new(
            "preexec_fn en subprocess.Popen",
            "evitá preexec_fn, por ejemplo con start_new_session o process_group",
            "preexec_fn no es seguro en programas con hilos y puede trabar el proceso hijo.",
            "Popen(cmd, preexec_fn=os.setsid) pasa a Popen(cmd, start_new_session=True).",
        ),
    ),
    (
        PythonRule::UP033,
        RuleText::new(
            "@lru_cache(maxsize=None) en vez de @cache",
            "usá functools.cache",
            "Desde Python 3.9 functools.cache es un lru_cache sin límite.",
            "@lru_cache(maxsize=None) pasa a @cache.",
        ),
    ),
    (
        PythonRule::B003,
        RuleText::new(
            "Asignación a os.environ",
            "cambiá las variables con os.environ[...] u os.environ.update",
            "Reemplazar os.environ sólo cambia el nombre; el entorno del proceso y los procesos hijos siguen con las variables viejas.",
            "os.environ = {} pasa a os.environ.clear().",
        ),
    ),
    (
        PythonRule::PLC0208,
        RuleText::new(
            "Recorrido de un conjunto escrito",
            "recorré una tupla o una lista",
            "Un conjunto no tiene orden definido y se arma sólo para recorrerlo, así que una tupla dice lo mismo y mantiene el orden.",
            "for x in {1, 2}: pasa a for x in (1, 2):.",
        ),
    ),
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
        PythonRule::UP010,
        RuleText::new(
            "Import de __future__ que sobra",
            "sacá la característica del import",
            "Todas las características de este import ya son obligatorias en Python 3, así que no cambia nada.",
            "from __future__ import print_function se borra.",
        ),
    ),
    (
        PythonRule::RUF017,
        RuleText::new(
            "Listas aplanadas con sum()",
            "usá itertools.chain.from_iterable o una comprensión",
            "sum() con una lista vacía como inicio copia la lista acumulada en cada paso, que lleva tiempo cuadrático.",
            "sum(groups, []) pasa a list(itertools.chain.from_iterable(groups)).",
        ),
    ),
    (
        PythonRule::B033,
        RuleText::new(
            "Elemento repetido en un conjunto escrito",
            "sacá el elemento repetido",
            "Un conjunto guarda cada valor una sola vez, así que el repetido se descarta en silencio y suele esconder un error de tipeo.",
            "{\"a\", \"b\", \"a\"} pasa a {\"a\", \"b\"}.",
        ),
    ),
    (
        PythonRule::SIM101,
        RuleText::new(
            "Llamadas a isinstance repetidas sobre el mismo valor",
            "juntalas en un solo isinstance con una tupla",
            "isinstance acepta una tupla de tipos, así que una sola llamada dice lo mismo sin repetir.",
            "isinstance(x, int) or isinstance(x, str) pasa a isinstance(x, (int, str)).",
        ),
    ),
    (
        PythonRule::B019,
        RuleText::new(
            "Decorador de caché en un método",
            "cacheá una función del módulo o usá cached_property",
            "functools.cache y lru_cache guardan self en la caché, así que las instancias no se liberan nunca.",
            "@lru_cache en def total(self) pasa a @cached_property.",
        ),
    ),
    (
        PythonRule::PLR1704,
        RuleText::new(
            "Parámetro pisado por un for, un with o un except",
            "usá otro nombre para la variable nueva",
            "La variable del for, del with o del except pisa el parámetro y el resto de la función ya no ve el valor que recibió.",
            "for item in items: dentro de def f(item) pasa a for element in items:.",
        ),
    ),
    (
        PythonRule::RUF046,
        RuleText::new(
            "int() de un valor que ya es entero",
            "sacá la llamada a int()",
            "El valor ya es un int (un entero escrito, len(), round() sin decimales, math.floor() y compañía), así que la conversión no hace nada.",
            "int(len(items)) pasa a len(items).",
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
];
