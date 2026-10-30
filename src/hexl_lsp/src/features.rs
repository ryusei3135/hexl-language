//! LSP の各機能(診断 / 補完 / ホバー / 定義 / 参照 / リネーム / シグネチャ /
//! セマンティックトークン / 折りたたみ / シンボル)

use crate::index::{Doc, Index};
use crate::scan::{func_paren, Sym, SymKind, TokKind, DIRECTIVES, KEYWORDS};
use crate::util::{is_ident_char, is_ident_start, path_to_uri};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

// ---------------------------------------------------------------- 共通

pub fn range(l1: usize, c1: usize, l2: usize, c2: usize) -> Value {
    json!({"start":{"line":l1,"character":c1},"end":{"line":l2,"character":c2}})
}

fn loc(uri: &str, line: usize, col: usize, len: usize) -> Value {
    json!({"uri": uri, "range": range(line, col, line, col + len)})
}

pub fn position(p: &Value) -> (usize, usize) {
    (
        p["position"]["line"].as_u64().unwrap_or(0) as usize,
        p["position"]["character"].as_u64().unwrap_or(0) as usize,
    )
}

fn get_doc(ix: &mut Index, p: &Value) -> Option<Rc<Doc>> {
    ix.doc(p["textDocument"]["uri"].as_str()?)
}

/// 見つかった定義
#[derive(Clone)]
struct Found {
    doc: Rc<Doc>,
    idx: usize,
}

impl Found {
    fn sym(&self) -> &Sym {
        &self.doc.r.syms[self.idx]
    }
    fn container_name(&self) -> Option<&str> {
        self.sym().container.map(|c| self.doc.r.syms[c].name.as_str())
    }
}

fn symbol_kind(f: &Found) -> u32 {
    let s = f.sym();
    match s.kind {
        SymKind::Struct => 23,
        SymKind::Variant => 10,
        SymKind::Impl => 5,
        SymKind::Func => 12,
        SymKind::Method => 6,
        SymKind::Type => 26,
        SymKind::Define => 14,
        SymKind::Var => 13,
        SymKind::Field => match s.container.map(|c| f.doc.r.syms[c].kind) {
            Some(SymKind::Variant) => 22,
            _ => 8,
        },
    }
}

fn completion_kind(f: &Found) -> u32 {
    let s = f.sym();
    match s.kind {
        SymKind::Struct => 22,
        SymKind::Variant => 13,
        SymKind::Impl => 7,
        SymKind::Func => 3,
        SymKind::Method => 2,
        SymKind::Type => 25,
        SymKind::Define => 21,
        SymKind::Var => 6,
        SymKind::Field => match s.container.map(|c| f.doc.r.syms[c].kind) {
            Some(SymKind::Variant) => 20,
            _ => 5,
        },
    }
}

fn kind_label(k: SymKind) -> &'static str {
    match k {
        SymKind::Struct => "struct",
        SymKind::Variant => "variant",
        SymKind::Impl => "impl",
        SymKind::Func => "関数",
        SymKind::Method => "メゾット",
        SymKind::Type => "type",
        SymKind::Define => "#define",
        SymKind::Field => "メンバー",
        SymKind::Var => "変数",
    }
}

pub fn keyword_doc(k: &str) -> &'static str {
    match k {
        "ret" => "関数から値を返す",
        "cond" => "条件分岐: `cond 条件 { .. }` / `cond { 条件 => { .. } else => { .. } }`",
        "loop" => "ループ: `loop 条件 { .. }`",
        "break" => "ループを抜ける",
        "continue" => "ループの次の周回へ進む",
        "pub" => "定義を公開する",
        "struct" => "構造体の定義",
        "variant" => "列挙型(バリアント)の定義。`variant \"unsafe\" Name` はタグなしの共用体",
        "const" => "定数",
        "static" => "静的変数",
        "mut" => "可変",
        "Self" => "メゾット内で自身の構造体を表す予約語",
        "impl" => "型にメゾットを追加する: `impl Name { func() { .. } }`",
        "else" => "条件の else 側",
        "sizeof" => "型のサイズ",
        "type" => "型エイリアス",
        "extern" => "外部シンボル",
        _ => "hexl の予約語",
    }
}

