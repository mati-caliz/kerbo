use crate::i18n::Explanation;
use crate::rules::RuleId;

const FILE_LENGTH: Explanation = Explanation {
    fix: "partí el archivo por responsabilidad",
    why: "Un archivo largo mezcla responsabilidades: cuesta encontrar algo y cada cambio toca más de lo necesario.",
    example: "Mové cada grupo de funciones que cambian juntas a su propio módulo y dejá el original como fachada si hace falta.",
};
const FUNCTION_LENGTH: Explanation = Explanation {
    fix: "extraé pasos a funciones con nombre propio",
    why: "Una función larga hace varias cosas: el nombre deja de describirla y los tests no pueden apuntar a cada parte.",
    example: "Cada bloque con un comentario que explica qué hace es una función esperando su nombre.",
};
const CYCLOMATIC_COMPLEXITY: Explanation = Explanation {
    fix: "reemplazá ramas por retornos tempranos, tablas o polimorfismo",
    why: "Cada if, case, && o || es un camino más: con muchos, ningún test los cubre todos.",
    example: "Un switch que elige una acción por tipo se vuelve un mapa de tipo a función.",
};
const COGNITIVE_COMPLEXITY: Explanation = Explanation {
    fix: "aplaná el anidamiento con retornos tempranos y funciones auxiliares",
    why: "El anidamiento cuesta más que la cantidad de ramas: cada nivel es contexto que el lector tiene que sostener.",
    example: "if (valido) { ...todo... } se vuelve if (!valido) return; ...todo...",
};
const PARAMETER_COUNT: Explanation = Explanation {
    fix: "agrupá los parámetros que viajan juntos en un objeto",
    why: "Muchos parámetros posicionales se confunden de orden y anuncian que la función hace demasiado.",
    example: "crear(nombre, mail, edad, pais, plan) se vuelve crear(datos: NuevoUsuario).",
};
const SUPPRESSION_ADDED: Explanation = Explanation {
    fix: "arreglá la causa en vez de silenciar la herramienta",
    why: "Un eslint-disable, noqa, nolint o @SuppressWarnings nuevo esconde el problema en lugar de resolverlo.",
    example: "Si es un falso positivo comprobado, va como excepción en kerbo.toml con su reason, a la vista de todos.",
};
const TEST_DISABLED: Explanation = Explanation {
    fix: "arreglá el test; no lo saltees ni lo enfoques",
    why: "Un skip deja de verificar el comportamiento; un only deja sin correr todos los demás tests del archivo.",
    example: "Si el test está mal, corregilo; si el comportamiento cambió, actualizá lo que verifica.",
};
const TEST_REMOVED: Explanation = Explanation {
    fix: "restaurá los tests o justificá el borrado",
    why: "Borrar un test que falla hace pasar el gate sin arreglar nada.",
    example: "Si el código que probaba se fue, decilo en el commit; si no, el test vuelve.",
};
const NESTING_DEPTH: Explanation = Explanation {
    fix: "sacá el bloque interno a una función o invertí la condición con un retorno temprano",
    why: "Cada nivel de anidamiento es una condición más que el lector tiene que sostener para entender una línea.",
    example: "for (...) { if (ok) { ... } } se vuelve for (...) { if (!ok) continue; ... }",
};
const MAGIC_NUMBER: Explanation = Explanation {
    fix: "dale nombre: una constante que diga qué es el número",
    why: "Un 86400 suelto obliga a adivinar qué representa y, cuando cambia, a buscar cada copia.",
    example: "if (edad >= 18) se vuelve const MAYORIA_DE_EDAD = 18; if (edad >= MAYORIA_DE_EDAD).",
};
const SHORT_NAME: Explanation = Explanation {
    fix: "usá un nombre que diga qué guarda la variable",
    why: "Con una letra hay que leer todo el bloque para saber qué es, y l, O e I se confunden con 1 y 0.",
    example: "for (const p of pedidos) se vuelve for (const pedido of pedidos).",
};
const DUPLICATE_CODE: Explanation = Explanation {
    fix: "extraé lo repetido a una función o un tipo compartido y llamalo desde los dos lugares",
    why: "Un bloque copiado se arregla en un lugar y queda roto en el otro: cada copia es un cambio que hay que acordarse de repetir.",
    example: "Dos servicios que validan igual un pedido pasan a usar el mismo validarPedido(pedido).",
};
const COMMENTED_CODE: Explanation = Explanation {
    fix: "borrá el código comentado; si hace falta volver a él, está en el historial de git",
    why: "El código comentado no se compila ni se prueba: envejece en silencio y el lector no sabe si sigue valiendo.",
    example: "# total = sumar(pedidos) se borra; si era una alternativa, va al mensaje del commit.",
};
const TYPE_ESCAPE: Explanation = Explanation {
    fix: "tipá el valor; con datos externos, unknown y validación",
    why: "any y as unknown as apagan el compilador justo donde más falta hace.",
    example: "const datos: any = await res.json() se vuelve const datos = esquema.parse(await res.json()).",
};

