use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{NodeId, Other, Span, Stmt};
use crate::formatter::python::builders::{CommaSeparated, TrailingComma, parenthesize_if_expands};
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::trailing_comments;
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::format_node_with;
use crate::formatter::python::parentheses::parenthesized;
use crate::formatter::python::trivia::{is_line_break, is_python_whitespace};

const COMMA: &str = ",";
const DOT: &str = ".";
const WILDCARD: &str = "*";
const LINE_CONTINUATION: char = '\\';
const IMPORT: &str = "import";

const fn is_between_parts(character: char) -> bool {
    is_python_whitespace(character) || is_line_break(character) || character == LINE_CONTINUATION
}

// `DotDelimitedIdentifier` de ruff: `a . b` y un nombre partido con una barra invertida se
// escriben de corrido.
fn fmt_dotted_name(f: &mut Formatter<'_>, name: Span) {
    let text = f.text_of(name);
    if text.contains(is_between_parts) {
        let joined: String = text
            .chars()
            .filter(|character| !is_between_parts(*character))
            .collect();
        f.text(&joined);
    } else {
        f.text(text);
    }
}

fn fmt_alias(f: &mut Formatter<'_>, alias: NodeId) -> Formatted {
    let Some(Other::Alias { name, asname }) = f.ast.other(alias) else {
        return Err(f.unsupported(alias, IMPORT));
    };
    format_node_with(f, alias, |f| {
        fmt_dotted_name(f, *name);
        if let Some(asname) = asname {
            f.space();
            f.token("as");
            f.space();
            f.source_text(*asname);
        }
        let dangling = f.comments.dangling(alias);
        trailing_comments(f, dangling);
        if dangling.iter().any(SourceComment::is_own_line) {
            f.line(LineMode::Hard);
        }
        Ok(())
    })
}

fn fmt_lazy(f: &mut Formatter<'_>, is_lazy: bool) {
    if is_lazy {
        f.token("lazy");
        f.space();
    }
}

pub fn fmt_import(f: &mut Formatter<'_>, names: &[NodeId], is_lazy: bool) -> Formatted {
    fmt_lazy(f, is_lazy);
    f.token(IMPORT);
    f.space();
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            f.token(COMMA);
            f.space();
        }
        fmt_alias(f, *name)?;
    }
    Ok(())
}

fn is_wildcard(f: &Formatter<'_>, names: &[NodeId]) -> bool {
    match names {
        [alias] => matches!(
            f.ast.other(*alias),
            Some(Other::Alias { name, .. }) if f.text_of(*name) == WILDCARD
        ),
        _ => false,
    }
}

// Los nombres de un `from` entran en una línea sin paréntesis o van de a uno por línea entre
// paréntesis, con coma al final.
pub fn fmt_import_from(f: &mut Formatter<'_>, statement: NodeId) -> Formatted {
    let Some(Stmt::ImportFrom {
        module,
        names,
        level,
        is_lazy,
    }) = f.ast.stmt(statement)
    else {
        return Err(f.unsupported(statement, IMPORT));
    };
    fmt_lazy(f, *is_lazy);
    f.token("from");
    f.space();
    for _ in 0..*level {
        f.token(DOT);
    }
    if let Some(module) = module {
        fmt_dotted_name(f, *module);
    }
    f.space();
    f.token(IMPORT);
    f.space();
    if is_wildcard(f, names) {
        f.token(WILDCARD);
        return Ok(());
    }
    let end = f.ast.end(statement);
    let fmt_names = |formatter: &mut Formatter<'_>| {
        let mut joiner = CommaSeparated::new(end).with_trailing_comma(TrailingComma::OneOrMore);
        for name in names {
            joiner.entry(formatter, formatter.ast.end(*name), |nested| {
                fmt_alias(nested, *name)
            })?;
        }
        joiner.finish(formatter);
        Ok(())
    };
    let parenthesized_comments = f.comments.dangling(statement);
    if parenthesized_comments.is_empty() {
        parenthesize_if_expands(f, true, fmt_names)
    } else {
        parenthesized(f, ("(", ")"), parenthesized_comments, fmt_names)
    }
}
