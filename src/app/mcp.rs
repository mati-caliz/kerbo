use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use super::fixing::{FixRequest, fix_files};
use super::{EXIT_CLEAN, FindingsRequest, Terminal, findings, write_error};
use crate::cli::ScanArgs;
use crate::i18n::{Lang, Problem, Text};
use crate::language::Stack;
use crate::report::{Format, render};
use crate::rules::RuleId;

const DEFAULT_PROTOCOL_VERSION: &str = "2025-06-18";
const METHOD_NOT_FOUND: i64 = -32_601;
const PARSE_ERROR: i64 = -32_700;
const INVALID_PARAMS: i64 = -32_602;
const CHECK_TOOL: &str = "kerbo_check";
const EXPLAIN_TOOL: &str = "kerbo_explain";
const FIX_TOOL: &str = "kerbo_fix";
const NEWLINE: &str = "\n";
const CURRENT_DIRECTORY: &str = ".";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckArguments {
    #[serde(default)]
    paths: Vec<String>,
    diff: Option<String>,
    #[serde(default)]
    languages: Vec<Stack>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixArguments {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    rules: Vec<String>,
    #[serde(default)]
    languages: Vec<Stack>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplainArguments {
    rule: String,
}

pub fn serve(directory: &Path, lang: Lang, terminal: &mut Terminal<'_>) -> Result<u8, Problem> {
    let reader = BufReader::new(&mut *terminal.input);
    for line in reader.lines() {
        let line = line.map_err(|error| Problem::ReadRequest(error.to_string()))?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = respond(&line, directory, lang) {
            writeln!(terminal.report, "{response}")
                .and_then(|()| terminal.report.flush())
                .map_err(|error| write_error(&error))?;
        }
    }
    Ok(EXIT_CLEAN)
}

fn respond(line: &str, directory: &Path, lang: Lang) -> Option<Value> {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return Some(error_response(
            &Value::Null,
            PARSE_ERROR,
            &lang.problem(&Problem::InvalidRequest),
        ));
    };
    let id = message.get("id")?.clone();
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(initialize(&params, lang)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools(lang) })),
        "tools/call" => call_tool(&params, directory, lang),
        _ => Err((METHOD_NOT_FOUND, Problem::UnknownMethod(method.to_owned()))),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, problem)) => error_response(&id, code, &lang.problem(&problem)),
    })
}

fn error_response(id: &Value, code: i64, text: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": text } })
}

fn initialize(params: &Value, lang: Lang) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "kerbo", "version": env!("CARGO_PKG_VERSION") },
        "instructions": lang.text(Text::McpInstructions),
    })
}

fn tools(lang: Lang) -> Value {
    let rules: Vec<&str> = RuleId::ALL.iter().map(|rule| rule.as_str()).collect();
    json!([
        {
            "name": CHECK_TOOL,
            "description": lang.text(Text::McpCheckDescription),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "paths": { "type": "array", "items": { "type": "string" }, "description": lang.text(Text::McpPathsDescription) },
                    "diff": { "type": "string", "description": lang.text(Text::McpDiffDescription) },
                    "languages": { "type": "array", "items": { "type": "string", "enum": ["typescript", "python", "java", "go"] } },
                },
            },
        },
        {
            "name": FIX_TOOL,
            "description": lang.text(Text::McpFixDescription),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "paths": { "type": "array", "items": { "type": "string" }, "description": lang.text(Text::McpPathsDescription) },
                    "rules": { "type": "array", "items": { "type": "string", "enum": rules }, "description": lang.text(Text::McpRulesDescription) },
                    "languages": { "type": "array", "items": { "type": "string", "enum": ["typescript", "python", "java", "go"] } },
                    "dry_run": { "type": "boolean", "description": lang.text(Text::McpDryRunDescription) },
                },
            },
        },
        {
            "name": EXPLAIN_TOOL,
            "description": lang.text(Text::McpExplainDescription),
            "inputSchema": {
                "type": "object",
                "properties": { "rule": { "type": "string", "enum": rules } },
                "required": ["rule"],
            },
        },
    ])
}

fn call_tool(params: &Value, directory: &Path, lang: Lang) -> Result<Value, (i64, Problem)> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let invalid = |error: serde_json::Error| (INVALID_PARAMS, Problem::InvalidArguments(error.to_string()));
    let outcome = match name {
        CHECK_TOOL => run_check(
            serde_json::from_value(arguments).map_err(invalid)?,
            directory,
            lang,
        ),
        FIX_TOOL => run_fix(
            serde_json::from_value(arguments).map_err(invalid)?,
            directory,
            lang,
        ),
        EXPLAIN_TOOL => run_explain(
            &serde_json::from_value::<ExplainArguments>(arguments).map_err(invalid)?,
            lang,
        ),
        _ => return Err((INVALID_PARAMS, Problem::UnknownTool(name.to_owned()))),
    };
    Ok(match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(problem) => {
            json!({ "content": [{ "type": "text", "text": lang.problem(&problem) }], "isError": true })
        }
    })
}

fn project_paths(paths: Vec<String>, directory: &Path) -> Vec<PathBuf> {
    let paths = if paths.is_empty() {
        vec![CURRENT_DIRECTORY.to_owned()]
    } else {
        paths
    };
    paths
        .into_iter()
        .map(|path| directory.join(PathBuf::from(path)))
        .collect()
}

fn run_fix(arguments: FixArguments, directory: &Path, lang: Lang) -> Result<String, Problem> {
    let rules = arguments
        .rules
        .iter()
        .map(|rule| rule.parse())
        .collect::<Result<Vec<RuleId>, Problem>>()?;
    let request = FixRequest {
        paths: project_paths(arguments.paths, directory),
        languages: &arguments.languages,
        rules: &rules,
        config: None,
        write: !arguments.dry_run,
        since: None,
    };
    let outcome = fix_files(&request, directory)?;
    let mut text = outcome.patch();
    text.push_str(&outcome.notices(lang).join(NEWLINE));
    Ok(text)
}

fn run_check(arguments: CheckArguments, directory: &Path, lang: Lang) -> Result<String, Problem> {
    let scan = ScanArgs {
        paths: project_paths(arguments.paths, directory),
        languages: arguments.languages,
        overrides: Vec::new(),
        config: None,
        no_cache: false,
    };
    let request = FindingsRequest {
        scan: &scan,
        diff: arguments.diff.as_deref(),
        use_baseline: true,
        osv: None,
    };
    let found = findings(&request, directory)?;
    let mut output = Vec::new();
    render(&found.diagnostics, Format::Agent, lang, &mut output).map_err(|error| write_error(&error))?;
    Ok(String::from_utf8_lossy(&output).trim_end().to_owned())
}

fn run_explain(arguments: &ExplainArguments, lang: Lang) -> Result<String, Problem> {
    let rule: RuleId = arguments.rule.parse()?;
    Ok(lang.explanation_text(rule))
}
