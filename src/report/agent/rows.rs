use std::io::{self, Write};

use super::{AUTOFIX_COMMAND, EXPLAIN_HINT, FileGroup, Report, RuleFix};

const INDENT: &str = "  ";
const COLUMNS: &str = "id,line,end,rule,value,max";

pub(in crate::report) fn write<W: Write + ?Sized>(report: &Report<'_>, writer: &mut W) -> io::Result<()> {
    writeln!(writer, "problems: {}", report.problems)?;
    writeln!(writer, "shown: {}", report.shown())?;
    if report.more() > 0 {
        writeln!(writer, "more: {}", report.more())?;
    }
    if !report.files.is_empty() {
        writeln!(writer, "files[{}]{{{COLUMNS}}}:", report.files.len())?;
        for file in &report.files {
            write_file(file, writer)?;
        }
    }
    write_patches(report, writer)?;
    if !report.fixes.is_empty() {
        writeln!(writer, "fix[{}]:", report.fixes.len())?;
        for fix in &report.fixes {
            write_fix(fix, writer)?;
        }
    }
    writeln!(writer, "explain: {EXPLAIN_HINT}")?;
    if report.fixable > 0 {
        writeln!(writer, "autofix[{}]: {AUTOFIX_COMMAND}", report.fixable)?;
    }
    Ok(())
}

fn write_file<W: Write + ?Sized>(file: &FileGroup<'_>, writer: &mut W) -> io::Result<()> {
    writeln!(writer, "{INDENT}{}[{}]:", file.path, file.rows.len())?;
    for row in &file.rows {
        let diagnostic = row.diagnostic;
        writeln!(
            writer,
            "{INDENT}{INDENT}{},{},{},{},{},{}",
            row.id,
            diagnostic.line,
            diagnostic.end_line,
            diagnostic.rule.as_str(),
            diagnostic.value,
            diagnostic.limit
        )?;
    }
    Ok(())
}

fn write_patches<W: Write + ?Sized>(report: &Report<'_>, writer: &mut W) -> io::Result<()> {
    let patches: Vec<(&str, &str)> = report
        .files
        .iter()
        .flat_map(|file| &file.rows)
        .filter_map(|row| {
            row.diagnostic
                .fix
                .as_ref()
                .map(|fix| (row.id.as_str(), fix.hunk.as_str()))
        })
        .collect();
    if patches.is_empty() {
        return Ok(());
    }
    writeln!(writer, "patch[{}]:", patches.len())?;
    for (id, hunk) in patches {
        writeln!(writer, "{INDENT}{id}:")?;
        for line in hunk.lines() {
            writeln!(writer, "{INDENT}{INDENT}{line}")?;
        }
    }
    Ok(())
}

fn write_fix<W: Write + ?Sized>(fix: &RuleFix, writer: &mut W) -> io::Result<()> {
    writeln!(writer, "{INDENT}{}: {}", fix.rule.as_str(), fix.text)
}