fn directive_doc(d: &str) -> &'static str {
    match d {
        "include" => "ファイルを取り込む。`#include \"a.hexl\"` / `Name=\"a.hexl\"` / `\"a.hexl\"::func` / `\"a.hexl\"::*` / `std::\"os/vec.hexl\"`",
        "define" => "マクロ定義。`#define N 10` / `#define F(a) ..` / `#define M{a} ..`",
        "if" => "条件付きコンパイル。`#if #windows` ... `#endif`",
        "elif" => "`#if` の続きの条件",
        "else" => "`#if` の else 側",
        "endif" => "`#if` の終わり",
        "line" => "行番号に置き換わる",
        "asm" => "インラインアセンブラ。`${式}` で式を埋め込める",
        _ => "マクロの展開",
    }
}

// ---------------------------------------------------------------- 診断

pub fn diagnostics(ix: &mut Index, doc: &Rc<Doc>) -> Vec<Value> {
    let mut v: Vec<Value> = doc
        .r
        .diags
        .iter()
        .map(|d| {
            json!({
                "range": range(d.line, d.col, d.line, d.col + d.len.max(1)),
                "severity": d.severity, "source": "hexl-lsp", "message": d.msg,
            })
        })
        .collect();
    if doc.path.is_some() {
        for inc in &doc.r.includes {
            // `.c` / `.h` や標準ライブラリも含め、ファイルが見つからなければ警告
            if ix.resolve_include(doc.path.as_deref(), inc).is_none() {
                v.push(json!({
                    "range": range(inc.line, inc.col, inc.line, inc.col + inc.len.max(1)),
                    "severity": 2, "source": "hexl-lsp",
                    "message": format!(
                        "インクルード先が見つかりません: {}{}", inc.path,
                        if inc.is_std { "(標準ライブラリ。initializationOptions.libraryPath か HEXL_LIBRARY で指定できます)" } else { "" }
                    ),
                }));
            }
        }
    }
    v
}

// ---------------------------------------------------------------- 定義の検索

/// `doc` から見える全ドキュメントの、条件に合うシンボル
fn find_syms<F: Fn(&Sym, &Doc) -> bool>(ix: &mut Index, doc: &Rc<Doc>, pred: F) -> Vec<Found> {
    let mut out = Vec::new();
    for d in ix.scope_docs(doc) {
        for (i, s) in d.r.syms.iter().enumerate() {
            if pred(s, &d) {
                out.push(Found { doc: Rc::clone(&d), idx: i });
            }
        }
    }
    out
}

/// カーソル位置の行を含む struct / variant / impl の名前
fn enclosing_container(doc: &Doc, line: usize) -> Option<String> {
    doc.r
        .syms
        .iter()
        .filter(|s| matches!(s.kind, SymKind::Struct | SymKind::Variant | SymKind::Impl))
        .find(|s| s.line <= line && line <= s.end_line)
        .map(|s| s.name.clone())
}

fn members_of(ix: &mut Index, doc: &Rc<Doc>, container: &str) -> Vec<Found> {
    find_syms(ix, doc, |s, d| {
        s.container.map_or(false, |c| d.r.syms[c].name == container)
    })
}

/// モジュール名 `module` で取り込まれたファイルの、最上位のシンボル
fn module_members(ix: &mut Index, doc: &Rc<Doc>, module: &str) -> Vec<Found> {
    let mut out = Vec::new();
    for (m, d) in ix.scope(doc) {
        if m.as_deref() == Some(module) {
            for (i, s) in d.r.syms.iter().enumerate() {
                if s.container.is_none() && s.kind != SymKind::Impl {
                    out.push(Found { doc: Rc::clone(&d), idx: i });
                }
            }
        }
    }
    out
}

