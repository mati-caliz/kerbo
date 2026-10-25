use super::codec::{Corrupt, Reader, Writer};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::{ImportRedundancy, LintFinding, LintKind};
use crate::rules::PythonRule;
use crate::rules::{EslintRule, GoRule, JavaRule, TaintRule};

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
const GO: u64 = 10;
const JAVA: u64 = 11;
const TAINT: u64 = 12;
const NO_FIX: u64 = 0;
const WITH_FIX: u64 = 1;
const WITH_UNSAFE_FIX: u64 = 2;

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
            writer.number(if fix.safe { WITH_FIX } else { WITH_UNSAFE_FIX });
            writer.list(&fix.edits, write_edit);
            writer.text(&fix.hunk);
            writer.size(fix.isolation.map_or(0, |group| group.saturating_add(1)));
        }
    }
}

pub fn read_fix(reader: &mut Reader<'_>) -> Result<Option<Fix>, Corrupt> {
    match reader.number()? {
        NO_FIX => Ok(None),
        code @ (WITH_FIX | WITH_UNSAFE_FIX) => Ok(Some(Fix {
            edits: reader.list(read_edit)?,
            hunk: reader.text()?,
            safe: code == WITH_FIX,
            isolation: reader.size()?.checked_sub(1),
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
        LintKind::Go(_) => GO,
        LintKind::Java(_) => JAVA,
        LintKind::Taint(_) => TAINT,
    };
    writer.number(code);
    match kind {
        LintKind::RedundantImport(ImportRedundancy::Duplicate { first_line }) => writer.size(first_line),
        LintKind::Python(rule) => writer.size(rule.position()),
        LintKind::Eslint(rule) => writer.size(rule.position()),
        LintKind::Go(rule) => writer.size(rule.position()),
        LintKind::Java(rule) => writer.size(rule.position()),
        LintKind::Taint(rule) => writer.size(rule.position()),
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
        GO => LintKind::Go(*GoRule::ALL.get(reader.size()?).ok_or(Corrupt)?),
        JAVA => LintKind::Java(*JavaRule::ALL.get(reader.size()?).ok_or(Corrupt)?),
        TAINT => LintKind::Taint(*TaintRule::ALL.get(reader.size()?).ok_or(Corrupt)?),
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
