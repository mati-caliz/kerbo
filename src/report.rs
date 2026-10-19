use std::io::{self, Write};

use clap::ValueEnum;

use crate::rules::Diagnostic;

mod agent;
mod sarif;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Json,
    Sarif,
    Agent,
}

pub fn render<W: Write + ?Sized>(
    diagnostics: &[Diagnostic],
    format: Format,
    writer: &mut W,
) -> io::Result<()> {
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
        Format::Sarif => {
            serde_json::to_writer(&mut *writer, &sarif::document(diagnostics))?;
            writeln!(writer)?;
        }
        Format::Agent => {
            serde_json::to_writer(&mut *writer, &agent::document(diagnostics))?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Format, render};
    use crate::rules::{Diagnostic, RuleId};

    fn sample() -> Vec<Diagnostic> {
        vec![Diagnostic {
            path: "src/a.go".to_owned(),
            line: 3,
            column: 1,
            end_line: 9,
            rule: RuleId::ParameterCount,
            message: "La función `f` recibe 5 parámetros (máximo 4)".to_owned(),
            value: 5,
            limit: 4,
            fingerprint: "parameter-count:src/a.go:f#1".to_owned(),
            relative_path: "src/a.go".to_owned(),
            absolute_path: PathBuf::from("/repo/src/a.go"),
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

    #[test]
    fn renders_sarif_that_github_can_read() {
        let mut output = Vec::new();
        render(&sample(), Format::Sarif, &mut output).expect("se escribe en memoria");
        let parsed: serde_json::Value = serde_json::from_slice(&output).expect("es JSON válido");
        assert_eq!(parsed["version"], "2.1.0");
        let run = &parsed["runs"][0];
        assert_eq!(
            run["tool"]["driver"]["rules"].as_array().map(Vec::len),
            Some(RuleId::ALL.len())
        );
        let result = &run["results"][0];
        assert_eq!(result["ruleId"], "parameter-count");
        assert_eq!(result["ruleIndex"], 4);
        assert_eq!(
            result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "src/a.go"
        );
        assert_eq!(result["locations"][0]["physicalLocation"]["region"]["endLine"], 9);
        assert_eq!(
            result["partialFingerprints"]["kerbo/v1"],
            "parameter-count:src/a.go:f#1"
        );
    }

    #[test]
    fn renders_a_short_prioritized_list_for_agents() {
        let mut diagnostics = sample();
        let mut cheating = sample().remove(0);
        cheating.rule = RuleId::SuppressionAdded;
        cheating.value = 1;
        cheating.limit = 0;
        diagnostics.push(cheating);
        let mut output = Vec::new();
        render(&diagnostics, Format::Agent, &mut output).expect("se escribe en memoria");
        let parsed: serde_json::Value = serde_json::from_slice(&output).expect("es JSON válido");
        assert_eq!(parsed["problems"], 2);
        assert_eq!(parsed["more"], 0);
        assert_eq!(
            parsed["items"][0]["rule"], "suppression-added",
            "las trampas van primero"
        );
        assert_eq!(
            parsed["items"][1]["fix"],
            "agrupá los parámetros que viajan juntos en un objeto"
        );
        assert_eq!(parsed["items"][1]["file"], "src/a.go");
    }
}
