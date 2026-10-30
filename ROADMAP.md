# Hoja de ruta

kerbo quiere hacer lo que hoy hacen por separado Checkstyle, PMD, SpotBugs, ESLint, ruff, mypy,
golangci-lint y los formateadores, con un solo binario, una sola configuración y un solo reporte,
para Java, TypeScript/JavaScript, Python y Go.

## Principios

- **Paridad medida, no aproximada.** Cada regla se compara contra la herramienta que reemplaza
  sobre repos reales y tiene que marcar exactamente lo mismo. Mientras una regla o un lenguaje no
  llegue, se sigue usando la herramienta original: un resultado "parecido" no sirve para un gate.
- **Liviano de verdad.** Un ejecutable nativo, sin runtimes ni plugins, que analiza archivo por
  archivo en paralelo. La meta es menos de 200 MB en el monorepo más grande y segundos en vez de
  minutos.
- **La vara más alta de fábrica.** El preset por defecto es estricto. Se puede aflojar una regla
  por repo, pero siempre con un motivo escrito que aparece en el reporte: nada se afloja en
  silencio.
- **Pensado también para agentes de IA.** Salida compacta y priorizada, identificadores estables
  por problema, arreglos como parche y avisos cuando un cambio esquiva una regla en vez de
  arreglarla.
- **No se reimplementan compiladores ni motores de tests.** Los tipos de TypeScript vienen del
  compilador; cada test lo ejecuta el motor de su lenguaje (JUnit Platform, Node, pytest,
  `go test`), así se comporta igual. Lo que lo rodea (elegir qué correr, compilar, transformar,
  arrancar y reusar procesos) es de kerbo.

## Qué sigue

Lo que kerbo ya hace está en el [README](README.md); acá queda sólo lo abierto, en orden. Cada
etapa se cierra con la paridad medida y los tests del caso que la motivó, y al cerrarse se borra de
acá.

### 10. Go, Python, TypeScript y shell completos

- Cada lenguaje contra su herramienta (golangci-lint, ruff, ESLint), regla por regla.
- Python contra ruff (empezada, 2026-10-21): las 55 reglas del conjunto clásico de ruff (`E4`,
  `E7`, `E9` y `F`, el que pide la tarea; ruff 0.17 cambió su conjunto por defecto por uno mucho más
  grande, con `B`, `PL`, `UP`, `RUF` y otros) salvo E902, F407, F621 y F622, con el mismo código en
  minúscula como nombre de regla (`f401`, `e711`), en una familia propia (`PythonRule`).
  - Frecuencia medida sobre los diez repos de abajo, de mayor a menor: F405 4.167, F821 2.194,
    E402 1.799, E741 1.451, F841 1.361, E701 1.283, E712 1.031, E731 684, F401 627, E722 255,
    E721 220, E702 208, F541 176, E711 99, E401 98, F403 97, E713 86, F811 79, F601 53, E703 47,
    E742 35, F402 30, F822 22, F507 22 y el resto con menos de 20.
  - Motor de alcances propio sobre tree-sitter, calcado del modelo semántico de ruff: alcances de
    módulo, clase, función, lambda, comprensión y tipo (PEP 695); cuerpos de funciones y lambdas
    diferidos al final del módulo; anotaciones entre comillas parseadas aparte con la posición
    exacta adentro del string (o la del string si tiene escapes o está concatenado), y anotaciones
    diferidas con `from __future__ import annotations` y en los `.pyi`; `typing.cast`, `TypeVar`,
    `NewType`, `NamedTuple`, `TypedDict`, `TypeForm`, `Literal`, `Annotated` y `TypeAlias`
    resueltos por nombre calificado; ramas de `if`, `try`, `match` y `with` para no marcar
    redefiniciones en caminos excluyentes.
  - Arreglos mecánicos con el `Fix` de siempre (salen como parche en `--format agent`, acción
    rápida del LSP y en `kerbo fix`): F401 (el import entero, el nombre de una lista o `pass` si el
    bloque queda vacío; no en `__init__.py` ni con otra sentencia en la línea), F541, E703, E711,
    E712, E713, E714, F632, F901 y el `as` sin usar de un `except` (F841).
  - `# noqa`, `# noqa: CÓDIGOS` y `# ruff: noqa`/`# flake8: noqa` como ruff: en un string de varias
    líneas o una línea continuada con `\` el `noqa` va en la última; F401 también mira la primera
    línea del import; `# noqa:` sin códigos válidos no silencia nada.
  - Paridad: cero diferencias por archivo, línea, columna, regla y cantidad contra ruff 0.17.1
    (`--isolated --select E4,E7,E9,F`) en requests, fastapi, flask, django, pydantic, sqlalchemy,
    ipython, ansible, sympy y `Lib/` de CPython: 15.916 avisos en unos 11.200 archivos. Lo que queda,
    explicado:
    - 82 archivos de CPython usan `lazy import` (PEP 810, de 3.15) y quedan fuera de la medición:
      tree-sitter-python 0.25 no lo conoce.
    - En los archivos donde tree-sitter devuelve errores no corren las reglas que dependen de los
      alcances (un árbol roto da falsos positivos, que rompen un gate): en CPython son
      `test_compile.py` (un `dedent` adentro de paréntesis) y `test_patma.py`, 103 avisos que ruff
      sí da. Las reglas locales (E70x, comparaciones, f-strings) corren igual.
    - E902 (5 avisos) es un error de lectura de ruff con archivos que no están en UTF-8, no una
      regla.
  - Lo que no se ve a simple vista quedó en `src/lint/python/fixtures/`, cada archivo con la salida
    de ruff al lado: los builtins de 3.11 (sin `PythonFinalizationError`), `__class__` sólo en
    métodos y `__module__`/`__qualname__` sólo en el cuerpo de la clase; la anotación sin valor que
    deja el nombre sin definir en una función pero no en una clase; el `del` condicional (adentro de
    un `if`, `while`, `match` o un `except`); el `global` de una función que define el nombre en el
    módulo cuando se lee la `def`, no cuando corre; los parámetros que se ligan recién al analizar
    el cuerpo (y por eso redefinen un import posterior); las variables de una comprensión dentro de
    un `for` que cuentan como variables del loop; las redefiniciones entre alcances que no miran las
    clases; la resolución de las anotaciones entre comillas que prueba primero el módulo; el
    `@overload` resuelto por nombre calificado (un `typing` reexportado no vale), y F507, que cuenta
    como un solo valor cualquier literal que no sea una tupla.
  - Las columnas cuentan bytes, como el resto de las reglas; ruff cuenta caracteres (la medición
    convierte).
