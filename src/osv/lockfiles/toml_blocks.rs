use super::{Dependency, numbered_lines};
use crate::osv::Ecosystem;

const PACKAGE_HEADER: &str = "[[package]]";
const TABLE_START: char = '[';
const NAME_KEY: &str = "name = ";
const VERSION_KEY: &str = "version = ";
const QUOTE: char = '"';

pub fn packages(text: &str, ecosystem: Ecosystem) -> Vec<Dependency> {
    let mut dependencies = Vec::new();
    let mut current: Option<Dependency> = None;
    for (number, line) in numbered_lines(text) {
        if line.starts_with(TABLE_START) {
            dependencies.extend(current.take());
            if line.trim_end() == PACKAGE_HEADER {
                current = Some(Dependency {
                    ecosystem,
                    name: String::new(),
                    version: String::new(),
                    line: number,
                });
            }
            continue;
        }
        let Some(package) = current.as_mut() else {
            continue;
        };
        if let Some(name) = quoted_value(line, NAME_KEY) {
            name.clone_into(&mut package.name);
            package.line = number;
        } else if let Some(version) = quoted_value(line, VERSION_KEY) {
            version.clone_into(&mut package.version);
        }
    }
    dependencies.extend(current);
    dependencies
}

fn quoted_value<'line>(line: &'line str, key: &str) -> Option<&'line str> {
    line.strip_prefix(key)?
        .trim()
        .strip_prefix(QUOTE)?
        .strip_suffix(QUOTE)
}
