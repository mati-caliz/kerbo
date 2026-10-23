use crate::i18n::catalog::RuleText;
use crate::rules::EslintRule;

const DEAD_BRANCH_WHY: &str =
    "Una rama que nunca corre es un error esperando: o la condición está mal o el código quedó de más.";

const UNKNOWN: RuleText = RuleText::new(
    "Una regla de ESLint",
    "seguí la regla",
    "ESLint la marca en su conjunto recomendado.",
    "Mirá la documentación de ESLint.",
);
const TEXTS: [(EslintRule, RuleText); 27] = [
    (
        EslintRule::NoVar,
        RuleText::new(
            "var en lugar de let o const",
            "declarala con let, o con const si nunca se reasigna",
            "var ignora los bloques y sube a toda la función: la variable existe antes de su línea y fuera de su bloque.",
            "for (var i = 0; ...) se vuelve for (let i = 0; ...).",
        ),
    ),
    (
        EslintRule::PreferConst,
        RuleText::new(
            "Un let que nunca se reasigna",
            "declarala con const",
            "const le dice al lector que el valor no cambia y deja que el compilador frene al que lo intente.",
            "let total = sumar(items); return total; se vuelve const total = ...",
        ),
    ),
    (
        EslintRule::NoUnusedVars,
        RuleText::new(
            "Una variable, función, import o parámetro que nadie usa",
            "borralo, o usalo si quedó olvidado",
            "Algo declarado que nunca se lee es ruido en el mejor caso y, en el peor, la señal de que se perdió el código que lo usaba.",
            "import { format, parse } from 'date-fns' con sólo parse en uso se vuelve import { parse } from 'date-fns'.",
        ),
    ),
    (
        EslintRule::NoUnusedExpressions,
        RuleText::new(
            "Una expresión cuyo resultado se descarta",
            "usá el resultado, o convertila en la sentencia que quería ser",
            "Una expresión suelta no hace nada: casi siempre falta una llamada, un return o una asignación.",
            "esValido && guardar(); se vuelve if (esValido) guardar();",
        ),
    ),
    (
        EslintRule::NoRequireImports,
        RuleText::new(
            "require() en lugar de import",
            "usá un import de ES",
            "require se resuelve en tiempo de ejecución y el compilador no puede tipar ni revisar lo que trae.",
            "const fs = require('fs') se vuelve import fs from 'fs'.",
        ),
    ),
    (
        EslintRule::NoExplicitAny,
        RuleText::new(
            "Un valor tipado como any",
            "dale su tipo real; con datos externos, unknown y validación",
            "any apaga el compilador para ese valor y para todo lo que lo toca.",
            "function parsear(entrada: any) se vuelve function parsear(entrada: unknown) con un chequeo antes de usarla.",
        ),
    ),
    (
        EslintRule::NoNonNullAssertion,
        RuleText::new(
            "Una aserción de no nulo con !",
            "revisá el valor, o tipalo para que no pueda ser null",
            "El ! le pide al compilador que te crea: si al final el valor es null, se rompe en tiempo de ejecución.",
            "usuario!.nombre se vuelve if (!usuario) throw ...; usuario.nombre.",
        ),
    ),
    (
        EslintRule::PreferRestParams,
        RuleText::new(
            "arguments en lugar de parámetros rest",
            "declará un parámetro ...resto",
            "arguments no es un arreglo, no anda en las funciones flecha y esconde lo que recibe la función.",
            "function log() { usar(arguments) } se vuelve function log(...mensajes) { usar(mensajes) }.",
        ),
    ),
    (
        EslintRule::NoPrototypeBuiltins,
        RuleText::new(
            "Una llamada a hasOwnProperty, isPrototypeOf o propertyIsEnumerable sobre el propio objeto",
            "usá Object.hasOwn(objeto, clave) u Object.prototype.hasOwnProperty.call",
            "El objeto puede tapar el método o no tenerlo (Object.create(null)): la llamada se rompe o miente.",
            "datos.hasOwnProperty(clave) se vuelve Object.hasOwn(datos, clave).",
        ),
    ),
    (
        EslintRule::NoEmpty,
        RuleText::new(
            "Un bloque vacío",
            "completalo, o dejá un comentario que diga por qué queda vacío",
            "Un bloque vacío, sobre todo un catch vacío, se traga lo que pasó sin dejar rastro.",
            "catch (error) {} se vuelve catch (error) { logger.warn(error) } o un comentario que lo explique.",
        ),
    ),
    (
        EslintRule::NoUselessEscape,
        RuleText::new(
            "Una barra invertida que no escapa nada",
            "borrá la barra",
            "Un escape de más ensucia y hace creer que el carácter significa algo que no significa.",
            "'\\-' se vuelve '-' y /[\\/]/ se vuelve /[/]/.",
        ),
    ),
    (
        EslintRule::NoCondAssign,
        RuleText::new(
            "Una asignación dentro de una condición",
            "compará con === o sacá la asignación de la condición",
            "if (a = b) casi siempre quería decir if (a === b): asigna y la condición depende del valor.",
            "if (usuario = buscar(id)) se vuelve const usuario = buscar(id); if (usuario).",
        ),
    ),
    (
        EslintRule::NoThisAlias,
        RuleText::new(
            "this guardado en una variable",
            "usá una función flecha, que conserva this",
            "const self = this es el parche de antes de las funciones flecha y deja en duda a qué objeto toca cada línea.",
            "const self = this; lista.forEach(function () { self.sumar() }) se vuelve lista.forEach(() => this.sumar()).",
        ),
    ),
    (
        EslintRule::BanTsComment,
        RuleText::new(
            "Un @ts-ignore, @ts-nocheck o @ts-expect-error sin motivo",
            "arreglá el error de tipos; si tiene que quedar, @ts-expect-error con una descripción",
            "El comentario apaga el compilador en esa línea o en todo el archivo, y @ts-ignore sigue callado aunque el error ya no exista.",
            "// @ts-ignore se vuelve el arreglo real, o // @ts-expect-error: los tipos de la biblioteca están mal (#123).",
        ),
    ),
    (
        EslintRule::NoEmptyObjectType,
        RuleText::new(
            "El tipo {} o una interfaz vacía",
            "usá object, unknown o Record<string, never>, o extendé el tipo directamente",
            "{} no quiere decir objeto vacío: acepta cualquier valor salvo null y undefined.",
            "function guardar(datos: {}) se vuelve function guardar(datos: object).",
        ),
    ),
    (
        EslintRule::NoCaseDeclarations,
        RuleText::new(
            "let, const, function o class sueltos dentro de un case",
            "envolvé el cuerpo del case entre llaves",
            "La declaración vive en todo el switch: los otros case la ven sin inicializar y fallan en tiempo de ejecución.",
            "case 1: const total = ...; se vuelve case 1: { const total = ...; break; }",
        ),
    ),
    (
        EslintRule::NoDebugger,
        RuleText::new(
            "Una sentencia debugger",
            "borrala",
            "Un debugger olvidado frena el programa a cualquiera que tenga abiertas las herramientas de desarrollo.",
            "debugger; se borra antes del commit.",
        ),
    ),
    (
        EslintRule::NoDuplicateCase,
        RuleText::new(
            "Un case repetido en el mismo switch",
            "borrá el case repetido o corregí su valor",
            DEAD_BRANCH_WHY,
            "case 'a': ... case 'a': se vuelve case 'a': ... case 'b':",
        ),
    ),
    (
        EslintRule::NoDupeElseIf,
        RuleText::new(
            "Un else if cuya condición ya cubrió uno anterior",
            "corregí la condición o borrá la rama",
            DEAD_BRANCH_WHY,
            "if (a) ... else if (a) ... se vuelve else if (b).",
        ),
    ),
    (
        EslintRule::NoDupeKeys,
        RuleText::new(
            "Una clave repetida en el mismo objeto",
            "borrá una de las dos o cambiale el nombre",
            "Gana la última sin avisar: el primer valor se pierde y el lector no sabe cuál vale.",
            "{ id: 1, nombre: 'a', id: 2 } se vuelve { id: 2, nombre: 'a' }.",
        ),
    ),
    (
        EslintRule::NoRedeclare,
        RuleText::new(
            "Una variable declarada dos veces en el mismo alcance",
            "borrá la segunda declaración o usá otro nombre",
            "La segunda pisa a la primera sin avisar: quien lee no sabe cuál vale ni desde dónde.",
            "var total = 0; ... var total = suma(); se vuelve total = suma();.",
        ),
    ),
    (
        EslintRule::NoFuncAssign,
        RuleText::new(
            "Se le asigna algo al nombre de una función declarada",
            "guardá el valor en otra variable",
            "Después de la asignación, quien llama a la función llama a otra cosa: casi siempre es un error.",
            "function cargar() {} cargar = otra; se vuelve const cargarOtra = otra;.",
        ),
    ),
    (
        EslintRule::NoWrapperObjectTypes,
        RuleText::new(
            "String, Number u Object como tipo en vez del primitivo",
            "usá el tipo primitivo en minúscula",
            "String como tipo también acepta los objetos que envuelven al primitivo (new String('a')), que se comparan por referencia y casi nunca son lo que se quiere.",
            "let nombre: String se vuelve let nombre: string.",
        ),
    ),
    (
        EslintRule::NoUnsafeFunctionType,
        RuleText::new(
            "Function como tipo",
            "escribí la firma: los parámetros y lo que devuelve",
            "Function acepta cualquier cosa invocable y la llamada devuelve any: se pierde todo el chequeo de tipos.",
            "callback: Function se vuelve callback: (error: Error | null) => void.",
        ),
    ),
    (
        EslintRule::TripleSlashReference,
        RuleText::new(
            "Una directiva /// <reference> en vez de un import",
            "importá el módulo o configurá types en el tsconfig",
            "La directiva esconde la dependencia del sistema de módulos: las herramientas no la ven y el orden de carga queda implícito.",
            "/// <reference path=\"./util.ts\" /> se vuelve import { util } from './util';.",
        ),
    ),
    (
        EslintRule::NoUnsafeFinally,
        RuleText::new(
            "Un return, throw, break o continue adentro de un finally",
            "sacalo del finally",
            "Lo que sale de un finally pisa lo que estaba haciendo el try: se pierde el valor devuelto o la excepción, sin aviso.",
            "finally { return valor; } se vuelve finally { cerrar(); } con el return después del try.",
        ),
    ),
    (
        EslintRule::PreferSpread,
        RuleText::new(
            "f.apply(this, args) en vez del operador spread",
            "llamá a la función con ...args",
            "El spread dice lo mismo sin repetir el this y sin que quien lee tenga que acordarse qué hace apply.",
            "Math.max.apply(Math, numeros) se vuelve Math.max(...numeros).",
        ),
    ),
];

pub fn eslint(rule: EslintRule) -> RuleText {
    TEXTS
        .iter()
        .find(|(candidate, _)| *candidate == rule)
        .map_or(UNKNOWN, |(_, text)| *text)
}