- Python contra el conjunto por defecto de ruff 0.17 (empezado, 2026-10-23). Frecuencia en los diez
  repos de arriba, sin el conjunto clásico: UP031 15.546, I001 6.736, RUF059 4.549, C408 4.413,
  UP045 4.371, RUF012 4.039, UP006 4.025, UP032 1.920, B018 1.692, UP025 1.634, SIM117 1.535, RUF100
  1.437, UP007 1.410, BLE001 1.367, UP035 1.169, UP004 995, ISC004 947, SIM102 708, SIM115 633,
  PIE790 587, B023 583, UP005 540, C405 532, TRY002 517 y el resto con menos de 500.
  - Hechas: UP031 (el formato con `%`, con el parser de especificadores de F501–F509 parte por parte
    del string y el `clean_params_dictionary` de pyupgrade para decidir si avisa en el string o en la
    operación) y C408 (`dict()`, `list()` y `tuple()` del builtin sin posicionales). Paridad: 15.540
    de 15.546 y 4.385 de 4.413, sin avisos de más; lo que falta está en carpetas ocultas (`.ci`,
    `.azure-pipelines`), que kerbo no recorre, y en los archivos de CPython que tree-sitter no
    parsea. Los casos quedaron en `src/lint/python/fixtures/modern.py`.
  - Hechas en una segunda tanda: UP025 (prefijo `u`), UP004 (heredar de `object`), UP005 (alias de
    `unittest`), UP009 (declaración de codificación utf-8), C405 (`set([...])`), B015 (comparación
    suelta), B018 (expresión suelta, con el `contains_effect` de ruff; una t-string no cuenta como
    string) y PIE790 (`pass` o `...` de más, salvo `...` en `if TYPE_CHECKING:`, protocolos y métodos
    abstractos). Paridad: 1.634 de 1.634, 975 de 995, 540 de 540, 396 de 397, 532 de 532, 249 de 249,
    1.667 de 1.692 y 587 de 587, sin avisos de más; lo que falta está en los archivos de CPython que
    tree-sitter no parsea y en carpetas ocultas. Los casos quedaron en `fixtures/upgrades.py`.
  - Hechas en una tercera tanda: UP006 (`List`, `Dict` y compañía de `typing`, en cualquier lectura y
    también adentro de anotaciones entre comillas), UP045 (`Optional[X]`) y UP007 (`Union[X, Y]`),
    con las excepciones de ruff: `NamedTuple` adentro, un elemento con `*`, y un elemento entre
    comillas cuando la expresión se evalúa en ejecución. El contexto de tipos sale de los ancestros:
    `if TYPE_CHECKING:`, o una anotación que Python no evalúa (con el import del futuro, o la de una
    variable local o `self.x` dentro de una función). La versión de Python por defecto de ruff 0.17 es
    la 3.11, así que las tres valen en todo el código. Paridad: 4.006 de 4.025, 1.404 de 1.410 y 4.371
    de 4.371, sin avisos de más; lo que falta está en CPython sin parsear y en carpetas ocultas. Los
    casos quedaron en `fixtures/pep604.py`.
  - Hechas en una cuarta tanda: SIM102 (`if` anidados que se juntan con `and`, salvo chequeos de
    `sys.version_info`/`sys.platform` con cualquier alias, `TYPE_CHECKING` y `__main__`), SIM117
    (`with` anidados, salvo los de timeout de asyncio, anyio y trio) y BLE001 (`except Exception`
    sin `raise` ni registro con `logging.exception` o `exc_info=`, con la heurística de loggers de
    ruff). Paridad: 708 de 708, 1.535 de 1.535 y 1.304 de 1.367, sin avisos de más; los 63 que faltan
    están en archivos de CPython con `lazy import`. Los casos quedaron en `fixtures/simplify.py`.
  - Hechas en una quinta tanda: TRY002 (`raise Exception`), PLC0414 (`import x as x`, fuera de stubs y
    de `__init__.py`), S102 (`exec`), S110 (`except` amplio con sólo `pass`), SIM118 (`in d.keys()`,
    en comparaciones, `for` y comprensiones), UP017 (`datetime.timezone.utc`), UP018 (`str("x")`,
    `int(1)` y compañía) y UP030 (`"{0} {1}".format`). Paridad: 516 de 517, 459 de 460, 417 de 450,
    251 de 265, 335 de 335, 340 de 343, 213 de 213 y 489 de 489, sin avisos de más; todo lo que falta
    está en archivos de CPython que tree-sitter no parsea. Los casos quedaron en `fixtures/hygiene.py`.
  - Hechas en una sexta tanda: UP008 (`super(Clase, self)`, con las clases anidadas, `__class__`,
    los nombres ligados en la función y `dataclass(slots=True)`), PIE808 (`range(0, n)`), B009 y
    B010 (`getattr`/`setattr` con nombre fijo), RUF015 (`list(x)[0]` y `.pop(0)`), B006 (defaults
    mutables) y B008 (llamadas en defaults). B006 y B008 usan un port de los helpers de mutabilidad
    de ruff (`semantic/mutability.rs`: tipos de retorno mutables e inmutables, anotaciones
    inmutables con `Optional`, `Union`, `|` y `Annotated`, y `NewType` sobre un tipo inmutable), que
    también va a servir para RUF012. Paridad: 423 de 449, 244 de 245, 181 de 185, 82 de 84, 417 de
    420, 214 de 221 y 348 de 351, sin avisos de más. Lo que falta está en archivos que tree-sitter no
    parsea, salvo un B009 con un identificador que lleva un selector de variación Unicode (la
    biblioteca estándar de Rust no expone XID; haría falta `unicode-ident`). Los casos quedaron en
    `fixtures/defaults.py`.
  - Hecha RUF012 (atributos de clase mutables): corre al final del recorrido, como los scopes
    diferidos de ruff. Los nombres se resuelven desde el scope donde está la clase (ruff usa la
    resolución que guardó al visitar cada nombre) y las bases heredadas desde el módulo (ruff usa
    `lookup_attribute` desde el scope global). Respeta `ClassVar` y `Final` (sólo si el archivo
    importa `typing`), las anotaciones inmutables, dataclasses y attrs, los modelos de pydantic,
    msgspec y sqlmodel (también heredados de una clase del archivo) y el `_fields_` de ctypes.
    Paridad: 4.009 de 4.039, sin avisos de más; lo que falta está en archivos que tree-sitter no
    parsea. Los casos quedaron en `fixtures/classes.py`.
  - Hecha RUF059 (variable desempaquetada sin usar): son los mismos candidatos que F841, con los
    bindings desempaquetados que F841 descarta. Paridad: 4.511 de 4.549, sin avisos de más; lo que
    falta está en archivos que tree-sitter no parsea.
  - Hechas SIM115 (abrir un archivo fuera de un `with`, con las excepciones de ruff: `.close()`
    inmediato, `return`, `ExitStack`/`AsyncExitStack`, los contextos de unittest y `__enter__`) y
    TRY004 (chequeo de tipo con `isinstance`, `issubclass` o `callable` que lanza una excepción
    genérica). Paridad: 612 de 633 y 212 de 219, sin avisos de más. Además de los archivos sin
    parsear y las carpetas ocultas, faltan los módulos de la biblioteca estándar que se usan a sí
    mismos (`open` dentro de `gzip.py`): ruff conoce el nombre del módulo de cada archivo y resuelve
    esas definiciones como `gzip.open`; kerbo no lo calcula. Los casos quedaron en
    `fixtures/resources.py`.
  - Hecha UP035 (imports de módulos de los que el nombre se mudó): las tablas de
    `deprecated/tables.rs` salen del fuente de ruff para Python 3.11, que es la versión por defecto
    de ruff 0.17 (lo confirma que `typing_extensions.override`, de 3.12, no se reporta). Un aviso
    por destino y uno por cada miembro de `typing` que cambió de nombre, como ruff. Paridad: 1.168 de
    1.169, sin avisos de más; el que falta es un `lazy from` de CPython.
  - Hechas ISC004 (concatenación implícita sin paréntesis dentro de una lista, tupla, conjunto o
    subíndice con varios elementos) y SIM114 (ramas seguidas de un `if` con el mismo cuerpo). SIM114
    aproxima el `ComparableStmt` de ruff comparando el árbol sin comentarios ni paréntesis de
    agrupación y los strings por su valor, y compara los comentarios en el mismo rango que
    `body_range` (desde la línea siguiente a la condición). Paridad: 947 de 947 y 245 de 245.
  - Hecha B023 (función o lambda en un loop que usa una variable del loop), sintáctica como en
    ruff: los nombres que lee cada función o lambda sin ser parámetros ni asignarlos, contra los que
    el loop asigna (destinos de asignaciones, `for` y comprensiones, sin entrar a funciones ni
    lambdas), con las lambdas que se consumen enseguida como seguras. `loops/names.rs` decide si un
    identificador de tree-sitter es una lectura, una escritura o no es un nombre en el AST de ruff.
    Paridad: 583 de 583.
  - Hechas RUF022 (`__all__` sin ordenar, en el módulo: asignación, `+=` y `.extend`) y RUF023
    (`__slots__` sin ordenar, en una clase: lista, tupla, conjunto o claves de un diccionario), con un
    port de `natord` y del orden "isort" de ruff (constantes, clases y el resto, cada grupo en orden
    natural). Paridad: 406 de 406 y 311 de 311. De paso, F822 lee también los nombres de `__all__`
    en una tupla sin paréntesis y en strings concatenados.
  - Hechas en una tanda de reglas chicas: FURB167 (alias de una letra de los flags de `re`),
    PLR2044 (comentario vacío, con los bloques de comentarios de ruff), SIM201 (`not a == b`),
    PLW1510 (`subprocess.run` sin `check`), PLR0124 (comparar algo consigo mismo), PLR0133 (comparar
    dos constantes), C401 y C402 (`set`/`dict` sobre un generador) y LOG015 (logger raíz). Paridad
    exacta salvo un C401 de CPython con `await` adentro del generador y dos casos en archivos sin
    parsear.
  - Hechas FLY002 (`" ".join([...])` fijo, con las reglas de flynt sobre qué elementos entran en
    una f-string y las comillas), PIE804 (`f(**{"a": 1})`) y RUF010 (`str()`, `repr()` o `ascii()`
    adentro de una f-string). Paridad: 242 de 242, 102 de 102 y 105 de 105.
  - Hecha I001 (bloque de imports sin ordenar o formatear), con un port del isort de ruff: arma el
    bloque esperado (secciones de futuro, estándar, terceros, del proyecto y relativos; imports del
    mismo módulo fusionados; comentarios arriba, en línea y al final de cada nombre; corte a 88
    columnas con la coma mágica) y lo compara con el texto, sin mirar la sangría. Respeta
    `# isort: skip_file`, `off`/`on`, `skip` y `split`, las líneas que siguen al bloque (dos antes
    de una `def` o `class`, una en los stubs) y los bloques que comparten línea con otra sentencia.
    La lista de la biblioteca estándar sale de ruff para 3.11 (`imports/stdlib.rs`) y lo propio se
    decide como ruff con la configuración por defecto: el paquete del archivo o un módulo que
    existe bajo `.` o `src`. Paridad contra `--isolated`: 6.917 de 7.021, sin avisos de más. Lo que
    falta: 64 archivos de CPython con `lazy import`, los que tree-sitter no parsea, la carpeta
    oculta `.ci` de sympy y un `.pyi` de pydantic-core sin explicar todavía. Los casos quedaron en
    `fixtures/isort.py` y `fixtures/isort_unsorted.py`.
  - Hecha UP032 (`"{}".format(x)` que puede ser f-string), con un port del parser de formatos de
    `ruff_python_literal` sobre el texto de cada parte del string (con `\N{…}` y el prefijo `r`):
    argumentos sin `*`/`**`, comillas ni saltos de línea, campos que apuntan a argumentos que
    existen, sin repetir uno con efectos, y nada adentro de un `gettext`/`gettext_lazy` de Django.
    De paso, el resumen de F521–F525 saltea `\N{…}` como ruff (daba un F524 de más en un string
    crudo). Paridad: 1.851 de 1.920, sin avisos de más; lo que falta está en archivos de CPython con
    `lazy import`, en `test_patma.py` y en una carpeta oculta de ansible. Los casos quedaron en
    `fixtures/format_calls.py`. I001 sale del preset `minimal`, con las demás reglas de estilo.
  - Hechas N999 (nombre de módulo, sólo adentro de un paquete; un `__init__.py` toma el nombre del
    paquete raíz y en `migrations`/`versions` se permite empezar con un dígito), SIM103 (`if` que
    devuelve `True`/`False`, con `else`, con el último `elif` y su `else`, o con el `return` que
    sigue), SIM905 (`split`/`rsplit` sobre un string fijo) y FURB157 (`Decimal("10")` y
    `Decimal(float("nan"))`). Paridad: 155 de 156, 162 de 162, 162 de 162 y 161 de 161, sin avisos
    de más; el N999 que falta es un archivo oculto (`.util.py`). Los casos quedaron en
    `fixtures/statics.py` y N999 en un test con un paquete en disco.
  - RUF100 (`noqa` sin usar, 1.437) queda para cuando kerbo tenga el conjunto por defecto completo:
    decidir si un código de un `noqa` se usa pide correr esa regla, y con un conjunto parcial
    marcaría como sobrante un `# noqa: B904` que en ruff sí silencia algo.
  - RUF046 (`int()` sobre algo que ya es entero) necesita un port de `ResolvedPythonType`, la
    inferencia de tipos de expresiones de ruff, y de los tipos de los parámetros de `round`.
  - Hechas UP034 (paréntesis de más, con el recorrido por tokens de ruff sobre las hojas del árbol),
    PLC0205 (`__slots__` con un único string), PLW0120 (`else` de un loop sin `break`) y RET501
    (`return None` en una función que sólo devuelve `None`, sin generadores, propiedades ni retornos
    anotados con otro tipo). Paridad: 145 de 145, 86 de 86, 83 de 83 y 82 de 86, sin avisos de más;
    los RET501 que faltan están en archivos que tree-sitter no parsea. Los casos quedaron en
    `fixtures/pylint_rules.py`.
  - Hechas PLR1711 (`return` inútil al final), PLR0402 (`import a.b as b`), B026 (`*args` después
    de un argumento con nombre), UP039 (`class C():`), UP020 (`io.open`) y RUF032 (`Decimal(0.1)`).
    Paridad: 65 de 66, 56 de 56, 56 de 56, 40 de 40, 53 de 55 y 55 de 55, sin avisos de más; lo que
    falta está en archivos de CPython con `lazy import`. Los casos quedaron en
    `fixtures/declarations.py`.
  - Hechas TRY201 (`raise error` en su propio `except`), UP012 (`"x".encode("utf-8")`), C400
    (`list(x for x in y)`), C409 (`tuple([1, 2])`), C413 (`list(sorted(x))`) y C419
    (`any([x for x in y])`). Paridad: 45 de 45, 61 de 61, 33 de 34, 24 de 24, 21 de 22 y 24 de 24,
    sin avisos de más. Faltan un C413 en un archivo con `lazy import` y un C400 en
    `list(await 2 for _ in [])` de `test_builtin.py`, que tree-sitter no arma como generador. Los
    casos quedaron en `fixtures/collections_calls.py`. Con 128 reglas, `wrap_python` (que arma la
    lista de reglas en tiempo de compilación) pasó a ser un loop: la recursión superaba el límite
    de llamadas anidadas de la evaluación `const`.
  - Hechas T100 (depuradores, en llamadas e imports), B017 (`assertRaises(Exception)` y
    `pytest.raises(Exception)`), B002 (`++x`), PLW0127 (`x = x`) y PLW0133 (una excepción construida
    sin `raise`). Paridad: 39 de 40, 47 de 47, 25 de 25, 34 de 34 y 24 de 24, sin avisos de más. El
    T100 que falta es `from . import embed` adentro de `IPython/terminal/`: ruff resuelve el import
    relativo con el nombre de módulo del archivo, que kerbo todavía no calcula (como en SIM115). De
    paso, F823 deja de mirar los alcances de clase al buscar qué nombre de afuera pisa una variable
    local (no se ven desde adentro de sus métodos). Los casos quedaron en `fixtures/suspicious.py`.
  - Hechas TRY203 (`try` cuyos `except` sólo vuelven a lanzar), PLE0704 (`raise` a secas fuera de
    un `except`; una función definida adentro de uno ya no cuenta como adentro), SIM210 (`True if c
    else False`) y B020 (la variable del `for` pisa lo que se recorre, con el `NameFinder` de ruff).
    Paridad: 37 de 37, 33 de 33, 27 de 27 y 44 de 44. Los casos quedaron en
    `fixtures/exception_flow.py`; los textos, en un catálogo nuevo (`i18n/*/python/checks.rs`).
  - Hechas UP024 (alias viejos de `OSError`) y UP041 (de `TimeoutError`, para 3.11) en el `except`,
    el `raise` y la llamada; UP003 (`type("")`), UP022 (`stdout` y `stderr` en `PIPE`), B004
    (`hasattr(x, "__call__")`) y UP011 (`@lru_cache()`). Paridad: 34 de 35, 16 de 18, 17 de 17, 17
    de 17, 23 de 23 y 29 de 30, sin avisos de más; lo que falta está en archivos de CPython con
    `lazy import`. Los casos quedaron en `fixtures/aliases.py`.
  - Hechas SIM202 (`not a != b`), PIE794 (atributo de clase repetido), PIE807 (`lambda: []`),
    PLW0128 (el mismo nombre dos veces entre los destinos) y PLW0642 (asignar a `self` o `cls`, con
    el `function_type::classify` de ruff y los decoradores reconocidos por nombre). Paridad exacta:
    18, 20, 12, 19 y 68. Los casos quedaron en `fixtures/class_bodies.py`; la tabla de fixtures pasó
    a `src/lint/python/tests/fixtures.rs`.
  - Hechas PLW0211 (método estático cuyo primer parámetro es `self` o `cls`), PLC3002 (`(lambda:
    x)()`) y DTZ002 (`datetime.today()` sin `.astimezone()` después). Paridad exacta: 20, 15 y 18.
    Los casos quedaron en `fixtures/methods.py`.
  - Hechas S112 (`try`/`except`/`continue`), C410 (`list([...])`), C406 (`dict([...])`), DTZ003
    (`datetime.utcnow()`), PLR1722 (`exit()` y `quit()`), UP021 (`universal_newlines` de
    `subprocess`), PLW1509 (`preexec_fn`), UP033 (`lru_cache(maxsize=None)`), B003 (asignar a
    `os.environ`) y PLC0208 (recorrer un conjunto escrito). Paridad: 7 de 8 S112 y exacta en el
    resto (10, 9, 10, 9, 7, 7, 8, 10 y 8); el S112 que falta está en un archivo de CPython con
    `lazy import`. Los casos quedaron en `fixtures/builtins_misc.py`.
  - Hechas UP037 (comillas de más en una anotación que Python no evalúa: con el import del futuro,
    en un stub o en una variable local de una función; también en un string adentro de otro) y
    UP044 (`Unpack[X]` adentro de un subíndice o como tipo de `*args`, también entre comillas).
    Paridad exacta: 172 y 183. Los casos quedaron en `fixtures/quoted.py`,
    `fixtures/quoted_future.py` y `fixtures/unpack.py`.
  - Hechas UP010 (`from __future__` de algo ya obligatorio), RUF017 (`sum(listas, [])`), B033
    (elemento repetido en un conjunto escrito, comparado por valor como el `HashableExpr` de ruff:
    `1`, `1.0` y `True` son el mismo), SIM101 (`isinstance` seguidos sobre el mismo valor en un
    `or`), B019 (`functools.cache` o `lru_cache` en un método de instancia, salvo en enumeraciones),
    PLR1704 (un `for`, `with` o `except` que pisa un parámetro de la función o de una que la
    contiene), RUF046 (`int()` de algo que ya es entero, con la aritmética entre enteros) y RUF013
    (`Optional` implícito, con el `TypingTarget` de ruff: lo que no es builtin ni de la biblioteca
    estándar cuenta como `Any`). Paridad exacta en B019 (37), B033 (29), RUF013 (96), RUF017 (19),
    RUF046 (107) y UP010 (22); 81 de 87 PLR1704 y 28 de 29 SIM101, sin avisos de más: lo que falta
    está en archivos de CPython con `lazy import`. Los casos quedaron en `fixtures/batch_misc.py`.
  - Hechas 17 reglas de `flake8-pyi`, en todos los archivos y no sólo en los stubs: PYI016 y PYI030
    (miembro repetido y varios `Literal` en una unión, también en un `Union[...]` que se evalúa en
    ejecución), PYI061 y PYI062 (`Literal[None]` y miembro repetido en un `Literal`), PYI064
    (`Final[Literal[1]]`, sólo con un literal escrito), PYI057 (`ByteString`, en el import y en
    `typing.ByteString`), PYI059 (`Generic[...]` que no es la última base), PYI050 (`NoReturn` en un
    parámetro, también entre comillas), PYI041 (`int | float` en un parámetro, salvo con
    `@f.register`), PYI032 (`Any` en `__eq__`), PYI063 (`__x` como sólo posicional), PYI058
    (`__iter__` de una sola sentencia que devuelve `Generator`), PYI029 (`__str__` repetido en un
    stub), PYI018 y PYI046 (`TypeVar` y protocolo privados sin usar), PYI042 (alias en minúscula,
    con `TypeAlias` resuelto) y PYI025 (`collections.abc.Set` sin alias). Paridad exacta en 15; faltan
    un PYI057 de CPython con `lazy import` y un PYI061 adentro de una anotación entre comillas
    (`-> 'Literal[None]'`), que todavía no se mira. Los casos quedaron en `fixtures/typing_hints.py`
    y `fixtures/typing_stub.pyi`. Quedan para otra tanda PYI034 y PYI019 (`Self`), PYI036 (la firma
    de `__exit__`) y PYI055 (`type[A] | type[B]`, que ruff no marca fuera de un contexto de tipos).
  - Hechas 30 reglas chicas en una tanda: los métodos especiales que devuelven un tipo equivocado
    (PLE0303, PLE0304, PLE0305, PLE0307, PLE0308 y PLE0309, también cuando no hay ningún `return` y
    devuelven `None` sin decirlo), PLE0101 (`__init__` que devuelve algo), PLR0206 (propiedad con
    parámetros), las comprensiones y conversiones de más (C403, C404, C411, C414 y C418), los
    booleanos (SIM208, SIM211, SIM222 y SIM223, con el `is_short_circuit` y el `Truthiness` de ruff),
    RUF034 (ternario con las dos ramas iguales), PERF102 (`d.items()` que descarta la clave o el
    valor; un nombre cuenta como usado si cualquier variable con ese nombre se lee en el alcance),
    PLR1730 (`if` que es un `min` o un `max`), PLW1508 (`os.getenv` u `os.environ.get` con un
    default que no es texto), RUF007 (`zip(x, x[1:])`), PLC0105 (la varianza en el nombre de un
    `TypeVar` asignado), B030, B016, PLW0129, PLW0177 (comparar con NaN), PIE800, PIE796 (valor
    repetido en un `enum.Enum` resuelto) y UP019 (`typing.Text`). Paridad exacta en todas salvo
    cuatro avisos de `test_compile.py`, uno de los archivos de CPython que tree-sitter no parsea.
    Los casos quedaron en `fixtures/small_checks.py`.
  - Hechas 24 reglas más en una tanda: PLC0206 (`for k in d:` que lee `d[k]`; como ruff, no entra
    en un subíndice que se lee pero sí en uno que se asigna), PERF402 (copiar una lista elemento
    por elemento, salvo con `async for`), RUF051 (`if k in d: del d[k]`) y el tipo de la variable
    con el `only_binding` de ruff (las asignaciones anteriores del alcance, `**kwargs`, el
    desempaquetado y la anotación de un solo tipo), PT031 y PT014 (pytest), PLW0602 (`global` sin
    asignar), RUF019, PLR1716, FURB192, FURB105, RUF057, PLE0605, PLW0131, RUF041, D419, FURB166,
    UP014, G201, B035, B005, PLW0245, B021, B012 y B013. Paridad exacta salvo un D419 y un FURB105 en
    dos archivos que no están en UTF-8 y que ruff no lee (su E902); kerbo los analiza igual. Los
    casos quedaron en `fixtures/mixed_checks.py`.
  - Hechas 27 reglas más en una tanda: UP028 (`for x in y: yield x`, también con tuplas; no en
    funciones `async` ni si los nombres se leen en otro lado del alcance), PIE810, C417, PLC0132,
    YTT201, YTT203, RUF040, PLR1733 y PLR1736 (hasta la sentencia que modifica la secuencia, sin
    entrar en los destinos), FURB136, PLR1708, FURB132 (sólo con un nombre o un literal), UP050,
    RUF020 (cada `|` en cualquier contexto), TRY401, FURB161, FURB168, PLE0117, LOG001, FURB163,
    DTZ004, PERF101, B032 (salvo `self.x: T`), PLW0711, ASYNC251, ASYNC230 y ASYNC115. Paridad exacta
    salvo cuatro avisos sueltos: dos C417 de `test_builtin.py`, donde kerbo no toma `list` como el
    builtin porque otras funciones del archivo lo reasignan, y dos PERF101 (`plugin_base.py` de
    sqlalchemy y `test_capi/test_time.py`) sin explicar todavía. Los casos quedaron en
    `fixtures/tail_checks.py`.
  - Pendientes que piden más análisis: PIE810 (si un nombre está ligado a una tupla, con el
    `check_type` de ruff), UP028 (las referencias de cada nombre al cerrar el alcance) y C414.
  - Hechas 27 reglas más en una tanda: FURB188, SIM401 (también la forma `d[k] if k in d else x`,
    estable en ruff 0.17), FURB122 y FURB129 (con el `is_io_base` de ruff sobre el binding al que
    resuelve el nombre), PLE1300 (formato `%` y `.format`, con el `FormatSpec::parse` de ruff),
    PLE1307, RUF063, RUF068, RUF009, ASYNC100, ASYNC222, PLE2502, PTH124, W605, UP049, PYI006, UP029,
    PLE0643, PLW0406 (sólo dentro de un paquete; en un `__init__.py` nunca coincide, como en ruff),
    INT002 (sin el plural de `ngettext`, que es de preview), EXE005, PLE0604, PYI020, PLE1142, TC005,
    UP036 (con 3.11 como versión mínima) y SIM113. Paridad: 173 de 178, sin avisos de más; los cinco
    que faltan están en archivos de CPython que tree-sitter no parsea (`lazy import` y
    `test_type_params.py`). Los casos quedaron en `fixtures/assorted_checks.py`,
    `fixtures/assorted_stub.pyi` y `fixtures/control_chars.py`.
  - Pendientes de esa tanda: W605 no mira los especificadores de formato de un f-string ni una barra
    al final de un tramo literal antes de `{`; SIM401 calcula el largo de la línea con el texto
    original y no con el generador de ruff; PYI020 no mira anotaciones entre comillas adentro de
    otras; RUF009 trata las clases de `attrs` como dataclasses comunes (sin `auto_attribs`); PLE1307
    no resuelve el tipo de una operación binaria (`"a" + "b"`). Ninguna de estas reglas trae arreglo
    todavía (ruff lo tiene en FURB188, SIM401, FURB122, FURB129, W605, UP049, UP029, PYI020, TC005,
    UP036 y RUF068).
  - Hecha RUF100 (`noqa` sin uso), con el `check_noqa` de ruff: cada aviso cuenta para el primer
    `noqa` que lo silencia (la línea de la sentencia y después la del aviso), los códigos viejos
    redirigen al nuevo, un código de una regla apagada sale como "non-enabled" y uno que ruff no
    conoce se deja (es RUF102). La lista de reglas de ruff 0.17.1, su conjunto por defecto (407) y las
    redirecciones quedaron en `noqa/ruff_codes.rs`. Lo que está apagado lo decide la configuración de
    kerbo, así que la medición apaga las 17 reglas que kerbo tiene y el conjunto por defecto de ruff
    0.17 no (E4, E7, F403, F405 y F722). De paso, `# noqa[E501]` dejó de ser una directiva, como en
    ruff. Paridad: 1.437 de 1.437, con uno de más (un `noqa` adentro de `if TYPE_CHECKING:` en
    `sqlalchemy/orm/decl_api.py` que en ruff silencia un TC004, regla que kerbo todavía no tiene).
    Las otras 281 reglas del conjunto por defecto no cambiaron.
  - Pendientes de RUF100: si kerbo no implementa una regla del conjunto por defecto de ruff, un
    código suyo en un `noqa` no se marca nunca (no se sabe si se usa); en un archivo que tree-sitter
    no parsea sólo se marcan los códigos de reglas apagadas o repetidos. Falta el arreglo (borrar el
    comentario o los códigos de más).
  - Hechas las 6 reglas que quedaban del conjunto por defecto (2026-10-24): PYI036 (la firma de
    `__exit__`/`__aexit__`, también con dos sobrecargas), PYI034 (el retorno que debería ser `Self`,
    con el `is_metaclass` de ruff y las bases heredadas de clases del archivo), PYI019 (`self: _S` con
    un `TypeVar` propio o un parámetro de tipo del método; con anotaciones diferidas se resuelve al
    final del módulo, como ruff), TC004 (un import de `if TYPE_CHECKING:` usado en ejecución; el
    contexto de cada referencia sale del árbol al final: anotaciones que no se evalúan, strings,
    cotas de parámetros de tipo y `__all__`), PYI055 (varios `type[...]` en una unión, sólo donde no
    se evalúa, también entre comillas) y RUF028 (los comentarios `fmt: off`/`on`/`skip` y `yapf:`,
    con el `SuppressionCommentVisitor` de ruff recorrido sobre el árbol de tree-sitter traducido a la
    forma del AST de ruff). Paridad exacta en las seis: 47, 26, 22, 17, 8 y 6 avisos, y RUF100 quedó en
    1.437 de 1.437 sin el aviso de más que dependía de TC004. Los casos quedaron en
    `fixtures/self_types.py`, `fixtures/self_types_stub.pyi`, `fixtures/type_checking_imports.py` y
    `fixtures/formatter_comments.py`. Con esto kerbo cubre todas las reglas del conjunto por defecto de
    ruff 0.17 que aparecen en el corpus.
  - Pendientes de esa tanda: ninguna de las seis trae arreglo (ruff lo tiene en todas). PYI016 no
    toma `Optional[X]` como una unión con `None` (`Optional[None]` da un PYI016 en ruff). PYI055 no
    mira un `|` que no está en una anotación aunque esté adentro de `if TYPE_CHECKING:`. PYI019 no
    exige que el archivo importe `typing` (el `seen_typing` de ruff). TC004 no tiene la opción
    `quote-annotations` ni las bases que ruff evalúa en ejecución (`runtime-evaluated-base-classes`).
    RUF028 no entra en las interpolaciones de un f-string ni separa los destinos de `a = b = c`.
  - Hechas 20 reglas de pylint que no aparecen en el corpus, medidas sólo con fixtures
    (`fixtures/pylint_errors.py`): PLE0100 (`yield` en `__init__`), PLE0115 (`nonlocal` y `global`),
    PLE0118 (un nombre usado antes de su `global`, también en el módulo), PLE1132 (clave repetida
    entre `k=` y `**{...}`), PLE1205 y PLE1206 (argumentos de un logging con `%`), PLE1310 (`strip`
    con caracteres repetidos, con `Known::String`/`Bytes` para las variables), PLE1507 (`os.getenv`
    con una clave que no es texto), PLE1519 y PLE1520 (`singledispatch` en un método y
    `singledispatchmethod` en una función), PLE1700 (`yield from` en una función `async`), PLE2510 y
    PLE2512–2515 (caracteres de control escritos tal cual en un string), PLW0604 (`global` en el
    módulo), PLW1501 (modo inválido de `open`), PLW1507 (`copy.copy(os.environ)`) y PLW2101 (un
    candado nuevo en un `with`). De paso, F704 marca un `yield` adentro de una comprensión (salvo en
    el primer iterable, que se evalúa afuera). Cero avisos de más en el corpus. PLE0116 (`continue`
    en un `finally`) no hace falta: ruff sólo la da con una versión de Python anterior a 3.8.
  - Pendientes de esa tanda: un byte NUL en el código rompe el parseo de tree-sitter, así que en ese
    archivo PLE2514 y las reglas de alcance no corren (Python tampoco lo acepta). Con código que
    Python rechaza (asignar un nombre antes de su `global`) ruff da además PLW0602, F811 y PLW0603
    que kerbo no imita. Ninguna trae arreglo (ruff lo tiene en PLE1519, PLE1520, PLE2510–2515 y
    PLW1507).
  - Hechas 24 reglas de `flake8-pyi` que no aparecen en el corpus, medidas con
    `fixtures/stub_rules.pyi` y `fixtures/stub_rules_runtime.py`: las condiciones de un `if` en un stub
    (PYI002, PYI003, PYI004, PYI005, PYI007, PYI008 y PYI066, cada parte de un `and`/`or` aplanada como
    en ruff), las asignaciones de un stub fuera de funciones (PYI001, PYI015, PYI017, PYI026, PYI035 y
    PYI052, con los valores simples de `is_valid_default_value_with_annotation` y las clases `enum`
    resueltas por sus bases), los cuerpos (PYI009, PYI010, PYI012, PYI048 y PYI013, que corre en
    cualquier archivo), PYI033 (`# type:`), PYI044 (`from __future__ import annotations`), PYI043 (alias
    privado terminado en `T`, en cualquier archivo), PYI045 (`__iter__` que devuelve `Iterable`) y
    PYI047 y PYI049 (alias y `TypedDict` privados sin usar; fuera de un stub sólo en el módulo y las
    funciones). Cero avisos de más en el corpus.
  - Pendientes de esa tanda: ninguna trae arreglo (ruff lo tiene en PYI009, PYI010, PYI012, PYI013,
    PYI015, PYI026 y PYI044). PYI042 y PYI043 no miran un `x: TypeAlias` sin valor. PYI049 toma como
    `TypedDict` funcional también una asignación anotada.
  - Hechas 22 reglas más que no aparecen en el corpus, medidas con `fixtures/ruff_upgrades.py`:
    UP001 (`__metaclass__ = type`), UP023 (`cElementTree`), UP026 (el paquete `mock`), las YTT que
    faltaban (YTT101, YTT102, YTT103, YTT202, YTT204, YTT301, YTT302 y YTT303), RUF008 (valor mutable
    en un campo de dataclass), RUF016 (índice de otro tipo sobre una lista, tupla o texto literal),
    RUF018 (`:=` en un `assert` usado afuera), RUF024 (`dict.fromkeys` con un valor mutable), RUF026
    (`defaultdict(default_factory=...)`), RUF030 (`assert x, print(...)`), RUF033 (`__post_init__` con
    valores por defecto), RUF048 (`map(int, __version__.split("."))`), RUF049 (dataclass que es un
    `enum`), RUF058 (`starmap` sobre un `zip`) y RUF101 (código viejo en un `noqa`). Cero avisos de más
    en el corpus. UP040, UP043, UP046, UP047 y RUF053 no hacen falta: ruff sólo las da con Python 3.12
    o 3.13 como versión mínima, y por defecto supone 3.11. RUF200 valida el
    `pyproject.toml`, que kerbo no lee como Python.
  - Pendientes de esa tanda: ninguna trae arreglo (ruff lo tiene en UP001, UP023, UP026, RUF024,
    RUF026, RUF030, RUF033, RUF058 y RUF101). RUF018 sólo mira el último binding de cada nombre.
  - Hechas 22 reglas más que no aparecen en el corpus, medidas con `fixtures/library_checks.py`:
    ASYNC105 (función async de trio sin `await`), ASYNC116 (`sleep` de más de un día), ASYNC210,
    ASYNC220 y ASYNC221 (HTTP y procesos que bloquean en una función async), B014, B025 y B029
    (excepciones repetidas o una tupla vacía en un `except`), B022 (`contextlib.suppress()` vacío),
    B039 (`ContextVar` con un default mutable), C415 (`sorted(x[::-1])`), SIM107 (`return` en el
    `finally`), SIM220 y SIM221 (`a and not a`, `a or not a`), SIM911 (`zip(d.keys(), d.values())`),
    FURB162 (el `Z` reemplazado a mano antes de `fromisoformat`), FURB169 (`type(x) == type(None)`),
    FURB177 (`Path().resolve()`), FURB181 (`.digest().hex()`), PTH210 (`with_suffix` sin punto),
    PGH005 (métodos de un mock sin llamar o que no existen) y PLC0131 (`TypeVar` covariante y
    contravariante). Cero avisos de más en el corpus. De paso, `Limits` pasó los 256 bytes con las
    reglas nuevas y ahora se pasa por referencia en todo el código.
  - Pendientes de esa tanda: B031 (reusar el generador de `itertools.groupby`) quedó afuera: pide
    seguir los usos del grupo por ramas y loops. Ninguna trae arreglo (ruff lo tiene en B014, SIM911,
    FURB162, FURB169, FURB177, FURB181, PTH210 y ASYNC105). PTH210 sólo reconoce un nombre ligado a una
    llamada a `Path(...)`, no a una anotación.
  - Los archivos que no están en UTF-8 (ruff da E902 y no los analiza) se analizan igual: falta
    saltearlos o avisar como ruff. Se hacen en tandas de 15 a 25 reglas por
    tema, con un solo gate y una sola medición por tanda.
  - Hechas las 18 reglas que quedaban del conjunto por defecto de ruff 0.17 y del clásico, medidas con
    `fixtures/plugin_checks.py` (2026-10-25): EXE001 y EXE002 (shebang contra el permiso de
    ejecución; como ruff, sólo en Unix y fuera de WSL), EXE004 (espacios antes del shebang), INT001 e
    INT003 (f-string y `%` adentro de `gettext`; INT002 pasó al análisis semántico para reconocer
    `import gettext as gt` como ruff), LOG002 (`getLogger(__file__)`), LOG009 (`logging.WARN`), LOG014
    (`exc_info=True` fuera de un `except`), G010 (`warn`), G101 (`extra` que pisa un atributo de
    `LogRecord`), G202 (`exc_info` de más en `exception`), PT010, PT020, PT025 y PT026 (pytest), TC010
    (un string en una unión `X | Y` que se evalúa), F407 (característica de `__future__` que no
    existe) y F622 (dos destinos con `*`). G201 se mudó con las otras de logging y ahora reconoce los
    loggers con las mismas reglas que ruff (`logging.getLogger().x`, `current_app.logger`, `LOG`,
    `from logging import error`). Corregido de paso `is_typing_only`: una anotación que se evalúa
    adentro de `if TYPE_CHECKING:` contaba como de ejecución (afectaba TC004, PYI055 y TC010). Cero
    diferencias en el corpus salvo EXE002 (ver pendientes).
  - No hacen falta: FA100 y FA102 (ruff sólo las da con Python anterior a 3.10, y kerbo supone 3.11),
    TC007 (ruff la calla con TC004 prendido, y la referencia del propio alias ya cuenta como uso en
    ejecución, así que en el conjunto por defecto no sale nunca) y F621 (en ruff 0.17 es código
    muerto: `starred_index` siempre es `None`).
  - La caché de Python no tenía en cuenta la ruta del archivo: dos archivos con el mismo contenido en
    lugares distintos compartían el resultado, aunque N999, PLW0406 y los stubs dependen de la ruta.
    Ahora la clave suma la ruta y el permiso de ejecución, como Shell suma su dialecto.
