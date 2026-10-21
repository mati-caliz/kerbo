use std::path::PathBuf;

use serde_json::{Value, json};

use super::{path_from_uri, utf16_column};
use crate::app::{EXIT_CLEAN, Terminal, run};
use crate::cli::Command;
use crate::i18n::{Lang, LangPreference};
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
    serve_in(Lang::Spanish, input)
}

fn serve_in(lang: Lang, input: &str) -> (u8, Vec<Value>) {
    let (mut report, mut log) = (Vec::new(), Vec::new());
    let mut bytes = input.as_bytes();
    let preference = LangPreference {
        flag: Some(lang),
        locale: Lang::Spanish,
    };
    let status = run(
        &Command::Lsp,
        preference,
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
    let wide = "export function ñandú(ab, bc, cd, de, ef) { return [ab, bc, cd, de, ef]; }\n";
    let narrow = "export function ñandú(ab) { return ab; }\n";
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
        opened[0]["range"]["end"]["character"], 74,
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

#[test]
fn speaks_the_chosen_language_to_the_editor() {
    let tree = TempTree::new("lsp-english");
    let path = tree.path().join("cart.ts");
    tree.write("cart.ts", "");
    let uri = format!("file://{}", path.to_str().expect("UTF-8").replace(' ', "%20"));
    let opened = json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": { "uri": uri, "text": "export function wide(ab, bc, cd, de, ef) {}\n" } } });
    let hover = json!({ "jsonrpc": "2.0", "id": 1, "method": "textDocument/hover", "params": {} });
    let input = format!("{}{}", frame(&opened), frame(&hover));
    let (_, english) = serve_in(Lang::English, &input);
    assert_eq!(
        english[0]["params"]["diagnostics"][0]["message"],
        "The function `wide` takes 5 parameters (maximum 4) — group the parameters that travel together into an object"
    );
    assert_eq!(
        english[1]["error"]["message"],
        "unknown method: textDocument/hover"
    );
    let (_, spanish) = serve(&input);
    assert_eq!(
        spanish[1]["error"]["message"],
        "método desconocido: textDocument/hover"
    );
}

#[test]
fn offers_the_mechanical_fix_as_a_quick_fix() {
    let tree = TempTree::new("lsp-actions");
    let path = tree.path().join("Cart.java");
    tree.write("Cart.java", "");
    let uri = format!("file://{}", path.to_str().expect("UTF-8").replace(' ', "%20"));
    let text =
        "import java.util.List;\nfinal class Cart {\n  void añadir(final int ñandú, int count) { }\n}\n";
    let range = json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 2, "character": 0 } });
    let messages = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": { "uri": uri, "text": text } } }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "textDocument/codeAction", "params": { "textDocument": { "uri": uri }, "range": range, "context": { "diagnostics": [] } } }),
        json!({ "jsonrpc": "2.0", "method": "textDocument/didClose", "params": { "textDocument": { "uri": uri } } }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "textDocument/codeAction", "params": { "textDocument": { "uri": uri }, "range": range } }),
    ];
    let input: String = messages.iter().map(frame).collect();
    let (_, replies) = serve(&input);
    assert_eq!(
        replies[0]["result"]["capabilities"]["codeActionProvider"]["codeActionKinds"],
        json!(["quickfix", "source.fixAll"])
    );
    let actions = replies[2]["result"].as_array().expect("una lista de acciones");
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[2]["kind"], "source.fixAll.kerbo");
    let whole = &actions[2]["edit"]["changes"][uri.as_str()];
    assert_eq!(whole.as_array().map(Vec::len), Some(1));
    assert_eq!(
        whole[0]["range"],
        json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 4, "character": 0 } })
    );
    assert_eq!(
        whole[0]["newText"],
        "final class Cart {
  void añadir(final int ñandú, final int count) { }
}
",
        "todas las vueltas, en una sola edición sobre el documento entero"
    );
    assert_eq!(actions[0]["kind"], "quickfix");
    assert_eq!(actions[0]["title"], "kerbo: borrá el import");
    assert_eq!(actions[0]["diagnostics"][0]["code"], "unused-import");
    let removal = &actions[0]["edit"]["changes"][uri.as_str()][0];
    assert_eq!(removal["range"]["start"], json!({ "line": 0, "character": 0 }));
    assert_eq!(removal["range"]["end"], json!({ "line": 1, "character": 0 }));
    assert_eq!(removal["newText"], "");
    let insertion = &actions[1]["edit"]["changes"][uri.as_str()][0];
    assert_eq!(
        insertion["range"]["start"],
        json!({ "line": 2, "character": 31 }),
        "la columna del arreglo se cuenta en UTF-16"
    );
    assert_eq!(insertion["newText"], "final ");
    assert_eq!(
        replies[4]["result"],
        json!([]),
        "cerrado, se lee del disco y el archivo está vacío"
    );
}

#[test]
fn answers_only_the_diagnostics_and_kinds_the_editor_asks_for() {
    let tree = TempTree::new("lsp-context");
    let path = tree.path().join("Cart.java");
    let uri = format!("file://{}", path.to_str().expect("UTF-8").replace(' ', "%20"));
    let text = "import java.util.List;\nfinal class Cart {\n  void add(int count) { }\n}\n";
    let whole = json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 3, "character": 0 } });
    let reported = json!([
        { "range": { "start": { "line": 2, "character": 2 }, "end": { "line": 2, "character": 9 } }, "source": "kerbo", "code": "final-parameter" },
        { "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 9 } }, "source": "otro", "code": "unused-import" },
    ]);
    let action = |id: u64, context: &Value| json!({ "jsonrpc": "2.0", "id": id, "method": "textDocument/codeAction", "params": { "textDocument": { "uri": uri }, "range": whole, "context": context } });
    let messages = [
        json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": { "uri": uri, "text": text } } }),
        action(1, &json!({ "diagnostics": reported, "only": ["quickfix"] })),
        action(2, &json!({ "diagnostics": [], "only": ["source.fixAll"] })),
        action(3, &json!({ "diagnostics": [], "only": ["refactor"] })),
    ];
    let input: String = messages.iter().map(frame).collect();
    let (_, replies) = serve(&input);
    let asked = replies[1]["result"].as_array().expect("una lista de acciones");
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0]["diagnostics"][0]["code"], "final-parameter");
    let everything = replies[2]["result"].as_array().expect("una lista de acciones");
    assert_eq!(everything.len(), 1);
    assert_eq!(
        everything[0]["title"],
        "kerbo: arreglar todo lo mecánico del archivo"
    );
    assert_eq!(replies[3]["result"], json!([]));
}
