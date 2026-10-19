use std::path::PathBuf;

use serde_json::{Value, json};

use super::{path_from_uri, utf16_column};
use crate::app::{EXIT_CLEAN, Terminal, run};
use crate::cli::Command;
use crate::test_support::TempTree;

fn frame(message: &Value) -> String {
    let body = message.to_string();
    format!("Content-Length: {}\r\n\r\n{body}", body.len())
}

fn replies(output: &str) -> Vec<Value> {
    output
        .split("Content-Length: ")
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.split_once("\r\n\r\n"))
        .map(|(_, body)| serde_json::from_str(body).expect("cada respuesta es JSON"))
        .collect()
}

fn serve(input: &str) -> (u8, Vec<Value>) {
    let (mut report, mut log) = (Vec::new(), Vec::new());
    let mut bytes = input.as_bytes();
    let status = run(
        &Command::Lsp,
        &PathBuf::from("."),
        &mut Terminal {
            input: &mut bytes,
            report: &mut report,
            log: &mut log,
        },
    );
    (status, replies(&String::from_utf8_lossy(&report)))
}

#[test]
fn publishes_problems_while_typing_and_clears_them_on_close() {
    let tree = TempTree::new("lsp");
    let path = tree.path().join("cart.ts");
    tree.write("cart.ts", "");
    let uri = format!("file://{}", path.to_str().expect("UTF-8").replace(' ', "%20"));
    let wide = "export function ñandú(ab, bc, cd, de, ef) {}\n";
    let narrow = "export function ñandú(ab) {}\n";
    let messages = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }),
        json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": { "uri": uri, "text": wide } } }),
        json!({ "jsonrpc": "2.0", "method": "textDocument/didChange", "params": { "textDocument": { "uri": uri }, "contentChanges": [{ "text": narrow }] } }),
        json!({ "jsonrpc": "2.0", "method": "textDocument/didClose", "params": { "textDocument": { "uri": uri } } }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover", "params": {} }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "shutdown" }),
        json!({ "jsonrpc": "2.0", "method": "exit" }),
    ];
    let input: String = messages.iter().map(frame).collect();
    let (status, replies) = serve(&input);
    assert_eq!(status, EXIT_CLEAN);
    assert_eq!(replies.len(), 6);
    let capabilities = replies
        .first()
        .map(|reply| reply["result"]["capabilities"]["textDocumentSync"]["change"].clone());
    assert_eq!(capabilities, Some(json!(1)));
    let opened = &replies[1]["params"]["diagnostics"];
    assert_eq!(opened.as_array().map(Vec::len), Some(1));
    assert_eq!(opened[0]["code"], "parameter-count");
    assert_eq!(
        opened[0]["range"]["end"]["character"], 44,
        "el fin de línea se cuenta en UTF-16"
    );
    assert_eq!(
        replies[2]["params"]["diagnostics"],
        json!([]),
        "al corregir, desaparece"
    );
    assert_eq!(replies[3]["params"]["diagnostics"], json!([]));
    assert_eq!(replies[4]["error"]["code"], -32_601);
    assert_eq!(replies[5]["result"], Value::Null);
}

#[test]
fn reads_uris_columns_and_bad_input() {
    assert_eq!(
        path_from_uri("file:///home/a%20b/c%C3%B1.ts"),
        Some(PathBuf::from("/home/a b/cñ.ts"))
    );
    assert_eq!(path_from_uri("file:///x%2"), Some(PathBuf::from("/x%2")));
    assert_eq!(path_from_uri("untitled:Untitled-1"), None);
    assert_eq!(utf16_column("ñandú(a)", 7), 5);
    let saved = json!({ "jsonrpc": "2.0", "method": "textDocument/didSave", "params": { "textDocument": { "uri": "file:///no/existe.ts" } } });
    let (status, replies) = serve(&frame(&saved));
    assert_eq!(status, EXIT_CLEAN);
    assert_eq!(replies[0]["params"]["diagnostics"], json!([]));
    let (_, untitled) = serve(&frame(
        &json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": { "uri": "untitled:1", "text": "x" } } }),
    ));
    assert_eq!(untitled[0]["params"]["diagnostics"], json!([]));
    assert_ne!(serve("Content-Length: 5\r\n\r\n{bad}").0, EXIT_CLEAN);
    assert_ne!(serve("Sin-Largo: 1\r\n\r\n{}").0, EXIT_CLEAN);
}