- Pendientes de Python contra ruff:
  - Hecha B031 (2026-10-25), el grupo de `itertools.groupby` usado más de una vez, con el
    `GroupNameFinder` de ruff traducido al árbol de tree-sitter: cuenta por rama en `if` y `match`,
    vuelve a cero con `return`, `break` y `continue`, y deja de buscar cuando se reasigna el nombre.
    Paridad exacta en `fixtures/groupby_reuse.py` (25 avisos); en el corpus no aparece. Con esto
    están todas las reglas del conjunto por defecto de ruff 0.17 que pueden salir con Python 3.11;
    del clásico sólo queda E902, que no es una regla.
  - EXE002 en el corpus: kerbo da 10.905 avisos que ruff no, porque el corpus se analiza montado
    desde Windows en Docker, donde todo archivo aparece como ejecutable, y ruff corre en Windows, donde
    la regla no existe. Ruff en ese mismo contenedor daría lo mismo (no es WSL), así que no se tocó;
    falta medirlo con ruff en Linux sobre un clon con permisos reales.
  - Ninguna de las 18 reglas de la última tanda trae arreglo (ruff lo tiene en EXE004, LOG002, LOG009,
    LOG014, G010, PT020, PT025 y PT026). TC007 con TC004 apagado tampoco se imita.
