use std::path::{Path, PathBuf};

use clap::Parser;

use super::{EXIT_CLEAN, EXIT_FAILURE, EXIT_FINDINGS, Terminal, run};
use crate::cli::{Cli, Command, try_parse};
use crate::i18n::{Lang, LangPreference};
use crate::test_support::TempTree;

const LONG_SIGNATURE: &str = "package sample\n\nfunc Many(a, b, c, d, e int) {}\n";

struct Outcome {
    status: u8,
    report: String,
    log: String,
}

fn execute(arguments: &[&str], directory: &Path) -> Outcome {
    execute_with_input(arguments, directory, "")
}

fn execute_with_input(arguments: &[&str], directory: &Path, input: &str) -> Outcome {
    let cli = try_parse(Lang::Spanish, arguments).expect("los argumentos son válidos");
    let (mut report, mut log) = (Vec::new(), Vec::new());
    let mut input = input.as_bytes();
    let preference = LangPreference {
        flag: cli.lang,
        locale: Lang::Spanish,
    };
    let status = run(
        &cli.command,
        preference,
        directory,
        &mut Terminal {
            input: &mut input,
            report: &mut report,
            log: &mut log,
        },
    );
    Outcome {
        status,
        report: String::from_utf8_lossy(&report).into_owned(),
        log: String::from_utf8_lossy(&log).into_owned(),
    }
}

fn path_of(tree: &TempTree) -> &str {
    tree.path().to_str().expect("la ruta temporal es UTF-8")
}

#[test]
fn parses_check_with_defaults_and_flags() {
    let cli = Cli::try_parse_from(["kerbo", "check"]).expect("es válido");
    let Command::Check(arguments) = cli.command else {
        panic!("se esperaba check");
    };
    assert_eq!(arguments.scan.paths, [PathBuf::from(".")]);
    assert!(arguments.diff.is_none() && !arguments.no_baseline && arguments.scan.config.is_none());
    let tuned = Cli::try_parse_from([
        "kerbo",
        "check",
        "--max",
        "parameter-count=0",
        "--diff",
        "main",
        "--no-baseline",
    ])
    .expect("es válido");
    let Command::Check(tuned) = tuned.command else {
        panic!("se esperaba check");
    };
    assert_eq!(
        (
            tuned.scan.overrides.len(),
            tuned.diff.as_deref(),
            tuned.no_baseline
        ),
        (1, Some("main"), true)
    );
    assert!(Cli::try_parse_from(["kerbo", "check", "--max", "nope=1"]).is_err());
    assert!(matches!(
        Cli::try_parse_from(["kerbo", "baseline"]).map(|parsed| parsed.command),
        Ok(Command::Baseline(_))
    ));
}

#[test]
fn reports_findings_and_summary() {
    let tree = TempTree::new("app-findings");
    tree.write("many.go", LONG_SIGNATURE);
    let outcome = execute(&["kerbo", "check", path_of(&tree)], tree.path());
    assert_eq!(outcome.status, EXIT_FINDINGS);
    assert!(outcome.report.contains("parameter-count"));
    assert!(outcome.log.contains("1 archivos, 1 problemas"));
    let sarif = execute(
        &["kerbo", "check", "--format", "sarif", path_of(&tree)],
        tree.path(),
    );
    assert!(sarif.report.contains("\"version\":\"2.1.0\""));
}

#[test]
fn baseline_hides_known_problems_until_they_change() {
    let tree = TempTree::new("app-baseline");
    tree.write("many.go", LONG_SIGNATURE);
    let saved = execute(&["kerbo", "baseline", path_of(&tree)], tree.path());
    assert_eq!(saved.status, EXIT_CLEAN);
    assert!(saved.log.contains("1 problemas guardados"));
    let after = execute(&["kerbo", "check", path_of(&tree)], tree.path());
    assert_eq!(after.status, EXIT_CLEAN);
    assert!(after.log.contains("0 problemas (y 1 en la línea de base)"));
    let everything = execute(&["kerbo", "check", "--no-baseline", path_of(&tree)], tree.path());
    assert_eq!(everything.status, EXIT_FINDINGS);
    tree.write(
        "many.go",
        "package sample\n\nfunc Many(a, b, c, d, e, f int) {}\n",
    );
    assert_eq!(
        execute(&["kerbo", "check", path_of(&tree)], tree.path()).status,
        EXIT_FINDINGS,
        "empeoró"
    );
}

