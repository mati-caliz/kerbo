mod constraint;

use super::printer::{ESCAPE, Printer};
use constraint::{Constraint, parse, plus_build_lines, split_go_build, split_plus_build};

const GO_BUILD_LINE: &str = "//go:build ";
const TOO_COMPLEX: &str = "// +build error: expression too complex for // +build lines";
const NEWLINE: u8 = b'\n';
const FORMFEED: u8 = 0x0c;
const SLASH: u8 = b'/';

pub fn is_go_build(text: &str) -> bool {
    split_go_build(text).is_some()
}

pub fn is_plus_build(text: &str) -> bool {
    split_plus_build(text).is_some()
}

const fn is_newline(byte: u8) -> bool {
    byte == NEWLINE || byte == FORMFEED
}

// Port de `fixGoBuildLines` de go/printer: junta los `//go:build` y `// +build` arriba de todo,
// escribe la expresión en su forma canónica y regenera las líneas `// +build` a partir de ella.
pub fn fix(printer: &mut Printer<'_, '_>) {
    if printer.go_build.is_empty() && printer.plus_build.is_empty() {
        return;
    }
    let output = std::mem::take(&mut printer.output);
    let mut insert = insertion_point(&output);
    if let Some(first) = printer.go_build.first().filter(|first| **first < insert) {
        insert = *first;
    } else if let Some(first) = printer.plus_build.first().filter(|first| **first < insert) {
        insert = *first;
    }
    let block = constraint_block(&output, &printer.go_build, &printer.plus_build);
    let mut deleted: Vec<usize> = printer
        .go_build
        .iter()
        .chain(&printer.plus_build)
        .copied()
        .collect();
    deleted.sort_unstable();
    let mut after = Vec::new();
    let mut start = insert;
    for end in deleted {
        if end < start {
            continue;
        }
        append_lines(&mut after, output.get(start..end).unwrap_or_default());
        start = end.saturating_add(line_at(&output, end).len());
    }
    append_lines(&mut after, output.get(start..).unwrap_or_default());
    if let [.., second_last, last] = after.as_slice()
        && is_newline(*second_last)
        && is_newline(*last)
    {
        after.pop();
    }
    let mut fixed = output.get(..insert).unwrap_or_default().to_vec();
    fixed.extend_from_slice(&block);
    fixed.extend_from_slice(&after);
    printer.output = fixed;
}

fn insertion_point(output: &[u8]) -> usize {
    let mut insert = 0;
    let mut pos = 0;
    loop {
        let mut blank = true;
        while matches!(output.get(pos), Some(b' ' | b'\t')) {
            pos = pos.saturating_add(1);
        }
        let starts_comment = pos.saturating_add(3) < output.len()
            && output.get(pos) == Some(&ESCAPE)
            && output.get(pos.saturating_add(1)) == Some(&SLASH)
            && output.get(pos.saturating_add(2)) == Some(&SLASH);
        if starts_comment {
            blank = false;
            while output.get(pos).is_some_and(|byte| !is_newline(*byte)) {
                pos = pos.saturating_add(1);
            }
        }
        if !output.get(pos).is_some_and(|byte| is_newline(*byte)) {
            return insert;
        }
        pos = pos.saturating_add(1);
        if blank {
            insert = pos;
        }
    }
}

fn constraint_block(output: &[u8], go_build: &[usize], plus_build: &[usize]) -> Vec<u8> {
    let expression = match go_build {
        [] => synthesized(output, plus_build),
        [single] => parse(&comment_text_at(output, *single)),
        _ => None,
    };
    let mut block = Vec::new();
    match expression {
        None => {
            for start in go_build.iter().chain(plus_build) {
                block.extend_from_slice(line_at(output, *start));
            }
        }
        Some(expression) => {
            escaped_line(&mut block, &format!("{GO_BUILD_LINE}{}", expression.render()));
            if !plus_build.is_empty() {
                let lines = plus_build_lines(&expression).unwrap_or_else(|| vec![TOO_COMPLEX.to_owned()]);
                for line in lines {
                    escaped_line(&mut block, &line);
                }
            }
        }
    }
    block.push(NEWLINE);
    block
}

fn synthesized(output: &[u8], plus_build: &[usize]) -> Option<Constraint> {
    let mut expression: Option<Constraint> = None;
    for start in plus_build {
        let parsed = parse(&comment_text_at(output, *start))?;
        expression = Some(match expression {
            None => parsed,
            Some(previous) => Constraint::And(Box::new(previous), Box::new(parsed)),
        });
    }
    expression
}

fn escaped_line(block: &mut Vec<u8>, line: &str) {
    block.push(ESCAPE);
    block.extend_from_slice(line.as_bytes());
    block.push(ESCAPE);
    block.push(NEWLINE);
}

fn append_lines(target: &mut Vec<u8>, lines: &[u8]) {
    let starts_blank = lines.first().is_some_and(|byte| is_newline(*byte));
    let ends_blank = target.is_empty()
        || matches!(target.as_slice(), [.., second_last, last] if is_newline(*second_last) && is_newline(*last));
    let skipped = usize::from(starts_blank && ends_blank);
    target.extend_from_slice(lines.get(skipped..).unwrap_or_default());
}

fn line_at(output: &[u8], start: usize) -> &[u8] {
    let rest = output.get(start..).unwrap_or_default();
    let end = rest
        .iter()
        .position(|byte| is_newline(*byte))
        .map_or(rest.len(), |index| index.saturating_add(1));
    rest.get(..end).unwrap_or_default()
}

fn comment_text_at(output: &[u8], start: usize) -> String {
    let rest = output.get(start..).unwrap_or_default();
    let rest = rest.strip_prefix(&[ESCAPE]).unwrap_or(rest);
    let end = rest
        .iter()
        .position(|byte| *byte == ESCAPE || is_newline(*byte))
        .unwrap_or(rest.len());
    String::from_utf8_lossy(rest.get(..end).unwrap_or_default()).into_owned()
}