- Arreglos de Python contra `ruff check --fix` (empezados, 2026-10-25). De las 412 reglas que tiene
  kerbo, ruff arregla 216 (114 siempre, 102 a veces); kerbo arreglaba 10.
  - Tanda 0, la base: cada arreglo es seguro o inseguro, como en ruff; `kerbo fix` (y `kerbo_fix` del
    MCP) sólo aplica los inseguros con `--unsafe-fixes`, y el "arreglar todo" del LSP nunca. Los
    arreglos también se pueden aislar por grupo (el `isolate` de ruff: dos sentencias borradas del
    mismo bloque no van en la misma vuelta, que lo dejaría vacío). `statement_removal.rs` es el
    `delete_stmt` de ruff: `pass` si la sentencia queda sola, sólo ella y su `;` si comparte la línea,
    y si no las líneas enteras con su comentario.
  - La paridad se mide con `tests/fix_parity.rs`: cada fixture tiene al lado lo que deja
    `ruff check --fix` (`.fixed`) y con `--unsafe-fixes` (`.unsafe`), limitado con `--fixable` a las
    reglas que kerbo arregla; kerbo aplica sus arreglos en vueltas como ruff y tiene que dejar el mismo
    archivo, byte por byte. Con `KERBO_FIX_DUMP=<carpeta>` deja lo suyo para compararlo a mano.
  - Los 10 que había quedaron idénticos a ruff en los 54 fixtures: F401 (borra la línea con su
    comentario y se anima con `;`), F841 (ahora con los cuatro casos de ruff, inseguros: borrar la
    asignación, dejar el valor si tiene efectos, `_x` al desempaquetar una tupla literal y sacar el
    `as x` de un `with`; el del `except` sigue seguro), E711 y E712 (inseguros, y `x == True` pasa a
    `x`), F541 (inseguro en posición de docstring), F632, E713, E714, E703 y F901.
  - Tanda 1 (19 arreglos, idénticos a ruff en los 55 fixtures; `fixtures/fix_edges.py` junta los
    casos borde): UP003, UP004, UP005, UP008, UP009, UP010, UP011, UP012, UP019, UP020, UP025, UP034,
    UP037, UP039, PIE790, PIE807, PIE808, B009 y SIM118. Piezas nuevas que usan las tandas que siguen:
    `fix_edits.rs` (el `remove_argument` y el `pad` de ruff, y la regla de que un comentario en lo que
    se borra vuelve inseguro el arreglo) y `semantic/importer.rs` (el `get_or_import_symbol` de ruff
    para `import módulo`: reusa un import que ya está, con un no-op sobre él para chocar con un F401 de
    la misma vuelta, o inserta el import después del último de primer nivel, del último
    `from __future__` o del docstring y los comentarios del principio). Dos arreglos que se tocan
    ahora chocan, como en ruff (`a = b = 1` sin usar se borra en dos vueltas). De paso, una anotación
    entre comillas que termina en un comentario daba F722: el envoltorio que la parsea cerraba el
    paréntesis adentro del comentario.
  - Pendientes de esa tanda: PIE807 no mira si `list` o `dict` están tapados (ruff usa
    `builtins.list`); B009 marca inseguro cualquier nombre no ASCII, y ruff sólo los que cambia NFKC.
  - Tanda 2 (15 arreglos, idénticos a ruff en 56 fixtures; `fixtures/import_fixes.py` junta los casos
    del importer): UP006 (también adentro de una anotación entre comillas simple), UP017, UP024, UP033,
    UP041, FURB167, LOG002, LOG009, LOG014, G010, PT025, PT026 y EXE004. El importer quedó completo
    como el de ruff: `from módulo import miembro` (sumándolo con libcst a un `from` que ya está, con la
    misma coma que el último), `import módulo.sub` sirve para `módulo.x`, entre varios gana el último
    del alcance, y no hay arreglo si el import está después del uso o es sólo de tipado y el uso en
    ejecución. PT020 no tiene arreglo en ruff sin preview.
  - Pendientes de esa tanda: UP006 adentro de un string no arregla `Deque` ni `DefaultDict` (piden el
    import) ni un builtin tapado; G010 sobre `from logging import warn` deja `warning` sin importarlo,
    como ruff.
  - Tanda 3 (6 arreglos y el generador, idénticos a ruff en 57 fixtures): `codegen.rs` es el
    `unparse_expr` del generador de ruff sobre el árbol de tree-sitter (precedencias, `and`/`or` en
    cadena, tuplas sin paréntesis donde no hacen falta, `(1).real`, las anotaciones de tree-sitter
    como `generic_type` y `union_type`); los strings y números quedan como en el código, y ruff los
    vuelve a escribir desde el valor (sólo cambia con escapes o números en otra base). Con él: UP007 y
    UP045 (también adentro de una anotación entre comillas simple; ruff sólo los arregla en una
    definición de tipo), SIM201, SIM202, SIM208 y B010.
  - B010 no arregla `(setattr(x, "a", v))`: quedaría `(x.a = v)`, que no es Python. Ruff lo arregla
    igual, ve el error de sintaxis y revierte todos los arreglos del archivo; ese caso quedó solo en
    `fixtures/parenthesized_setattr.py`.
  - Pendientes de esa tanda: SIM208 en una expresión que no es una condición pone `bool(x)` sin mirar
    si `bool` está tapado (ruff entonces no arregla).
  - Tanda 4 (11 arreglos de comprehensions, idénticos a ruff en 57 fixtures; todos inseguros, como
    en ruff): C400, C401, C402, C403, C404, C405, C409, C410, C411, C418 y C419, en
    `semantic/comprehension_fixes.rs`. Las de ruff que cambian llaves o corchetes trabajan sobre
    tokens y quedaron calcadas; las que usan libcst (C402, C404, C411, C419 con un argumento) se
    imitan conservando los espacios como lo hace libcst.
  - Pendientes de esa tanda: C402 y C404 no se arreglan si el argumento tiene paréntesis de más, y C419
    con un solo argumento no se arregla si la comprensión tiene comentarios o saltos de línea (libcst
    los reubica).
  - Tanda 5 (4 arreglos de comprehensions, entre ellos el más frecuente del corpus, idénticos a ruff
    en 60 fixtures; `quote_style.py` y `collection_edges.py` juntan los bordes): C408 (`dict(a=1)` pasa
    a `{"a": 1}` con la comilla del primer string del archivo que no es triple, o la otra adentro de un
    f-string), C406, C413 (sólo con un nombre suelto afuera, como ruff; `reversed(...)` invierte o suma
    `reverse=` con la negación sin paréntesis de libcst) y C414.
  - Tanda 6 (7 arreglos sueltos, idénticos a ruff en 61 fixtures; `small_fixes.py` junta los
    bordes): RUF059 (`_x`, inseguro), PLC0414 (inseguro), PLR2044 (la línea entera, desde el código o
    detrás de una continuación), B033 (el `remove_member` de ruff, que comparte la mitad de
    `remove_argument`), RET501, PLR1711 (respeta un comentario al lado y deja `pass` si queda sola) y
    FURB157 (signo, ceros y separadores `_` como ruff). PLW1510 no va: el arreglo de ruff es sólo para
    mostrar y nunca se aplica.
  - Tanda 7: RUF100, idéntico a ruff en 62 fixtures (`noqa_fixes.py` junta los bordes). Cada
    directiva `noqa` guarda ahora su rango, como en ruff (`# noqa` o hasta el último código); sin
    códigos que sigan sirviendo se borra con el `delete_comment` de ruff (la línea entera si está
    sola, desde el código o sólo la directiva si después viene otro comentario), y si no, se reescribe
    con los que se usan (`# noqa: `, `# ruff: noqa: ` o `# flake8: noqa: `).
  - Tanda 8: I001, idéntico a ruff en todos los fixtures. Reescribe el bloque ordenado con la sangría del
    bloque, y el orden en que se aplican los arreglos quedó calcado del `apply_fixes` de ruff: por
    dónde empiezan, y a igualdad por el puesto del aviso en la lista de ruff (los del árbol, después
    los I001 y al final los RUF100; los silenciados por `noqa` salen con `swap_remove`, así que los
    últimos avisos pasan a ocupar su lugar y eso decide si gana I001 o F401 sobre el mismo import).
    F401 y UP010 con nombres que quedan reescriben la sentencia entera, como libcst en ruff.
  - Pendientes de esa tanda: entre los avisos del árbol, el puesto sigue el orden en que los genera
    kerbo y no el de ruff (que corre antes las reglas de tokens y saca F401 alcance por alcance); sólo
    importa si dos arreglos así empiezan en el mismo lugar.
  - Tanda 9 (8 arreglos, idénticos a ruff en todos los fixtures; `format_arguments.py` y
    `small_fixes_two.py` juntan los bordes): F504, F522 y F523 (el `retain` de libcst: cada elemento
    se lleva su coma y el espacio hasta el siguiente; F523 sólo saca posicionales del final), E401,
    W605 (barra doble o raw según los escapes válidos de la parte; ahora también avisa en el formato
    de una interpolación de f-string, como ruff), PLC0208, PLR0402 y PLR1722 (`sys.exit` con el
    importer).
  - Pendientes de esa tanda: los efectos de F504/F522/F523 toman cualquier nombre como builtin (ruff
    mira si está tapado); E401 detrás de algo que no es `;` (`if x: import a, b`) no se arregla, y
    ruff lo arregla, ve el error de sintaxis y revierte el archivo.
  - Tanda 10 (37 arreglos, idénticos a ruff en todos los fixtures): UP018 (el literal o el valor cero,
    con los paréntesis de la precedencia de ruff), RUF010 (`{x!s}`, reescribiendo la parte del
    f-string como libcst), PIE804, PIE800 (sobre tokens, como ruff), PERF102, PIE810 y SIM101 (las
    llamadas juntas), TRY201, PLW0133, SIM210, SIM211, SIM222, SIM223, B004, UP021, UP022, B013,
    LOG001, FURB129, FURB132, FURB161, FURB163, FURB166, FURB168, FURB192, PLR1733 y PLR1736 (con los
    no-op de ruff sobre las variables del loop), RUF019, RUF051, RUF020 (`Never | T`), UP028, UP029,
    PIE794, PYI016, PYI018, PT014 y TC005. `scripts/parity/fix-dump.sh` muestra todas las diferencias
    con ruff de una vez y deja en `target/fix-dump/*.passes` qué arreglo se aplicó en cada vuelta.
    `loop_lookups` pasó antes que `literal_checks` en el orden de los avisos: ruff genera PLR1733
    antes que PERF102 y el empate entre los dos se decide por ese orden.
  - Pendientes de esa tanda: F601 y F602 no tienen arreglo (no hay casos en los fixtures para
    medirlo); RUF020 en `Union[...]` y PYI016 con un `Optional` adentro no se arreglan; SIM210 no
    mira si `bool` está tapado; UP028 no mira `global`/`nonlocal`; FURB192 avisa aunque `reverse` no
    sea un literal (ruff no); SIM222/SIM223 cortan en el primer par sin decidir y ruff sigue mirando;
    RUF010 no le pone paréntesis a un generador sin paréntesis; RUF019 no distingue los efectos
    "quizás" de ruff (los trata como seguros).
  - Tanda 11 (40 arreglos, idénticos a ruff en todos los fixtures): RUF046, RUF057 (con el
    `unwrapped_call_argument` de ruff), FURB105, ISC004, RUF015, PYI064, PYI062, PYI059, RUF032,
    RUF007, PLR1716, PYI032, FURB136, PYI029, PYI020, UP050, PERF101, PLR1730, PYI036, ASYNC115,
    PYI058, RUF068, SIM401, FURB188, RUF017, FURB122, RUF041, PYI061, PYI030, PYI026, PYI041, PYI055 y
    UP044 (también adentro de una anotación entre comillas), RUF013, SIM103, FLY002, UP030 (con la
    misma regex de ruff y sus rarezas), PLW0120, UP035 (el `ImportReplacer` de ruff) y F811. Dos
    cambios de orden para desempatar como ruff: los avisos del recorrido semántico van antes que los
    de los alcances diferidos (F401 y compañía), y los arreglos de F811 se aplican antes que todos.
  - Pendientes de esa tanda: E731, SIM905, SIM114, B006 y PYI025 quedan para la próxima (piden
    reescribir sentencias enteras o renombrar en todo el archivo). FURB136, PLR1730, SIM103 y
    FURB192 no miran si `min`, `max` o `bool` están tapados; RUF046 y RUF057 tratan como inseguro
    todo `round` de algo que no está escrito (ruff mira el tipo); PYI061 y RUF013 asumen Python 3.10
    o más (`X | None` y nunca `Optional`); FLY002 no arregla cuando habría que escapar algo más que
    las llaves; PLW0120 desindenta línea por línea (un string de varias líneas en el `else` se
    rompería; ruff usa libcst).
  - Tanda 12 (33 arreglos, idénticos a ruff en todos los fixtures): E731 (la lambda pasa a `def`
    con el cuerpo tal como está), PLE2510, PLE2512, PLE2513, PLE2515, PYI009, PYI010, PYI012,
    PYI013, PYI015, PYI044, FURB162, FURB169, FURB177, FURB181, SIM911, SIM220, SIM221, PTH210,
    PLW1507, RUF024, RUF026, RUF030, RUF058, RUF101, B014, ASYNC105, ASYNC116, PLE1519, PLE1520,
    UP001, UP023 y UP026. Dos ajustes más del orden de ruff: los avisos de una sentencia van detrás
    de los de sus expresiones (ruff analiza la sentencia al final), y F401, F841 y RUF059 (los de los
    alcances diferidos) van detrás de todos los del árbol.
  - Pendientes de esa tanda: E731 no arregla con una anotación `Callable` (ruff la pasa a la firma);
    RUF030 sólo arregla `print("texto")` (ruff arma una f-string con varios argumentos); UP026 sólo
    arregla `import mock` con un nombre en la sentencia. Quedan para la próxima B006, C417, SIM905,
    SIM114, UP049, PYI025, RUF033, UP014, F601 y F602.
  - Tanda 13 (7 arreglos, idénticos a ruff en todos los fixtures): SIM905 (la lista de pedazos, con
    los espacios y los límites de `str.split`), B006 (el default pasa a `None` y se inicializa al
    principio del cuerpo), SIM114, SIM102 y SIM117 (el `if` o el `with` de adentro sube un nivel, como
    el libcst de ruff), RUF022 y RUF023 (orden de `__all__` y `__slots__` en una línea).
  - Pendientes de esa tanda: RUF022 y RUF023 no se arreglan con varias líneas (ruff conserva los
    comentarios de cada elemento); SIM102 y SIM117 desindentan línea por línea (un string de varias
    líneas adentro quedaría corrido) y miden 88 columnas fijas; SIM117 no junta la forma con
    paréntesis. Siguen pendientes C417, PYI025, UP014, UP049, RUF033, F601, F602 y PLW0245.
  - Tanda 14 (10 arreglos, idénticos a ruff en todos los fixtures; `small_fixes_three.py` junta los
    bordes de PLW0245, F601 y F602): PLW0245 (también `(super).x()`), F601 y F602 (sólo cuando el
    valor también se repite, desde el final del elemento anterior), RUF028 (con el `delete_comment`
    de RUF100, que pasó a `fix_edits.rs`), RUF033 (el parámetro pasa a un campo `InitVar` arriba de
    `__post_init__`, con el importer), UP014 (la clase impresa con el generador y la sangría del
    archivo), C417 (generador o comprensión con el cuerpo de la lambda tal cual), UP036 (las cuatro
    formas de ruff: borrar el `if`, el `elif` que pasa a `if`, el cuerpo que sube un nivel con el
    `dedent_to` de ruff y el `elif` que pasa a `else`), PYI025 y UP049 (un renombrador sobre los
    bindings del modelo, como el `Renamer` de ruff). PYI025 ahora sale de los bindings, al final del
    análisis semántico, como en los alcances diferidos de ruff.
  - Pendientes de esa tanda: el renombrador no sigue los `global`/`nonlocal` ni renombra adentro de
    `__all__` o de anotaciones entre comillas (en esos casos no arregla); UP036 y el cuerpo que sube
    un nivel no arreglan si hay un string de varias líneas o sangría mezclada (ruff usa libcst);
    UP014 no mira los nombres dunder ni los `*` de los campos al detectar; RUF033 asume `\n` como
    fin de línea.
  - Tanda 15 (UP031 y UP032, unos 16.000 arreglos del corpus; idénticos a ruff en todos los
    fixtures, con `printf_fixes.py` y `fstring_fixes.py` para los bordes): UP031 pasa `%` a
    `.format(...)` con el `percent_to_format` de ruff (flags, ancho, precisión, `{{` y `\N{…}`, los
    paréntesis entre el string y el `%`, tuplas de un elemento y diccionarios en una o varias líneas),
    y UP032 pasa `.format(...)` a f-string con el `try_convert` de ruff (prefijos `u` y `r`, partes
    vacías o sólo literales, paréntesis según el contexto, índices con la otra comilla y el espacio
    detrás de `return`). UP032 pierde el empate con F52x, UP030, FLY002 y SIM905 en la misma llamada,
    como en el orden de avisos de ruff. `scripts/parity/reverting_lines.py` encuentra las líneas por
    las que ruff revierte un archivo entero.
  - Pendientes de esa tanda: ruff deja f-strings inválidas en `"{}".format(lambda: x)`,
    `"{0!x}".format(1)` y `"{}".format({…}.get(1))`, ve el error de sintaxis y revierte todos los
    arreglos del archivo; kerbo no ofrece esos arreglos y sí aplica el resto (`reverted_formats.py`
    tiene los casos donde los dos coinciden). UP031 con `(("…")) % x` repite los paréntesis como
    ruff y también rompe la sintaxis.
  - `kerbo fix` (y el LSP) ahora deja el archivo como estaba si una vuelta deja más errores de
    sintaxis, como ruff, en vez de quedarse con las vueltas anteriores: pueden ser parte de la cadena
    que lo rompió. Vale para todos los lenguajes.
  - Tanda 16 (PYI034 y TC004, idénticos a ruff en todos los fixtures; `self_return_fixes.py` y
    `type_checking_moves.py` juntan los bordes): PYI034 pasa el retorno a `Self` con el importer y
    le saca la anotación a un `self: Clase` o `cls: type[Clase]` (también `typing.Type`); si la
    clase podría ser genérica (parámetros de tipo, o una base `X[...]` con `AnyStr`, un atributo, un
    `*X`, un nombre desconocido o uno ligado una sola vez a `TypeVar`, `ParamSpec` o
    `TypeVarTuple`) ruff sólo lo muestra y kerbo no lo ofrece. TC004 es el `move_imports` de ruff:
    saca los nombres de la sentencia y los importa después del último import de primer nivel antes
    del primer uso, con la sentencia reconstruida como el `retain_imports` de libcst.
  - Esa reconstrucción (cada nombre con su coma y lo que la sigue, la coma final para el último salvo
    que la suya tenga un comentario) es ahora también la de F401 y los demás que sacan nombres de un
    import, como el `remove_imports` de ruff: antes se borraban líneas y un import de varias líneas
    con comentarios quedaba sin arreglo.
  - `kerbo fix` hace hasta 100 vueltas, como ruff (eran 10): TC004 mueve una sentencia por vuelta en
    cada bloque, así que un bloque con muchas no llegaba a vaciarse.
  - Pendientes de esa tanda: con varias sentencias para mover, ruff las recorre en el orden de un
    `FxHashMap` (por el número de nodo de la sentencia) y kerbo en el del código; los imports
    insertados quedan en otro orden, que I001 después iguala (con `--select TC004` solo pueden
    diferir). Si un nombre de la sentencia está silenciado con `noqa`, ruff mueve sólo los demás y
    kerbo los mueve todos. La estrategia de poner comillas (`quote-annotations`, apagada por
    defecto en ruff) no está. PYI034 con `self: (Clase)` borra hasta antes del paréntesis y rompe la
    sintaxis, en ruff y en kerbo: los dos revierten el archivo.
  - Tanda 17: PYI019, idéntico a ruff en todos los fixtures (`self_type_variables.py` y
    `self_type_variables_future.py` juntan los bordes). Importa `Self`, le saca la anotación al
    primer parámetro, saca el parámetro de tipo de la lista si lo era y cambia por `Self` cada uso de
    la variable después de esa anotación y dentro del método, también adentro de una anotación entre
    comillas. Si algún uso de la variable en el archivo está en un string con escapes no hay arreglo,
    y es inseguro si alguna edición se lleva un comentario, como en ruff. Como ruff lo calcula al
    analizar los bindings, kerbo avisa al recorrer el árbol y arma el arreglo al final, cuando el
    modelo ya tiene todas las referencias.
  - Con eso no quedan arreglos de ruff pendientes en el corpus (medido el 2026-10-26 con
    `../kerbo-parity/scripts/pending_by_frequency.txt`); PLW1510 no va (el arreglo de ruff es sólo
    para mostrar).
  - Las reglas de alcance en archivos que tree-sitter no parsea, y `lazy import` cuando la
    gramática lo sume.
  - `noqa` no pasa por el registro de supresiones de `src/suppressions.rs`: lo silenciado no aparece
    como suprimido con su motivo.
  - `short-name` de Python marca los mismos `l`, `O` e `I` que `e741`: hoy salen los dos.
  - I001: el aviso que falta en `pydantic-core/python/pydantic_core/_pydantic_core/__init__.pyi`
    (ruff lo da y kerbo no; sospecha: cómo se clasifica `pydantic_core` desde un paquete que sólo
    tiene `__init__.pyi`).
  - El arnés de esta paridad (ruff en JSON contra kerbo en JSON con la conversión de columnas)
    quedó fuera del repo, como los anteriores.