#[test]
fn counts_configuration_deviations_and_fails_on_bad_setup() {
    let tree = TempTree::new("app-config");
    tree.write("many.go", LONG_SIGNATURE);
    tree.write(
        "kerbo.toml",
        "[[limits]]\nrule = \"parameter-count\"\nmax = 5\nreason = \"acordado\"\n",
    );
    let tuned = execute(&["kerbo", "check", path_of(&tree)], tree.path());
    assert_eq!(tuned.status, EXIT_CLEAN);
    assert!(tuned.log.contains("1 desvíos del preset"));
    tree.write("kerbo.toml", "preset = \"nope\"\n");
    let broken = execute(&["kerbo", "check", path_of(&tree)], tree.path());
    assert_eq!(broken.status, EXIT_FAILURE);
    assert!(broken.log.contains("kerbo.toml"));
    assert_eq!(
        execute(&["kerbo", "baseline", path_of(&tree)], tree.path()).status,
        EXIT_FAILURE
    );
}

#[test]
fn fails_on_missing_paths_and_outside_git_with_diff() {
    let tree = TempTree::new("app-failures");
    let missing = tree.path().join("missing");
    let missing = missing.to_str().expect("UTF-8");
    let outcome = execute(&["kerbo", "check", missing], tree.path());
    assert_eq!(outcome.status, EXIT_FAILURE);
    assert_eq!(
        execute(&["kerbo", "baseline", missing], tree.path()).status,
        EXIT_FAILURE
    );
    let no_git = execute(&["kerbo", "check", "--diff", "main", path_of(&tree)], tree.path());
    assert_eq!(no_git.status, EXIT_FAILURE);
    assert!(no_git.log.contains("git"));
}

#[test]
fn explains_a_rule() {
    let tree = TempTree::new("app-explain");
    let outcome = execute(&["kerbo", "explain", "cognitive-complexity"], tree.path());
    assert_eq!(outcome.status, EXIT_CLEAN);
    assert!(outcome.report.contains("Por qué:") && outcome.report.contains("retornos tempranos"));
    assert!(Cli::try_parse_from(["kerbo", "explain", "nope"]).is_err());
}

#[test]
fn hook_gives_feedback_only_on_problems() {
    let tree = TempTree::new("app-hook");
    tree.write("many.go", LONG_SIGNATURE);
    tree.write("few.go", "package sample\n\nfunc Few(a int) {}\n");
    tree.write("notes.md", "# nada\n");
    let event = |file: &str| {
        format!(
            "{{\"cwd\": \"{}\", \"tool_input\": {{\"file_path\": \"{file}\"}}}}",
            path_of(&tree)
        )
    };
    let flagged = execute_with_input(&["kerbo", "hook"], tree.path(), &event("many.go"));
    assert_eq!(flagged.status, 2, "Claude Code le muestra el stderr al agente");
    assert!(flagged.log.contains("parameter-count") && flagged.log.contains("kerbo explain"));
    assert_eq!(
        execute_with_input(&["kerbo", "hook"], tree.path(), &event("few.go")).status,
        EXIT_CLEAN
    );
    assert_eq!(
        execute_with_input(&["kerbo", "hook"], tree.path(), &event("notes.md")).status,
        EXIT_CLEAN
    );
    assert_eq!(
        execute_with_input(&["kerbo", "hook"], tree.path(), "{}").status,
        EXIT_CLEAN
    );
    let broken = execute_with_input(&["kerbo", "hook"], tree.path(), "no es json");
    assert_eq!(
        broken.status, EXIT_FINDINGS,
        "un error del hook no bloquea al agente"
    );
}