const UNUSED_IMPORT: Explanation = Explanation {
    fix: "borrá el import",
    why: "Un import sin usar hace creer que el archivo depende de algo que no usa y ensucia cada diff que toca la cabecera.",
    example: "import java.util.List; sin ningún List en el archivo se borra.",
};
const REDUNDANT_IMPORT: Explanation = Explanation {
    fix: "borrá el import repetido, el de java.lang o el del mismo paquete",
    why: "Las clases de java.lang y las del mismo paquete se ven sin importarlas, y repetir un import no agrega nada.",
    example: "import java.lang.String; se borra: String ya está a la vista.",
};
const STAR_IMPORT: Explanation = Explanation {
    fix: "importá cada clase por su nombre",
    why: "Un import con comodín esconde de dónde sale cada nombre, y una clase nueva en el paquete puede chocar con otra que ya usás.",
    example: "import java.util.*; se vuelve import java.util.List; import java.util.Map;",
};
const REDUNDANT_MODIFIER: Explanation = Explanation {
    fix: "borrá el modificador",
    why: "Repetir lo que el lenguaje ya asume (public en una interfaz, final en un método privado) es ruido y hace dudar si significa algo más.",
    example: "En una interfaz, public abstract void guardar(); se vuelve void guardar();",
};
const FINAL_PARAMETER: Explanation = Explanation {
    fix: "marcá el parámetro como final",
    why: "Reasignar un parámetro pisa el valor con el que se llamó a la función y confunde al que lee el resto del cuerpo.",
    example: "void cobrar(Pedido pedido) se vuelve void cobrar(final Pedido pedido).",
};
const FINAL_CLASS: Explanation = Explanation {
    fix: "declará la clase como final",
    why: "Si sus constructores son privados nadie de afuera puede heredarla: final lo dice en la firma y lo hace cumplir el compilador.",
    example: "class Fechas { private Fechas() {} } se vuelve final class Fechas { private Fechas() {} }.",
};

pub const fn explanation(rule: RuleId) -> Explanation {
    match rule {
        RuleId::FileLength => FILE_LENGTH,
        RuleId::FunctionLength => FUNCTION_LENGTH,
        RuleId::CyclomaticComplexity => CYCLOMATIC_COMPLEXITY,
        RuleId::CognitiveComplexity => COGNITIVE_COMPLEXITY,
        RuleId::ParameterCount => PARAMETER_COUNT,
        RuleId::SuppressionAdded => SUPPRESSION_ADDED,
        RuleId::TestDisabled => TEST_DISABLED,
        RuleId::TestRemoved => TEST_REMOVED,
        RuleId::NestingDepth => NESTING_DEPTH,
        RuleId::MagicNumber => MAGIC_NUMBER,
        RuleId::ShortName => SHORT_NAME,
        RuleId::DuplicateCode => DUPLICATE_CODE,
        RuleId::CommentedCode => COMMENTED_CODE,
        RuleId::TypeEscape => TYPE_ESCAPE,
        RuleId::UnusedImport => UNUSED_IMPORT,
        RuleId::RedundantImport => REDUNDANT_IMPORT,
        RuleId::StarImport => STAR_IMPORT,
        RuleId::RedundantModifier => REDUNDANT_MODIFIER,
        RuleId::FinalParameter => FINAL_PARAMETER,
        RuleId::FinalClass => FINAL_CLASS,
        RuleId::Migration(rule) => super::deploy::migration(rule).explanation,
        RuleId::Dockerfile(rule) => super::dockerfile::dockerfile(rule).explanation,
        RuleId::Compose(rule) => super::deploy::compose(rule).explanation,
    }
}