- TypeScript y JavaScript contra ESLint (empezada, 2026-10-21): 20 reglas de `@eslint/js`
  recommended y de `typescript-eslint` recommended sin tipos, con el nombre de ESLint sin el prefijo
  del plugin, en una familia propia (`EslintRule`) y un submódulo propio (`src/lint/ecmascript/`).
  - Frecuencia medida con todo `recommended` de los dos (más `no-var`, `prefer-const`,
    `prefer-rest-params`, `prefer-spread` y `no-non-null-assertion`) sobre next.js, react, VS Code,
    zod y express, de mayor a menor: no-var 79.279, no-unused-expressions 34.661, no-unused-vars
    9.431, prefer-const 8.552, no-require-imports 8.221, no-explicit-any 6.688,
    no-non-null-assertion 2.720, prefer-rest-params 1.780, no-useless-assignment 1.778,
    no-prototype-builtins 1.480, no-redeclare 1.449, no-empty 953, no-useless-escape 711,
    no-fallthrough 700, no-cond-assign 655, no-this-alias 612, ban-ts-comment 552, no-func-assign
    508, prefer-spread 487, no-empty-object-type 293, no-misleading-character-class 290,
    no-constant-condition 248 y el resto con menos de 160. Casi todo `no-var` y
    `no-unused-expressions` sale del código compilado de next.js, que su `.ignore` deja afuera.
  - Hechas: `no-unused-vars` (la de typescript-eslint, también en JavaScript), `prefer-const`,
    `no-var`, `no-unused-expressions`, `no-require-imports`, `no-explicit-any`,
    `no-non-null-assertion`, `prefer-rest-params`, `no-prototype-builtins`, `no-empty`,
    `no-useless-escape`, `no-cond-assign`, `no-this-alias`, `ban-ts-comment`,
    `no-empty-object-type`, `no-case-declarations`, `no-debugger`, `no-duplicate-case`,
    `no-dupe-else-if` y `no-dupe-keys` (las siguientes, más abajo).
  - Motor de alcances calcado de `@typescript-eslint/scope-manager` sobre tree-sitter
    (`src/lint/ecmascript/scope/`): los mismos alcances (también los de tipos, firmas, tipos
    mapeados y condicionales, enums y namespaces), definiciones de tipo y de valor por separado,
    referencias de tipo, de valor y dobles (`export { x }`), la resolución al cerrar cada alcance en
    el mismo orden que ESLint (el orden de las referencias decide `prefer-const` y la posición de
    `no-unused-vars`), el `arguments` implícito, la referencia a `React` del primer JSX y las marcas
    de `typescript-eslint` (parámetros de firmas y setters, propiedades de parámetro, miembros de
    enum, la clase dentro de su propio alcance, `for (k in o) return`, declaraciones ambientales y
    `.d.ts`).
  - Arreglos con el `Fix` de siempre (parche en `--format agent`, acción rápida del LSP y
    `kerbo fix`): `prefer-const` y `no-var` con las mismas condiciones que ESLint (cada uno contra
    `eslint --fix` en `src/lint/ecmascript/fixtures/*.fixed`), `no-useless-escape` (ESLint sólo lo
    sugiere: kerbo borra la barra) y `no-debugger` cuando la sentencia está sola en una lista.
  - Con la antitrampa: en `--diff` y el hook, un `any` o un `@ts-ignore` agregados salen sólo como
    `type-escape` o `suppression-added`, no repetidos como `no-explicit-any` o `ban-ts-comment` en la
    misma línea (`src/guard/overlap.rs`).
  - Los avisos van a la caché por contenido con el resto del análisis (formato 6).
  - Paridad contra ESLint 10.13 y typescript-eslint 8.72 (parser de typescript-eslint en todos los
    archivos, JSX en los `.js`, `noInlineConfig` y sólo estas reglas; en los `.ts` apagado lo que
    apaga `eslint-recommended`), por archivo, línea, columna, regla y cantidad, en next.js (24.682
    archivos fuera de su `.ignore`), react (4.462), VS Code (722), zod (521) y express (141): 42.488
    avisos iguales, ninguno de más y 587 de menos. Lo que queda, explicado:
    - Archivos que tree-sitter-typescript 0.23 no parsea y kerbo no analiza (un árbol roto da falsos
      positivos): `typeof import('x')` y `import('x').A['b']` en tipos, `let as`, `export type *`,
      `abstract?:` en una interfaz, un template con argumentos de tipo y escapes dentro de un tipo
      de template. Son 521 avisos en unas cuatro decenas de archivos (la mayoría de los tests de
      Copilot en VS Code y algunos de next.js).
    - 15 archivos `.js` de react-devtools con anotaciones de tipos y argumentos de tipo en llamadas
      (`useState<boolean>(false)`): TypeScript los lee en modo JavaScript como comparaciones
      (`useState < boolean > (false)`) y kerbo, que los parsea con la gramática de TSX, los deja
      afuera; son 66 avisos.
    - Para que no sobre nada, el árbol se repara antes de rendirse: un `.js` que la gramática de
      JavaScript no parsea se vuelve a parsear con la de TSX (TypeScript acepta anotaciones en
      JavaScript); los `&` sueltos en JSX (`Vercel & Next.js`, `?a=1&b=2`), que tree-sitter no
      entiende, se blanquean, igual que la varianza `in`/`out` de los parámetros de tipo; y se
      descartan los archivos con sintaxis de Flow que tree-sitter acepta y TypeScript no
      (`(x: T)`, `import typeof`, `<T: U>`, `{| |}`, `declare module.exports`). Un archivo que ESLint
      no parsea tampoco lo analiza kerbo; queda uno en next.js que kerbo sí lee.
    - tree-sitter-typescript aplica el `!` de no nulo a toda la expresión de la izquierda
      (`a || b < c!`) y parte `x satisfies a.b.C` en un acceso a miembro: las dos se corrigen al
      ubicar el aviso.
    - Las columnas cuentan bytes, como el resto de las reglas; ESLint cuenta unidades de UTF-16 (la
      medición convierte).
  - Lo que no se ve a simple vista quedó en `src/lint/ecmascript/fixtures/`, cada archivo con la
    salida de ESLint al lado (`.eslint`) y, para los arreglos, el resultado de `eslint --fix`.
- `no-redeclare` y `no-func-assign` (hecho, 2026-10-22), sobre el motor de alcances que ya existía:
  la primera avisa en cada declaración de una variable después de la primera, en los alcances que
  mira ESLint (programa, funciones, bloques, `for`, `switch` y bloques estáticos; no los `catch` ni
  el nombre de una función con nombre); la segunda, en cada escritura que no es la inicialización a
  una variable cuya primera declaración es una función. Como en typescript-eslint, las dos se apagan
  en TypeScript. Paridad contra ESLint en express, zod, react, vscode y next.js: 149 de 149 y 132
  de 132, sin avisos de más ni de menos. El ejemplo `fixtures/redeclare.js` fija los casos (función
  y `var`, parámetro y `var`, bucles, `catch`, asignación compuesta y desestructuración).
- `no-wrapper-object-types`, `no-unsafe-function-type`, `triple-slash-reference`,
  `no-unsafe-finally` y `prefer-spread` (hecho, 2026-10-23), las cinco que no piden el grafo de
  control:
  - Las dos de tipos deciden si el nombre es el global como `isReferenceToGlobalFunction` de
    typescript-eslint: miran sólo la primera referencia con ese nombre creada en el alcance del uso
    (un `type Boolean` declarado más abajo en el módulo exime a todos los `Boolean` del módulo, y un
    `const Number` no exime a un tipo). `no-wrapper-object-types` trae el arreglo de ESLint (el
    nombre en minúscula), salvo en `implements` y `extends` de una interfaz, igual que ESLint.
  - `triple-slash-reference` con las opciones de fábrica: `path` siempre avisa, `types` sólo si el
    mismo módulo se importa (un aviso por cada import, con `import x = require()` incluido), `lib`
    nunca; sólo los comentarios antes del primer token.
  - `no-unsafe-finally` y `prefer-spread`, calcadas de ESLint: el corte en funciones, clases, loops
    y `switch` según la sentencia y su etiqueta; el `this` esperado comparado token a token.
  - Paridad contra ESLint en express, zod, react, vscode y next.js: 152 de 152, 107 de 113, 70 de
    70, 36 de 36 y 40 de 40, ninguno de más. Los 6 que faltan están en archivos que kerbo no
    analiza (el `webpack-config.ts` de next.js, que tree-sitter no parsea, y tres `.js` de
    react-devtools con anotaciones). La frecuencia de abajo contaba también el código compilado
    que next.js ignora; por eso daba más. Los casos quedaron en `fixtures/wrappers.ts`,
    `finally.js`, `spread.js` y `applies.ts`.
- `no-constant-condition` y `no-constant-binary-expression` (hecho, 2026-10-23), sobre un port de
  `isConstant` y `isLogicalIdentity` de ESLint (`src/lint/ecmascript/constants.rs`) que mira el
  árbol como ESTree: sin paréntesis, `&&`, `||` y `??` como expresiones lógicas, `as`, `!` y
  `satisfies` como lo que no es constante, y las llamadas opcionales (`Boolean?.(x)`) como cadenas.
  `undefined`, `Boolean`, `String` y compañía cuentan como globales si ninguna declaración los
  tapa. `no-constant-condition` va con `checkLoops: "allExceptWhileTrue"` y perdona un loop con un
  `yield` del mismo generador adentro (en un `for`, el de la inicialización no cuenta, como en
  ESLint). Paridad en express, zod, react, vscode y next.js: 155 de 155 y 27 de 27, sin avisos de
  más. Los casos quedaron en `fixtures/conditions.js` y `fixtures/binaries.js`.
- `no-const-assign`, `no-class-assign`, `no-import-assign`, `no-global-assign`,
  `no-shadow-restricted-names` y `no-unassigned-vars` (hecho, 2026-10-23), sobre el motor de
  alcances: las escrituras que no inicializan (una por identificador, como
  `getModifyingReferences`), los miembros de un `import * as` que se pisan, se borran, se iteran o
  pasan por `Object.assign`/`Reflect.set` y compañía, los globales de ECMAScript de sólo lectura
  (la misma lista que ESLint) y los `let`/`var` sin valor que se leen y nunca se escriben (no los
  de `declare`). Las tres primeras se apagan en TypeScript, como en typescript-eslint. Paridad en
  express, zod, react, vscode y next.js: 11 de 11, 2 de 2, 3 de 3, 1 de 1, 34 de 36 y 11 de 23, sin
  avisos de más. Faltan los de dos archivos que tree-sitter no parsea (`base-server.ts` y
  `router.ts` de next.js) y 12 de una salida de tests de next.js que redeclara cuatro veces el mismo
  import como `const` (código inválido). Los casos quedaron en `fixtures/assignments.js` y
  `fixtures/declared.ts`.
- `valid-typeof`, `no-empty-pattern`, `no-sparse-arrays`, `require-yield`, `no-useless-catch`,
  `no-compare-neg-zero`, `no-async-promise-executor`, `use-isnan`, `no-unused-labels` y
  `no-self-assign` (hecho, 2026-10-23), con las opciones de fábrica (`use-isnan` mira los `switch`
  y no `indexOf`; `no-self-assign` mira también las propiedades, con el `isSameReference` y el
  `getStaticPropertyName` de ESLint en `src/lint/ecmascript/self_assign.rs` y `members.rs`). Paridad
  en express, zod, react, vscode y next.js: 4 de 4, 21 de 21, 8 de 8, 14 de 15, 10 de 10, 5 de 5,
  8 de 8, 4 de 4, 4 de 4 y 3 de 3, sin avisos de más; el que falta está en un test de Copilot que
  tree-sitter no parsea. Los casos quedaron en `fixtures/checks.js`.
- `no-setter-return`, `no-unsafe-optional-chaining`, `no-non-null-asserted-optional-chain`,
  `preserve-caught-error`, `no-extra-boolean-cast`, `no-namespace` y `no-array-constructor`
  (hecho, 2026-10-23), con las opciones de fábrica. Un `ChainExpression` de ESTree se reconoce en
  tree-sitter como un acceso o una llamada con `?.` en su cadena que no sigue encadenado sin
  paréntesis (`src/lint/ecmascript/optional_chains.rs`). `no-setter-return` mira también los
  descriptores de `Object.defineProperty` y compañía, y se apaga en TypeScript. `no-array-constructor`
  trae el arreglo de typescript-eslint (el literal con los mismos argumentos); `no-extra-boolean-cast`
  todavía no, porque el de ESLint decide los paréntesis por precedencia. Paridad en express, zod,
  react, vscode y next.js: 23 de 23, 24 de 25, 1 de 1, 3 de 3, 4 de 4 y 64 de 66, sin avisos de
  más (no-array-constructor no aparece); los que faltan están en archivos de next.js que tree-sitter
  no parsea. Los casos quedaron en `fixtures/errors.js` y `fixtures/namespaces.ts`.
- El análisis de caminos de ESLint (`code-path-analysis`) portado sobre tree-sitter (hecho,
  2026-10-23), en `src/lint/ecmascript/paths/`: segmentos y contextos de bifurcación con sus
  caminos dobles adentro de un `finally`, los contextos de elección (`&&`, `||`, `??`, `if`,
  ternarios, cadenas opcionales), loops con condiciones literales, `switch` con el `default` en el
  medio, `try`/`catch`/`finally` con el primer punto que puede tirar, etiquetas, y un camino por
  programa, función, bloque estático e inicializador de campo. El recorrido traduce el árbol de
  tree-sitter a los nodos de ESTree (sin paréntesis, un método como clave más función, la cadena
  opcional como `ChainExpression`) y lleva la pila de ancestros, porque `Node::parent` de tree-sitter
  busca desde la raíz y volvía cuadrático el análisis en archivos grandes (next.js tardaba 152 s en
  vez de 60). Encima, `no-unreachable` (con los rangos de sentencias seguidas y los campos de una
  clase derivada sin `super()`) y `no-fallthrough` (con el comentario `falls through` y las líneas en
  blanco de un `case` vacío). Paridad en express, zod, react, vscode y next.js: 37 de 37 y 51 de 51,
  sin avisos de más. Los casos quedaron en `fixtures/reachability.js`.
- `no-useless-assignment` (hecho, 2026-10-23), sobre el análisis de caminos: cada camino guarda los
  segmentos alcanzables activos en cada asignación, el primer y el último identificador de cada
  segmento y sus bloques `try`, y la regla recorre los segmentos siguientes como ESLint (sin pasar
  de uno que vuelve a asignar, con las lecturas desde otra función como uso y las variables
  exportadas afuera). Paridad en express, zod, react, vscode y next.js:
  599 de 602, sin avisos de más; los tres que faltan están en archivos que tree-sitter no parsea. La
  frecuencia de arriba contaba el código compilado que next.js ignora. Los casos quedaron en
  `fixtures/assigned.js`.
- `getter-return` (hecho, 2026-10-23): getters de clases y objetos y los de los descriptores de
  `Object.defineProperty` y compañía (como `no-setter-return`), con el `return;` vacío y la salida
  alcanzable del análisis de caminos. Se apaga en TypeScript. Paridad: 11 de 11. Los casos quedaron
  en `fixtures/getters.js`.
- `no-loss-of-precision` y `no-control-regex` (hecho, 2026-10-23): el número escrito contra el que
  guarda el `double` (en base 10 con la precisión pedida, en las otras bases por los bits
  significativos), y los caracteres de control de un patrón (literal o primer argumento de
  `RegExp`) escritos tal cual o con `\x` y `\u`. Paridad: 14 de 14 y 33 de 33. Los casos quedaron
  en `fixtures/literals.js`.
- `no-unexpected-multiline` y `no-unused-private-class-members` (hecho, 2026-10-23): la línea que
  sigue a la anterior por un `(`, `[`, template o `/…/flags` sin punto y coma (una flecha sin
  paréntesis como lo llamado es un error de tree-sitter, que no ve el punto y coma automático), y
  los `#miembros` que la clase sólo escribe, incrementa como sentencia o desestructura. Paridad: 2
  de 2 y 3 de 3. Los casos quedaron en `fixtures/multiline.js`.
- `no-undef` (hecho, 2026-10-27): las referencias que no resuelven en ningún alcance, salvo
  `typeof x`, los nombres de `/* global a, b: writable */` (comas o espacios, `: off` no declara) y
  los globales conocidos. Como kerbo no lee la configuración de ESLint, los globales son la unión de
  los entornos que suele declarar un repo: ES, navegador, Node, CommonJS, workers y Jest, Mocha,
  Jasmine y Vitest (`src/lint/ecmascript/globals.txt`, del paquete `globals` que usa ESLint). Un
  `window` en código de Node no se avisa; a cambio no hay falsos positivos por el entorno. Las
  referencias de tipo también cuentan (en un `.js` con anotaciones), con la lib `esnext` que
  typescript-eslint declara en el alcance global (`typescript_lib.txt`). Se apaga en TypeScript.
  - Paridad contra ESLint 10.13 con esos mismos globales y los comentarios activos, en express, VS
    Code, juice-shop, react y zod: unos 6.000 avisos, 27 de menos y 22 de más, todos en react: `.js`
    con anotaciones que TypeScript lee como comparaciones en modo JavaScript
    (`useState<boolean>(false)`) o con sintaxis de Flow, y `using`/`await using`, que tree-sitter no
    parsea. Los casos quedaron en `fixtures/undefined.js`, y los goldens de los demás fixtures de
    JavaScript suman los `no-undef` de ESLint.
- `constructor-super` y `no-this-before-super` (hecho, 2026-10-27), sobre el análisis de caminos:
  el analizador lleva, en el camino del constructor de una clase con `extends`, lo mismo que los
  listeners de ESLint (por segmento, si `super()` se llamó en algún camino o en todos, las llamadas
  válidas y los `this`/`super.x` de antes), con los eventos de vuelta de un loop y el
  `traverseSegments` de ESLint (`paths/traversal.rs`). `isPossibleConstructor` decide si lo que se
  extiende puede ser un constructor y `extends null` no pide `super()` para `this`. Las dos se
  apagan en TypeScript, como en typescript-eslint. En el corpus no aparecen fuera del código
  compilado de next.js; contra los 145 casos de los tests de ESLint 10.13
  (`tests/lib/rules/*.js`, un archivo por caso): 65 de 65 y 25 de 25, sin avisos de más. Los
  casos quedaron en `fixtures/constructors.js`.
