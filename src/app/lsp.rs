use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{EXIT_CLEAN, Terminal, write_error};
use crate::baseline::{BASELINE_FILE, Baseline};
use crate::config::Config;
use crate::explain::explain;
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

pub fn serve(terminal: &mut Terminal<'_>) -> Result<u8, String> {
    let mut reader = BufReader::new(&mut *terminal.input);
    while let Some(message) = read_message(&mut reader)? {
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        if method == "exit" {
            break;
        }
        for reply in handle(method, &message) {
            write_message(terminal.report, &reply).map_err(|error| write_error(&error))?;
        }
    }
    Ok(EXIT_CLEAN)
}

fn handle(method: &str, message: &Value) -> Vec<Value> {
    let id = message.get("id").cloned();
    let text_at = |pointer: &str| message.pointer(pointer).and_then(Value::as_str);
    let uri = text_at("/params/textDocument/uri").unwrap_or_default();
    match method {
        "initialize" => respond(id, &initialize_result()),
        "shutdown" => respond(id, &Value::Null),
        "textDocument/didOpen" => vec![publish(uri, text_at("/params/textDocument/text"))],
        "textDocument/didChange" => {
            let text = message
                .pointer("/params/contentChanges")
                .and_then(Value::as_array)
                .and_then(|changes| changes.last())
                .and_then(|change| change.get("text"))
                .and_then(Value::as_str);
            vec![publish(uri, text)]
        }
        "textDocument/didSave" => vec![publish(uri, text_at("/params/text"))],
        "textDocument/didClose" => vec![notification(uri, &[])],
        _ => id.map_or_else(Vec::new, |id| {
            let error =
                json!({ "code": METHOD_NOT_FOUND, "message": format!("método desconocido: {method}") });
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
        },
        "serverInfo": { "name": "kerbo", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn publish(uri: &str, text: Option<&str>) -> Value {
    let Some(path) = path_from_uri(uri) else {
        return notification(uri, &[]);
    };
    let text = text.map_or_else(|| fs::read_to_string(&path).unwrap_or_default(), str::to_owned);
    let lines: Vec<&str> = text.lines().collect();
    let diagnostics: Vec<Value> = diagnose(&path, &text)
        .iter()
        .map(|diagnostic| to_lsp(diagnostic, &lines))
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

fn to_lsp(diagnostic: &Diagnostic, lines: &[&str]) -> Value {
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
        "message": format!("{} — {}", diagnostic.message, explain(diagnostic.rule).fix),
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

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut length: Option<usize> = None;
    loop {
        let mut header = String::new();
        let read = reader
            .read_line(&mut header)
            .map_err(|error| format!("no se pudo leer del editor: {error}"))?;
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
    let length = length.ok_or("falta el encabezado Content-Length")?;
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(|error| format!("mensaje incompleto: {error}"))?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| format!("el mensaje no es JSON válido: {error}"))
}

fn write_message(writer: &mut dyn Write, message: &Value) -> std::io::Result<()> {
    let body = message.to_string();
    write!(writer, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    writer.flush()
}

#[cfg(test)]
mod tests;
