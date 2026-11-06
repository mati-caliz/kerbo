use crate::i18n::catalog::RuleText;
use crate::rules::EslintRule;

pub const RECOMMENDED_TEXTS: [(EslintRule, RuleText); 24] = [
    (
        EslintRule::ForDirection,
        RuleText::new(
            "for cuyo contador se aleja de la condición de corte",
            "cambiá el incremento, o la condición",
            "El loop no corre nunca o no termina nunca.",
            "for (let i = 0; i < 10; i--) se vuelve for (let i = 0; i < 10; i++).",
        ),
    ),
    (
        EslintRule::NoDeleteVar,
        RuleText::new(
            "delete sobre una variable",
            "asigná undefined, o sacá el delete",
            "delete sólo borra propiedades de un objeto; sobre una variable no hace nada o es un error.",
            "delete a; se vuelve a = undefined;.",
        ),
    ),
    (
        EslintRule::NoDupeArgs,
        RuleText::new(
            "Nombre de parámetro repetido en una función",
            "renombrá uno de los parámetros",
            "El último tapa a los demás, y en modo estricto ni siquiera parsea.",
            "function f(a, a) se vuelve function f(a, b).",
        ),
    ),
    (
        EslintRule::NoDupeClassMembers,
        RuleText::new(
            "Miembro de clase repetido",
            "renombralo, o sacá uno",
            "La última definición reemplaza a las anteriores sin aviso.",
            "a() {} a() {} deja uno solo.",
        ),
    ),
    (
        EslintRule::NoEmptyStaticBlock,
        RuleText::new(
            "Bloque static vacío",
            "sacalo, o agregá un comentario que lo explique",
            "No hace nada, así que probablemente es código sin terminar.",
            "static {} se borra.",
        ),
    ),
    (
        EslintRule::NoExAssign,
        RuleText::new(
            "Asignación a la excepción de un catch",
            "usá otra variable",
            "Se pierde el error original, y lo que sigue ve otra cosa.",
            "catch (e) { e = 1; } usa otra variable.",
        ),
    ),
    (
        EslintRule::NoNewNativeNonconstructor,
        RuleText::new(
            "new sobre Symbol o BigInt",
            "llamalo sin new",
            "No son constructores: new tira un TypeError.",
            "new Symbol() se vuelve Symbol().",
        ),
    ),
    (
        EslintRule::NoNonoctalDecimalEscape,
        RuleText::new(
            "\\8 o \\9 en un texto",
            "escribí el dígito sin la barra",
            "No son escapes válidos, y el modo estricto los prohíbe en templates.",
            "\"\\8\" se vuelve \"8\".",
        ),
    ),
    (
        EslintRule::NoObjCalls,
        RuleText::new(
            "Llamada a un global que no es una función",
            "llamá a uno de sus miembros",
            "Math, JSON, Reflect y compañía son objetos comunes: llamarlos tira un error.",
            "Math() se vuelve Math.floor(x).",
        ),
    ),
    (
        EslintRule::NoOctal,
        RuleText::new(
            "Número con un 0 adelante",
            "sacá el 0, o escribí 0o",
            "Se lee como octal: 010 es 8.",
            "010 se vuelve 0o10.",
        ),
    ),
    (
        EslintRule::NoUnsafeNegation,
        RuleText::new(
            "! a la izquierda de in o instanceof",
            "usá paréntesis: !(a in b)",
            "El ! se aplica primero, así que compara un booleano.",
            "!a in b se vuelve !(a in b).",
        ),
    ),
    (
        EslintRule::NoWith,
        RuleText::new(
            "Sentencia with",
            "usá el objeto explícito, o una variable",
            "No deja saber de dónde sale cada nombre, y el modo estricto la prohíbe.",
            "with (a) b; se vuelve a.b;.",
        ),
    ),
    (
        EslintRule::NoDuplicateEnumValues,
        RuleText::new(
            "Valor repetido en un enum",
            "dale a cada miembro su propio valor",
            "Dos nombres con el mismo valor no se pueden distinguir.",
            "A = 1, B = 1 se vuelve A = 1, B = 2.",
        ),
    ),
    (
        EslintRule::NoExtraNonNullAssertion,
        RuleText::new(
            "! que no agrega nada",
            "sacá el ! de más",
            "a!! es lo mismo que a!, y a!?.b se contradice.",
            "a!!; se vuelve a!;.",
        ),
    ),
    (
        EslintRule::NoMisusedNew,
        RuleText::new(
            "Constructor declarado de la forma equivocada",
            "usá una clase, o un tipo con new (): T",
            "El constructor() de una interfaz es un método, y el new() de una clase también: ninguno construye nada.",
            "interface A { new(): A } se vuelve class A {}.",
        ),
    ),
    (
        EslintRule::NoUnnecessaryTypeConstraint,
        RuleText::new(
            "extends any o extends unknown en un parámetro de tipo",
            "sacá la restricción",
            "Los parámetros de tipo ya aceptan cualquier cosa.",
            "<T extends any> se vuelve <T>.",
        ),
    ),
    (
        EslintRule::NoUnsafeDeclarationMerging,
        RuleText::new(
            "Clase e interfaz con el mismo nombre",
            "renombrá una de las dos",
            "El compilador da por hecho que los miembros de la interfaz existen, sin revisar que se inicialicen.",
            "interface A {} class A {} usa dos nombres.",
        ),
    ),
    (
        EslintRule::PreferAsConst,
        RuleText::new(
            "Tipo literal que repite el valor",
            "usá as const",
            "Repite el mismo literal dos veces, y pueden quedar distintos.",
            "'a' as 'a' se vuelve 'a' as const.",
        ),
    ),
    (
        EslintRule::PreferNamespaceKeyword,
        RuleText::new(
            "module donde corresponde namespace",
            "usá namespace",
            "module es el nombre viejo, y se confunde con la declaración de un módulo externo.",
            "module A {} se vuelve namespace A {}.",
        ),
    ),
    (
        EslintRule::NoInvalidRegexp,
        RuleText::new(
            "Expresión regular inválida en RegExp()",
            "corregí el patrón o las flags",
            "Tira un SyntaxError cuando corre esa línea.",
            "new RegExp(\"[\") se vuelve new RegExp(\"\\\\[\").",
        ),
    ),
    (
        EslintRule::NoEmptyCharacterClass,
        RuleText::new(
            "Clase de caracteres vacía en una expresión regular",
            "agregá los caracteres, o escapá el corchete",
            "[] no coincide con nada, así que la expresión entera nunca coincide.",
            "/a[]/ se vuelve /a\\\\[\\\\]/.",
        ),
    ),
    (
        EslintRule::NoRegexSpaces,
        RuleText::new(
            "Varios espacios seguidos en una expresión regular",
            "usá un cuantificador",
            "Cuesta contar cuántos espacios espera.",
            "/a  b/ se vuelve /a {2}b/.",
        ),
    ),
    (
        EslintRule::NoUselessBackreference,
        RuleText::new(
            "Referencia hacia atrás que siempre coincide con nada",
            "movela después de su grupo, o sacala",
            "El grupo todavía no coincidió o no puede coincidir por ese camino, así que no agrega nada.",
            "/\\\\1(a)/ se vuelve /(a)\\\\1/.",
        ),
    ),
    (
        EslintRule::NoIrregularWhitespace,
        RuleText::new(
            "Espacio irregular",
            "cambialo por un espacio normal",
            "Parece un espacio, pero algunos parsers lo tratan distinto y rompe las búsquedas.",
            "Un espacio de no separación se vuelve un espacio común.",
        ),
    ),
];