/// カーソル位置の名前の定義を探す(`a::b` / `x.m` / `self->m` / `#MACRO` / 通常の名前)
fn lookup(ix: &mut Index, doc: &Rc<Doc>, tidx: usize) -> Vec<Found> {
    let tkns = &doc.r.tkns;
    let t = &tkns[tidx];
    let prev = tidx.checked_sub(1).map(|i| &tkns[i]);
    let prev2 = tidx.checked_sub(2).map(|i| &tkns[i]);

    if t.kind == TokKind::Preproc {
        let name = t.text.trim_start_matches('#').to_string();
        if DIRECTIVES.contains(&name.as_str()) {
            return Vec::new();
        }
        return find_syms(ix, doc, |s, _| s.kind == SymKind::Define && s.name == name);
    }
    if t.kind != TokKind::Name {
        return Vec::new();
    }
    let name = t.text.clone();

    if let (Some(p), Some(q)) = (prev, prev2) {
        if p.is_op("::") && q.kind == TokKind::Name {
            let qual = q.text.clone();
            let mut r = module_members(ix, doc, &qual)
                .into_iter()
                .filter(|f| f.sym().name == name)
                .collect::<Vec<_>>();
            r.extend(
                members_of(ix, doc, &qual)
                    .into_iter()
                    .filter(|f| f.sym().name == name),
            );
            if !r.is_empty() {
                return r;
            }
        }
        if (p.is_op(".") || p.is_op("->")) && (q.is_kw("self") || (q.kind == TokKind::Name && q.text == "self")) {
            if let Some(c) = enclosing_container(doc, t.line) {
                let r: Vec<Found> = members_of(ix, doc, &c)
                    .into_iter()
                    .filter(|f| f.sym().name == name)
                    .collect();
                if !r.is_empty() {
                    return r;
                }
            }
        }
    }
    if prev.map_or(false, |p| p.is_op(".") || p.is_op("->")) {
        let r = find_syms(ix, doc, |s, _| {
            s.name == name && matches!(s.kind, SymKind::Field | SymKind::Method)
        });
        if !r.is_empty() {
            return r;
        }
    }
    let mut r = find_syms(ix, doc, |s, _| {
        s.name == name && s.container.is_none() && s.kind != SymKind::Impl
    });
    if r.is_empty() {
        // 取り込んでいなくても、ワークスペース内にあれば候補にする
        for d in ix.all_docs(Some(doc)) {
            for (i, s) in d.r.syms.iter().enumerate() {
                if s.name == name && s.container.is_none() && s.kind != SymKind::Impl {
                    r.push(Found { doc: Rc::clone(&d), idx: i });
                }
            }
        }
    }
    r
}

pub fn definition(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return Value::Null };
    let (line, ch) = position(p);
    let Some(ti) = doc.tok_idx_at(line, ch) else { return Value::Null };
    let t = &doc.r.tkns[ti];
    // `#include "..."` のパスからファイルへ
    if t.kind == TokKind::Str {
        if let Some(inc) = doc.r.includes.iter().find(|i| i.line == t.line) {
            if let Some(path) = ix.resolve_include(doc.path.as_deref(), inc) {
                return json!([{"uri": path_to_uri(&path), "range": range(0, 0, 0, 0)}]);
            }
        }
        return Value::Null;
    }
    let found = lookup(ix, &doc, ti);
    if found.is_empty() {
        return Value::Null;
    }
    json!(found
        .iter()
        .map(|f| loc(&f.doc.uri, f.sym().line, f.sym().col, f.sym().name_len))
        .collect::<Vec<_>>())
}