#[test]
fn mcp_server_lists_and_runs_its_tools() {
    let tree = TempTree::new("app-mcp");
    tree.write("many.go", LONG_SIGNATURE);
    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"kerbo_check","arguments":{}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"kerbo_explain","arguments":{"rule":"parameter-count"}}}"#,
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"kerbo_explain","arguments":{"rule":"nope"}}}"#,
        r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"kerbo_check","arguments":{"diff":"HEAD"}}}"#,
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"nope"}}"#,
        r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"kerbo_check","arguments":{"extra":1}}}"#,
        r#"{"jsonrpc":"2.0","id":9,"method":"ping"}"#,
        r#"{"jsonrpc":"2.0","id":10,"method":"resources/list"}"#,
        "no es json",
        "",
    ];
    let outcome = execute_with_input(&["kerbo", "mcp"], tree.path(), &requests.join("\n"));
    assert_eq!(outcome.status, EXIT_CLEAN);
    let responses: Vec<serde_json::Value> = outcome
        .report
        .lines()
        .map(|line| serde_json::from_str(line).expect("cada respuesta es JSON"))
        .collect();
    assert_eq!(responses.len(), 11, "la notificación no lleva respuesta");
    let by_id = |id: u64| {
        responses
            .iter()
            .find(|response| response["id"] == id)
            .expect("hay respuesta")
    };
    assert_eq!(by_id(1)["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(by_id(2)["result"]["tools"].as_array().map(Vec::len), Some(2));
    let check_text = by_id(3)["result"]["content"][0]["text"]
        .as_str()
        .expect("texto")
        .to_owned();
    assert!(check_text.contains("parameter-count") && check_text.starts_with("problems: 1\n"));
    assert!(
        by_id(4)["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("Por qué"))
    );
    assert_eq!(by_id(5)["result"]["isError"], true);
    assert_eq!(
        by_id(6)["result"]["isError"],
        true,
        "sin git, el diff falla como herramienta"
    );
    assert_eq!(by_id(7)["error"]["code"], -32_602);
    assert_eq!(by_id(8)["error"]["code"], -32_602);
    assert_eq!(by_id(9)["result"], serde_json::json!({}));
    assert_eq!(by_id(10)["error"]["code"], -32_601);
    assert!(
        responses
            .iter()
            .any(|response| response["error"]["code"] == -32_700)
    );
}

#[test]
fn speaks_english_when_asked_by_flag_or_configuration() {
    let tree = TempTree::new("app-english");
    tree.write("many.go", LONG_SIGNATURE);
    let flagged = execute(&["kerbo", "--lang", "en", "check", path_of(&tree)], tree.path());
    assert!(
        flagged
            .report
            .contains("The function `Many` takes 5 parameters (maximum 4)")
    );
    assert!(flagged.log.contains("1 file, 1 problem, "));
    tree.write("kerbo.toml", "lang = \"en\"\n");
    let configured = execute(&["kerbo", "check", path_of(&tree)], tree.path());
    assert!(configured.log.contains("1 file, 1 problem, "));
    let overridden = execute(&["kerbo", "check", "--lang", "es", path_of(&tree)], tree.path());
    assert!(overridden.log.contains("1 archivos, 1 problemas"));
    let explained = execute(&["kerbo", "explain", "cognitive-complexity"], tree.path());
    assert!(explained.report.contains("Why:") && explained.report.contains("early returns"));
    tree.write(
        "kerbo.toml",
        "[[limits]]\nrule = \"parameter-count\"\nreason = \" \"\n",
    );
    let broken = execute(&["kerbo", "--lang", "en", "check", path_of(&tree)], tree.path());
    assert!(broken.log.contains("every adjustment needs a `reason`"));
}

#[test]
fn the_hook_and_the_mcp_server_follow_the_language() {
    let tree = TempTree::new("app-english-agents");
    tree.write("many.go", LONG_SIGNATURE);
    let event = format!(
        "{{\"cwd\": \"{}\", \"tool_input\": {{\"file_path\": \"many.go\"}}}}",
        path_of(&tree)
    );
    let hook = execute_with_input(&["kerbo", "hook", "--lang", "en"], tree.path(), &event);
    assert!(hook.log.contains("1 problem in what you just edited:"));
    assert!(hook.log.contains("parameter-count: group the parameters"));
    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"kerbo_explain","arguments":{"rule":"magic-number"}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#,
    ];
    let mcp = execute_with_input(
        &["kerbo", "mcp", "--lang", "en"],
        tree.path(),
        &requests.join("\n"),
    );
    let responses: Vec<serde_json::Value> = mcp
        .report
        .lines()
        .map(|line| serde_json::from_str(line).expect("cada respuesta es JSON"))
        .collect();
    let instructions = responses[0]["result"]["instructions"].as_str().expect("texto");
    assert!(instructions.starts_with("Use kerbo_check"));
    let explained = responses[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("texto");
    assert!(explained.contains("Why: A bare 86400"));
    assert_eq!(responses[2]["error"]["message"], "unknown method: nope");
}
