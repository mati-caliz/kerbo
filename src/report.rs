use std::io::{self, Write};

use clap::ValueEnum;

use crate::rules::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Json,
}

pub fn render(diagnostics: &[Diagnostic], format: Format, writer: &mut impl Write) -> io::Result<()> {
    match format {
        Format::Text => {
            for diagnostic in diagnostics {
                writeln!(
                    writer,
                    "{}:{}:{}: {} {}",
                    diagnostic.path,
                    diagnostic.line,
                    diagnostic.column,
                    diagnostic.rule.as_str(),
                    diagnostic.message
                )?;
            }
        }
        Format::Json => {
            serde_json::to_writer(&mut *writer, diagnostics)?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Format, render};
    use crate::rules::{Diagnostic, RuleId};

    fn sample() -> Vec<Diagnostic> {
        vec![Diagnostic {
            path: "src/a.go".to_owned(),
            line: 3,
            column: 1,
            rule: RuleId::ParameterCount,
            message: "La función `f` recibe 5 parámetros (máximo 4)".to_owned(),
            value: 5,
            limit: 4,
        }]
    }

    #[test]
    fn renders_one_line_per_problem_as_text() {
        let mut output = Vec::new();
        render(&sample(), Format::Text, &mut output).expect("se escribe en memoria");
        assert_eq!(
            String::from_utf8_lossy(&output),
            "src/a.go:3:1: parameter-count La función `f` recibe 5 parámetros (máximo 4)\n"
        );
    }

    #[test]
    fn renders_json_with_stable_rule_ids() {
        let mut output = Vec::new();
        render(&sample(), Format::Json, &mut output).expect("se escribe en memoria");
        let parsed: serde_json::Value = serde_json::from_slice(&output).expect("es JSON válido");
        assert_eq!(parsed[0]["rule"], "parameter-count");
        assert_eq!(parsed[0]["limit"], 4);
    }
}
