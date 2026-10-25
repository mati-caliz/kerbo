use std::io::{self, Write};

use clap::ValueEnum;

use crate::i18n::Lang;
use crate::rules::Diagnostic;
use crate::suppressions::Suppressed;

mod agent;
mod json;
mod sarif;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Json,
    Sarif,
    Agent,
    AgentJson,
}

pub fn render<W: Write + ?Sized>(
    diagnostics: &[Diagnostic],
    format: Format,
    lang: Lang,
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
                    lang.message(&diagnostic.message)
                )?;
            }
        }
        Format::Json => {
            serde_json::to_writer(&mut *writer, &json::document(diagnostics, lang))?;
            writeln!(writer)?;
        }
        Format::Sarif => {
            serde_json::to_writer(&mut *writer, &sarif::document(diagnostics, &[], lang))?;
            writeln!(writer)?;
        }
        Format::Agent => agent::write(&agent::build(diagnostics, lang), writer)?,
        Format::AgentJson => {
            serde_json::to_writer(&mut *writer, &agent::document(&agent::build(diagnostics, lang)))?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

pub fn render_sarif<W: Write + ?Sized>(
    diagnostics: &[Diagnostic],
    suppressed: &[Suppressed],
    lang: Lang,
    writer: &mut W,
) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, &sarif::document(diagnostics, suppressed, lang))?;
    writeln!(writer)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Format, render, render_sarif};
    use crate::fix::Fix;
    use crate::i18n::{FunctionName, Lang, Message};
    use crate::rules::{Diagnostic, RuleId};
    use crate::suppressions::Suppressed;

    const UNUSED_IMPORT_HUNK: &str = "@@ -3 +2,0 @@\n-import java.util.List;";

    fn sample() -> Vec<Diagnostic> {
        vec![Diagnostic {
            path: "src/a.go".to_owned(),
            line: 3,
            column: 1,
            end_line: 9,
            rule: RuleId::ParameterCount,
            message: Message::TooManyParameters {
                function: FunctionName::Named("f".to_owned()),
                count: 5,
                limit: 4,
            },
            value: 5,
            limit: 4,
            fingerprint: "parameter-count:src/a.go:f#1".to_owned(),
            relative_path: "src/a.go".to_owned(),
            absolute_path: PathBuf::from("/repo/src/a.go"),
            fix: None,
        }]
    }

    #[test]
    fn renders_one_line_per_problem_as_text() {
        let mut output = Vec::new();
        render(&sample(), Format::Text, Lang::Spanish, &mut output).expect("se escribe en memoria");
        assert_eq!(
            String::from_utf8_lossy(&output),
            "src/a.go:3:1: parameter-count La función `f` recibe 5 parámetros (máximo 4)\n"
        );
    }

    #[test]
    fn renders_json_with_stable_rule_ids() {
        let mut output = Vec::new();
        render(&sample(), Format::Json, Lang::Spanish, &mut output).expect("se escribe en memoria");
        let parsed: serde_json::Value = serde_json::from_slice(&output).expect("es JSON válido");
        assert_eq!(parsed[0]["rule"], "parameter-count");
        assert_eq!(parsed[0]["limit"], 4);
    }

    #[test]
    fn sarif_marks_what_the_code_silences_as_suppressed_with_its_reason() {
        let suppressed: Vec<Suppressed> = sample()
            .into_iter()
            .map(|diagnostic| Suppressed {
                diagnostic,
                reason: "lo exige la API".to_owned(),
            })
            .chain(sample().into_iter().map(|diagnostic| Suppressed {
                diagnostic,
                reason: String::new(),
            }))
            .collect();
        let mut output = Vec::new();
        render_sarif(&[], &suppressed, Lang::Spanish, &mut output).expect("se escribe en memoria");
        let parsed: serde_json::Value = serde_json::from_slice(&output).expect("es JSON válido");
        let results = &parsed["runs"][0]["results"];
        assert_eq!(results[0]["suppressions"][0]["kind"], "inSource");
        assert_eq!(results[0]["suppressions"][0]["justification"], "lo exige la API");
        assert!(results[1]["suppressions"][0].get("justification").is_none());
    }

    #[test]
    fn renders_sarif_that_github_can_read() {
        let mut output = Vec::new();
        render(&sample(), Format::Sarif, Lang::Spanish, &mut output).expect("se escribe en memoria");
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
    fn renders_agent_json_grouped_by_file_with_one_fix_per_rule() {
        let mut diagnostics = sample();
        let mut cheating = sample().remove(0);
        cheating.rule = RuleId::SuppressionAdded;
        cheating.value = 1;
        cheating.limit = 0;
        cheating.fingerprint = "suppression-added:src/a.go:nolint#1".to_owned();
        diagnostics.push(cheating);
        let mut output = Vec::new();
        render(&diagnostics, Format::AgentJson, Lang::Spanish, &mut output).expect("se escribe en memoria");
        let parsed: serde_json::Value = serde_json::from_slice(&output).expect("es JSON válido");
        assert_eq!(parsed["problems"], 2);
        assert_eq!(parsed["more"], 0);
        let file = &parsed["files"][0];
        assert_eq!(file["path"], "src/a.go");
        assert_eq!(parsed["files"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            file["items"][0]["rule"], "suppression-added",
            "las trampas van primero"
        );
        assert_eq!(file["items"][1]["max"], 4);
        assert_eq!(
            parsed["fix"]["parameter-count"],
            "agrupá los parámetros que viajan juntos en un objeto"
        );
        assert!(
            file["items"][1].get("patch").is_none(),
            "sin arreglo mecánico no hay parche"
        );
    }

    #[test]
    fn hands_agents_the_patch_of_a_mechanical_fix() {
        let mut diagnostics = sample();
        diagnostics[0].rule = RuleId::UnusedImport;
        diagnostics[0].fix = Some(Fix {
            edits: Vec::new(),
            hunk: UNUSED_IMPORT_HUNK.to_owned(),
            safe: true,
            isolation: None,
        });
        let parsed: serde_json::Value =
            serde_json::from_str(&rendered_of(&diagnostics, Format::AgentJson)).expect("es JSON válido");
        assert_eq!(parsed["files"][0]["items"][0]["patch"], UNUSED_IMPORT_HUNK);
        assert_eq!(parsed["fix"]["unused-import"], "borrá el import");
        let rows = rendered_of(&diagnostics, Format::Agent);
        assert!(rows.contains("patch[1]:\n"));
        assert!(rows.contains("    @@ -3 +2,0 @@\n    -import java.util.List;\n"));
    }

    fn rendered_of(diagnostics: &[Diagnostic], format: Format) -> String {
        let mut output = Vec::new();
        render(diagnostics, format, Lang::Spanish, &mut output).expect("se escribe en memoria");
        String::from_utf8_lossy(&output).into_owned()
    }

    fn rendered(format: Format, lang: Lang) -> String {
        let mut output = Vec::new();
        render(&sample(), format, lang, &mut output).expect("se escribe en memoria");
        String::from_utf8_lossy(&output).into_owned()
    }

    #[test]
    fn english_changes_only_the_text_meant_for_people() {
        assert_eq!(
            rendered(Format::Text, Lang::English),
            "src/a.go:3:1: parameter-count The function `f` takes 5 parameters (maximum 4)\n"
        );
        let json: serde_json::Value =
            serde_json::from_str(&rendered(Format::Json, Lang::English)).expect("es JSON válido");
        assert_eq!(
            json[0]["message"],
            "The function `f` takes 5 parameters (maximum 4)"
        );
        assert_eq!(json[0]["fingerprint"], "parameter-count:src/a.go:f#1");
        let sarif: serde_json::Value =
            serde_json::from_str(&rendered(Format::Sarif, Lang::English)).expect("es JSON válido");
        assert_eq!(
            sarif["runs"][0]["tool"]["driver"]["rules"][4]["shortDescription"]["text"],
            "The function takes too many parameters"
        );
        let english: serde_json::Value =
            serde_json::from_str(&rendered(Format::AgentJson, Lang::English)).expect("es JSON válido");
        let spanish: serde_json::Value =
            serde_json::from_str(&rendered(Format::AgentJson, Lang::Spanish)).expect("es JSON válido");
        assert_eq!(
            english["fix"]["parameter-count"],
            "group the parameters that travel together into an object"
        );
        assert_eq!(english["files"], spanish["files"]);
        assert_eq!(english["explain"], spanish["explain"]);
        assert!(rendered(Format::Agent, Lang::English).contains("parameter-count: group the parameters"));
    }
}