- `eslint-disable` en el registro de supresiones de `src/suppressions.rs` (hecho, 2026-10-27), así lo
  silenciado aparece como suprimido con su motivo (lo de después de ` -- `) y entra en la
  antitrampa: `eslint-disable-line` y `eslint-disable-next-line` en cualquier comentario,
  `/* eslint-disable */` hasta su `/* eslint-enable */` por posición (en un comentario de línea no
  valen, como en ESLint), y las reglas apagadas en todo el archivo con `/* eslint regla: 0 */`. Las
  directivas salen de los comentarios del árbol, no de strings. Los nombres de typescript-eslint
  valen sin el prefijo del plugin, y `complexity`, `max-lines`, `max-lines-per-function`,
  `max-params`, `max-depth`, `no-magic-numbers` y `sonarjs/cognitive-complexity` silencian sus
  reglas universales.
  - Paridad contra ESLint 10.13 con los comentarios activos, en express, VS Code, juice-shop, react
    y zod: de los 40 avisos que silencian los comentarios, kerbo silencia los mismos 40 y ninguno
    más. A propósito, `no-unused-vars` y `@typescript-eslint/no-unused-vars` son la misma regla para
    kerbo: un `eslint-disable-next-line no-unused-vars` la silencia aunque el repo use la de
    typescript-eslint (ESLint no; son 16 avisos en tests de react, que usa la del núcleo).
  - El arnés quedó en `../kerbo-parity/eslint/` (config con todo `recommended` de los dos limitado a
    las reglas de kerbo, `INLINE=1` para leer los comentarios, y `compare.py`).
- Pendientes de TypeScript y JavaScript contra ESLint, por frecuencia:
  - `no-misleading-character-class` (290, casi todo en código compilado),
    `no-irregular-whitespace` (10) y las que no aparecieron en el corpus.
  - `no-useless-escape` no mira las expresiones regulares con el flag `v`.
  - Los `.js` con argumentos de tipo en llamadas y los huecos de la gramática de arriba.
  - Los `/* global x */` sólo los lee `no-undef`: con los comentarios activos ESLint avisa también
    `no-unused-vars` en los globales declarados ahí que no se usan (3 en react).
  - Un `/* eslint-enable regla */` adentro de un `/* eslint-disable */` de todas no la vuelve a
    prender (ESLint sí, sólo a ella).
  - Las reglas con tipos de `typescript-eslint` (`recommended-type-checked`): esperan a los tipos
    del compilador.
  - El arnés de esta paridad (ESLint con un `eslint.config.mjs` de sólo estas reglas, kerbo en JSON
    y la conversión de columnas) quedó fuera del repo, como los anteriores.
- Shell contra ShellCheck (empezada, 2026-10-21): `.sh`, `.bash` y los archivos sin extensión con
  shebang de `sh`, `bash`, `dash` o busybox (también por `env`), con tree-sitter-bash 0.25.1
  (MIT). Las 22 reglas más frecuentes de ShellCheck, con el código en minúscula como nombre
  (`sc2086`) en una familia propia (`ShellRule`, en `src/rules/families/shell.rs`): SC2004, SC2005, SC2006, SC2015, SC2016, SC2034, SC2046, SC2059,
  SC2064, SC2068, SC2069, SC2086, SC2116, SC2148, SC2155, SC2162, SC2164, SC2166, SC2181, SC2206,
  SC2207 y SC2223. `kerbo explain SC2086` acepta el código en mayúsculas.
  - La severidad es la de ShellCheck: el preset recomendado deja afuera las de estilo y el mínimo,
    también las informativas.
  - Las partes de ShellCheck que deciden si avisa, portadas de su código: si una expansión está
    libre de comillas, si una palabra es nombre de comando o condición, las opciones de cada
    comando al estilo GNU, BSD o genérico, el dialecto (shebang, `# shellcheck shell=`, extensión) y
    el flujo de datos: un grafo de control con estados que se juntan en ramas, loops, `case` con
    `;&` y `;;&`, subshells y pipelines (con la rareza de que cualquier `set -o` prende `lastpipe`),
    funciones llamadas con su marco de variables locales, la caché de llamadas por dependencias
    como ShellCheck, y el valor de cada variable como vacío, limpio o con espacios para SC2086
    (también los enteros de `declare -i`, `let`, `(( ))`, `read`, `mapfile`, `printf -v` y
    `wait -p`). SC2034 sale sin flujo, como en ShellCheck, y los arreglos asociativos no cuentan
    sus índices como aritmética.
  - Arreglos mecánicos con el `Fix` de siempre (`--format agent`, LSP y `kerbo fix`): comillas en
    SC2086, `$(…)` en vez de backticks en SC2006, `-r` en SC2162 y `|| exit` en SC2164. Las
    violaciones de deploy y de shell guardan su arreglo en la caché (formato 7).
  - `# shellcheck disable=` con el alcance real de ShellCheck, en el registro de
    `src/suppressions.rs` (así lo silenciado entra en la antitrampa con su motivo): antes del primer
    comando vale para el archivo entero, antes de un comando compuesto para todo el bloque, al
    final de una línea no vale, y antes de `elif`, `else` o un caso de `case` tampoco. Acepta
    `SC2086`, `2086`, rangos (`SC2000-SC2099`), `all` y varias claves `disable=` en el mismo
    comentario.
  - Paridad contra ShellCheck 0.11.0 (`koalaman/shellcheck-alpine`) en 1.752 scripts de
    kubernetes (`hack/`, `cluster/`, `build/`, `test/`), moby (`hack/`, `contrib/`), nvm, rustup,
    ohmyzsh, docker-install, acme.sh, bash-it, dokku, neofetch y los scripts de kerbo: 1.680
    avisos, cero diferencias por archivo, línea, columna y regla. Sacando todas las directivas
    `# shellcheck` de esos mismos archivos (para medir las reglas sin que nadie las haya
    silenciado): 3.215 avisos de ShellCheck, 3.210 de kerbo, ninguno de más. kerbo tarda 1,5 s en
    todo el corpus; ShellCheck tarda 18 s sólo en `nvm.sh`.
  - Lo que no se ve a simple vista quedó en `src/shell/fixtures/flow.sh`, con la salida de
    ShellCheck al lado, y en los tests de `src/shell/tests.rs`.
  - tree-sitter-bash tiene dos errores que cambian lo que se lee:
    - En `$A/$B/V` deja un `$` suelto. Antes de analizar, las expansiones pegadas a un separador
      se reescriben como `${NOMBRE}`, se vuelve a parsear y las posiciones, los arreglos y los
      textos vuelven al original.
    - Un comentario entre líneas continuadas con `\` se come el resto del comando ("líneas
      tragadas"). Esos archivos se saltean enteros, como los que tienen nodos de error.
- Lo de bash en un script de sh (hecho, 2026-10-30): las 64 reglas de `checkBashisms` de
  ShellCheck 0.11.0 (`sc3001` a `sc3067`, sin SC3013 ni SC3027, que no existen, ni SC3051, que sólo
  sale cuando ShellCheck sigue el `source`), en `src/shell/rules/bashisms/`, para `sh`, `dash` y
  busybox con sus excepciones (busybox tiene `[[ ]]`, `$'…'`, `&>`, `source` y los recortes de
  `${…}`; dash tiene `local`, `echo -n` y los nombres de señal en minúscula). Se calca el orden de
  las cláusulas de un comando simple: `echo` con opciones, `exec -x`, `let` y `set` cortan antes de
  la general, así `let` no es también SC3044 ni `exec -a` es SC3045; `echo`, `let` y `set` valen
  también con ruta (`/bin/echo`), como `isCommand`. Las expansiones `${…}` se miran con las mismas
  expresiones regulares de ShellCheck, las variables de bash (SC3028) salvo que el script las
  asigne, y en `let` se buscan `++`, `--` y `**` en el texto, porque tree-sitter no lo lee como
  aritmética. tree-sitter-bash lee `{fd}>` como un argumento pegado a la redirección y `$"…"` como
  `$` y un string: los dos se reconocen así. En dash ShellCheck las marca como error y kerbo como
  advertencia: el preset las muestra igual.
  - Paridad: las 137 pruebas de `checkBashisms` del fuente de ShellCheck
    (`scripts/parity/shell/extract_props.py`) y 20 scripts propios con lo que más aparece: 226
    avisos idénticos, ninguno de más; faltan 4, todos en líneas que tree-sitter-bash no parsea
    (`!(*.hs)`, `10#$y`, `[ a \< b ]`). Quedan como fixtures en `src/shell/fixtures/bashisms*.sh`
    con la salida de ShellCheck al lado (150 avisos). En el corpus: 96 de 145, ninguno de más; de
    los 49 que faltan, 46 están en archivos que se saltean y 3 son lecturas de tree-sitter
    (abajo).
  - El arnés de shell volvió al repo (`scripts/parity/shell/`): clonar el corpus, correr ShellCheck
    con y sin directivas (`--norc`, porque kerbo no lee `.shellcheckrc`), kerbo en release,
    comparar, probes con fixtures y `register.py` para sumar reglas. El corpus vive en
    `kerbo-parity/shell-corpus` y el fuente de ShellCheck en `kerbo-parity/shellcheck-src`.
- Los chequeos de comandos de ShellCheck (hecho, 2026-10-30): 55 reglas de `Checks/Commands.hs` en
  `src/shell/rules/command_checks/`: `tr` (SC2018 a SC2021, SC2060), `grep` (SC2022, SC2062,
  SC2063), `expr` (SC2003, SC2304 a SC2308), `find` (SC2061, SC2146, SC2150, SC2156, SC2185,
  SC2227), `return` y `exit` (SC2151, SC2152, SC2241, SC2242), `trap` (SC2172, SC2173), `set`
  (SC2121), `export` y `read` con `$` (SC2163, SC2229, SC2313), `alias` (SC2139, SC2142), `unset`
  (SC2184), `local` fuera de función (SC2168), `let` (SC2219), `eval` de arreglos (SC2293, SC2294),
  `.` con argumentos (SC2240), `time -f` (SC2023), `echo` con escapes (SC2028), `mkdir -p -m`
  (SC2174), `su` (SC2117), `ssh` (SC2029), `tempfile`, `egrep` y `fgrep` (SC2186, SC2196, SC2197),
  `rm -r` de directorios del sistema con la expansión de llaves (SC2114, SC2115), `mv`, `cp` y `ln`
  sin destino (SC2224 a SC2226), `sudo` (SC2024, SC2232), `chmod -r` (SC2253) y `xargs -i`
  (SC2267). Se calca `checkCommand`: con ruta sólo valen los chequeos por nombre de archivo y con
  `builtin` los exactos del comando que sigue. `Opt` de las opciones guarda ahora también la
  palabra de la opción.
  - Paridad: las 220 pruebas de esas funciones del fuente de ShellCheck: 148 idénticas, 2 de más y
    las demás de reglas que no son de la tanda o líneas que tree-sitter-bash lee mal (`$1/$2`, el
    `>=` de `expr` que toma como redirección, backticks anidados con escapes, extglob). Quedan
    como fixtures en `src/shell/fixtures/commands*.sh` (113 avisos). En el corpus, 51 avisos más
    sin directivas y 25 con ellas, ninguno de más.
  - tree-sitter-bash pega a veces la línea siguiente al comando cuando empieza con `\.`, como una
    palabra que empieza con el salto de línea (17 SC2240 de más en los tests de nvm): los
    chequeos de comandos cortan los argumentos ahí. Saltear esos archivos perdía 300 avisos que
    coinciden.
- Pendientes de Shell contra ShellCheck:
  - De `Checks/Commands.hs` quedan `getopts` con su `case` (SC2213, SC2214, SC2220), `time` en un
    pipeline (SC2176, SC2177), la cantidad de argumentos de `printf` (SC2182, SC2183), los espacios
    repetidos de `echo` (SC2291) y los opcionales.
  - tree-sitter-bash pone las palabras que siguen a una redirección como destinos extra de ella
    (`/bin/echo >&2 -e "…"`): kerbo no las ve como argumentos (2 SC3037 en moby), y lo mismo
    afecta a cualquier regla que mire argumentos.
  - El `nvm.sh` actual tiene nodos de error desde la línea 2006 y no se saltea: el `{` de la línea
    11 no queda como bloque y su `# shellcheck disable=SC2016` no cubre el archivo (87 de más con
    directivas). Un archivo con errores de parseo que kerbo no saltea, a revisar.
  - `${x:=valor}` asigna `x` para el flujo de ShellCheck (después no hay SC2086 sobre `x`) y
    `{fd}>archivo` asigna `fd` (SC2034); kerbo no lo modela todavía.
  - 55 archivos del corpus (260 avisos de ShellCheck, 375 sin directivas) quedan afuera porque
    tree-sitter-bash no los parsea o se traga líneas; entre ellos el `acme.sh` principal.
  - Sin directivas faltan 5 avisos: 3 SC2034 en `dnsapi/dns_ali.sh`, `deploy/ali_cdn.sh` y
    `deploy/ali_dcdn.sh` de acme.sh (el `$` suelto que la reparación no alcanza, y el archivo se
    saltea), y 2 SC2086 en `nvm.sh` (líneas 5037 y 5202, `EXIT_CODE=$?; return $EXIT_CODE`) sin
    explicar todavía.
  - El resto de las reglas de ShellCheck: SC1090/SC1091 (`source`, que tampoco se sigue), SC2154,
    SC2329, SC2031, los SC3xxx de `sh` y las demás, sin medir todavía la frecuencia.
  - ShellCheck sobre el shell de cada `RUN` de los Dockerfile.
  - `.shellcheckrc` no se lee.
  - Las columnas cuentan bytes, como el resto de las reglas; ShellCheck cuenta caracteres (la
    medición convierte).
- Go contra golangci-lint (empezada, 2026-10-21): `ineffassign`, la única regla del conjunto por
  defecto de golangci-lint 2 (`standard`: errcheck, govet, ineffassign, staticcheck y unused) que no
  pide los tipos del compilador, con el nombre del linter, en una familia propia (`GoRule`).
  - Frecuencia del conjunto por defecto en gin, cobra, chi, caddy, prometheus, hugo, etcd y consul
    (golangci-lint 2.15, 2026-10-21), de mayor a menor: errcheck 5.647, staticcheck ST1005 844,
    SA1019 321, QF1008 316, unused 201, QF1012 58, govet inline 38, QF1004 32, govet stdmethods 28,
    ST1012 19 y el resto con menos de 15. ineffassign da 2: casi todos esos repos ya corren
    golangci-lint en su CI, así que lo que sobrevive es lo que ellos apagan (errcheck) o lo que
    trae la versión nueva de staticcheck. Para ineffassign la medición suma la biblioteca estándar
    de Go, terraform y golang/tools, que no usan golangci-lint.
  - Port 1:1 de ineffassign sobre un go/ast mínimo armado desde tree-sitter (`src/lint/go/ast/`),
    recorrido en el orden de campos de `ast.Walk`; la resolución de nombres de go/parser
    (`resolver.go`: alcances, `:=` que redeclara, etiquetas por función, claves de literales
    compuestos que se resuelven sin quedar pendientes, parámetros de tipo del receptor que se
    declaran sin ligarse y los nombres del paquete resueltos al final contra el mismo archivo), y
    el grafo de bloques de ineffassign con sus rarezas: un global declarado más abajo en el archivo
    cuenta como local, la clave de `point{x: 3}` se resuelve contra una variable local `x` y la
    usa, y un `defer` hace que cualquier operación que puede entrar en pánico lea los resultados con
    nombre.
  - Archivos generados como golangci-lint en modo `lax` (`code generated`, `do not edit`,
    `autogenerated file` o el encabezado de Swagger Codegen en los comentarios hasta la primera
    declaración después de `package`), y los que tree-sitter no parsea se saltean.
  - `//nolint` con el alcance de golangci-lint en el registro de `src/suppressions.rs`, así entra en
    la antitrampa con su motivo: el rango es el grupo de comentarios armado como go/parser (los del
    final de una línea van solos) y se estira hasta el final de cualquier nodo que empiece en la
    línea siguiente y en la misma columna; antes de `package`, vale para el archivo entero. Sin
    lista o con `all` vale para todas las reglas; con lista, para las que se llaman igual.
  - Paridad contra golangci-lint 2.15 (`--no-config --enable-only ineffassign
    --uniq-by-line=false`), restringida a los archivos que `go list` incluye para linux/amd64 con
    tests: cero diferencias por archivo, línea, columna y variable en la biblioteca estándar de Go
    (87), golang/tools (10), terraform (3), hugo (2), caddy (los 4 que silencia con `//nolint`),
    syncthing (1), consul, gin, cobra, chi, prometheus, etcd y fzf: 103 avisos en unos 10.500
    archivos. tailscale queda afuera: `go list` no lista sus archivos con el Go de la imagen.
  - Lo que no se ve a simple vista quedó en `src/lint/go/fixtures/`, con la salida de golangci-lint
    al lado.
