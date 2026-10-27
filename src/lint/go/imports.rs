use std::collections::HashMap;

use tree_sitter::Node;

use crate::syntax::descendants;

const IMPORT_SPEC: &str = "import_spec";
const PATH_FIELD: &str = "path";
const NAME_FIELD: &str = "name";
const QUOTES: &[char] = &['"', '`'];
const PATH_SEPARATOR: char = '/';
const QUALIFIER_SEPARATOR: char = '.';
const VERSION_PREFIX: char = 'v';
const IGNORED_NAMES: &[&str] = &["_", "."];

// Los paquetes importados por el nombre con que se usan: `sql` es `database/sql`, `gin` es
// `github.com/gin-gonic/gin`. El `/v4` del final de una ruta no cuenta.
#[derive(Debug, Default)]
pub struct Imports {
    packages: HashMap<String, String>,
}

impl Imports {
    pub fn of(root: Node<'_>, source: &[u8]) -> Self {
        let text = |node: Node<'_>| node.utf8_text(source).unwrap_or_default();
        let packages = descendants(root)
            .into_iter()
            .filter(|node| node.kind() == IMPORT_SPEC)
            .filter_map(|spec| {
                let path = without_version(text(spec.child_by_field_name(PATH_FIELD)?).trim_matches(QUOTES));
                let name = spec
                    .child_by_field_name(NAME_FIELD)
                    .map_or_else(|| default_name(&path), |name| text(name).to_owned());
                (!IGNORED_NAMES.contains(&name.as_str())).then_some((name, path))
            })
            .collect();
        Self { packages }
    }

    pub fn package(&self, name: &str) -> Option<&str> {
        self.packages.get(name).map(String::as_str)
    }

    // `sql.DB` es `database/sql.DB`; lo que no empieza con un paquete importado queda igual.
    pub fn qualify(&self, written: &str) -> String {
        written
            .split_once(QUALIFIER_SEPARATOR)
            .and_then(|(package, member)| {
                self.package(package)
                    .map(|path| format!("{path}{QUALIFIER_SEPARATOR}{member}"))
            })
            .unwrap_or_else(|| written.to_owned())
    }
}

fn is_version(segment: &str) -> bool {
    segment
        .strip_prefix(VERSION_PREFIX)
        .is_some_and(|number| !number.is_empty() && number.chars().all(|digit| digit.is_ascii_digit()))
}

fn without_version(path: &str) -> String {
    match path.rsplit_once(PATH_SEPARATOR) {
        Some((rest, last)) if is_version(last) => rest.to_owned(),
        _ => path.to_owned(),
    }
}

// El nombre que toma un paquete sin alias: el último segmento, sin el `.v2` de gopkg.in.
fn default_name(path: &str) -> String {
    let last = path.rsplit(PATH_SEPARATOR).next().unwrap_or(path);
    last.split(QUALIFIER_SEPARATOR).next().unwrap_or(last).to_owned()
}