pub fn hover(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return Value::Null };
    let (line, ch) = position(p);
    let Some(ti) = doc.tok_idx_at(line, ch) else { return Value::Null };
    let t = doc.r.tkns[ti].clone();
    let text = match t.kind {
        TokKind::Keyword => format!("**{}**: {}", t.text, keyword_doc(&t.text)),
        TokKind::Str => {
            let Some(inc) = doc.r.includes.iter().find(|i| i.line == t.line) else {
                return Value::Null;
            };
            match ix.resolve_include(doc.path.as_deref(), inc) {
                Some(p) => format!("`{}`", p.display()),
                None => format!("`{}` (見つかりません)", inc.path),
            }
        }
        TokKind::Preproc | TokKind::Name => {
            let name = t.text.trim_start_matches('#');
            if t.kind == TokKind::Preproc && DIRECTIVES.contains(&name) {
                format!("**#{}**: {}", name, directive_doc(name))
            } else {
                let found = lookup(ix, &doc, ti);
                let Some(f) = found.first() else { return Value::Null };
                let s = f.sym();
                let qual = match f.container_name() {
                    Some(c) => format!("{}::{}", c, s.name),
                    None => s.name.clone(),
                };
                let mut out = format!(
                    "**{}** ({}{})\n\n```hexl\n{}\n```",
                    qual,
                    kind_label(s.kind),
                    if s.public { ", pub" } else { "" },
                    s.sig
                );
                if !s.doc.is_empty() {
                    out.push_str(&format!("\n\n{}", s.doc));
                }
                if f.doc.uri != doc.uri {
                    out.push_str(&format!("\n\n*{}*", f.doc.file_name()));
                }
                if found.len() > 1 {
                    out.push_str(&format!("\n\n(他に {} 件の候補)", found.len() - 1));
                }
                out
            }
        }
        _ => return Value::Null,
    };
    json!({"contents": {"kind":"markdown","value":text},
        "range": range(t.line, t.col, t.line, t.col + t.len)})
}

// ---------------------------------------------------------------- 補完

fn sym_item(f: &Found) -> Value {
    let s = f.sym();
    let mut v = json!({
        "label": s.name, "kind": completion_kind(f),
        "detail": s.sig,
    });
    let mut docs = String::new();
    if let Some(c) = f.container_name() {
        docs.push_str(&format!("{}::{}\n\n", c, s.name));
    }
    docs.push_str(&s.doc);
    if !docs.trim().is_empty() {
        v["documentation"] = json!({"kind":"markdown","value":docs});
    }
    v
}

fn snippet(label: &str, detail: &str, body: &str) -> Value {
    json!({"label": label, "kind": 15, "detail": detail,
        "insertText": body, "insertTextFormat": 2})
}

fn dedupe(items: Vec<Value>) -> Vec<Value> {
    let mut seen: HashSet<(String, u64)> = HashSet::new();
    items
        .into_iter()
        .filter(|v| {
            seen.insert((
                v["label"].as_str().unwrap_or("").to_string(),
                v["kind"].as_u64().unwrap_or(0),
            ))
        })
        .collect()
}

/// `#include "..."` のパス補完
fn include_path_items(ix: &Index, doc: &Doc, partial: &str, std: bool) -> Vec<Value> {
    let mut bases: Vec<std::path::PathBuf> = Vec::new();
    if std {
        if let Some(l) = &ix.library {
            bases.push(l.clone());
        }
        if let Ok(l) = std::env::var("HEXL_LIBRARY") {
            bases.push(l.into());
        }
        bases.extend(ix.roots.iter().map(|r| r.join("library")));
    } else {
        if let Some(dir) = doc.path.as_deref().and_then(|p| p.parent()) {
            bases.push(dir.to_path_buf());
            if let Some(par) = dir.parent() {
                bases.push(par.to_path_buf());
            }
        }
        bases.extend(ix.roots.iter().cloned());
    }
    let (dir_part, _) = match partial.rfind('/') {
        Some(i) => (&partial[..=i], &partial[i + 1..]),
        None => ("", partial),
    };
    let mut items = Vec::new();
    for b in bases {
        let Ok(rd) = std::fs::read_dir(b.join(dir_part)) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let is_dir = e.path().is_dir();
            let ok = is_dir || [".hexl", ".c", ".h"].iter().any(|x| name.ends_with(x));
            if ok {
                items.push(json!({"label": name, "kind": if is_dir {19} else {17},
                    "insertText": if is_dir { format!("{}/", name) } else { name.clone() }}));
            }
        }
    }
    dedupe(items)
}

