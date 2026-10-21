use super::codec::{Corrupt, Reader, Writer};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::{ImportRedundancy, LintFinding, LintKind};
use crate::rules::EslintRule;
use crate::rules::PythonRule;

const UNUSED_IMPORT: u64 = 0;
const JAVA_LANG_IMPORT: u64 = 1;
const SAME_PACKAGE_IMPORT: u64 = 2;
const DUPLICATE_IMPORT: u64 = 3;
const STAR_IMPORT: u64 = 4;
const REDUNDANT_MODIFIER: u64 = 5;
const FINAL_PARAMETER: u64 = 6;
const FINAL_CLASS: u64 = 7;
const PYTHON: u64 = 8;
const ESLINT: u64 = 9;
const NO_FIX: u64 = 0;
const WITH_FIX: u64 = 1;

pub fn write_lint(writer: &mut Writer, finding: &LintFinding) {
    write_kind(writer, finding.kind);
    writer.size(finding.line);
    writer.size(finding.column);
    writer.text(&finding.subject);
    write_fix(writer, finding.fix.as_ref());
}

pub fn read_lint(reader: &mut Reader<'_>) -> Result<LintFinding, Corrupt> {
    Ok(LintFinding {
        kind: read_kind(reader)?,
        line: reader.size()?,
        column: reader.size()?,
        subject: reader.text()?,
        fix: read_fix(reader)?,
    })
}

pub fn write_fix(writer: &mut Writer, fix: Option<&Fix>) {
    match fix {
        None => writer.number(NO_FIX),
        Some(fix) => {
            writer.number(WITH_FIX);
            writer.list(&fix.edits, write_edit);
            writer.text(&fix.hunk);
        }
    }
}

pub fn read_fix(reader: &mut Reader<'_>) -> Result<Option<Fix>, Corrupt> {
    match reader.number()? {
        NO_FIX => Ok(None),
        WITH_FIX => Ok(Some(Fix {
            edits: reader.list(read_edit)?,
            hunk: reader.text()?,
        })),
        _ => Err(Corrupt),
    }
}

fn write_kind(writer: &mut Writer, kind: LintKind) {
    let code = match kind {
        LintKind::UnusedImport => UNUSED_IMPORT,
        LintKind::RedundantImport(ImportRedundancy::JavaLang) => JAVA_LANG_IMPORT,
        LintKind::RedundantImport(ImportRedundancy::SamePackage) => SAME_PACKAGE_IMPORT,
        LintKind::RedundantImport(ImportRedundancy::Duplicate { .. }) => DUPLICATE_IMPORT,
        LintKind::StarImport => STAR_IMPORT,
        LintKind::RedundantModifier => REDUNDANT_MODIFIER,
        LintKind::FinalParameter => FINAL_PARAMETER,
        LintKind::FinalClass => FINAL_CLASS,
        LintKind::Python(_) => PYTHON,
        LintKind::Eslint(_) => ESLINT,
    };
    writer.number(code);
    match kind {
        LintKind::RedundantImport(ImportRedundancy::Duplicate { first_line }) => writer.size(first_line),
        LintKind::Python(rule) => writer.size(rule.position()),
        LintKind::Eslint(rule) => writer.size(rule.position()),
        LintKind::UnusedImport
        | LintKind::RedundantImport(ImportRedundancy::JavaLang | ImportRedundancy::SamePackage)
        | LintKind::StarImport
        | LintKind::RedundantModifier
        | LintKind::FinalParameter
        | LintKind::FinalClass => {}
    }
}

fn read_eslint_rule(reader: &mut Reader<'_>) -> Result<EslintRule, Corrupt> {
    EslintRule::ALL.get(reader.size()?).copied().ok_or(Corrupt)
}

fn read_kind(reader: &mut Reader<'_>) -> Result<LintKind, Corrupt> {
    Ok(match reader.number()? {
        UNUSED_IMPORT => LintKind::UnusedImport,
        JAVA_LANG_IMPORT => LintKind::RedundantImport(ImportRedundancy::JavaLang),
        SAME_PACKAGE_IMPORT => LintKind::RedundantImport(ImportRedundancy::SamePackage),
        DUPLICATE_IMPORT => LintKind::RedundantImport(ImportRedundancy::Duplicate {
            first_line: reader.size()?,
        }),
        STAR_IMPORT => LintKind::StarImport,
        REDUNDANT_MODIFIER => LintKind::RedundantModifier,
        FINAL_PARAMETER => LintKind::FinalParameter,
        FINAL_CLASS => LintKind::FinalClass,
        PYTHON => LintKind::Python(*PythonRule::ALL.get(reader.size()?).ok_or(Corrupt)?),
        ESLINT => LintKind::Eslint(read_eslint_rule(reader)?),
        _ => return Err(Corrupt),
    })
}

fn write_edit(writer: &mut Writer, edit: &TextEdit) {
    for point in [edit.start, edit.end] {
        writer.size(point.line);
        writer.size(point.column);
    }
    writer.text(&edit.replacement);
}

fn read_edit(reader: &mut Reader<'_>) -> Result<TextEdit, Corrupt> {
    let mut point = || -> Result<TextPoint, Corrupt> {
        Ok(TextPoint {
            line: reader.size()?,
            column: reader.size()?,
        })
    };
    let start = point()?;
    let end = point()?;
    Ok(TextEdit {
        start,
        end,
        replacement: reader.text()?,
    })
}
