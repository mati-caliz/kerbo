mod annotate;
mod blocks;
mod directives;
mod format;
mod model;
mod normalize;
mod order;
mod sections;
mod stdlib;

use std::path::Path;

use tree_sitter::Node;

use super::Report;
use super::ast::COMMENT;
use crate::rules::PythonRule;
use blocks::{Builder, ImportGroup, Trailer};
use directives::Directives;
use model::SourceText;
use normalize::{ImportBlock, normalize};
use order::{Ordered, order};
use sections::{Location, SECTION_ORDER, Section};

const PYTHON_WHITESPACE: [char; 3] = [' ', '\t', '\u{c}'];
const HASH: char = '#';
const NEWLINE: &str = "\n";

// El archivo que se revisa: su texto y dónde está.
pub struct ImportFile<'path, 'source> {
    pub source: &'source [u8],
    pub path: Option<&'path Path>,
    pub stub: bool,
}

// I001: un bloque de imports que no está ordenado y formateado como lo dejaría isort.
pub fn reports(root: Node<'_>, nodes: &[Node<'_>], file: &ImportFile<'_, '_>) -> Vec<Report> {
    let text_source = SourceText { bytes: file.source };
    let mut comments: Vec<Node<'_>> = nodes
        .iter()
        .copied()
        .filter(|node| node.kind() == COMMENT)
        .collect();
    comments.sort_by_key(Node::start_byte);
    let directives = Directives::read(&comments, text_source);
    if directives.skip_file {
        return Vec::new();
    }
    let mut builder = Builder::new(directives, text_source);
    builder.visit_body(Some(root));
    let location = Location::of(file.path);
    builder
        .into_groups()
        .iter()
        .filter_map(|group| unsorted(group, &comments, &location, file))
        .collect()
}

fn unsorted(
    group: &ImportGroup<'_>,
    comments: &[Node<'_>],
    location: &Location,
    file: &ImportFile<'_, '_>,
) -> Option<Report> {
    let text_source = SourceText { bytes: file.source };
    let (first, last) = (*group.imports.first()?, *group.imports.last()?);
    let line_start = text_source.line_start(first.start_byte());
    let indentation = text_source.slice(line_start, first.start_byte());
    let after = text_source
        .slice(last.end_byte(), text_source.line_end(last.end_byte()))
        .trim_start();
    let shares_line = !indentation.trim().is_empty() || !(after.is_empty() || after.starts_with(HASH));
    if shares_line {
        return Some(Report::at(PythonRule::I001, first, ""));
    }
    let block_end = text_source.full_line_end(last.end_byte());
    let block_comments: Vec<Node<'_>> = comments
        .iter()
        .copied()
        .filter(|comment| comment.start_byte() >= first.start_byte() && comment.end_byte() <= block_end)
        .collect();
    let annotated = annotate::annotate(&group.imports, &block_comments, text_source);
    let mut expected = format_sections(normalize(annotated), location, indentation);
    let trailer = group
        .trailer
        .map(|trailer| if file.stub { Trailer::Sibling } else { trailer });
    expected.push_str(match trailer {
        Some(Trailer::Sibling) => NEWLINE,
        Some(Trailer::Definition) => "\n\n",
        None => "",
    });
    let actual_end = if group.trailer.is_some() {
        trailing_lines_end(text_source, block_end)
    } else {
        block_end
    };
    let actual = text_source.slice(line_start, actual_end);
    (!matches_ignoring_indentation(actual, &expected)).then(|| Report::at(PythonRule::I001, first, ""))
}

// `format_import_block`: las secciones en orden, separadas por una línea en blanco.
fn format_sections(imports: ImportBlock<'_>, location: &Location, indentation: &str) -> String {
    let mut output = String::new();
    for section_imports in by_section(imports, location) {
        if !output.is_empty() {
            output.push_str(NEWLINE);
        }
        for (index, import) in order(section_imports).into_iter().enumerate() {
            let is_first = index == 0;
            output.push_str(&match import {
                Ordered::Straight(import) => format::format_straight(&import, is_first),
                Ordered::From(statement) => format::format_from(&statement, is_first, indentation),
            });
        }
    }
    output
}

// Los imports repartidos en sus secciones, en el orden de las secciones y sin las vacías.
fn by_section<'source>(imports: ImportBlock<'source>, location: &Location) -> Vec<ImportBlock<'source>> {
    let ImportBlock { straight, from } = imports;
    let mut sections: Vec<(Section, ImportBlock<'source>)> = SECTION_ORDER
        .iter()
        .map(|section| (*section, ImportBlock::default()))
        .collect();
    for import in straight {
        let section = location.section(import.alias.name, false);
        if let Some((_, section_imports)) = sections.iter_mut().find(|(candidate, _)| *candidate == section) {
            section_imports.straight.push(import);
        }
    }
    for statement in from {
        let section = location.section(&statement.module.name(), statement.module.level > 0);
        if let Some((_, section_imports)) = sections.iter_mut().find(|(candidate, _)| *candidate == section) {
            section_imports.from.push(statement);
        }
    }
    sections
        .into_iter()
        .map(|(_, section_imports)| section_imports)
        .filter(|section_imports| !section_imports.straight.is_empty() || !section_imports.from.is_empty())
        .collect()
}

// `trailing_lines_end`: después del bloque, las líneas en blanco que lo siguen.
fn trailing_lines_end(text_source: SourceText<'_>, block_end: usize) -> usize {
    let mut end = block_end;
    while end < text_source.bytes.len() {
        let next = text_source.full_line_end(end);
        if !text_source
            .slice(end, next)
            .trim_matches(|character: char| {
                PYTHON_WHITESPACE.contains(&character) || character == '\n' || character == '\r'
            })
            .is_empty()
        {
            break;
        }
        end = next;
    }
    end
}

fn universal_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').map(|line| line.trim_end_matches('\r')).collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn matches_ignoring_indentation(actual: &str, expected: &str) -> bool {
    let actual_lines = universal_lines(actual);
    let expected_lines = universal_lines(expected);
    actual_lines.len() == expected_lines.len()
        && actual_lines.iter().zip(&expected_lines).all(|(left, right)| {
            left.trim_start_matches(PYTHON_WHITESPACE) == right.trim_start_matches(PYTHON_WHITESPACE)
        })
}