pub fn completion(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return json!([]) };
    let (line, ch) = position(p);
    let prefix = doc.line_prefix(line, ch);
    let mut ws = prefix.len();
    while ws > 0 && is_ident_char(prefix[ws - 1]) {
        ws -= 1;
    }
    let before: String = prefix[..ws].iter().collect();
    let line_str: String = prefix.iter().collect();

    // 1. `#include "` の中
    if line_str.trim_start().starts_with("#include") && line_str.matches('"').count() % 2 == 1 {
        let partial = line_str.rsplit('"').next().unwrap_or("");
        let std = line_str.contains("std::");
        return json!(include_path_items(ix, &doc, partial, std));
    }
    // 2. `#` の後ろ: 命令とマクロ
    if before.ends_with('#') {
        let mut items: Vec<Value> = DIRECTIVES
            .iter()
            .map(|d| json!({"label": d, "kind": 14, "detail": directive_doc(d)}))
            .collect();
        for os in ["windows", "linux"] {
            items.push(json!({"label": os, "kind": 21, "detail": "OSを表すシンボル"}));
        }
        for f in find_syms(ix, &doc, |s, _| s.kind == SymKind::Define) {
            items.push(sym_item(&f));
        }
        return json!(dedupe(items));
    }
    // 3. `Name::` の後ろ: モジュールの中身 / 型のメゾット・バリアント
    if before.ends_with("::") {
        let head: Vec<char> = before.trim_end_matches(':').chars().collect();
        let mut s = head.len();
        while s > 0 && is_ident_char(head[s - 1]) {
            s -= 1;
        }
        let qual: String = head[s..].iter().collect();
        let mut found = module_members(ix, &doc, &qual);
        found.retain(|f| f.sym().public || f.sym().kind == SymKind::Define);
        found.extend(members_of(ix, &doc, &qual));
        return json!(dedupe(found.iter().map(sym_item).collect()));
    }
    // 4. `.` / `->` の後ろ: メンバー
    if before.ends_with('.') || before.ends_with("->") {
        let head: Vec<char> = before
            .trim_end_matches('.')
            .trim_end_matches("->")
            .trim_end_matches('-')
            .chars()
            .collect();
        let mut s = head.len();
        while s > 0 && is_ident_char(head[s - 1]) {
            s -= 1;
        }
        let recv: String = head[s..].iter().collect();
        let found = if recv == "self" {
            match enclosing_container(&doc, line) {
                Some(c) => members_of(ix, &doc, &c),
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let found = if found.is_empty() {
            // 受け手の型は分からないので、見えているメンバーを全部出す
            find_syms(ix, &doc, |s, _| matches!(s.kind, SymKind::Field | SymKind::Method))
        } else {
            found
        };
        return json!(dedupe(found.iter().map(sym_item).collect()));
    }
    // 5. 通常: 予約語 + スニペット + 定義 + モジュール名 + このファイル内の名前
    let mut items: Vec<Value> = KEYWORDS
        .iter()
        .map(|k| json!({"label": k, "kind": 14, "detail": keyword_doc(k)}))
        .collect();
    items.push(snippet("struct", "構造体", "struct ${1:Name} {\n\t$0\n}"));
    items.push(snippet("variant", "バリアント", "variant ${1:Name} {\n\t$0\n}"));
    items.push(snippet("impl", "メゾットの追加", "impl ${1:Name} {\n\t$0\n}"));
    items.push(snippet("loop", "ループ", "loop ${1:cond} {\n\t$0\n}"));
    items.push(snippet("cond", "条件分岐", "cond ${1:expr} {\n\t$0\n}"));
    items.push(snippet("fn", "関数の雛形", "${1:name}(${2}): ${3:int} {\n\t$0\n}"));
    items.push(snippet("#include", "取り込み", "#include \"$0\""));
    let mut known: HashSet<String> = HashSet::new();
    for f in find_syms(ix, &doc, |s, _| s.container.is_none() && s.kind != SymKind::Impl) {
        known.insert(f.sym().name.clone());
        items.push(sym_item(&f));
    }
    for (m, _) in ix.scope(&doc) {
        if let Some(m) = m {
            items.push(json!({"label": m, "kind": 9, "detail": "モジュール"}));
        }
    }
    let mut n = 0;
    for t in &doc.r.tkns {
        if t.kind == TokKind::Name && !known.contains(&t.text) && n < 500 {
            known.insert(t.text.clone());
            items.push(json!({"label": t.text, "kind": 6}));
            n += 1;
        }
    }
    json!(dedupe(items))
}

// ---------------------------------------------------------------- シグネチャヘルプ

/// `(a: int, b: T)` の引数を文字列に分ける
fn split_params(sig: &str) -> Vec<String> {
    let Some(open) = sig.find('(') else { return Vec::new() };
    let mut depth = 0i32;
    let mut cur = String::new();
    let mut out = Vec::new();
    for c in sig[open..].chars() {
        match c {
            '(' | '[' | '<' => {
                depth += 1;
                if depth > 1 {
                    cur.push(c);
                }
            }
            ')' | ']' | '>' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                cur.push(c);
            }
            ',' if depth == 1 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

pub fn signature_help(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return Value::Null };
    let (line, ch) = position(p);
    let tkns = &doc.r.tkns;
    // カーソルより前のトークン
    let n = tkns.iter().take_while(|t| (t.line, t.col) < (line, ch)).count();
    // 対応していない `(` を後ろ向きに探し、途中の `,` を数える
    let (mut depth, mut commas, mut open) = (0i32, 0usize, None);
    for k in (0..n).rev() {
        let t = &tkns[k];
        if t.kind != TokKind::Op {
            continue;
        }
        match t.text.as_str() {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                if depth == 0 {
                    if t.text == "(" {
                        open = Some(k);
                    }
                    break;
                }
                depth -= 1;
            }
            "," if depth == 0 => commas += 1,
            _ => {}
        }
    }
    let Some(open) = open else { return Value::Null };
    // `name<T>(` の `<..>` を読み飛ばして名前を探す
    let mut k = open;
    if k > 0 && tkns[k - 1].is_op(">") {
        let mut d = 0;
        while k > 0 {
            k -= 1;
            if tkns[k].is_op(">") {
                d += 1;
            } else if tkns[k].is_op("<") {
                d -= 1;
                if d == 0 {
                    break;
                }
            }
        }
    }
    if k == 0 || tkns[k - 1].kind != TokKind::Name {
        return Value::Null;
    }
    let ni = k - 1;
    let found = lookup(ix, &doc, ni);
    let Some(f) = found.iter().find(|f| matches!(f.sym().kind, SymKind::Func | SymKind::Method)) else {
        return Value::Null;
    };
    let s = f.sym();
    let mut params = split_params(&s.sig);
    // `x.method(..)` のときは `self` を引数に数えない
    let via_member = ni > 0 && (tkns[ni - 1].is_op(".") || tkns[ni - 1].is_op("->"));
    let mut shown = params.clone();
    if via_member && params.first().map_or(false, |p| p == "self" || p.starts_with("self:")) {
        params.remove(0);
        shown = params.clone();
    }
    let label = s.sig.clone();
    let infos: Vec<Value> = shown.iter().map(|p| json!({"label": p})).collect();
    let mut sig = json!({"label": label, "parameters": infos});
    if !s.doc.is_empty() {
        sig["documentation"] = json!({"kind":"markdown","value": s.doc});
    }
    json!({"signatures": [sig], "activeSignature": 0,
        "activeParameter": commas.min(shown.len().saturating_sub(1))})
}

// ---------------------------------------------------------------- 参照 / ハイライト / リネーム

/// 参照の照合に使う名前(`#NAME` は `NAME`)と、その範囲
fn ref_key(t: &crate::scan::Tok) -> Option<(String, usize, usize)> {
    match t.kind {
        TokKind::Name => Some((t.text.clone(), t.col, t.len)),
        TokKind::Preproc => {
            let n = t.text.trim_start_matches('#');
            if DIRECTIVES.contains(&n) {
                None
            } else {
                Some((n.to_string(), t.col + 1, t.len - 1))
            }
        }
        _ => None,
    }
}

fn refs_in(doc: &Doc, name: &str) -> Vec<(usize, usize, usize)> {
    doc.r
        .tkns
        .iter()
        .filter_map(ref_key)
        .zip(doc.r.tkns.iter().filter(|t| ref_key(t).is_some()))
        .filter(|((n, ..), _)| n == name)
        .map(|((_, c, l), t)| (t.line, c, l))
        .collect()
}

pub fn document_highlight(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return Value::Null };
    let (line, ch) = position(p);
    let Some(ti) = doc.tok_idx_at(line, ch) else { return Value::Null };
    let Some((name, ..)) = ref_key(&doc.r.tkns[ti]) else { return Value::Null };
    json!(refs_in(&doc, &name)
        .into_iter()
        .map(|(l, c, n)| json!({"range": range(l, c, l, c + n), "kind": 1}))
        .collect::<Vec<_>>())
}