- staticcheck ST1005 (hecho, 2026-10-22): `st1005`, mensajes de `errors.New` y `fmt.Errorf` (por la
  ruta del import, con alias) que terminan en `.`, `:`, `!` o salto de línea, o que empiezan con
  mayúscula. El mensaje tiene que ser constante: literales, sumas de literales y constantes del mismo
  archivo, con los escapes de Go. Como staticcheck, no avisa con una sola palabra, con una sigla o un
  nombre compuesto (`HTTP`, `ReadAll`), con un `(`, ni si la primera palabra es el nombre de una
  función, tipo, variable o método del paquete (también `Tipo.Metodo`); las constantes no eximen. En
  los `_test.go` sólo mira lo de fuera de las funciones (la inicialización de las variables del
  paquete). `//nolint:staticcheck` la silencia.
  - Lo que pide el paquete entero se resuelve así: cada archivo marca todos sus mensajes con mayúscula
    (y pasan por `//nolint` como cualquier aviso) y guarda en la caché su paquete, lo que declara y
    dónde están; al final del escaneo se descartan los que nombran algo del paquete (carpeta y
    nombre de `package`) y los de dentro de funciones de test. Los archivos generados no avisan pero
    sus nombres cuentan, como en golangci-lint.
  - Paridad contra golangci-lint (la corrida guardada del conjunto por defecto, en los archivos de
    linux/amd64): 1.073 coincidencias por archivo y línea, ninguno de más y 10 de menos, todos de
    terraform con el mensaje en una constante de otro archivo del paquete (`EnvInvalidName`,
    `errInitConfigError`). consul 840 de 840, hugo, golang/tools y fzf completos; caddy, chi, cobra,
    etcd, gin y prometheus sin avisos en los dos.
- `//nolint` con los linters que kerbo cubre con sus reglas universales (hecho, 2026-10-27):
  `cyclop` y `gocyclo` silencian `cyclomatic-complexity`, `gocognit` `cognitive-complexity`, `funlen`
  `function-length`, `revive` `parameter-count` y `file-length`, `mnd` `magic-number`, `nestif`
  `nesting-depth` y `dupl` `duplicate-code`.
- staticcheck QF1003 y QF1004 (hecho, 2026-10-27): `qf1003`, cadenas de `if`/`else if` (dos o más)
  que comparan siempre lo mismo con `==` (también `x == a || x == b`), sin inicialización, sin
  `break`, `continue` ni `fallthrough` en los cuerpos y sin llamadas, `&` ni `<-` en lo comparado; la
  igualdad es por tokens, con los paréntesis contando como en `astutil.Equal`. `qf1004`,
  `strings.Replace`, `SplitN` y `SplitAfterN` (y los de `bytes`, por la ruta del import) con `-1`
  literal como último argumento. Las dos son sintácticas en staticcheck y `//nolint:staticcheck` las
  silencia.
  - Paridad contra golangci-lint v2.15.0 (staticcheck sólo con QF1001, QF1003 y QF1004, cada módulo
    con sus dependencias) en chi, cobra, gin, fzf, caddy, hugo, golang/tools, prometheus, terraform,
    syncthing, etcd y consul: los 100 avisos (67 de QF1004 y 33 de QF1003) idénticos por archivo,
    línea y columna. Sobraban 15, todos en archivos con `//go:build ignore`, `tools` o
    `integration`, que dejaron de avisar con las restricciones de build.
- Restricciones de build (hecho, 2026-10-27): las reglas de golangci-lint sólo miran los archivos
  que `go/build` incluye en linux/amd64, como golangci-lint por defecto. Se calcan `goodOSArchFile`
  (`_windows.go`, `_linux_arm64_test.go`; `windows.go` solo no cuenta, y los que empiezan con `_` o
  `.` quedan afuera) y `shouldBuild`: el `//go:build` en cualquier lugar de la cabecera, y sin él las
  `// +build` hasta la última línea en blanco antes del código, con la sintaxis vieja (espacios,
  comas, `!!` como `ignore`). Los tags son los del Go 1.27 de golangci-lint v2.15.0: `linux`,
  `amd64`, `unix`, `cgo`, `gc`, `amd64.v1`, `go1.1` a `go1.27` y sus `goexperiment` prendidos. Una
  restricción que no parsea no descarta el archivo (`go list` falla el paquete entero). Un archivo
  descartado tampoco aporta nombres a ST1005. Las reglas universales siguen mirando todos los
  archivos. El veredicto del nombre entra a la caché como contexto; el de la cabecera ya va en el
  contenido.
  - Contra `go list -json` de esa imagen: 42 nombres y cabeceras, todos con el mismo veredicto
    (`src/lint/go/build_constraints/tests.rs`).
  - Paridad de las cuatro reglas de Go (ineffassign, ST1005, QF1003, QF1004) contra golangci-lint
    v2.15.0 en los 12 repos de arriba, cada módulo con sus dependencias: 1.369 coincidencias por
    archivo, línea y columna, **ninguna de más** y 12 de menos, todas ST1005 de terraform con el
    mensaje en una constante de otro archivo (resueltas abajo). syncthing pide `go run build.go assets` antes: sin
    `lib/api/auto` generado, golangci-lint no analiza los paquetes que dependen de él.
- ST1005 con el mensaje en una constante de otro archivo del paquete (hecho, 2026-10-27): cada
  archivo guarda en la caché sus constantes de texto del paquete (las que resuelve solo) y los
  mensajes que nombran una constante que no tiene, como texto y nombres en orden; emite el aviso de
  puntuación y uno pendiente, así pasan por `//nolint` como cualquier otro. Al final del escaneo se
  arma el texto con las constantes del paquete (carpeta y nombre de `package`) y cada aviso se
  descarta o pasa a nombrar la palabra con mayúscula, con las mismas exenciones que el resto. Un
  nombre declarado como variable o parámetro en la función de la llamada no es una constante.
  - Paridad de las cuatro reglas de Go contra golangci-lint v2.15.0 en los 12 repos: **1.381 avisos
    idénticos por archivo, línea y columna, ninguno de más ni de menos** (1.252 de ST1005, 67 de
    QF1004, 33 de QF1003 y 29 de ineffassign).
- Los directorios que `./...` saltea (hecho, 2026-10-27): las reglas de golangci-lint no miran lo
  que queda debajo de `testdata`, `vendor` o una carpeta que empieza con `_` o `.`, contando desde el
  `go.mod` más cercano (un `_examples` con su propio `go.mod` es un módulo y se mira, como cuando
  golangci-lint corre en él). Sin `go.mod` no se descarta nada. Entra al contexto de la caché junto
  con el nombre del archivo.
  - Con el binario anterior eran 199 avisos de Go en esas carpetas del corpus (187 de ineffassign,
    casi todos en los `testdata` de golang/tools) que el script de paridad filtraba. Sin el filtro:
    las mismas 1.381 coincidencias, ninguna de más ni de menos.
- La plataforma de Go configurable (hecho, 2026-10-27): `[go]` en `kerbo.toml` con `goos`, `goarch`
  y `build_tags` (como `run.build-tags` de golangci-lint). Sin nada, `GOOS` y `GOARCH` del entorno
  o la máquina donde corre kerbo, como golangci-lint (y como `[rust]`). cgo va prendido salvo que
  se apunte a otra plataforma o `CGO_ENABLED` lo apague; `unix`, android como linux, illumos como
  solaris e ios como darwin, como `matchTag`. Las configuraciones de golangci-lint no se leen: kerbo
  lo reemplaza y su configuración es la propia. La plataforma entra al contexto de la caché.
  - El README dejó de decir que en Go sólo está `ineffassign`.
- Tipos de Go y errcheck (hecho, 2026-10-27). La decisión: los tipos los saca `go/types`, el
  chequeo de tipos del propio Go, a través de un helper chico en Go (`src/go_types/helper.go`, sólo
  biblioteca estándar) que kerbo lleva embebido y compila una vez con el `go` del sistema en su
  caché, como Java toma las firmas de los `.class` en vez de compilar. El helper corre `go list
  -export -deps -test -compiled` en el módulo (con la plataforma y los build tags de `[go]`), chequea
  cada paquete y su variante de test importando lo exportado de las dependencias, y escribe hechos
  por archivo: las llamadas que devuelven un `error` (con los nombres de errcheck para excluirlas,
  también por interfaces embebidas, y el tipo del primer argumento), los `<-` de canales de errores
  y los `range` sobre funciones que entregan uno. Las reglas siguen en Rust.
  - `errcheck`: cada archivo emite candidatos sintácticos (llamada suelta, `go`, `defer`, `<-` suelto
    o en un `case`, `range` sin clave o valor), con los saltos del visitor de errcheck (no entra en
    aserciones de tipo ni en asignaciones de una sola aserción), y pasan por `//nolint` y la caché
    como cualquier aviso. Al final del escaneo se confirman con los hechos del módulo (el `go.mod`
    más cercano) y la lista de exclusiones de errcheck v1.30.0, y el aviso nombra la función como
    golangci-lint (`f.Close`). Sin `go`, el escaneo falla con el motivo (o `errcheck = "off"`).
  - Los hechos se guardan con la huella del helper, la plataforma y cada `.go`, `go.mod` y `go.sum`
    (tamaño y fecha); si el helper no pudo leer lo exportado de alguna dependencia, la salida se usa
    pero no se guarda. Con la caché de build de Go caliente, consul (2.470 archivos) tarda 23 s de
    punta a punta.
  - Paridad contra golangci-lint v2.15.0 (sólo errcheck) en los 12 repos: **10.586 avisos idénticos
    por archivo, línea y columna, ninguno de más** y 168 de menos, todos explicados abajo (symlinks
    y la gramática de tree-sitter-go, resuelta después).
- La sintaxis de Go 1.26 y 1.27 (hecho, 2026-10-27): tree-sitter-go 0.25.0, la última publicada,
  no parsea `new(expresión)` ni los métodos con parámetros de tipo, y kerbo salteaba las reglas de
  Go en esos archivos (113 avisos de errcheck en etcd y golang/tools). kerbo lleva su copia en
  `grammars/tree-sitter-go`, publicada como `kerbo-tree-sitter-go` (el release la sube antes que
  kerbo; no puede vivir en el crate de kerbo porque el binding pide `unsafe`). Los dos cambios de
  `grammar.js`: una alternativa `new(expresión)` con precedencia dinámica menor que `new(Tipo)`,
  para que lo que ya parseaba como tipo no cambie de forma, y `type_parameters` en
  `method_declaration`. Se regenera con `tree-sitter-cli` 0.25.8, como upstream; el corpus de
  upstream pasa entero.
  - errcheck contra golangci-lint en los 12 repos: 10.699 avisos idénticos, ninguno de más; los 55
    que faltan son los symlinks de etcd. Las otras cuatro reglas siguen en 1.381 de 1.381.
- staticcheck QF1008 y QF1012 (hecho, 2026-10-27), sobre el mismo helper de tipos: QF1008, el
  campo embebido que se puede sacar de una cadena de selectores (`extractSelectors` y la
  comparación de caminos de `LookupFieldOrMethod`, calcadas); QF1012, `x.Write([]byte(fmt.Sprintf
  (...)))` y `x.WriteString(fmt.Sprint*(...))` cuando `x` (o `*x` si es un tipo con nombre) es un
  io.Writer (y un io.StringWriter). Los candidatos son sintácticos (cada campo intermedio de una
  cadena; cada `Write` o `WriteString` de un argumento) y se confirman al final del escaneo como
  los de errcheck. Contra la corrida del conjunto por defecto en los 12 repos: 811 de 811 y 342 de
  342, ninguno de más.
- staticcheck SA1019 (hecho, 2026-10-27), el uso de lo marcado con `Deprecated:`. Lo exportado no
  trae comentarios, así que el helper lee el fuente de cada paquete listado (sólo los archivos que
  nombran `Deprecated: `) con el recorrido del analizador `deprecated` de staticcheck y guarda las
  deprecaciones por paquete y nombre (`Tipo.Metodo`, `Tipo.Campo`, también de interfaces); el uso
  se busca con el mismo nombre desde el objeto de go/types. Lo de la biblioteca estándar sólo con
  la tabla de staticcheck v0.8.2 (copiada en `src/go_types/stdlib_deprecations.go`, MIT) y si la
  versión de Go del módulo o del archivo ya lo tenía deprecado; las excepciones de staticcheck
  (lo propio, los tests externos, la función que está deprecada ella misma), los campos de un
  literal de struct y los imports de paquetes deprecados.
  - Es la primera regla "directa": no hay candidatos por archivo (serían todos los `x.y` del
    código); el helper decide cada aviso y la pasada del escaneo lo convierte en aviso de los
    archivos escaneados, con sus límites, sin los generados y pasado por los `//nolint` del archivo.
  - Contra la corrida del conjunto por defecto en los 12 repos: 592 de 592, ninguno de más.
- staticcheck QF1001 (hecho, 2026-10-27), la segunda regla directa: `!(a op b)` con la negación de
  un binario (los paréntesis no cuentan), salvo que alguna subexpresión sea un float según
  go/types o que haya una función literal o un tipo de función fuera de un struct o interfaz
  (`astutil.CopyExpr` no los copia). Sin tipos sobraban 9 de 34; con ellos, 136 de 136 contra
  golangci-lint en los 12 repos, ninguno de más.
- govet `stdmethods` (hecho, 2026-10-27), regla directa: un método con el nombre de una interfaz
  conocida (`Seek`, `ReadByte`, `MarshalJSON`, `Unwrap`...) con otra firma, con la tabla y las
  excepciones de x/tools v0.52.0 (`WriteTo` de más de un argumento; `Is`, `As` y `Unwrap` sólo si
  el tipo es un error). `//nolint:govet` la silencia. 28 de 28 contra golangci-lint.
- La cola chica de staticcheck (hecho, 2026-10-27), siete reglas directas en
  `src/go_types/staticcheck_tail.go`, calcadas de v0.8.2: ST1012 (variable de error del paquete
  sin `err`/`Err`), QF1011 y ST1023 (tipo de un `var` local que el valor ya da; ST1023 además
  saltea `_`, lo que no sea literal y los paquetes que importan `syscall` o `unsafe`), QF1002
  (`switch` sin valor que compara siempre lo mismo con `==`, con `astutil.Equal` y
  `MayHaveSideEffects` de staticcheck), S1005 (`_` que sobra en una recepción o un `range`),
  SA9003 (`if` o `else` vacío fuera de los `Example` de los tests) y QF1006 (`for` infinito que
  empieza con `if cond { break }`). 135 de 135 contra golangci-lint, ninguno de más.
- Diez más de la cola de staticcheck (hecho, 2026-10-27), en `src/go_types/staticcheck_patterns.go`:
  ST1019 (import repetido; como golangci-lint, marca también cada import de más, que staticcheck
  da como "related information"), SA1029 (clave básica, struct vacío o no comparable en
  `context.WithValue`), SA9004 (grupo de constantes con tipo sólo en la primera), QF1009
  (`time.Time` con `==`), ST1013 (código HTTP como número, salvo 200, 400, 404 y 500), S1000
  (`select` de un solo case), ST1017 (condición Yoda), ST1006 (receptor `self`, `this` o `_`,
  salvo en tipos genéricos), S1008 (`if c { return true }; return false`) y S1002 (comparar con
  `true` o `false`, fuera de los tests). 45 de 45 contra golangci-lint, ninguno de más.
- S1034 de staticcheck (hecho, 2026-10-27), en el mismo archivo: `switch x.(type)` sin asignar con
  case que vuelven a hacer `x.(T)` del tipo del case (salvo que haya otra aserción en el case);
  marca el switch y cada aserción, como golangci-lint con la "related information". 24 de 24.
- 42 reglas más de staticcheck (hecho, 2026-10-30), directas, calcadas de v0.8.2 sobre go/ast y
  go/types en `src/go_types/staticcheck_simple.go`, `staticcheck_checks.go` y
  `staticcheck_shapes.go`: S1004, S1006, S1009, S1010, S1011, S1012, S1017, S1023, S1024, S1028,
  S1029, S1030, S1032, S1033, S1035, S1037, S1038, S1039, S1040, SA1006, SA1012, SA1013, SA2000,
  SA2001, SA3001, SA4001, SA4004, SA4011, SA4013, SA4016, SA4021, SA4022, SA4024, SA4028, SA5002,
  SA6002, SA6003, SA6005, SA6006, SA9009, ST1008 y QF1007. Las variables repetidas de los patrones
  de staticcheck se comparan como su matcher (campo por campo, sin posiciones ni paréntesis), y
  las de IR (S1029, SA6003, SA6002, ST1008) se resuelven sobre la sintaxis: la conversión
  `[]rune(s)` escrita en el `range`, el tipo del argumento antes de pasar a interfaz y las firmas
  de funciones y literales. Contra golangci-lint v2.15.0 en un módulo con casos positivos y
  negativos de cada una (`scripts/parity/go/testdata/staticcheck`): 120 avisos idénticos, ninguno
  de más. En los 12 repos del corpus (`scripts/parity/go/corpus-helper.sh` y `compare_corpus.py`,
  contra la corrida guardada del conjunto por defecto): los 26 avisos que dan, idénticos; lo que el
  helper marca de más son líneas con `//nolint:staticcheck` y archivos generados (los mocks de
  mockery de consul), que kerbo descarta después.
  - Pendientes: S1029 y SA6003 no siguen la conversión guardada en una variable (`rs :=
    []rune(s)`), que staticcheck ve por IR. SA6006 asume Go 1.18 o más. S1011 no usa el análisis
    de pureza de staticcheck: una llamada a una función pura en el slice cuenta como efecto.
- unused (hecho, 2026-10-27), regla directa: el grafo de usos de staticcheck v0.8.2 copiado entero
  (`src/go_types/unused_graph.go`, `unused_implements.go` y `unused_unify.go`, MIT y BSD) con las
  opciones por defecto de golangci-lint, un paquete por vez, y lo que golangci-lint agrega encima:
  sin parámetros de tipo y sin repetir un objeto que aparece usado en el mismo archivo, línea y
  nombre. Lo propio (`src/go_types/unused.go`) reemplaza lo que venía de otros paquetes: archivos
  generados, `//lint:ignore U1000`, la posición que se muestra y un `CoreType` que sólo mira la
  única unión de la restricción (staticcheck usa `NormalTerms` de x/exp, que calcula
  intersecciones). Los loops sobre los iteradores de go/types de Go 1.24 pasaron a índices para
  que el helper siga compilando con Go 1.22. 249 de 249 contra golangci-lint, ninguno de más; los
  2 que kerbo marca en un módulo dentro de `testdata` de golang/tools también los marca
  golangci-lint corrido ahí (el arnés saltea esos módulos).
