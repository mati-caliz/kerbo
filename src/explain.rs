use crate::rules::RuleId;

pub struct Explanation {
    pub fix: &'static str,
    pub why: &'static str,
    pub example: &'static str,
}

pub const fn explain(rule: RuleId) -> Explanation {
    match rule {
        RuleId::FileLength => Explanation {
            fix: "partí el archivo por responsabilidad",
            why: "Un archivo largo mezcla responsabilidades: cuesta encontrar algo y cada cambio toca más de lo necesario.",
            example: "Mové cada grupo de funciones que cambian juntas a su propio módulo y dejá el original como fachada si hace falta.",
        },
        RuleId::FunctionLength => Explanation {
            fix: "extraé pasos a funciones con nombre propio",
            why: "Una función larga hace varias cosas: el nombre deja de describirla y los tests no pueden apuntar a cada parte.",
            example: "Cada bloque con un comentario que explica qué hace es una función esperando su nombre.",
        },
        RuleId::CyclomaticComplexity => Explanation {
            fix: "reemplazá ramas por retornos tempranos, tablas o polimorfismo",
            why: "Cada if, case, && o || es un camino más: con muchos, ningún test los cubre todos.",
            example: "Un switch que elige una acción por tipo se vuelve un mapa de tipo a función.",
        },
        RuleId::CognitiveComplexity => Explanation {
            fix: "aplaná el anidamiento con retornos tempranos y funciones auxiliares",
            why: "El anidamiento cuesta más que la cantidad de ramas: cada nivel es contexto que el lector tiene que sostener.",
            example: "if (valido) { ...todo... } se vuelve if (!valido) return; ...todo...",
        },
        RuleId::ParameterCount => Explanation {
            fix: "agrupá los parámetros que viajan juntos en un objeto",
            why: "Muchos parámetros posicionales se confunden de orden y anuncian que la función hace demasiado.",
            example: "crear(nombre, mail, edad, pais, plan) se vuelve crear(datos: NuevoUsuario).",
        },
        RuleId::SuppressionAdded => Explanation {
            fix: "arreglá la causa en vez de silenciar la herramienta",
            why: "Un eslint-disable, noqa, nolint o @SuppressWarnings nuevo esconde el problema en lugar de resolverlo.",
            example: "Si es un falso positivo comprobado, va como excepción en kerbo.toml con su reason, a la vista de todos.",
        },
        RuleId::TestDisabled => Explanation {
            fix: "arreglá el test; no lo saltees ni lo enfoques",
            why: "Un skip deja de verificar el comportamiento; un only deja sin correr todos los demás tests del archivo.",
            example: "Si el test está mal, corregilo; si el comportamiento cambió, actualizá lo que verifica.",
        },
        RuleId::TestRemoved => Explanation {
            fix: "restaurá los tests o justificá el borrado",
            why: "Borrar un test que falla hace pasar el gate sin arreglar nada.",
            example: "Si el código que probaba se fue, decilo en el commit; si no, el test vuelve.",
        },
        RuleId::NestingDepth => Explanation {
            fix: "sacá el bloque interno a una función o invertí la condición con un retorno temprano",
            why: "Cada nivel de anidamiento es una condición más que el lector tiene que sostener para entender una línea.",
            example: "for (...) { if (ok) { ... } } se vuelve for (...) { if (!ok) continue; ... }",
        },
        RuleId::MagicNumber => Explanation {
            fix: "dale nombre: una constante que diga qué es el número",
            why: "Un 86400 suelto obliga a adivinar qué representa y, cuando cambia, a buscar cada copia.",
            example: "if (edad >= 18) se vuelve const MAYORIA_DE_EDAD = 18; if (edad >= MAYORIA_DE_EDAD).",
        },
        RuleId::TypeEscape => Explanation {
            fix: "tipá el valor; con datos externos, unknown y validación",
            why: "any y as unknown as apagan el compilador justo donde más falta hace.",
            example: "const datos: any = await res.json() se vuelve const datos = esquema.parse(await res.json()).",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::explain;
    use crate::rules::RuleId;

    #[test]
    fn every_rule_has_a_fix_a_reason_and_an_example() {
        for rule in RuleId::ALL {
            let explanation = explain(rule);
            assert_ne!(explanation.fix, "");
            assert_ne!(explanation.why, "");
            assert_ne!(explanation.example, "");
        }
    }
}