pub fn references(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return Value::Null };
    let (line, ch) = position(p);
    let Some(ti) = doc.tok_idx_at(line, ch) else { return Value::Null };
    let Some((name, ..)) = ref_key(&doc.r.tkns[ti]) else { return Value::Null };
    let mut out = Vec::new();
    for d in ix.all_docs(Some(&doc)) {
        for (l, c, n) in refs_in(&d, &name) {
            out.push(loc(&d.uri, l, c, n));
        }
    }
    json!(out)
}

fn is_keyword_or_bad(name: &str) -> bool {
    name.is_empty()
        || KEYWORDS.contains(&name)
        || !name.chars().next().map_or(false, is_ident_start)
        || !name.chars().all(is_ident_char)
}

pub fn prepare_rename(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return Value::Null };
    let (line, ch) = position(p);
    let Some(ti) = doc.tok_idx_at(line, ch) else { return Value::Null };
    let t = &doc.r.tkns[ti];
    let Some((name, col, len)) = ref_key(t) else { return Value::Null };
    // 定義が見つかる名前(関数 / 型 / メンバー / マクロ)だけを対象にする。
    // ローカル変数は同名の別物と区別できないので、リネームしない
    if lookup(ix, &doc, ti).is_empty() {
        return Value::Null;
    }
    json!({"range": range(t.line, col, t.line, col + len), "placeholder": name})
}

