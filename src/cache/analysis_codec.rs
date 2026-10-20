use super::codec::{Corrupt, Reader, Writer};
use crate::analysis::FileAnalysis;
use crate::duplicates::{CpdToken, DuplicateUnits, RepeatedBlock, RepeatedKind, SyntaxNode};
use crate::metrics::FunctionMetrics;
use crate::metrics::commented::CommentedCode;
use crate::metrics::magic::MagicNumber;
use crate::metrics::names::ShortName;
use crate::metrics::nesting::NestingFinding;

const NO_UNITS: u64 = 0;
const TOKEN_UNITS: u64 = 1;
const SYNTAX_UNITS: u64 = 2;
const REPEATED_UNITS: u64 = 3;
const REPEATED_FUNCTION: u64 = 0;
const REPEATED_BRANCH: u64 = 1;

pub fn encode(analysis: &FileAnalysis) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.size(analysis.file_lines);
    writer.list(&analysis.functions, write_function);
    writer.list(&analysis.nesting, |writer, found| {
        write_position(writer, found.line, found.column);
        writer.size(found.depth);
    });
    writer.list(&analysis.magic_numbers, |writer, found| {
        write_position(writer, found.line, found.column);
        writer.text(&found.text);
    });
    writer.list(&analysis.short_names, |writer, found| {
        write_position(writer, found.line, found.column);
        writer.text(&found.name);
    });
    writer.list(&analysis.commented_code, |writer, found| {
        write_position(writer, found.line, found.column);
        writer.text(&found.text);
    });
    write_units(&mut writer, analysis.duplicate_units.as_ref());
    writer.into_bytes()
}

pub fn decode(bytes: &[u8]) -> Result<FileAnalysis, Corrupt> {
    let mut reader = Reader::new(bytes);
    let analysis = FileAnalysis {
        file_lines: reader.size()?,
        functions: reader.list(read_function)?,
        nesting: reader.list(|reader| {
            Ok(NestingFinding {
                line: reader.size()?,
                column: reader.size()?,
                depth: reader.size()?,
            })
        })?,
        magic_numbers: reader.list(|reader| {
            Ok(MagicNumber {
                line: reader.size()?,
                column: reader.size()?,
                text: reader.text()?,
            })
        })?,
        short_names: reader.list(|reader| {
            Ok(ShortName {
                line: reader.size()?,
                column: reader.size()?,
                name: reader.text()?,
            })
        })?,
        commented_code: reader.list(|reader| {
            Ok(CommentedCode {
                line: reader.size()?,
                column: reader.size()?,
                text: reader.text()?,
            })
        })?,
        duplicate_units: read_units(&mut reader)?,
    };
    if reader.is_empty() {
        Ok(analysis)
    } else {
        Err(Corrupt)
    }
}

fn write_position(writer: &mut Writer, line: usize, column: usize) {
    writer.size(line);
    writer.size(column);
}

fn write_function(writer: &mut Writer, function: &FunctionMetrics) {
    writer.text(&function.name);
    write_position(writer, function.line, function.column);
    writer.size(function.end_line);
    writer.optional_size(function.lines);
    writer.optional_size(function.cyclomatic);
    writer.optional_size(function.cognitive);
    writer.optional_size(function.parameters);
}

fn read_function(reader: &mut Reader<'_>) -> Result<FunctionMetrics, Corrupt> {
    Ok(FunctionMetrics {
        name: reader.text()?,
        line: reader.size()?,
        column: reader.size()?,
        end_line: reader.size()?,
        lines: reader.optional_size()?,
        cyclomatic: reader.optional_size()?,
        cognitive: reader.optional_size()?,
        parameters: reader.optional_size()?,
    })
}

fn write_units(writer: &mut Writer, units: Option<&DuplicateUnits>) {
    match units {
        None => writer.number(NO_UNITS),
        Some(DuplicateUnits::Tokens(tokens)) => {
            writer.number(TOKEN_UNITS);
            writer.list(tokens, |writer, token| {
                writer.fixed(token.image);
                writer.size(token.line);
                writer.size(token.end_line);
            });
        }
        Some(DuplicateUnits::Syntax(nodes)) => {
            writer.number(SYNTAX_UNITS);
            writer.list(nodes, write_syntax_node);
        }
        Some(DuplicateUnits::Repeated(blocks)) => {
            writer.number(REPEATED_UNITS);
            writer.list(blocks, write_repeated_block);
        }
    }
}

fn read_units(reader: &mut Reader<'_>) -> Result<Option<DuplicateUnits>, Corrupt> {
    let units = match reader.number()? {
        NO_UNITS => return Ok(None),
        TOKEN_UNITS => DuplicateUnits::Tokens(reader.list(|reader| {
            Ok(CpdToken {
                image: reader.fixed()?,
                line: reader.size()?,
                end_line: reader.size()?,
            })
        })?),
        SYNTAX_UNITS => DuplicateUnits::Syntax(reader.list(read_syntax_node)?),
        REPEATED_UNITS => DuplicateUnits::Repeated(reader.list(read_repeated_block)?),
        _ => return Err(Corrupt),
    };
    Ok(Some(units))
}

fn write_syntax_node(writer: &mut Writer, node: &SyntaxNode) {
    writer.number(u64::from(u32::from_le_bytes(node.kind.to_le_bytes())));
    writer.size(node.start);
    writer.size(node.end);
    writer.size(node.owns);
    writer.size(node.line);
    writer.size(node.end_line);
}

fn read_syntax_node(reader: &mut Reader<'_>) -> Result<SyntaxNode, Corrupt> {
    let kind = u32::try_from(reader.number()?).map_err(|_| Corrupt)?;
    Ok(SyntaxNode {
        kind: i32::from_le_bytes(kind.to_le_bytes()),
        start: reader.size()?,
        end: reader.size()?,
        owns: reader.size()?,
        line: reader.size()?,
        end_line: reader.size()?,
    })
}

fn write_repeated_block(writer: &mut Writer, block: &RepeatedBlock) {
    writer.number(match block.kind {
        RepeatedKind::Function => REPEATED_FUNCTION,
        RepeatedKind::Branch => REPEATED_BRANCH,
    });
    writer.size(block.line);
    writer.size(block.end_line);
    writer.size(block.original_line);
    writer.size(block.lines);
    writer.fixed(block.content);
}

fn read_repeated_block(reader: &mut Reader<'_>) -> Result<RepeatedBlock, Corrupt> {
    let kind = match reader.number()? {
        REPEATED_FUNCTION => RepeatedKind::Function,
        REPEATED_BRANCH => RepeatedKind::Branch,
        _ => return Err(Corrupt),
    };
    Ok(RepeatedBlock {
        kind,
        line: reader.size()?,
        end_line: reader.size()?,
        original_line: reader.size()?,
        lines: reader.size()?,
        content: reader.fixed()?,
    })
}
