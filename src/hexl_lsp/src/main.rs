//! hexl 用の LSP サーバー (stdio)。
//!
//! hexl_frontend はコンパイラ本体(crate::compiler::*)に依存していて単体では
//! ビルドできないため、このサーバーは独自の軽量な字句解析(scan.rs)で動く。
//!
//! 機能: 診断 / 補完 / ホバー / 定義 / 参照 / ハイライト / リネーム / シグネチャヘルプ /
//!       ドキュメント・ワークスペースシンボル / 折りたたみ / セマンティックトークン /
//!       `#include` をまたいだ解決

mod features;
mod index;
mod scan;
mod util;

use index::Index;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn read_msg(stdin: &mut impl BufRead) -> Option<Value> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if stdin.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            len = v.trim().parse().ok();
        }
    }
    let mut buf = vec![0u8; len?];
    stdin.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn send(v: &Value) {
    let s = v.to_string();
    let mut out = io::stdout().lock();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{}", s.len(), s);
    let _ = out.flush();
}

fn reply(id: &Value, result: Value) {
    send(&json!({"jsonrpc":"2.0","id":id,"result":result}));
}

fn reply_err(id: &Value, code: i64, msg: &str) {
    send(&json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":msg}}));
}

fn publish(ix: &mut Index, uri: &str) {
    if let Some(doc) = ix.doc(uri) {
        let ds = features::diagnostics(ix, &doc);
        send(&json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics",
            "params":{"uri":uri,"diagnostics":ds}}));
    }
}

fn capabilities() -> Value {
    json!({
        "textDocumentSync": {"openClose": true, "change": 1},
        "completionProvider": {"triggerCharacters": ["#", ".", ":", ">", "\"", "/"]},
        "hoverProvider": true,
        "signatureHelpProvider": {"triggerCharacters": ["(", ","]},
        "definitionProvider": true,
        "referencesProvider": true,
        "documentHighlightProvider": true,
        "documentSymbolProvider": true,
        "workspaceSymbolProvider": true,
        "renameProvider": {"prepareProvider": true},
        "foldingRangeProvider": true,
        "semanticTokensProvider": {
            "legend": {"tokenTypes": features::TOKEN_TYPES, "tokenModifiers": []},
            "full": true
        }
    })
}

fn init(ix: &mut Index, p: &Value) {
    if let Some(ws) = p["workspaceFolders"].as_array() {
        for w in ws {
            if let Some(path) = w["uri"].as_str().and_then(util::uri_to_path) {
                ix.roots.push(path);
            }
        }
    }
    if ix.roots.is_empty() {
        if let Some(path) = p["rootUri"].as_str().and_then(util::uri_to_path) {
            ix.roots.push(path);
        }
    }
    if let Some(l) = p["initializationOptions"]["libraryPath"].as_str() {
        ix.library = Some(l.into());
    }
}

/// リクエストを処理して結果を返す。未対応のメソッドは `None`
fn handle(ix: &mut Index, method: &str, p: &Value) -> Option<Value> {
    Some(match method {
        "textDocument/completion" => features::completion(ix, p),
        "textDocument/hover" => features::hover(ix, p),
        "textDocument/signatureHelp" => features::signature_help(ix, p),
        "textDocument/definition" => features::definition(ix, p),
        "textDocument/references" => features::references(ix, p),
        "textDocument/documentHighlight" => features::document_highlight(ix, p),
        "textDocument/documentSymbol" => features::document_symbols(ix, p),
        "textDocument/prepareRename" => features::prepare_rename(ix, p),
        "textDocument/rename" => features::rename(ix, p),
        "textDocument/foldingRange" => features::folding_ranges(ix, p),
        "textDocument/semanticTokens/full" => features::semantic_tokens(ix, p),
        "workspace/symbol" => features::workspace_symbols(ix, p["query"].as_str().unwrap_or("")),
        _ => return None,
    })
}

fn main() {
    // パニックしても落ちないようにする(メッセージは stderr へ)
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut ix = Index::default();
    let mut shutdown = false;

    while let Some(msg) = read_msg(&mut stdin) {
        let method = msg["method"].as_str().unwrap_or("").to_string();
        let id = msg.get("id").cloned();
        let p = msg["params"].clone();
        let uri = p["textDocument"]["uri"].as_str().unwrap_or("").to_string();
        let rid = id.clone().unwrap_or(Value::Null);

        let r = catch_unwind(AssertUnwindSafe(|| match method.as_str() {
            "initialize" => {
                init(&mut ix, &p);
                reply(
                    &rid,
                    json!({"capabilities": capabilities(),
                        "serverInfo": {"name": "hexl-lsp", "version": "0.2.0"}}),
                );
            }
            "shutdown" => {
                shutdown = true;
                reply(&rid, Value::Null);
            }
            "exit" => std::process::exit(if shutdown { 0 } else { 1 }),
            "textDocument/didOpen" => {
                ix.set_open(&uri, p["textDocument"]["text"].as_str().unwrap_or(""));
                publish(&mut ix, &uri);
            }
            "textDocument/didChange" => {
                // 全文同期なので最後の変更が全文
                if let Some(text) = p["contentChanges"]
                    .as_array()
                    .and_then(|a| a.last())
                    .and_then(|c| c["text"].as_str())
                {
                    ix.set_open(&uri, text);
                    publish(&mut ix, &uri);
                }
            }
            "textDocument/didClose" => {
                ix.close(&uri);
                send(&json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics",
                    "params":{"uri":uri,"diagnostics":[]}}));
            }
            "workspace/didChangeWatchedFiles" | "workspace/didChangeWorkspaceFolders" => {
                ix.invalidate_workspace();
            }
            m => match handle(&mut ix, m, &p) {
                Some(result) => reply(&rid, result),
                None => {
                    // `$/...` などの通知は無視。リクエストには MethodNotFound
                    if id.is_some() && !m.starts_with("$/") {
                        reply_err(&rid, -32601, &format!("未対応のメソッド: {}", m));
                    } else if id.is_some() {
                        reply(&rid, Value::Null);
                    }
                }
            },
        }));
        if r.is_err() && id.is_some() {
            reply(&rid, Value::Null);
        }
    }
}