- Los `.go` que son symlinks (hecho, 2026-10-27): como en `go list`, un symlink a un archivo es parte
  del paquete de su carpeta, así que se recorre y su ruta resuelve las carpetas pero no el archivo
  (`canonical_location` en `src/scan/walk.rs`, la misma clave con la que el helper guarda sus
  hechos). En los demás lenguajes los symlinks se siguen salteando. errcheck queda en 10.754 de
  10.754 contra golangci-lint (los 55 de los `example_*_test.go` de etcd), ninguno de más.
- Pendientes de Go contra golangci-lint:
  - ST1005: no se verificó si las variables del paquete eximen (en el corpus no hubo caso; se
    eximen, como los tipos).
  - Lo que queda del conjunto por defecto, por volumen en el corpus (14.755 avisos de golangci-lint,
    el 99 % ya en kerbo): govet `inline` (130), SA4006 (15, pide SSA) y una cola de staticcheck
    con menos de 5 cada una.
  - govet `inline`: 32 son "debería inlinearse" (lo anotado con `//go:fix inline`) y 98 son
    "cannot inline", que salen de correr el inliner de x/tools (versión de Go del archivo,
    inferencia de tipos genéricos y otros casos): calcarlo es reimplementar un motor de refactor.
  - errcheck en el editor y en `kerbo fix`: el texto sin guardar no tiene tipos, así que ahí no se
    muestra. Y los `exclude-functions`, `check-blank` y `check-type-assertions` de errcheck no se
    configuran todavía.
  - Los tags de versión de arquitectura sólo están para amd64 (`amd64.v1`); `arm64.v8.0` y los
    demás no cumplen. Los `goexperiment` son los fijos de Go 1.27.
  - `uniq-by-line` (prendido por defecto en golangci-lint) deja un solo aviso por línea y linter;
    kerbo muestra todos.
  - El arnés de esta paridad (golangci-lint en JSON y `go list` contra kerbo en JSON) quedó fuera
    del repo, como los anteriores; desde la tanda de 42 reglas de staticcheck, la parte del helper
    está en `scripts/parity/go/` (compilar, módulo de prueba, corpus y `register.py`). Falta pasar
    ahí la corrida de golangci-lint sobre el corpus y la comparación de kerbo entero.
- Código muerto entre archivos: exports que nadie importa y dependencias declaradas que nadie
  usa, con paridad contra knip, vulture y depcheck donde las haya.

### 11. Tests y chequeos inteligentes, dependencias y secretos

- Correr sólo los tests que afecta el cambio y reusar resultados.
- Primero medir, por lenguaje, cuánto de una corrida con un cambio chico es arranque y cuánto son
  los tests.
- Runner propio sobre el motor de cada lenguaje, para bajar ese arranque sin cambiar cómo se
  comporta un test:
  - Java: el launcher de JUnit Platform en una JVM que queda viva, sin pasar por Gradle ni Maven,
    compilando sólo las clases cambiadas; los contextos de Spring quedan en caché entre corridas.
  - JavaScript/TypeScript: transformación propia con caché por contenido y workers de Node
    reusados.
  - Python: un proceso con los imports ya hechos que lanza cada corrida con un fork.
  - Go: `go test` ya compila y guarda resultados en caché; sólo la selección de tests.
- Procesos vivos con tope de memoria y servicios de los tests (bases en contenedores) reusados en
  vez de levantados de cero.
- Los chequeos de tipos en caliente, sin reimplementarlos: `tsc` (o `tsrs`, su port a Rust, que
  chequea lo mismo con un tercio de la memoria) incremental y sólo en los paquetes del monorepo
  que toca el cambio, y el demonio de mypy (`dmypy`) vivo entre corridas.
- **Mutation testing sólo sobre lo cambiado**: con el mapa de qué test ejecuta qué código, mutar
  las líneas del cambio y correr sólo los tests que las cubren. Marca el test que pasa sin
  verificar nada, que es la trampa típica de un agente.
- `kerbo gate`: el gate entero (análisis, tipos, tests, cobertura) con un planificador propio que
  decide el orden y cuántos pasos en paralelo según la memoria libre.
- Vulnerabilidades de dependencias con OSV y secretos versionados (hecho en main).
  - **`committed-secret`**: las 222 reglas de la configuración por defecto de gitleaks v8.30.1,
    copiadas tal cual en `src/secrets/gitleaks.toml` y leídas al arrancar, con sus palabras clave,
    allowlists (globales y por regla, con `condition`, `regexTarget`, rutas y stopwords),
    entropía, `secretGroup`, reglas sólo por nombre de archivo, `gitleaks:allow`, la
    decodificación de base64, hex, URL y escapes Unicode hasta 5 niveles, el descarte de binarios
    por firma y la precedencia de una regla específica sobre `generic-api-key` en la misma línea.
    Las regex de Go se traducen al dialecto de Rust (las llaves que Go toma literales) y se
    compilan cuando la palabra clave aparece. Mira todos los archivos, también los ocultos
    (`.env`, `.github/`), salvo `.git`, y respeta `.gitignore`. El secreto nunca sale entero: el
    reporte y la caché sólo guardan los primeros cuatro caracteres y el largo. Entra en la caché
    por contenido con la huella de qué patrones de ruta matchean, así el resultado no depende de
    la ruta (la caché pasa al formato 4 al sumar este tipo de entrada). Suma una regla propia, `connection-string-password` (`src/secrets/extra-rules.toml`),
    para contraseñas en URLs de bases, colas y FTP.
  - Paridad contra `gitleaks dir` (docker, configuración por defecto forzada con `--config` y sin
    `.gitleaksignore`), 2026-10-20, en 15 repos: gitleaks (con sus fixtures), trufflehog (miles de
    secretos falsos en los tests de sus detectores), Yelp/detect-secrets, OWASP juice-shop, flask,
    express, requests, fastapi, django, cpython, spring-boot, junit4, commons-lang, Guava y parte
    de VS Code: **2.251 hallazgos idénticos (archivo, línea y regla), 0 faltan y 0 sobran**. La
    regla propia suma 66, todos en fixtures de tests (65 de trufflehog, uno de spring-boot).
  - **`vulnerable-dependency`**, sólo con `kerbo check --osv` (la única regla con red): lee
    `Cargo.lock`, `package-lock.json`/`npm-shrinkwrap.json` (v1 a v3), `pnpm-lock.yaml` (v5 a v9),
    `yarn.lock` (clásico y berry), `go.mod` con `replace` y `go.sum` (sin repetir lo de `go.mod`),
    `poetry.lock`, `uv.lock`, `Pipfile.lock`, `requirements*.txt` fijados con `==`, `pom.xml` con
    los padres locales cuyas coordenadas coinciden, sus propiedades y su `dependencyManagement`, y
    `gradle.lockfile`. Consulta `api.osv.dev/v1/querybatch` (con paginado) y baja cada aviso;
    agrupa los que son alias entre sí en un solo problema con la versión que lo arregla. No usa
    un cliente HTTP propio: ureq con rustls no pasa `deny.toml` (webpki-roots tiene licencia
    CDLA-Permissive-2.0, y ring arrastra un segundo windows-sys), native-tls rompe el binario
    estático de musl, así que habla con OSV a través del `curl` del sistema, con 10 s para
    conectar, 60 s por pedido y los avisos en paralelo en un solo proceso. Las respuestas de
    `querybatch` quedan en la caché un día; los avisos, hasta que OSV informe otro `modified`.
    `kerbo osv-download [ecosistemas]` baja los `all.zip` de OSV (crates.io, npm, Go, PyPI y
    Maven: 280 MB, 31 s y 29 MB de memoria con el índice armado) y `--osv=offline` los lee sin red
    con un lector de zip propio (zip64 incluido) y miniz_oxide, evaluando rangos `SEMVER` y
    `ECOSYSTEM` y listas de versiones con comparadores de semver, PEP 440 y Maven.
  - Paridad contra osv-scanner 2.7.0 (docker), 2026-10-20: NodeGoat (package-lock), Vite 4.0
    (pnpm v5), Vue 2.6 (yarn), ripgrep 13 (Cargo), gin 1.7 (go.mod y go.sum), Poetry 1.1
    (poetry.lock), pydantic 2.10 (uv.lock) y djangoproject.com (uv.lock): **los 958 avisos que
    reporta osv-scanner, sin uno de más**; con la base bajada, 957 (el que falta OSV lo resuelve
    por un rango `GIT`). Los paquetes empaquetados dentro de otro (`inBundle`) no se reportan, como
    en osv-scanner. En WebGoat 8 (Maven, contra `--no-resolve`) están los 779 y sobran 12, todos
    reales: versiones que vienen de propiedades del POM raíz que osv-scanner no resuelve sin red.
  - Pendientes:
    - Maven sin resolución transitiva: osv-scanner por defecto le pide el árbol a deps.dev; kerbo
      sólo ve lo declarado en los POM locales (ni BOMs ni padres remotos como
      `spring-boot-starter-parent`). Gradle sólo con `gradle.lockfile`.
    - `requirements.txt` no sigue los `-r`/`-c` ni resuelve rangos; `go.mod` no suma la versión de
      `go`/`toolchain` como `stdlib` (osv-scanner tampoco lo hace hoy). Faltan `bun.lock`,
      `deno.lock`, `composer.lock`, `Gemfile.lock`, `packages.lock.json` y `pubspec.lock`.
    - Comparadores de versiones aproximados para el modo offline: PEP 440 sin segmentos locales y
      Maven sin las listas anidadas de `ComparableVersion`; los rangos `GIT` se ignoran.
    - Secretos sólo en el árbol actual: falta la historia de git (`gitleaks git`), donde queda lo
      que se borró después de commitear, y los archivos comprimidos (gitleaks tampoco los abre por
      defecto). Los archivos de más de 32 MB que no son código se saltean.
    - Las supresiones en el código valen para los secretos de los archivos de código; en los que
      no son código (`.env`, YAML, JSON) sólo vale `gitleaks:allow` o una excepción en `kerbo.toml`.
    - Memoria: los secretos suman las cachés por hilo de las regex que se usan (trufflehog pasa de
      120 a 250 MB; Guava, de 305 a 360 MB, con el mismo tiempo de CPU). Achicar la caché del DFA
      perezoso la baja un 30 % a cambio de un 40 % más de CPU; falta una estrategia mejor (regex
      por grupo de reglas o un prefiltro común).
    - La caché de OSV es un solo archivo: dos corridas a la vez pisan sus consultas (como la de
      análisis). Las respuestas viejas no se podan.
    - `kerbo baseline`, el hook y el servidor MCP todavía no consultan OSV; el hook y el LSP sí
      buscan secretos en el archivo editado.
    - Una regla de kerbo o una allowlist propia de secretos en `kerbo.toml`, además de las
      excepciones por ruta, y un `kerbo explain` por cada tipo de secreto.

### 13. Formateo propio (empezada: Go medido contra gofmt)

- Lenguaje por lenguaje, y sólo cuando la salida sea idéntica a la de la herramienta actual.
- Go (2026-10-22; la primera versión la armó un agente en una rama aparte que quedó cortada):
  `kerbo fmt [rutas]` reescribe los `.go` y `kerbo fmt --check` sólo avisa cuáles cambiarían. No
  reimplementa gofmt a ojo: arma desde tree-sitter el mismo AST que `go/parser` y lo imprime con un
  port de `go/printer` (`src/formatter/go/`: tabwriter, comentarios, cortes de línea, orden de
  imports, paréntesis que se sacan de las condiciones). `new(expresión)` de Go 1.26, que
  tree-sitter-go todavía no conoce, se parsea cambiando `new` por un nombre del mismo largo.
  - Un archivo que tree-sitter-go no lee, o que lee distinto que Go, queda como está y se avisa:
    formatearlo cambiaría el programa. Las lecturas distintas que se detectan: `if x {...}` seguido
    de un bloque suelto (tree-sitter toma `x{...}` como literal), `*(*T)(p).f` (lo lee como el campo
    de una conversión al tipo `*(*T)`) y `chan<- chan T` (lo lee como un `chan` de `<-chan T`).
    Las dos últimas, sin la guarda, cambiaban el significado del archivo.
  - Paridad contra gofmt (2026-10-22) sobre 19.143 archivos (la biblioteca estándar de Go, golang/
    tools, terraform, consul, hugo, caddy, etcd, prometheus, syncthing, tailscale, gin, cobra, chi y
    fzf), en tres versiones: el original, uno desordenado a propósito (sangrías, espacios y líneas en
    blanco cambiados sin tocar textos ni comentarios) y el desordenado con CRLF. Idénticos a gofmt:
    19.043 de 19.071 originales, 19.018 de 19.068 desordenados y 19.030 de 19.068 con CRLF (los que
    gofmt mismo no parsea, excluidos). Formatear dos veces no cambia nada salvo en 7 archivos.
  - Lo que falta, por tipo, en los originales: 16 archivos que tree-sitter-go no lee (sintaxis nueva
    como los métodos genéricos, o las lecturas distintas de arriba) y 12 comentarios de
    documentación que gofmt reescribe desde Go 1.19 (`go/doc/comment`: `//texto` pasa a `// texto`,
    las sangrías a bloques de código con tab y las listas se reacomodan) y kerbo deja como están. En
    los desordenados se suman los mismos comentarios.
  - Lo que se arregló con esta medición: un grupo de comentarios con `/* */` en posición de
    documentación ya no se junta en una línea, las directivas (`//go:noinline`, `//export`) van al
    final de la documentación separadas por `//` y el texto pierde las líneas en blanco de los
    bordes, y las líneas en blanco entre el último import y el `)` se juntan como en `SortImports`.
  - El arnés (desordenar, gofmt y kerbo sobre el mismo corpus y comparar archivo por archivo) corre
    entero en un volumen de Docker, así no choca con el largo máximo de las rutas de Windows; quedó
    fuera del repo, como los demás.
  - Pendientes: el reformateo de `go/doc/comment`, y recién después de eso recomendar `kerbo fmt`.
    Después de Go: ruff format (Python), Prettier (JS/TS) y google-java-format (Java).
- Con el formateador, compilar los tests con cobertura queda al borde de los 1,5 GB del gate: sin
  la caché caliente lo mata el límite de memoria (SIGKILL). Pasa, pero hay que ganar margen
  (partir el crate o bajar la información de depuración de los tests) antes de que crezca más.
- El test intermitente de osv (`reports_what_curl_says_when_it_fails`, "Broken pipe" cuando el curl
  falso termina antes de leer el cuerpo) quedó arreglado (2026-10-22): si curl sale antes de leer, la
  escritura se ignora y vale lo que curl dice al salir. Pasaba igual con el curl de verdad.

### 14. Más migraciones: Django y Rails

- Migraciones de Django (sobre el árbol de Python que ya existe: `AddIndex` sin
  `AddIndexConcurrently`, `AddField` con `null=False` sin default, `RemoveField`, `RenameField`) y
  de Rails (`add_index` sin `algorithm: :concurrently`, `remove_column`, `change_column`), con
  strong_migrations como referencia de Rails. Rails pide sumar una gramática de Ruby. Viene de la
  etapa 12.

### 15. Java: Javadoc, paridad con PMD y Qodana, y la API nueva

Lo que quedó de la etapa 8 cuando se cerraron las reglas de forma de `sun_checks.xml`.

- Las reglas de Javadoc de `sun_checks.xml`, por frecuencia sobre Guava, spring-boot,
  commons-lang y junit4: `JavadocVariable` (29.407), `MissingJavadocMethod` (27.721),
  `JavadocMethod` (19.370), `JavadocType` (1.921), `JavadocPackage` (1.508),
  `InvalidJavadocPosition` (13). Piden leer el Javadoc como árbol; tree-sitter lo deja como un
  comentario.
- La misma paridad contra PMD para quien usa el quickstart en vez de Checkstyle:
  `UnnecessaryImport` resuelve tipos con el classpath y no va a coincidir sin las firmas de la
  etapa 9; `UnnecessaryModifier` y `ClassWithOnlyPrivateConstructorsShouldBeFinal`, sí.
- Sumar Qodana al arnés de paridad para las reglas que coinciden con una inspección (imports sin
  usar y modificadores redundantes).
- Por lo que más aparece en Qodana: la API nueva que reemplaza a la vieja (`getFirst()`,
  `List.of`, `Math.clamp`, bloques de texto) y las redundancias (valor por defecto repetido en una
  anotación, escapes Unicode de más).

### 16. Tipos y firmas completas

Lo que la etapa 9 dejó porque pide saber más que lo que se ve en el archivo y el índice.

- Las excepciones de las firmas (`Exceptions` de cada método de los `.class`) y, con eso,
  `RedundantThrows` de IntelliJ (28 en Qodana, casi todos `throws Exception` en tests).
- El JDK desde `ct.sym` para el `release` del proyecto, en vez de la tabla propia.
- Tipos genéricos: `Map.get` y lo que devuelve una lambda en el flujo de nulos.
- El tipo de retorno de los métodos del proyecto, para seguir cadenas que pasan por ellos, y la
  inferencia de parámetros no nulos para todos los métodos (hoy sólo los `@Override`), para avisar
  de un `null` pasado a uno.
- En JS/TS, los tipos del compilador de TypeScript para reconocer conexiones y objetos del pedido
  sin depender del nombre de la variable.