pub fn rename(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return Value::Null };
    let (line, ch) = position(p);
    let new_name = p["newName"].as_str().unwrap_or("");
    if is_keyword_or_bad(new_name) {
        return Value::Null;
    }
    let Some(ti) = doc.tok_idx_at(line, ch) else { return Value::Null };
    let Some((name, ..)) = ref_key(&doc.r.tkns[ti]) else { return Value::Null };
    if lookup(ix, &doc, ti).is_empty() {
        return Value::Null;
    }
    let mut changes: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for d in ix.all_docs(Some(&doc)) {
        let edits: Vec<Value> = refs_in(&d, &name)
            .into_iter()
            .map(|(l, c, n)| json!({"range": range(l, c, l, c + n), "newText": new_name}))
            .collect();
        if !edits.is_empty() {
            changes.insert(d.uri.clone(), edits);
        }
    }
    json!({"changes": changes})
}

// ---------------------------------------------------------------- シンボル

pub fn document_symbols(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return json!([]) };
    let syms = &doc.r.syms;
    let node = |i: usize| -> Value {
        let s = &syms[i];
        let f = Found { doc: Rc::clone(&doc), idx: i };
        json!({
            "name": s.name, "kind": symbol_kind(&f), "detail": s.sig,
            "range": range(s.line, 0, s.end_line, 10_000),
            "selectionRange": range(s.line, s.col, s.line, s.col + s.name_len),
            "children": [],
        })
    };
    let mut nodes: Vec<Value> = (0..syms.len()).map(node).collect();
    // 子を親にぶら下げる(後ろから処理して添字を保つ)
    let mut top = Vec::new();
    for i in (0..syms.len()).rev() {
        let n = nodes[i].clone();
        match syms[i].container {
            Some(c) => {
                if let Some(arr) = nodes[c]["children"].as_array_mut() {
                    arr.insert(0, n);
                }
            }
            None => top.push(n),
        }
    }
    top.reverse();
    json!(top)
}

