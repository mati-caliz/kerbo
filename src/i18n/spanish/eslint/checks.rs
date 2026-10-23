use crate::i18n::catalog::RuleText;
use crate::rules::EslintRule;

pub const CHECK_TEXTS: [(EslintRule, RuleText); 29] = [
    (
        EslintRule::NoConstAssign,
        RuleText::new(
            "Se le asigna algo a una constante",
            "usá let si el valor tiene que cambiar, o guardalo en otra variable",
            "Asignarle a una const tira un TypeError cuando esa línea corre: el error queda escondido hasta ese momento.",
            "const total = 0; total += 1; se vuelve let total = 0; total += 1;.",
        ),
    ),
    (
        EslintRule::NoClassAssign,
        RuleText::new(
            "Se le asigna algo al nombre de una clase",
            "guardá el valor en otra variable",
            "Después de la asignación el nombre ya no es la clase: quien la instancia más abajo recibe otra cosa.",
            "class Pedido {} Pedido = otra; se vuelve const PedidoAlternativo = otra;.",
        ),
    ),
    (
        EslintRule::NoImportAssign,
        RuleText::new(
            "Se le asigna algo a un import",
            "copiá el valor a una variable local",
            "Los imports son de sólo lectura: asignarles tira un error, y mutar un namespace importado también.",
            "import { limite } from './config'; limite = 10; se vuelve let limiteLocal = limite; limiteLocal = 10;.",
        ),
    ),
    (
        EslintRule::NoGlobalAssign,
        RuleText::new(
            "Se le asigna algo a un global de JavaScript",
            "usá otro nombre",
            "Pisar undefined, Object o Math rompe todo el código que confía en ellos, y en modo estricto es un error.",
            "undefined = 1; o Object = {}; se borran o usan otro nombre.",
        ),
    ),
    (
        EslintRule::NoShadowRestrictedNames,
        RuleText::new(
            "Una declaración con el nombre de un global reservado",
            "elegí otro nombre",
            "Tapar undefined, NaN, Infinity, arguments, eval o globalThis confunde a quien lee y puede cambiar lo que hace el código.",
            "function f(undefined) {} se vuelve function f(valor) {}.",
        ),
    ),
    (
        EslintRule::NoUnassignedVars,
        RuleText::new(
            "Una variable que se lee pero nunca recibe valor",
            "asignale un valor o borrala",
            "Siempre vale undefined: casi siempre falta la asignación o es un resto de código.",
            "let total; return total; se vuelve let total = calcular(); return total;.",
        ),
    ),
    (
        EslintRule::ValidTypeof,
        RuleText::new(
            "typeof comparado con un tipo que no existe",
            "escribí uno de los tipos que devuelve typeof: string, number, object, etc.",
            "typeof nunca devuelve otra cosa: la comparación da siempre lo mismo y casi siempre es un error de tipeo.",
            "typeof valor === 'strng' se vuelve typeof valor === 'string'.",
        ),
    ),
    (
        EslintRule::NoEmptyPattern,
        RuleText::new(
            "Una desestructuración vacía",
            "nombrá lo que querés sacar o borrá el patrón",
            "{} o [] en un patrón no saca nada; casi siempre se quiso escribir un valor por defecto.",
            "const { a: {} } = objeto; se vuelve const { a = {} } = objeto;.",
        ),
    ),
    (
        EslintRule::NoSparseArrays,
        RuleText::new(
            "Un arreglo con huecos",
            "escribí cada elemento, con undefined si hace falta",
            "Dos comas seguidas dejan un hueco que se confunde con un error de tipeo y que los métodos de arreglo saltean.",
            "[1, , 3] se vuelve [1, undefined, 3].",
        ),
    ),
    (
        EslintRule::RequireYield,
        RuleText::new(
            "Un generador sin yield",
            "agregá el yield o convertilo en una función común",
            "Un function* sin yield no produce ningún valor: o falta el yield o no tenía que ser un generador.",
            "function* ids() { return 1; } se vuelve function ids() { return 1; }.",
        ),
    ),
    (
        EslintRule::NoUselessCatch,
        RuleText::new(
            "Un catch que sólo vuelve a tirar el error",
            "borrá el try/catch, o el catch si hay un finally",
            "Atrapar un error para tirarlo igual agrega ruido sin cambiar nada.",
            "try { cargar(); } catch (e) { throw e; } se vuelve cargar();.",
        ),
    ),
    (
        EslintRule::NoCompareNegZero,
        RuleText::new(
            "Una comparación contra -0",
            "usá Object.is(valor, -0)",
            "Las comparaciones no distinguen -0 de 0: x === -0 también es verdadero con 0.",
            "x === -0 se vuelve Object.is(x, -0).",
        ),
    ),
    (
        EslintRule::NoAsyncPromiseExecutor,
        RuleText::new(
            "Una función async como ejecutor de new Promise",
            "sacá el async y usá await afuera, o devolvé la promesa directamente",
            "Si el ejecutor async tira un error, la promesa nunca se rechaza: el error se pierde.",
            "new Promise(async (resolver) => ...) se vuelve una función async que devuelve el valor.",
        ),
    ),
    (
        EslintRule::UseIsnan,
        RuleText::new(
            "Una comparación contra NaN",
            "usá Number.isNaN(valor)",
            "NaN no es igual a nada, ni a sí mismo: la comparación siempre da lo mismo.",
            "valor === NaN se vuelve Number.isNaN(valor).",
        ),
    ),
    (
        EslintRule::NoUnusedLabels,
        RuleText::new(
            "Una etiqueta que nadie usa",
            "borrala",
            "Una etiqueta sin break ni continue que la nombre es ruido, o un objeto literal mal escrito.",
            "afuera: for (...) { ... } se vuelve for (...) { ... }.",
        ),
    ),
    (
        EslintRule::NoSelfAssign,
        RuleText::new(
            "Una asignación de algo a sí mismo",
            "borrala o asigná lo que correspondía",
            "x = x no cambia nada: casi siempre es un error de tipeo en uno de los dos lados.",
            "this.nombre = this.nombre; se vuelve this.nombre = nombre;.",
        ),
    ),
    (
        EslintRule::NoSetterReturn,
        RuleText::new(
            "Un setter que devuelve un valor",
            "sacá el valor del return",
            "Lo que devuelve un setter se descarta siempre: el return con valor engaña a quien lee.",
            "set nombre(valor) { return this._nombre = valor; } se vuelve set nombre(valor) { this._nombre = valor; }.",
        ),
    ),
    (
        EslintRule::NoUnsafeOptionalChaining,
        RuleText::new(
            "Un ?. en un lugar donde undefined tira un error",
            "asegurá el valor antes o usá ?. también en el resto de la cadena",
            "Si la cadena opcional corta, el resultado es undefined y llamarlo, desestructurarlo o iterarlo tira un TypeError.",
            "(objeto?.metodo)() se vuelve objeto?.metodo?.();.",
        ),
    ),
    (
        EslintRule::NoNonNullAssertedOptionalChain,
        RuleText::new(
            "Un ! de no nulo sobre un ?.",
            "sacá el ! o el ?.",
            "El ?. dice que puede no haber valor y el ! afirma lo contrario: una de las dos cosas sobra.",
            "usuario?.nombre! se vuelve usuario?.nombre o usuario.nombre.",
        ),
    ),
    (
        EslintRule::PreserveCaughtError,
        RuleText::new(
            "Un error nuevo tirado desde un catch sin el original",
            "pasá el error atrapado en cause",
            "Sin cause se pierde la pila y el mensaje del error original: depurar se vuelve adivinar.",
            "throw new Error('falló la carga'); se vuelve throw new Error('falló la carga', { cause: error });.",
        ),
    ),
    (
        EslintRule::NoExtraBooleanCast,
        RuleText::new(
            "Un !! o Boolean() donde el valor ya se usa como booleano",
            "sacá la conversión",
            "Un if, un ternario o un ! ya convierten a booleano: la conversión extra es ruido.",
            "if (!!lista.length) se vuelve if (lista.length).",
        ),
    ),
    (
        EslintRule::NoNamespace,
        RuleText::new(
            "Un namespace de TypeScript en vez de módulos",
            "usá import y export de ES",
            "Los namespaces son anteriores a los módulos de ES: las herramientas modernas no los analizan bien y no hacen falta.",
            "namespace Utilidades { export function f() {} } se vuelve export function f() {} en su propio archivo.",
        ),
    ),
    (
        EslintRule::NoArrayConstructor,
        RuleText::new(
            "Array(...) o new Array(...) en vez de un literal",
            "usá el literal [ ]",
            "Con un solo número crea un arreglo de ese largo y con varios, uno con esos elementos: el literal no tiene esa trampa.",
            "new Array(1, 2) se vuelve [1, 2].",
        ),
    ),
    (
        EslintRule::NoUnreachable,
        RuleText::new(
            "Código que nunca corre",
            "borralo o revisá el return, throw, break o continue que lo corta",
            "Lo que sigue a un return o un throw no se ejecuta nunca: o sobra o el corte está mal puesto.",
            "return total; registrar(total); se vuelve registrar(total); return total;.",
        ),
    ),
    (
        EslintRule::NoFallthrough,
        RuleText::new(
            "Un case que sigue de largo al siguiente",
            "agregá break o un comentario // falls through si es a propósito",
            "Seguir de largo sin avisar es uno de los errores más comunes de un switch: corre el código del case siguiente.",
            "case 1: cargar(); case 2: ... se vuelve case 1: cargar(); break; case 2: ...",
        ),
    ),
    (
        EslintRule::NoUselessAssignment,
        RuleText::new(
            "Un valor asignado que nunca se lee",
            "borrá la asignación o usá el valor",
            "El valor se pisa o se descarta antes de leerse: o la asignación sobra o falta usarlo en algún lado.",
            "let total = 0; total = calcular(); se vuelve let total = calcular();.",
        ),
    ),
    (
        EslintRule::GetterReturn,
        RuleText::new(
            "Un getter que puede terminar sin devolver nada",
            "devolvé un valor en todos los caminos",
            "Quien lee la propiedad recibe undefined en ese camino, casi siempre sin querer.",
            "get total() { if (lista) return suma(lista); } se vuelve get total() { return lista ? suma(lista) : 0; }.",
        ),
    ),
    (
        EslintRule::NoLossOfPrecision,
        RuleText::new(
            "Un número que JavaScript no puede guardar tal cual",
            "escribilo con la precisión que entra en un double, o usá BigInt",
            "El valor se redondea al compilar: el número que corre no es el que está escrito.",
            "9007199254740993 se vuelve 9007199254740993n o 9007199254740992.",
        ),
    ),
    (
        EslintRule::NoControlRegex,
        RuleText::new(
            "Un carácter de control en una expresión regular",
            "escribí el escape a propósito o sacalo",
            "Los caracteres de U+0000 a U+001F casi nunca aparecen en el texto que se busca: suele ser un error al escapar.",
            "/\\x1f/ se vuelve /\\s/ o lo que se quería buscar.",
        ),
    ),
];
