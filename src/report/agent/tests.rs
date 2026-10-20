use std::path::PathBuf;

use regex::Regex;

use super::short_id::shortest_unique;
use super::{SHOWN_LIMIT, build, write};
use crate::i18n::{Lang, Message};
use crate::rules::{Diagnostic, RuleId};

const LARGE_SAMPLE_FILES: usize = 8;
const LARGE_SAMPLE_PER_FILE: usize = 6;
const LARGE_SAMPLE_TOKEN_CEILING: usize = 650;
const TOKEN_PIECES: &str = r"[\p{L}]+|[0-9]{1,3}|[^\s\p{L}0-9]";

#[derive(Clone, Copy)]
struct Measure {
    value: usize,
    limit: usize,
}

const fn measure(value: usize, limit: usize) -> Measure {
    Measure { value, limit }
}

fn diagnostic(path: &str, rule: RuleId, line: usize, measure: Measure) -> Diagnostic {
    let Measure { value, limit } = measure;
    Diagnostic {
        path: path.to_owned(),
        line,
        column: 1,
        end_line: line + value,
        rule,
        message: Message::TooManyParameters {
            function: "f".to_owned(),
            count: value,
            limit,
        },
        value,
        limit,
        fingerprint: format!("{}:{path}:symbol{line}#1", rule.as_str()),
        relative_path: path.to_owned(),
        absolute_path: PathBuf::from("/repo").join(path),
        fix: None,
    }
}

fn small_sample() -> Vec<Diagnostic> {
    vec![
        diagnostic("src/a.go", RuleId::ParameterCount, 3, measure(5, 4)),
        diagnostic("src/b.ts", RuleId::FunctionLength, 10, measure(80, 50)),
        diagnostic("src/a.go", RuleId::ParameterCount, 40, measure(7, 4)),
        diagnostic("src/b.ts", RuleId::SuppressionAdded, 12, measure(1, 0)),
    ]
}

fn large_sample() -> Vec<Diagnostic> {
    let rules = [
        RuleId::ParameterCount,
        RuleId::FunctionLength,
        RuleId::MagicNumber,
    ];
    (0..LARGE_SAMPLE_FILES)
        .flat_map(|file| {
            (0..LARGE_SAMPLE_PER_FILE).map(move |index| {
                let rule = rules
                    .get(index % rules.len())
                    .copied()
                    .unwrap_or(RuleId::MagicNumber);
                diagnostic(
                    &format!("src/module{file}/service.ts"),
                    rule,
                    index * 20 + 1,
                    measure(5 + index, 4),
                )
            })
        })
        .collect()
}

fn rendered(diagnostics: &[Diagnostic], lang: Lang) -> String {
    let mut output = Vec::new();
    write(&build(diagnostics, lang), &mut output).expect("se escribe en memoria");
    String::from_utf8_lossy(&output).into_owned()
}

fn estimated_tokens(text: &str) -> usize {
    Regex::new(TOKEN_PIECES)
        .expect("la expresión es válida")
        .find_iter(text)
        .count()
}

#[test]
fn fixes_the_agent_output() {
    let expected = "problems: 4
shown: 4
files[2]{id,line,end,rule,value,max}:
  src/b.ts[2]:
    ad7a,12,13,suppression-added,1,0
    a470,10,90,function-length,80,50
  src/a.go[2]:
    c0d6,40,47,parameter-count,7,4
    7cd7,3,8,parameter-count,5,4
fix[3]:
  suppression-added: arreglá la causa en vez de silenciar la herramienta
  function-length: extraé pasos a funciones con nombre propio
  parameter-count: agrupá los parámetros que viajan juntos en un objeto
explain: kerbo explain <rule>
";
    assert_eq!(rendered(&small_sample(), Lang::Spanish), expected);
}

#[test]
fn short_ids_are_stable_and_grow_only_to_avoid_collisions() {
    let alone = shortest_unique(&["parameter-count:src/a.go:f#1"]);
    assert_eq!(alone, ["cec3"]);
    let crowded: Vec<String> = (0..4096)
        .map(|index| format!("magic-number:src/a.go:{index}#1"))
        .collect();
    let ids = shortest_unique(&crowded.iter().map(String::as_str).collect::<Vec<_>>());
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "dentro del reporte no chocan");
    assert!(ids.iter().any(|id| id.len() > alone[0].len()));
    let again = shortest_unique(&crowded.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(ids, again, "la misma entrada da los mismos ids");
    let repeated = shortest_unique(&["x", "x"]);
    assert_eq!(repeated[0], repeated[1]);
}

#[test]
fn caps_the_list_and_says_how_many_are_left() {
    let output = rendered(&large_sample(), Lang::English);
    let problems = LARGE_SAMPLE_FILES * LARGE_SAMPLE_PER_FILE;
    assert!(output.starts_with(&format!(
        "problems: {problems}\nshown: {SHOWN_LIMIT}\nmore: {}\n",
        problems - SHOWN_LIMIT
    )));
    assert_eq!(
        output.matches("magic-number: ").count(),
        1,
        "cada arreglo va una sola vez"
    );
    assert_eq!(
        output.matches("src/module0/service.ts").count(),
        1,
        "cada ruta va una sola vez"
    );
}

#[test]
fn keeps_a_large_report_under_its_token_ceiling() {
    let tokens = estimated_tokens(&rendered(&large_sample(), Lang::Spanish));
    assert!(
        tokens <= LARGE_SAMPLE_TOKEN_CEILING,
        "el reporte de ejemplo pasó de {LARGE_SAMPLE_TOKEN_CEILING} a {tokens} tokens"
    );
}

#[test]
fn an_empty_report_is_two_lines_and_the_hint() {
    assert_eq!(
        rendered(&[], Lang::Spanish),
        "problems: 0\nshown: 0\nexplain: kerbo explain <rule>\n"
    );
}
