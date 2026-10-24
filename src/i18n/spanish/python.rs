mod checks;
mod formats;
mod idioms;
mod modern;

use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;
use formats::FORMAT_TEXTS;

const SYNTAX_WHY: &str = "Python lo rechaza al compilar el módulo: el archivo no llega a importarse.";
const ALWAYS_TRUE_WHY: &str =
    "Una tupla con elementos siempre es verdadera: la condición no depende de nada.";
const AMBIGUOUS_WHY: &str = "En muchas fuentes l, I y O se confunden con 1 y 0: el código se lee mal.";

const UNKNOWN: RuleText = RuleText::new(
    "Regla de ruff",
    "seguí la indicación de la regla",
    "ruff la marca en su conjunto por defecto.",
    "Ver la documentación de ruff.",
);
const TEXTS: [(PythonRule, RuleText); 41] = [
    (
        PythonRule::F401,
        RuleText::new(
            "Import sin usar",
            "borrá el import, o reexportalo con __all__ o `import x as x`",
            "Un import que nadie usa carga el módulo igual, agrega dependencias falsas y esconde los que sí importan.",
            "import os sin ningún os. después se borra.",
        ),
    ),
    (
        PythonRule::F402,
        RuleText::new(
            "La variable del for pisa un import",
            "renombrá la variable del loop",
            "Después del loop el nombre ya no es el módulo: cualquier uso posterior falla de forma confusa.",
            "import path y for path in paths: pasa a for item in paths:.",
        ),
    ),
    (
        PythonRule::F403,
        RuleText::new(
            "Import con comodín",
            "importá explícitamente los nombres que usás",
            "Con import * no se sabe de dónde sale cada nombre y ningún chequeo puede detectar los que no existen.",
            "from os.path import * pasa a from os.path import join, exists.",
        ),
    ),
    (
        PythonRule::F404,
        RuleText::new(
            "from __future__ después de otro código",
            "mové el import de __future__ al principio del archivo",
            SYNTAX_WHY,
            "El from __future__ import annotations va antes de cualquier otro import.",
        ),
    ),
    (
        PythonRule::F405,
        RuleText::new(
            "Nombre que puede no existir o venir de un import con comodín",
            "importá el nombre explícitamente",
            "Con un import * en el módulo no se puede saber si el nombre existe: si no, falla recién al ejecutarse.",
            "from os.path import * y join(a, b) pasa a from os.path import join.",
        ),
    ),
    (
        PythonRule::F406,
        RuleText::new(
            "Import con comodín fuera del nivel del módulo",
            "importá los nombres explícitamente",
            SYNTAX_WHY,
            "def f(): from os import * pasa a def f(): from os import path.",
        ),
    ),
    (
        PythonRule::F541,
        RuleText::new(
            "f-string sin ningún reemplazo",
            "sacale la f",
            "Una f-string sin llaves es un string común que hace pensar que falta interpolar algo.",
            "f\"hola\" pasa a \"hola\".",
        ),
    ),
    (
        PythonRule::F601,
        RuleText::new(
            "Clave literal repetida en un diccionario",
            "dejá una sola entrada por clave",
            "La última pisa a las anteriores sin aviso: el valor de la primera se pierde.",
            "{\"a\": 1, \"a\": 2} es {\"a\": 2}.",
        ),
    ),
    (
        PythonRule::F602,
        RuleText::new(
            "Variable repetida como clave de un diccionario",
            "dejá una sola entrada por clave",
            "Si las dos son la misma variable, la última pisa a la primera sin aviso.",
            "{key: 1, key: 2} es {key: 2}.",
        ),
    ),
    (
        PythonRule::F631,
        RuleText::new(
            "assert sobre una tupla",
            "sacá los paréntesis: assert condición, \"mensaje\"",
            ALWAYS_TRUE_WHY,
            "assert (x > 0, \"positivo\") pasa a assert x > 0, \"positivo\".",
        ),
    ),
    (
        PythonRule::F632,
        RuleText::new(
            "is contra un literal",
            "compará con == o !=",
            "is compara identidad: con números o strings depende de detalles del intérprete y puede dar False.",
            "x is \"a\" pasa a x == \"a\".",
        ),
    ),
    (
        PythonRule::F633,
        RuleText::new(
            "print >> de Python 2",
            "usá print(..., file=destino)",
            "En Python 3 print es una función: >> no redirige, desplaza bits y falla.",
            "print >> sys.stderr, \"x\" pasa a print(\"x\", file=sys.stderr).",
        ),
    ),
    (
        PythonRule::F634,
        RuleText::new(
            "if sobre una tupla",
            "sacá la coma o los paréntesis que arman la tupla",
            ALWAYS_TRUE_WHY,
            "if (a, b): pasa a if a and b:.",
        ),
    ),
    (
        PythonRule::F701,
        RuleText::new(
            "break fuera de un loop",
            "sacá el break o movelo adentro del for o while",
            SYNTAX_WHY,
            "Un break en el else de un for corta el loop de afuera, no ese.",
        ),
    ),
    (
        PythonRule::F702,
        RuleText::new(
            "continue fuera de un loop",
            "sacá el continue o movelo adentro del for o while",
            SYNTAX_WHY,
            "Un continue en el cuerpo de una función que no está en un loop no compila.",
        ),
    ),
    (
        PythonRule::F704,
        RuleText::new(
            "yield o await fuera de una función",
            "movelo adentro de una función (async, para await)",
            SYNTAX_WHY,
            "Un await a nivel del módulo va adentro de una async def que se corre con asyncio.run.",
        ),
    ),
    (
        PythonRule::F706,
        RuleText::new(
            "return fuera de una función",
            "sacá el return o movelo adentro de una función",
            SYNTAX_WHY,
            "Un return en el cuerpo de una clase no compila.",
        ),
    ),
    (
        PythonRule::F707,
        RuleText::new(
            "except: sin tipo antes de otros except",
            "dejá el except sin tipo al final",
            SYNTAX_WHY,
            "except: y después except ValueError: pasa a except ValueError: y al final except Exception:.",
        ),
    ),
    (
        PythonRule::F722,
        RuleText::new(
            "Anotación entre comillas que no es Python válido",
            "corregí la sintaxis de la anotación",
            "El chequeo de tipos no puede leerla y typing.get_type_hints falla al evaluarla.",
            "x: \"List[int\" pasa a x: \"List[int]\".",
        ),
    ),
    (
        PythonRule::F811,
        RuleText::new(
            "Redefinición de algo que nunca se usó",
            "borrá la definición anterior o renombrá una de las dos",
            "La primera definición queda pisada sin usarse: casi siempre es un import o una función duplicada por error.",
            "Dos def parse(...) seguidas: la primera no se llama nunca.",
        ),
    ),
    (
        PythonRule::F821,
        RuleText::new(
            "Nombre no definido",
            "definí o importá el nombre, o corregí el error de tipeo",
            "El código falla con NameError recién cuando llega a esa línea, en producción si no hay un test que pase por ahí.",
            "print(contador) sin ningún contador definido antes.",
        ),
    ),
    (
        PythonRule::F822,
        RuleText::new(
            "__all__ exporta un nombre que no existe",
            "definí el nombre o sacalo de __all__",
            "from módulo import * falla con AttributeError al buscar ese nombre.",
            "__all__ = [\"parse\"] sin ninguna parse en el módulo.",
        ),
    ),
    (
        PythonRule::F823,
        RuleText::new(
            "Variable local usada antes de asignarse",
            "asignala antes de usarla, o declarala global o nonlocal",
            "Como la función la asigna en algún lado, Python la toma como local en toda la función: el uso anterior falla con UnboundLocalError.",
            "x = 1 y def f(): print(x); x = 2 falla en el print.",
        ),
    ),
    (
        PythonRule::F841,
        RuleText::new(
            "Variable local asignada y nunca usada",
            "borrá la asignación o usá la variable",
            "Una asignación que nadie lee es código muerto o un error: quizá se quería usar en otro lado.",
            "except ValueError as error: pass pasa a except ValueError: pass.",
        ),
    ),
    (
        PythonRule::F842,
        RuleText::new(
            "Variable local anotada y nunca usada",
            "borrá la anotación",
            "Declarar el tipo de algo que nunca se asigna ni se lee no sirve para nada.",
            "total: int sin ningún uso de total en la función.",
        ),
    ),
    (
        PythonRule::F901,
        RuleText::new(
            "raise NotImplemented en lugar de NotImplementedError",
            "levantá NotImplementedError",
            "NotImplemented es un valor para los operadores, no una excepción: el raise falla con TypeError.",
            "raise NotImplemented pasa a raise NotImplementedError.",
        ),
    ),
    (
        PythonRule::E401,
        RuleText::new(
            "Varios módulos en un mismo import",
            "un import por línea",
            "Cada import en su línea se lee, se ordena y se borra sin tocar a los demás.",
            "import os, sys pasa a import os e import sys.",
        ),
    ),
    (
        PythonRule::E402,
        RuleText::new(
            "Import del módulo que no está al principio del archivo",
            "mové el import arriba, antes del resto del código",
            "Los imports juntos al principio muestran de qué depende el módulo; uno escondido más abajo se pierde.",
            "Un import json después de una función pasa a la lista de imports del principio.",
        ),
    ),
    (
        PythonRule::E701,
        RuleText::new(
            "Varias sentencias en una línea después de dos puntos",
            "pasá el cuerpo a la línea siguiente, indentado",
            "Un cuerpo en la misma línea que el if o el for se pierde al leer y complica los diffs.",
            "if x: return pasa a if x: y return en la línea siguiente.",
        ),
    ),
    (
        PythonRule::E702,
        RuleText::new(
            "Varias sentencias en una línea separadas con punto y coma",
            "una sentencia por línea",
            "Con ; en el medio cuesta ver dónde termina cada sentencia.",
            "a = 1; b = 2 pasa a dos líneas.",
        ),
    ),
    (
        PythonRule::E703,
        RuleText::new(
            "Punto y coma al final de la sentencia",
            "sacá el punto y coma",
            "En Python el ; final no hace nada: es ruido traído de otro lenguaje.",
            "x = 1; pasa a x = 1.",
        ),
    ),
    (
        PythonRule::E711,
        RuleText::new(
            "Comparación con None usando == o !=",
            "usá is None o is not None",
            "== se puede redefinir y dar cualquier cosa; None es único y se compara por identidad.",
            "if x == None: pasa a if x is None:.",
        ),
    ),
    (
        PythonRule::E712,
        RuleText::new(
            "Comparación con True o False usando == o !=",
            "usá la condición directamente, o is si importa que sea exactamente un booleano",
            "x == True es más largo que x y con valores que no son booleanos no hace lo que parece.",
            "if listo == True: pasa a if listo:.",
        ),
    ),
    (
        PythonRule::E713,
        RuleText::new(
            "not x in y en lugar de x not in y",
            "usá not in",
            "not in se lee como se dice y no depende de la precedencia de not.",
            "if not x in y: pasa a if x not in y:.",
        ),
    ),
    (
        PythonRule::E714,
        RuleText::new(
            "not x is y en lugar de x is not y",
            "usá is not",
            "is not se lee como se dice y no depende de la precedencia de not.",
            "if not x is None: pasa a if x is not None:.",
        ),
    ),
    (
        PythonRule::E721,
        RuleText::new(
            "Comparación de tipos con == o !=",
            "usá isinstance() o, si tiene que ser exactamente ese tipo, is",
            "== entre tipos ignora las subclases y se puede redefinir en la metaclase.",
            "type(x) == int pasa a isinstance(x, int) o type(x) is int.",
        ),
    ),
    (
        PythonRule::E722,
        RuleText::new(
            "except: sin tipo",
            "atrapá las excepciones que esperás, o except Exception si de verdad son todas",
            "Un except pelado también atrapa KeyboardInterrupt y SystemExit: el programa no se puede cortar y se tragan errores.",
            "except: pasa a except (ValueError, KeyError):.",
        ),
    ),
    (
        PythonRule::E731,
        RuleText::new(
            "Lambda asignada a una variable",
            "escribila como def",
            "Una def tiene nombre en los tracebacks, admite docstring y anotaciones; la lambda asignada no gana nada.",
            "doble = lambda x: x * 2 pasa a def doble(x): return x * 2.",
        ),
    ),
    (
        PythonRule::E741,
        RuleText::new(
            "Variable con un nombre ambiguo (l, I u O)",
            "usá un nombre descriptivo",
            AMBIGUOUS_WHY,
            "l = len(items) pasa a count = len(items).",
        ),
    ),
    (
        PythonRule::E742,
        RuleText::new(
            "Clase con un nombre ambiguo (l, I u O)",
            "usá un nombre descriptivo",
            AMBIGUOUS_WHY,
            "class I: pasa a class Identity:.",
        ),
    ),
    (
        PythonRule::E743,
        RuleText::new(
            "Función con un nombre ambiguo (l, I u O)",
            "usá un nombre descriptivo",
            AMBIGUOUS_WHY,
            "def l(): pasa a def lines():.",
        ),
    ),
];

pub fn python(rule: PythonRule) -> RuleText {
    TEXTS
        .iter()
        .chain(FORMAT_TEXTS.iter())
        .chain(modern::extended_texts())
        .find(|(candidate, _)| *candidate == rule)
        .map_or(UNKNOWN, |(_, text)| *text)
}
