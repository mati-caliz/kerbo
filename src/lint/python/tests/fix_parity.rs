use std::path::Path;

use super::fixtures::FIXTURES;
use super::{lints_at, strict};
use crate::fix::{apply, compatible, edits_of};

const FIXTURE_DIRECTORY: &str = "src/lint/python/fixtures";
const SAFE_SUFFIX: &str = "fixed";
const UNSAFE_SUFFIX: &str = "unsafe";
const MAX_FIX_PASSES: usize = 10;
const PACKAGE_INIT: &str = "/__init__";
const PACKAGE_INIT_STEM: &str = "_init";
const DUMP_VARIABLE: &str = "KERBO_FIX_DUMP";

// Lo que hace `ruff check --fix`: aplica los arreglos que no se pisan, vuelve a analizar y repite.
fn fixed_like_ruff(path: &str, source: &str, unsafe_fixes: bool) -> String {
    let mut text = source.to_owned();
    for _ in 0..MAX_FIX_PASSES {
        let found = lints_at(path, &text, &strict());
        let chosen = compatible(
            found
                .iter()
                .filter_map(|finding| finding.fix.as_ref())
                .filter(|fix| fix.safe || unsafe_fixes),
        );
        if chosen.is_empty() {
            break;
        }
        let Some(next) = apply(&edits_of(&chosen), &text) else {
            break;
        };
        text = next;
    }
    text
}

// La salida de ruff al lado del fixture; si no está, ruff no cambió nada.
fn golden(path: &str, suffix: &str, source: &str) -> String {
    let flat = path.replace(PACKAGE_INIT, PACKAGE_INIT_STEM).replace('/', "_");
    let stem = flat.rsplit_once('.').map_or(flat.as_str(), |(stem, _)| stem);
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_DIRECTORY)
        .join(format!("{stem}.{suffix}"));
    std::fs::read_to_string(file).unwrap_or_else(|_| source.to_owned())
}

// Para revisar a mano: con `KERBO_FIX_DUMP=<carpeta>` deja ahí lo que arregla kerbo.
fn dump(path: &str, suffix: &str, fixed: &str) {
    if let Some(directory) = std::env::var_os(DUMP_VARIABLE) {
        let flat = path.replace(PACKAGE_INIT, PACKAGE_INIT_STEM).replace('/', "_");
        let _written = std::fs::write(Path::new(&directory).join(format!("{flat}.{suffix}")), fixed);
    }
}

fn first_difference(left: &str, right: &str) -> Option<String> {
    let mut wanted = right.lines();
    for (index, line) in left.lines().enumerate() {
        let expected = wanted.next();
        if expected != Some(line) {
            return Some(format!("línea {}: kerbo {line:?}, ruff {expected:?}", index + 1));
        }
    }
    wanted
        .next()
        .map(|extra| format!("ruff sigue con {extra:?}"))
        .or_else(|| (left != right).then(|| String::from("difieren los finales de línea")))
}

#[test]
fn fixes_exactly_what_ruff_fixes_with_its_default_rules() {
    let differences: Vec<String> = FIXTURES
        .iter()
        .flat_map(|(path, source, _)| {
            [(SAFE_SUFFIX, false), (UNSAFE_SUFFIX, true)]
                .into_iter()
                .filter_map(move |(suffix, unsafe_fixes)| {
                    let fixed = fixed_like_ruff(path, source, unsafe_fixes);
                    dump(path, suffix, &fixed);
                    first_difference(&fixed, &golden(path, suffix, source))
                        .map(|difference| format!("{path} ({suffix}): {difference}"))
                })
        })
        .collect();
    assert!(
        differences.is_empty(),
        "contra ruff --fix:\n{}",
        differences.join("\n")
    );
}
