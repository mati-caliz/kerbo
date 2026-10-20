use std::collections::HashSet;

use tree_sitter::Node;

use super::java::{PROGRAM, STATIC, children, qualified_name};
use super::references::referenced_names;
use super::{ImportRedundancy, LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::syntax::{named_children, one_based};

const IMPORT: &str = "import_declaration";
const PACKAGE: &str = "package_declaration";
const ASTERISK: &str = "asterisk";
const DOT: &str = ".";
const NAME_KINDS: [&str; 2] = ["scoped_identifier", "identifier"];
const JAVA_LANG: &str = "java.lang";
const JAVA_LANG_PREFIX: &str = "java.lang.";
const SEPARATOR: char = '.';

#[derive(Debug)]
struct Import<'tree> {
    declaration: Node<'tree>,
    name: Node<'tree>,
    text: String,
    is_static: bool,
    is_star: bool,
}

impl Import<'_> {
    fn line(&self) -> usize {
        one_based(self.name.start_position().row)
    }

    fn package(&self) -> &str {
        self.text.rsplit_once(SEPARATOR).map_or("", |(front, _)| front)
    }

    fn simple_name(&self) -> &str {
        self.text
            .rsplit_once(SEPARATOR)
            .map_or(self.text.as_str(), |(_, last)| last)
    }

    fn removal(&self, source: &[u8]) -> Fix {
        Fix::remove(self.declaration, source)
    }
}

pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    if root.kind() != PROGRAM {
        return Vec::new();
    }
    let imports = imports(root, source);
    if imports.is_empty() {
        return Vec::new();
    }
    let mut findings = star_imports(&imports);
    findings.extend(redundant_imports(root, &imports, source));
    findings.extend(unused_imports(root, &imports, source));
    findings
}

fn imports<'tree>(root: Node<'tree>, source: &[u8]) -> Vec<Import<'tree>> {
    named_children(root)
        .into_iter()
        .filter(|declaration| declaration.kind() == IMPORT)
        .filter_map(|declaration| {
            let tokens = children(declaration);
            Some(Import {
                declaration,
                name: tokens
                    .iter()
                    .copied()
                    .find(|token| NAME_KINDS.contains(&token.kind()))?,
                text: qualified_name(declaration, source),
                is_static: tokens.iter().any(|token| token.kind() == STATIC),
                is_star: tokens.iter().any(|token| token.kind() == ASTERISK),
            })
        })
        .collect()
}

fn star_imports(imports: &[Import<'_>]) -> Vec<LintFinding> {
    imports
        .iter()
        .filter(|import| import.is_star)
        .filter_map(|import| {
            let dot = children(import.declaration)
                .into_iter()
                .find(|token| token.kind() == DOT)?;
            Some(finding(LintKind::StarImport, dot, &import.text, None))
        })
        .collect()
}

fn redundant_imports(root: Node<'_>, imports: &[Import<'_>], source: &[u8]) -> Vec<LintFinding> {
    let package = named_children(root)
        .into_iter()
        .find(|declaration| declaration.kind() == PACKAGE)
        .map(|declaration| qualified_name(declaration, source));
    let mut findings = Vec::new();
    for (index, import) in imports.iter().enumerate() {
        let report = |redundancy| {
            finding(
                LintKind::RedundantImport(redundancy),
                import.declaration,
                &import.text,
                Some(import.removal(source)),
            )
        };
        if !import.is_static {
            if import.package() == JAVA_LANG {
                findings.push(report(ImportRedundancy::JavaLang));
            } else if package.as_deref() == Some(import.package()) {
                findings.push(report(ImportRedundancy::SamePackage));
            }
        }
        let earlier = imports.get(..index).unwrap_or_default();
        findings.extend(
            earlier
                .iter()
                .filter(|previous| previous.is_static == import.is_static && previous.text == import.text)
                .map(|previous| {
                    report(ImportRedundancy::Duplicate {
                        first_line: previous.line(),
                    })
                }),
        );
    }
    findings
}

fn unused_imports(root: Node<'_>, imports: &[Import<'_>], source: &[u8]) -> Vec<LintFinding> {
    let referenced: HashSet<&str> = referenced_names(root, source);
    imports
        .iter()
        .filter(|import| !import.is_star)
        .filter(|import| !referenced.contains(import.simple_name()) || is_java_lang_class(&import.text))
        .map(|import| {
            finding(
                LintKind::UnusedImport,
                import.name,
                &import.text,
                Some(import.removal(source)),
            )
        })
        .collect()
}

fn is_java_lang_class(text: &str) -> bool {
    text.strip_prefix(JAVA_LANG_PREFIX)
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_alphabetic()))
}
