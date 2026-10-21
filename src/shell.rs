mod commands;
mod context;
mod dialect;
mod directives;
mod environment;
mod flow;
mod parameters;
mod quoting;
mod repair;
mod rules;
mod severity;
mod syntax;
mod test_expression;
mod variables;
mod words;

use std::collections::HashMap;
use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::path::Path;

use tree_sitter::{Node, Parser};

use crate::analysis::{ParseFailed, SourceUnit};
use crate::deploy::Violation;
use crate::language::Language;
use crate::limits::Preset;
use crate::rules::{RuleId, ShellRule};
use crate::syntax::one_based;
use context::{Context, Finding, Origin};
use dialect::{Dialect, Interpreter, interpreter};
pub use directives::Coverage;
use directives::Directives;
use severity::Severity;

const SHELL_EXTENSIONS: &[&str] = &["sh", "bash"];
const SHEBANG_PROBE_BYTES: u64 = 256;
const LINE_FEED: char = '\n';
const CARRIAGE_RETURN: char = '\r';
const SHEBANG: &str = "#!";

pub fn detect(path: &Path) -> Option<Language> {
    let is_shell = path.extension().map_or_else(
        || has_shell_shebang(path),
        |extension| {
            extension
                .to_str()
                .is_some_and(|extension| SHELL_EXTENSIONS.contains(&extension))
        },
    );
    is_shell.then_some(Language::Shell)
}

fn has_shell_shebang(path: &Path) -> bool {
    let mut probe = String::new();
    let read = File::open(path).and_then(|file| file.take(SHEBANG_PROBE_BYTES).read_to_string(&mut probe));
    if read.is_err() && probe.is_empty() {
        return false;
    }
    let first_line = probe.split(LINE_FEED).next().unwrap_or_default();
    matches!(
        interpreter(first_line.trim_end_matches(CARRIAGE_RETURN)),
        Interpreter::Named(Some(
            Dialect::Sh | Dialect::Bash | Dialect::Dash | Dialect::Busybox
        ))
    )
}

pub fn analyze(parser: &mut Parser, unit: SourceUnit<'_>) -> Option<Result<Vec<Violation>, ParseFailed>> {
    (unit.language == Language::Shell).then(|| {
        let fallback = unit.path.and_then(fallback_dialect);
        let source = unit.source;
        let tree = parser.parse(source, None).ok_or(ParseFailed)?;
        let root = tree.root_node();
        if root.has_error() || repair::has_swallowed_lines(root) {
            return Ok(Vec::new());
        }
        if !repair::has_dangling_dollar(root, source) {
            return Ok(violations(root, source, fallback));
        }
        let repaired = repair::brace_mid_word_expansions(source);
        let repaired_tree = parser.parse(&repaired.source, None).ok_or(ParseFailed)?;
        let repaired_root = repaired_tree.root_node();
        if repaired_root.has_error()
            || repair::has_swallowed_lines(repaired_root)
            || repair::has_dangling_dollar(repaired_root, &repaired.source)
        {
            return Ok(Vec::new());
        }
        Ok(violations(repaired_root, &repaired.source, fallback)
            .into_iter()
            .map(|violation| repair::restore(violation, &repaired, source))
            .collect())
    })
}

pub fn analysis_context(language: Language, path: &Path) -> Option<u64> {
    (language == Language::Shell).then(|| {
        let mut hasher = DefaultHasher::new();
        fallback_dialect(path).hash(&mut hasher);
        hasher.finish()
    })
}

fn fallback_dialect(path: &Path) -> Option<Dialect> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(Dialect::from_extension)
}

fn violations(root: Node<'_>, source: &[u8], fallback: Option<Dialect>) -> Vec<Violation> {
    let directives = Directives::read(root, source);
    let Some(context) = context(root, source, &directives, fallback) else {
        return Vec::new();
    };
    let mut found: Vec<Finding> = rules::check(&context)
        .into_iter()
        .map(|finding| at_token(finding, source))
        .collect();
    found.sort_by_key(|finding| (finding.point.row, finding.point.column));
    found
        .into_iter()
        .map(|finding| Violation {
            rule: RuleId::Shell(finding.rule),
            line: one_based(finding.point.row),
            column: one_based(finding.point.column),
            subject: finding.subject,
            fix: finding.fix,
        })
        .collect()
}

fn context<'tree, 'source>(
    root: Node<'tree>,
    source: &'source [u8],
    directives: &Directives,
    fallback: Option<Dialect>,
) -> Option<Context<'tree, 'source>> {
    let text = std::str::from_utf8(source).unwrap_or_default();
    let first_line = text.split(LINE_FEED).next().unwrap_or_default();
    let first_line = first_line.trim_end_matches(CARRIAGE_RETURN);
    let shebang = first_line.strip_prefix(SHEBANG).map(str::trim_start);
    let overridden = directives.shell.as_deref().and_then(Dialect::from_executable);
    let dialect = match (overridden, interpreter(first_line)) {
        (None, Interpreter::Unsupported) => return None,
        (Some(dialect), _) | (None, Interpreter::Named(Some(dialect))) => dialect,
        (None, Interpreter::Named(None) | Interpreter::Missing) => fallback.unwrap_or(Dialect::Bash),
    };
    let origin = Origin {
        dialect,
        shebang,
        shell_specified: directives.shell.is_some() || fallback.is_some(),
    };
    Some(Context::new(root, source, &origin))
}

fn at_token(finding: Finding, source: &[u8]) -> Finding {
    if finding.rule == ShellRule::Sc2148 {
        return finding;
    }
    let (byte, point) = syntax::token_start(finding.byte, finding.point, source);
    Finding {
        byte,
        point,
        subject: finding.subject.trim_start().to_owned(),
        ..finding
    }
}

pub fn relaxed_by(preset: Preset) -> Vec<RuleId> {
    let relaxed: &[Severity] = match preset {
        Preset::Strict => &[],
        Preset::Recommended => &[Severity::Style],
        Preset::Minimal => &[Severity::Style, Severity::Info],
    };
    ShellRule::ALL
        .into_iter()
        .filter(|rule| relaxed.contains(&severity::severity(*rule)))
        .map(RuleId::Shell)
        .collect()
}

pub fn directive_coverage(source: &str) -> HashMap<usize, Coverage> {
    let mut parser = Parser::new();
    if parser.set_language(&tree_sitter_bash::LANGUAGE.into()).is_err() {
        return HashMap::new();
    }
    parser.parse(source, None).map_or_else(HashMap::new, |tree| {
        Directives::read(tree.root_node(), source.as_bytes()).coverage
    })
}

pub fn disables(code: &str, rule: RuleId) -> bool {
    directives::disables(code, rule.as_str())
}

#[cfg(test)]
mod tests;
