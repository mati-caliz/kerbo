use tree_sitter::Node;

use super::java::text;
use crate::syntax::named_children;

const QUALIFIER_SEPARATOR: char = '.';
const PROGRAM: &str = "program";
const IMPORT: &str = "import_declaration";
const NULLNESS_NAMES: &[&str] = &["Nullable", "CheckForNull", "NonNull", "Nonnull", "NotNull"];
// Los paquetes de las anotaciones de nulidad que IntelliJ reconoce sin configurar nada
// (`NullableNotNullManager`). Una anotación propia con el mismo nombre (`org.apache.maven.api
// .annotations.Nullable`) no cuenta.
const KNOWN_PACKAGES: &[&str] = &[
    "org.jetbrains.annotations",
    "javax.annotation",
    "jakarta.annotation",
    "edu.umd.cs.findbugs.annotations",
    "android.support.annotation",
    "androidx.annotation",
    "com.android.annotations",
    "org.checkerframework.checker.nullness.qual",
    "org.checkerframework.checker.nullness.compatqual",
    "org.eclipse.jdt.annotation",
    "org.jspecify.annotations",
    "org.jspecify.nullness",
    "io.reactivex.annotations",
    "io.reactivex.rxjava3.annotations",
    "org.springframework.lang",
    "reactor.util.annotation",
    "io.micronaut.core.annotation",
    "lombok",
    "javax.validation.constraints",
    "jakarta.validation.constraints",
];

// Si la anotación que se escribe así cuenta para IntelliJ: las que no son de nulidad siempre; las de
// nulidad si su nombre calificado o su import son de un paquete conocido (sin import puede ser del
// mismo paquete o de un `*`: se da por buena).
pub fn is_recognized(name: Node<'_>, source: &[u8]) -> bool {
    let written = text(name, source);
    let simple = written.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(written);
    if !NULLNESS_NAMES.contains(&simple) {
        return true;
    }
    if let Some((package, _)) = written.rsplit_once(QUALIFIER_SEPARATOR) {
        return KNOWN_PACKAGES.contains(&package);
    }
    imported_package(name, simple, source).is_none_or(|package| KNOWN_PACKAGES.contains(&package))
}

fn imported_package<'source>(node: Node<'_>, simple: &str, source: &'source [u8]) -> Option<&'source str> {
    let program =
        std::iter::successors(Some(node), Node::parent).find(|ancestor| ancestor.kind() == PROGRAM)?;
    named_children(program)
        .into_iter()
        .filter(|child| child.kind() == IMPORT)
        .filter_map(|import| named_children(import).into_iter().last())
        .map(|path| text(path, source))
        .find_map(|path| {
            let (package, last) = path.rsplit_once(QUALIFIER_SEPARATOR)?;
            (last == simple).then_some(package)
        })
}
