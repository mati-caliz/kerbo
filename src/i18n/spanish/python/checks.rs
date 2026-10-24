use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: chequeos de excepciones, loops y llamadas.
pub const CHECK_TEXTS: [(PythonRule, RuleText); 10] = [
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
];
