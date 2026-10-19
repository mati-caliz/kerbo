use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use super::{EXIT_CLEAN, FindingsRequest, Terminal, findings, write_error};
use crate::cli::ScanArgs;
use crate::explain::explain;
use crate::language::Stack;
use crate::report::{Format, render};
use crate::rules::RuleId;

const DEFAULT_PROTOCOL_VERSION: &str = "2025-06-18";
const METHOD_NOT_FOUND: i64 = -32_601;
const PARSE_ERROR: i64 = -32_700;
const INVALID_PARAMS: i64 = -32_602;
const CHECK_TOOL: &str = "kerbo_check";
const EXPLAIN_TOOL: &str = "kerbo_explain";
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplainArguments {
    rule: String,
}

pub fn serve(directory: &Path, terminal: &mut Terminal<'_>) -> Result<u8, String> {
    let reader = BufReader::new(&mut *terminal.input);
    for line in reader.lines() {
        let line = line.map_err(|error| format!("no se pudo leer el pedido: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = respond(&line, directory) {
            writeln!(terminal.report, "{response}")
                .and_then(|()| terminal.report.flush())
                .map_err(|error| write_error(&error))?;
        }
    }
    Ok(EXIT_CLEAN)
}

fn respond(line: &str, directory: &Path) -> Option<Value> {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return Some(error_response(
            &Value::Null,
            PARSE_ERROR,
            "el pedido no es JSON válido",
        ));
    };
    let id = message.get("id")?.clone();
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool(&params, directory),
        _ => Err((METHOD_NOT_FOUND, format!("método desconocido: {method}"))),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, text)) => error_response(&id, code, &text),
    })
}

fn error_response(id: &Value, code: i64, text: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": text } })
}

fn initialize(params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "kerbo", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "Usá kerbo_check después de editar código: con diff (por ejemplo HEAD) mira sólo lo cambiado y detecta trampas como supresiones nuevas o tests salteados. kerbo_explain dice cómo arreglar cada regla.",
    })
}

fn tools() -> Value {
    let rules: Vec<&str> = RuleId::ALL.iter().map(|rule| rule.as_str()).collect();
    json!([
        {
            "name": CHECK_TOOL,
            "description": "Analiza archivos o carpetas y devuelve los problemas priorizados, con id estable y pista de arreglo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "paths": { "type": "array", "items": { "type": "string" }, "description": "Archivos o carpetas; por defecto la carpeta del proyecto" },
                    "diff": { "type": "string", "description": "Referencia de git: sólo lo cambiado desde ahí, con antitrampa" },
                    "languages": { "type": "array", "items": { "type": "string", "enum": ["typescript", "python", "java", "go"] } },
                },
            },
        },
        {
            "name": EXPLAIN_TOOL,
            "description": "Explica una regla de kerbo: por qué existe, cómo se arregla y un ejemplo.",
            "inputSchema": {
                "type": "object",
                "properties": { "rule": { "type": "string", "enum": rules } },
                "required": ["rule"],
            },
        },
    ])
}

fn call_tool(params: &Value, directory: &Path) -> Result<Value, (i64, String)> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let invalid = |error: serde_json::Error| (INVALID_PARAMS, format!("argumentos inválidos: {error}"));
    let outcome = match name {
        CHECK_TOOL => run_check(serde_json::from_value(arguments).map_err(invalid)?, directory),
        EXPLAIN_TOOL => run_explain(&serde_json::from_value::<ExplainArguments>(arguments).map_err(invalid)?),
        _ => return Err((INVALID_PARAMS, format!("herramienta desconocida: {name}"))),
    };
    Ok(match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
    })
}

fn run_check(arguments: CheckArguments, directory: &Path) -> Result<String, String> {
    let paths = if arguments.paths.is_empty() {
        vec![CURRENT_DIRECTORY.to_owned()]
    } else {
        arguments.paths
    };
    let scan = ScanArgs {
        paths: paths
            .into_iter()
            .map(|path| directory.join(PathBuf::from(path)))
            .collect(),
        languages: arguments.languages,
        overrides: Vec::new(),
        config: None,
    };
    let request = FindingsRequest {
        scan: &scan,
        diff: arguments.diff.as_deref(),
        use_baseline: true,
    };
    let found = findings(&request, directory)?;
    let mut output = Vec::new();
    render(&found.diagnostics, Format::Agent, &mut output).map_err(|error| write_error(&error))?;
    Ok(String::from_utf8_lossy(&output).trim_end().to_owned())
}

fn run_explain(arguments: &ExplainArguments) -> Result<String, String> {
    let rule: RuleId = arguments.rule.parse()?;
    let explanation = explain(rule);
    Ok(format!(
        "{}: {}\nPor qué: {}\nCómo se arregla: {}\nEjemplo: {}",
        rule.as_str(),
        rule.description(),
        explanation.why,
        explanation.fix,
        explanation.example
    ))
}
