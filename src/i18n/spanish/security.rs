use super::counted;
use crate::i18n::{Advisory, Explanation, OsvProblem, SecretLeak};

pub const COMMITTED_SECRET_DESCRIPTION: &str =
    "Un secreto (clave, token o contraseña) quedó versionado en el repositorio";
pub const VULNERABLE_DEPENDENCY_DESCRIPTION: &str = "Una dependencia con una vulnerabilidad publicada en OSV";

pub const COMMITTED_SECRET: Explanation = Explanation {
    fix: "revocá el secreto, sacalo del código y leelo de una variable de entorno o de un gestor de secretos",
    why: "Lo que se versiona lo lee cualquiera con acceso al repositorio y queda en la historia aunque después se borre: hay que darlo por filtrado.",
    example: "const clave = \"AKIA…\" se vuelve const clave = process.env.AWS_ACCESS_KEY_ID; si es un dato de prueba, va como excepción en kerbo.toml con su reason.",
};
pub const VULNERABLE_DEPENDENCY: Explanation = Explanation {
    fix: "actualizá la dependencia a una versión arreglada y regenerá el lockfile",
    why: "Una vulnerabilidad publicada tiene la receta a la vista: quien ataca busca justo esas versiones.",
    example: "lodash 4.17.20 en package-lock.json pasa a 4.17.21 con npm install lodash@4.17.21; corré kerbo check --osv para confirmarlo.",
};

pub fn secret(leak: &SecretLeak) -> String {
    let SecretLeak {
        kind,
        preview,
        length,
    } = leak;
    if *length == 0 {
        return format!("Un archivo que suele guardar claves privadas ({kind}) está versionado");
    }
    if preview.is_empty() {
        let characters = counted(*length, "carácter", "caracteres");
        return format!("Posible secreto versionado ({kind}) de {characters}");
    }
    let characters = counted(*length, "carácter", "caracteres");
    format!("Posible secreto versionado ({kind}): `{preview}…`, {characters}")
}

pub fn advisory(advisory: &Advisory) -> String {
    let aliases = if advisory.aliases.is_empty() {
        String::new()
    } else {
        format!(" ({})", advisory.aliases.join(", "))
    };
    let fixed = advisory.fixed.as_ref().map_or_else(
        || ". Todavía no hay una versión arreglada".to_owned(),
        |version| format!(". Arreglada en {version}"),
    );
    format!(
        "`{}` {} tiene {}{aliases}: {}{fixed}",
        advisory.package, advisory.version, advisory.id, advisory.summary
    )
}

pub fn problem(problem: &OsvProblem) -> String {
    match problem {
        OsvProblem::Unavailable(detail) => format!("no se pudo consultar OSV: {detail}"),
        OsvProblem::InvalidResponse(detail) => format!("OSV devolvió algo inesperado: {detail}"),
        OsvProblem::DatabaseMissing(path) => {
            format!("falta la base de OSV en {path}: bajala con `kerbo osv-download`")
        }
    }
}

pub fn database_saved(ecosystem: &str, advisories: usize, path: &str) -> String {
    let saved = counted(advisories, "aviso", "avisos");
    format!("{saved} de {ecosystem} guardados en {path}")
}