pub fn workspace_symbols(idx: &mut Index, query: &str) -> Value {
    let q = query.to_lowercase();
    let mut out = Vec::new();
    for d in idx.all_docs(None) {
        for (i, s) in d.r.syms.iter().enumerate() {
            if s.kind == SymKind::Impl || !s.name.to_lowercase().contains(&q) {
                continue;
            }
            let f = Found { doc: Rc::clone(&d), idx: i };
            out.push(json!({
                "name": s.name, "kind": symbol_kind(&f),
                "containerName": f.container_name().unwrap_or(""),
                "location": loc(&d.uri, s.line, s.col, s.name_len),
            }));
            if out.len() >= 200 {
                return json!(out);
            }
        }
    }
    json!(out)
}

// ---------------------------------------------------------------- 折りたたみ

pub fn folding_ranges(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return json!([]) };
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut out = Vec::new();
    for f in &doc.r.folds {
        if f.end > f.start && seen.insert((f.start, f.end)) {
            let mut v = json!({"startLine": f.start, "endLine": f.end});
            if f.comment {
                v["kind"] = json!("comment");
            }
            out.push(v);
        }
    }
    json!(out)
}

// ---------------------------------------------------------------- セマンティックトークン

pub const TOKEN_TYPES: &[&str] = &[
    "keyword", "type", "function", "macro", "string", "number", "property", "method", "namespace",
];

pub fn semantic_tokens(ix: &mut Index, p: &Value) -> Value {
    let Some(doc) = get_doc(ix, p) else { return json!({"data": []}) };
    let mut types: HashSet<String> = HashSet::new();
    let mut modules: HashSet<String> = HashSet::new();
    for (m, d) in ix.scope(&doc) {
        if let Some(m) = m {
            modules.insert(m);
        }
        for s in &d.r.syms {
            if matches!(s.kind, SymKind::Struct | SymKind::Variant | SymKind::Type) {
                types.insert(s.name.clone());
            }
        }
    }
    let tkns = &doc.r.tkns;
    let mut data: Vec<u32> = Vec::new();
    let (mut pl, mut pc) = (0usize, 0usize);
    for (i, t) in tkns.iter().enumerate() {
        let prev = i.checked_sub(1).map(|k| &tkns[k]);
        let next = tkns.get(i + 1);
        let after_member = prev.map_or(false, |p| p.is_op(".") || p.is_op("->"));
        let ty: Option<usize> = match t.kind {
            TokKind::Keyword => Some(0),
            TokKind::Str => Some(4),
            TokKind::Number => Some(5),
            TokKind::Preproc => Some(3),
            TokKind::Name => {
                let call = func_paren(tkns, i + 1).is_some();
                if types.contains(&t.text) {
                    Some(1)
                } else if next.map_or(false, |n| n.is_op("::")) && modules.contains(&t.text) {
                    Some(8)
                } else if call {
                    Some(if after_member { 7 } else { 2 })
                } else if after_member {
                    Some(6)
                } else if prev.map_or(false, |p| p.is_op(":")) && !line_has_question(tkns, i) {
                    // `x: Type` の型
                    Some(1)
                } else {
                    None
                }
            }
            TokKind::Op => None,
        };
        let Some(ty) = ty else { continue };
        // 複数行にまたがる文字列は無い(1行で打ち切る)
        let dl = t.line - pl;
        let ds = if dl == 0 { t.col - pc } else { t.col };
        data.extend([dl as u32, ds as u32, t.len as u32, ty as u32, 0]);
        pl = t.line;
        pc = t.col;
    }
    json!({"data": data})
}

/// 三項演算子(`a ? b : c`)の `:` を型注釈と取り違えないための判定
fn line_has_question(tkns: &[crate::scan::Tok], i: usize) -> bool {
    let line = tkns[i].line;
    tkns[..i]
        .iter()
        .rev()
        .take_while(|t| t.line == line)
        .any(|t| t.is_op("?"))
}
