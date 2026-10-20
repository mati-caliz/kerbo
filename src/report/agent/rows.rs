use std::io::{self, Write};

use super::{EXPLAIN_HINT, FileGroup, Report, RuleFix};

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
    if !report.fixes.is_empty() {
        writeln!(writer, "fix[{}]:", report.fixes.len())?;
        for fix in &report.fixes {
            write_fix(fix, writer)?;
        }
    }
    writeln!(writer, "explain: {EXPLAIN_HINT}")
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

fn write_fix<W: Write + ?Sized>(fix: &RuleFix, writer: &mut W) -> io::Result<()> {
    writeln!(writer, "{INDENT}{}: {}", fix.rule.as_str(), fix.text)
}
