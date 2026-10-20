mod actions;

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{EXIT_CLEAN, Terminal, write_error};
use crate::baseline::{BASELINE_FILE, Baseline};
use crate::config::Config;
use crate::i18n::{Lang, Problem};
use crate::rules::Diagnostic;
use crate::scan::check_text;

const CONTENT_LENGTH: &str = "content-length:";
const FILE_SCHEME: &str = "file://";
const FULL_SYNC: u8 = 1;
const WARNING: u8 = 2;
const METHOD_NOT_FOUND: i64 = -32_601;
const HEX_RADIX: u32 = 16;
const ESCAPE: u8 = b'%';
const ESCAPED_WIDTH: usize = 3;

type Documents = HashMap<String, String>;

pub fn serve(lang: Lang, terminal: &mut Terminal<'_>) -> Result<u8, Problem> {
    let mut reader = BufReader::new(&mut *terminal.input);
    let mut documents = Documents::new();
    while let Some(message) = read_message(&mut reader)? {
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        if method == "exit" {
            break;
        }
        for reply in handle(method, &message, lang, &mut documents) {
            write_message(terminal.report, &reply).map_err(|error| write_error(&error))?;
        }
    }
    Ok(EXIT_CLEAN)
}

fn handle(method: &str, message: &Value, lang: Lang, documents: &mut Documents) -> Vec<Value> {
    let id = message.get("id").cloned();
    let text_at = |pointer: &str| message.pointer(pointer).and_then(Value::as_str);
    let uri = text_at("/params/textDocument/uri").unwrap_or_default();
    match method {
        "initialize" => respond(id, &initialize_result()),
        "shutdown" => respond(id, &Value::Null),
        "textDocument/didOpen" => {
            let text = document(documents, uri, text_at("/params/textDocument/text"));
            vec![publish(uri, &text, lang)]
        }
        "textDocument/didChange" => {
            let changed = message
                .pointer("/params/contentChanges")
                .and_then(Value::as_array)
                .and_then(|changes| changes.last())
                .and_then(|change| change.get("text"))
                .and_then(Value::as_str);
            let text = document(documents, uri, changed);
            vec![publish(uri, &text, lang)]
        }
        "textDocument/didSave" => {
            let text = document(documents, uri, text_at("/params/text"));
            vec![publish(uri, &text, lang)]
        }
        "textDocument/didClose" => {
            documents.remove(uri);
            vec![notification(uri, &[])]
        }
        "textDocument/codeAction" => {
            let text = document(documents, uri, None);
            let range = message.pointer("/params/range");
            respond(id, &actions::code_actions(uri, &text, range, lang))
        }
        _ => id.map_or_else(Vec::new, |id| {
            let text = lang.problem(&Problem::UnknownMethod(method.to_owned()));
            let error = json!({ "code": METHOD_NOT_FOUND, "message": text });
            vec![json!({ "jsonrpc": "2.0", "id": id, "error": error })]
        }),
    }
}

fn respond(id: Option<Value>, result: &Value) -> Vec<Value> {
    id.map_or_else(Vec::new, |id| {
        vec![json!({ "jsonrpc": "2.0", "id": id, "result": result })]
    })
}

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "positionEncoding": "utf-16",
            "textDocumentSync": { "openClose": true, "change": FULL_SYNC, "save": { "includeText": true } },
            "codeActionProvider": { "codeActionKinds": [actions::QUICK_FIX] },
        },
        "serverInfo": { "name": "kerbo", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn document(documents: &mut Documents, uri: &str, text: Option<&str>) -> String {
    if let Some(text) = text {
        documents.insert(uri.to_owned(), text.to_owned());
        return text.to_owned();
    }
    documents.get(uri).cloned().unwrap_or_else(|| {
        path_from_uri(uri)
            .and_then(|path| fs::read_to_string(path).ok())
            .unwrap_or_default()
    })
}

fn publish(uri: &str, text: &str, lang: Lang) -> Value {
    let Some(path) = path_from_uri(uri) else {
        return notification(uri, &[]);
    };
    let lines: Vec<&str> = text.lines().collect();
    let diagnostics: Vec<Value> = diagnose(&path, text)
        .iter()
        .map(|diagnostic| to_lsp(diagnostic, &lines, lang))
        .collect();
    notification(uri, &diagnostics)
}

fn diagnose(path: &Path, text: &str) -> Vec<Diagnostic> {
    let start = path.parent().unwrap_or(path);
    let Ok(config) = Config::load(None, start) else {
        return Vec::new();
    };
    let diagnostics = check_text(path, text.as_bytes(), &config).unwrap_or_default();
    match Baseline::load(&config.root().join(BASELINE_FILE)) {
        Ok(Some(baseline)) => baseline.filter(diagnostics).diagnostics,
        Ok(None) | Err(_) => diagnostics,
    }
}

fn notification(uri: &str, diagnostics: &[Value]) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": diagnostics },
    })
}

fn to_lsp(diagnostic: &Diagnostic, lines: &[&str], lang: Lang) -> Value {
    let line = diagnostic.line.saturating_sub(1);
    let text = lines.get(line).copied().unwrap_or_default();
    let start = utf16_column(text, diagnostic.column.saturating_sub(1));
    json!({
        "range": {
            "start": { "line": line, "character": start },
            "end": { "line": line, "character": text.encode_utf16().count().max(start) },
        },
        "severity": WARNING,
        "source": "kerbo",
        "code": diagnostic.rule.as_str(),
        "message": format!(
            "{} — {}",
            lang.message(&diagnostic.message),
            lang.explain(diagnostic.rule).fix
        ),
    })
}

fn utf16_column(line: &str, byte_column: usize) -> usize {
    line.char_indices()
        .take_while(|&(index, _)| index < byte_column)
        .map(|(_, character)| character.len_utf16())
        .sum()
}

fn path_from_uri(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix(FILE_SCHEME)?.as_bytes();
    let mut decoded = Vec::with_capacity(encoded.len());
    let mut index = 0;
    while let Some(&byte) = encoded.get(index) {
        let escaped = (byte == ESCAPE)
            .then(|| encoded.get(index + 1..index + ESCAPED_WIDTH))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, HEX_RADIX).ok());
        if let Some(value) = escaped {
            decoded.push(value);
            index += ESCAPED_WIDTH;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok().map(PathBuf::from)
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, Problem> {
    let mut length: Option<usize> = None;
    loop {
        let mut header = String::new();
        let read = reader
            .read_line(&mut header)
            .map_err(|error| Problem::ReadEditor(error.to_string()))?;
        if read == 0 {
            return Ok(None);
        }
        let header = header.trim();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.to_ascii_lowercase().strip_prefix(CONTENT_LENGTH) {
            length = value.trim().parse().ok();
        }
    }
    let length = length.ok_or(Problem::MissingContentLength)?;
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(|error| Problem::IncompleteMessage(error.to_string()))?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| Problem::InvalidMessage(error.to_string()))
}

fn write_message(writer: &mut dyn Write, message: &Value) -> std::io::Result<()> {
    let body = message.to_string();
    write!(writer, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    writer.flush()
}

#[cfg(test)]
mod tests;
