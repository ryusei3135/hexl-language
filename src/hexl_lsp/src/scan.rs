//! 軽量な字句解析。トークン / シンボル / `#include` / 診断 / 折りたたみ範囲を一度に作る。
//! 位置(line, col, len)は LSP の既定に合わせて 0 始まり・UTF-16 単位。

use crate::util::{is_ident_char, is_ident_start};
use std::collections::HashMap;

/// hexl_frontend/lex/lexer.rs のキーワード表と同じ
pub const KEYWORDS: &[&str] = &[
    "ret", "cond", "loop", "pub", "struct", "variant", "const", "static", "mut", "Self", "must",
    "of", "continue", "break", "where", "allow", "else", "impl", "extern", "register", "async",
    "await", "move", "type", "sizeof",
];

/// プリプロセッサの命令(`#include` など。lex/PREPROCESSOR.md)
pub const DIRECTIVES: &[&str] = &["include", "define", "if", "elif", "else", "endif", "line", "asm"];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TokKind {
    Name,
    Keyword,
    Number,
    Str,
    Op,
    /// `#include` `#define` `#NAME` など
    Preproc,
}

#[derive(Clone, Debug)]
pub struct Tok {
    pub kind: TokKind,
    pub text: String,
    pub line: usize,
    pub col: usize,
    pub len: usize,
    /// ソース(chars)上の開始/終了位置
    pub off: usize,
    pub end: usize,
}

impl Tok {
    pub fn is_op(&self, s: &str) -> bool {
        self.kind == TokKind::Op && self.text == s
    }
    pub fn is_kw(&self, s: &str) -> bool {
        self.kind == TokKind::Keyword && self.text == s
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SymKind {
    Struct,
    Variant,
    Impl,
    Func,
    Method,
    Type,
    Define,
    Field,
    Var,
}

#[derive(Clone, Debug)]
pub struct Sym {
    pub name: String,
    pub kind: SymKind,
    pub line: usize,
    pub col: usize,
    pub name_len: usize,
    /// 定義の最後の行(本体の `}` の行)
    pub end_line: usize,
    pub sig: String,
    /// 直前の `//` コメント
    pub doc: String,
    /// 所属する struct / variant / impl の `syms` 内の添字
    pub container: Option<usize>,
    /// `#if` の中にあるか
    pub in_cond: bool,
    /// `pub` が付いているか
    pub public: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Select {
    /// `#include "a.hexl"` / `Name="a.hexl"`: モジュール名で修飾して使う
    Module,
    /// `::*`
    Star,
    /// `::func`
    Func(String),
}

#[derive(Clone, Debug)]
pub struct Include {
    pub line: usize,
    pub path: String,
    pub is_std: bool,
    pub alias: Option<String>,
    pub select: Select,
    /// `#include mod::file` の旧書き方
    pub legacy: bool,
    /// 修飾に使うモジュール名(別名 or ファイル名)
    pub module: String,
    /// パスを書いたトークンの範囲(診断用)
    pub col: usize,
    pub len: usize,
}

#[derive(Clone, Debug)]
pub struct Diag {
    pub line: usize,
    pub col: usize,
    pub len: usize,
    pub msg: String,
    /// 1 = error, 2 = warning
    pub severity: u8,
}

#[derive(Clone, Debug)]
pub struct Fold {
    pub start: usize,
    pub end: usize,
    pub comment: bool,
}

pub struct ScanResult {
    pub tkns: Vec<Tok>,
    pub syms: Vec<Sym>,
    pub diags: Vec<Diag>,
    pub includes: Vec<Include>,
    pub folds: Vec<Fold>,
    /// `{` のトークン添字 -> 対応する `}` の添字
    pub brace_match: HashMap<usize, usize>,
}

pub fn scan(src: &str) -> ScanResult {
    let chars: Vec<char> = src.chars().collect();
    let mut tkns: Vec<Tok> = Vec::new();
    let mut diags: Vec<Diag> = Vec::new();
    let mut folds: Vec<Fold> = Vec::new();
    let mut line_comments: HashMap<usize, String> = HashMap::new();
    // 開き括弧のスタック: (文字, line, col, ループの `{` か)
    let mut stack: Vec<(char, usize, usize, bool)> = Vec::new();
    let mut pending_loop = false;

    let (mut i, mut line, mut col) = (0usize, 0usize, 0usize);

    macro_rules! adv {
        () => {{
            let c = chars[i];
            i += 1;
            if c == '\n' {
                line += 1;
                col = 0;
            } else {
                col += c.len_utf16();
            }
        }};
    }

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();

        if c.is_whitespace() {
            adv!();
            continue;
        }
        // 行コメント
        if c == '/' && next == Some('/') {
            let l = line;
            let mut s = String::new();
            while i < chars.len() && chars[i] != '\n' {
                s.push(chars[i]);
                adv!();
            }
            let s = s.trim_end_matches('\r').trim_start_matches('/').trim().to_string();
            line_comments.insert(l, s);
            continue;
        }
        // ブロックコメント
        if c == '/' && next == Some('*') {
            let (sl, sc) = (line, col);
            adv!();
            adv!();
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    adv!();
                    adv!();
                    closed = true;
                    break;
                }
                adv!();
            }
            if closed {
                if line > sl {
                    folds.push(Fold { start: sl, end: line, comment: true });
                }
            } else {
                diags.push(Diag {
                    line: sl,
                    col: sc,
                    len: 2,
                    msg: "ブロックコメント /* が閉じていません".into(),
                    severity: 1,
                });
            }
            continue;
        }

        let (sl, sc, so) = (line, col, i);
        let mut text = String::new();
        let kind;

        if c == '"' {
            kind = TokKind::Str;
            text.push(c);
            adv!();
            let mut closed = false;
            while i < chars.len() {
                let d = chars[i];
                if d == '\n' {
                    break;
                }
                text.push(d);
                adv!();
                if d == '\\' && i < chars.len() && chars[i] != '\n' {
                    text.push(chars[i]);
                    adv!();
                } else if d == '"' {
                    closed = true;
                    break;
                }
            }
            if !closed {
                diags.push(Diag {
                    line: sl,
                    col: sc,
                    len: col.saturating_sub(sc).max(1),
                    msg: "文字列リテラルが閉じていません".into(),
                    severity: 1,
                });
            }
        } else if c == '#' && next.map_or(false, is_ident_start) {
            kind = TokKind::Preproc;
            text.push(c);
            adv!();
            while i < chars.len() && is_ident_char(chars[i]) {
                text.push(chars[i]);
                adv!();
            }
        } else if is_ident_start(c) {
            while i < chars.len() && is_ident_char(chars[i]) {
                text.push(chars[i]);
                adv!();
            }
            kind = if KEYWORDS.contains(&text.as_str()) {
                TokKind::Keyword
            } else {
                TokKind::Name
            };
            if text == "loop" {
                pending_loop = true;
            }
            if (text == "break" || text == "continue") && !stack.iter().any(|s| s.3) {
                diags.push(Diag {
                    line: sl,
                    col: sc,
                    len: text.chars().count(),
                    msg: format!("`{}` がループ(loop)の外にあります", text),
                    severity: 2,
                });
            }
        } else if c.is_ascii_digit() {
            kind = TokKind::Number;
            while i < chars.len() {
                let d = chars[i];
                let float_dot = d == '.' && chars.get(i + 1).map_or(false, |x| x.is_ascii_digit());
                if is_ident_char(d) || float_dot {
                    text.push(d);
                    adv!();
                } else {
                    break;
                }
            }
        } else {
            kind = TokKind::Op;
            text.push(c);
            adv!();
            // 2文字の演算子(`::` `->` `=>`)だけ1トークンにする
            if let Some(n) = next {
                if (c == ':' && n == ':') || (c == '-' && n == '>') || (c == '=' && n == '>') {
                    text.push(n);
                    adv!();
                }
            }
            match c {
                '(' | '[' | '{' => {
                    let is_loop = c == '{' && pending_loop;
                    if c == '{' {
                        pending_loop = false;
                    }
                    stack.push((c, sl, sc, is_loop));
                }
                ')' | ']' | '}' => {
                    let want = match c {
                        ')' => '(',
                        ']' => '[',
                        _ => '{',
                    };
                    match stack.last() {
                        Some(&(o, ..)) if o == want => {
                            stack.pop();
                        }
                        Some(&(o, ol, oc, _)) => {
                            diags.push(Diag {
                                line: sl,
                                col: sc,
                                len: 1,
                                msg: format!(
                                    "括弧が対応していません: '{}' ({}行目{}列目)を閉じる位置に '{}' があります",
                                    o,
                                    ol + 1,
                                    oc + 1,
                                    c
                                ),
                                severity: 1,
                            });
                        }
                        None => diags.push(Diag {
                            line: sl,
                            col: sc,
                            len: 1,
                            msg: format!("対応する開き括弧がない '{}' です", c),
                            severity: 1,
                        }),
                    }
                }
                _ => {}
            }
        }

        let len = col - sc;
        tkns.push(Tok { kind, text, line: sl, col: sc, len, off: so, end: i });
    }

    for (o, l, c, _) in stack {
        diags.push(Diag {
            line: l,
            col: c,
            len: 1,
            msg: format!("'{}' が閉じていません", o),
            severity: 1,
        });
    }

    // `{` と `}` の対応
    let mut brace_match = HashMap::new();
    {
        let mut st: Vec<usize> = Vec::new();
        for (k, t) in tkns.iter().enumerate() {
            if t.is_op("{") {
                st.push(k);
            } else if t.is_op("}") {
                if let Some(o) = st.pop() {
                    brace_match.insert(o, k);
                    if tkns[o].line < t.line {
                        folds.push(Fold { start: tkns[o].line, end: t.line, comment: false });
                    }
                }
            }
        }
    }

    let (syms, includes) = find_symbols(&chars, &tkns, &brace_match, &line_comments, &mut folds);
    check_duplicates(&syms, &mut diags);
    ScanResult { tkns, syms, diags, includes, folds, brace_match }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 直前の連続した `//` コメントをまとめて返す
fn doc_for(line: usize, comments: &HashMap<usize, String>) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut l = line;
    while l > 0 {
        match comments.get(&(l - 1)) {
            Some(s) => parts.push(s),
            None => break,
        }
        l -= 1;
    }
    parts.reverse();
    parts.join("\n")
}

fn line_tail(chars: &[char], from: usize) -> String {
    let mut s = String::new();
    let mut k = from;
    while k < chars.len() && chars[k] != '\n' {
        s.push(chars[k]);
        k += 1;
    }
    let s = match s.find("//") {
        Some(p) => s[..p].to_string(),
        None => s,
    };
    collapse_ws(s.trim().trim_end_matches(','))
}

fn find_symbols(
    chars: &[char],
    tkns: &[Tok],
    matches: &HashMap<usize, usize>,
    comments: &HashMap<usize, String>,
    folds: &mut Vec<Fold>,
) -> (Vec<Sym>, Vec<Include>) {
    let mut syms: Vec<Sym> = Vec::new();
    let mut includes: Vec<Include> = Vec::new();
    let mut depth: usize = 0;
    let mut cond_depth: usize = 0;
    let mut cond_starts: Vec<usize> = Vec::new();
    // 直後の `{` で本体が始まる struct / variant / impl
    let mut pending: Option<usize> = None;
    // (syms の添字, 本体の深さ, 種類)
    let mut containers: Vec<(usize, usize, SymKind)> = Vec::new();
    let mut i = 0;

    while i < tkns.len() {
        let t = &tkns[i];
        let prev = if i > 0 { Some(&tkns[i - 1]) } else { None };
        let at_container = containers.last().map_or(false, |c| c.1 == depth);
        let top_level = depth == 0 || at_container;

        match (t.kind, t.text.as_str()) {
            (TokKind::Op, "{") => {
                depth += 1;
                if let Some(p) = pending.take() {
                    containers.push((p, depth, syms[p].kind));
                }
            }
            (TokKind::Op, "}") => {
                if containers.last().map_or(false, |c| c.1 == depth) {
                    containers.pop();
                }
                depth = depth.saturating_sub(1);
            }
            (TokKind::Preproc, "#if") => {
                cond_depth += 1;
                cond_starts.push(t.line);
            }
            (TokKind::Preproc, "#endif") => {
                cond_depth = cond_depth.saturating_sub(1);
                if let Some(s) = cond_starts.pop() {
                    if t.line > s {
                        folds.push(Fold { start: s, end: t.line, comment: false });
                    }
                }
            }
            (TokKind::Preproc, "#define") => {
                if let Some(n) = tkns.get(i + 1).filter(|n| n.kind == TokKind::Name && n.line == t.line) {
                    syms.push(Sym {
                        name: n.text.clone(),
                        kind: SymKind::Define,
                        line: n.line,
                        col: n.col,
                        name_len: n.len,
                        end_line: n.line,
                        sig: format!("#define {}", line_tail(chars, n.off)),
                        doc: doc_for(n.line, comments),
                        container: None,
                        in_cond: cond_depth > 0,
                        public: true,
                    });
                }
            }
            (TokKind::Preproc, "#include") => {
                if let Some(inc) = parse_include(tkns, i) {
                    includes.push(inc);
                }
            }
            (TokKind::Keyword, k @ ("struct" | "variant" | "type" | "impl")) if depth == 0 => {
                // `variant "unsafe" Name` のように Str を挟むことがある
                let mut j = i + 1;
                if tkns.get(j).map_or(false, |x| x.kind == TokKind::Str) {
                    j += 1;
                }
                if let Some(n) = tkns.get(j).filter(|n| n.kind == TokKind::Name) {
                    let kind = match k {
                        "struct" => SymKind::Struct,
                        "variant" => SymKind::Variant,
                        "type" => SymKind::Type,
                        _ => SymKind::Impl,
                    };
                    // 本体の `}` の行(`{` は名前の後ろの最初の `{`)
                    let end_line = tkns[j..]
                        .iter()
                        .position(|x| x.is_op("{"))
                        .and_then(|p| matches.get(&(j + p)))
                        .map_or(n.line, |&c| tkns[c].line);
                    let sig = if kind == SymKind::Type {
                        format!("type {}", line_tail(chars, n.off))
                    } else {
                        format!("{} {}", k, n.text)
                    };
                    syms.push(Sym {
                        name: n.text.clone(),
                        kind,
                        line: n.line,
                        col: n.col,
                        name_len: n.len,
                        end_line,
                        sig,
                        doc: doc_for(t.line, comments),
                        container: None,
                        in_cond: cond_depth > 0,
                        public: prev.map_or(false, |p| p.is_kw("pub")),
                    });
                    if kind != SymKind::Type {
                        pending = Some(syms.len() - 1);
                    }
                    i = j; // 名前まで読み飛ばす
                }
            }
            (TokKind::Keyword, k @ ("const" | "static")) if depth == 0 => {
                let mut j = i + 1;
                if tkns.get(j).map_or(false, |x| x.is_kw("mut")) {
                    j += 1;
                }
                if let Some(n) = tkns.get(j).filter(|n| n.kind == TokKind::Name && n.line == t.line) {
                    syms.push(Sym {
                        name: n.text.clone(),
                        kind: SymKind::Var,
                        line: n.line,
                        col: n.col,
                        name_len: n.len,
                        end_line: n.line,
                        sig: format!("{} {}", k, line_tail(chars, n.off)),
                        doc: doc_for(t.line, comments),
                        container: None,
                        in_cond: cond_depth > 0,
                        public: prev.map_or(false, |p| p.is_kw("pub")),
                    });
                }
            }
            (TokKind::Name, _) if top_level => {
                let member_access = prev.map_or(false, |p| p.is_op(".") || p.is_op("->") || p.is_op("::"));
                if !member_access {
                    // 関数 / メゾット
                    if let Some(lb) = func_paren(tkns, i + 1).and_then(|open| body_brace(tkns, open)) {
                        let end_line = matches.get(&lb).map_or(tkns[lb].line, |&c| tkns[c].line);
                        syms.push(Sym {
                            name: t.text.clone(),
                            kind: if at_container { SymKind::Method } else { SymKind::Func },
                            line: t.line,
                            col: t.col,
                            name_len: t.len,
                            end_line,
                            sig: collapse_ws(&chars[t.off..tkns[lb].off].iter().collect::<String>()),
                            doc: doc_for(t.line, comments),
                            container: if at_container { containers.last().map(|c| c.0) } else { None },
                            in_cond: cond_depth > 0,
                            public: prev.map_or(false, |p| p.is_kw("pub")),
                        });
                        // 引数や戻り値の型をメンバーと誤認しないよう、本体の `{` まで進める
                        i = lb;
                        continue;
                    }
                    // メンバー(`name: ty` / variant の `Mem` `Mem(ty)`)
                    if at_container {
                        let (cidx, _, ckind) = *containers.last().unwrap();
                        let starts_member = prev.map_or(true, |p| p.is_op("{") || p.line != t.line);
                        let next = tkns.get(i + 1);
                        let colon = next.map_or(false, |n| n.is_op(":"));
                        let variant_mem = ckind == SymKind::Variant
                            && starts_member
                            && next.map_or(true, |n| {
                                n.line != t.line || n.is_op("(") || n.is_op("}") || n.is_op(",")
                            });
                        if (colon && starts_member) || variant_mem {
                            syms.push(Sym {
                                name: t.text.clone(),
                                kind: SymKind::Field,
                                line: t.line,
                                col: t.col,
                                name_len: t.len,
                                end_line: t.line,
                                sig: line_tail(chars, t.off),
                                doc: doc_for(t.line, comments),
                                container: Some(cidx),
                                in_cond: cond_depth > 0,
                                public: false,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    (syms, includes)
}

/// `#include` の後ろ(同じ行)を読む。
/// `[Name=] ([std::]"path" | a::b) [::func | ::*]`
fn parse_include(tkns: &[Tok], at: usize) -> Option<Include> {
    let line = tkns[at].line;
    let rest: Vec<&Tok> = tkns[at + 1..].iter().take_while(|t| t.line == line).collect();
    let mut k = 0;
    let mut alias = None;
    if rest.len() >= 2 && rest[0].kind == TokKind::Name && rest[1].is_op("=") {
        alias = Some(rest[0].text.clone());
        k = 2;
    }
    let mut is_std = false;
    if rest.get(k).map_or(false, |t| t.kind == TokKind::Name && t.text == "std")
        && rest.get(k + 1).map_or(false, |t| t.is_op("::"))
        && rest.get(k + 2).map_or(false, |t| t.kind == TokKind::Str)
    {
        is_std = true;
        k += 2;
    }
    let first = rest.get(k)?;
    let (path, legacy, mut last, pcol, plen);
    match first.kind {
        TokKind::Str => {
            path = first.text.trim_matches('"').to_string();
            legacy = false;
            last = k;
            pcol = first.col;
            plen = first.len;
        }
        TokKind::Name if alias.is_none() => {
            // 旧書き方 `mod::file`
            let mut segs = vec![first.text.clone()];
            last = k;
            while rest.get(last + 1).map_or(false, |t| t.is_op("::"))
                && rest.get(last + 2).map_or(false, |t| t.kind == TokKind::Name)
            {
                segs.push(rest[last + 2].text.clone());
                last += 2;
            }
            path = segs.join("/") + ".hexl";
            legacy = true;
            pcol = first.col;
            plen = rest[last].col + rest[last].len - first.col;
        }
        _ => return None,
    }
    let mut select = Select::Module;
    if rest.get(last + 1).map_or(false, |t| t.is_op("::")) {
        if let Some(t) = rest.get(last + 2) {
            if t.is_op("*") {
                select = Select::Star;
            } else if t.kind == TokKind::Name {
                select = Select::Func(t.text.clone());
            }
            last += 2;
        }
    }
    let _ = last;
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(&path)
        .trim_end_matches(".hexl")
        .to_string();
    Some(Include {
        line,
        module: alias.clone().unwrap_or(stem),
        path,
        is_std,
        alias,
        select,
        legacy,
        col: pcol,
        len: plen,
    })
}

fn check_duplicates(syms: &[Sym], diags: &mut Vec<Diag>) {
    let mut seen: HashMap<(u8, &str), (usize, bool)> = HashMap::new();
    for s in syms.iter().filter(|s| s.container.is_none()) {
        let class = match s.kind {
            SymKind::Func => 0,
            SymKind::Struct | SymKind::Variant | SymKind::Type => 1,
            SymKind::Var => 2,
            _ => continue,
        };
        match seen.get(&(class, s.name.as_str())) {
            Some(&(first_line, first_cond)) if !first_cond && !s.in_cond => diags.push(Diag {
                line: s.line,
                col: s.col,
                len: s.name_len,
                msg: format!("'{}' が重複して定義されています({}行目)", s.name, first_line + 1),
                severity: 2,
            }),
            Some(_) => {}
            None => {
                seen.insert((class, s.name.as_str()), (s.line, s.in_cond));
            }
        }
    }
}

/// `name` の直後(idx)から、`<..>` を読み飛ばして `(` の位置を返す
pub fn func_paren(tkns: &[Tok], mut idx: usize) -> Option<usize> {
    if tkns.get(idx).map_or(false, |t| t.is_op("<")) {
        let mut d = 0usize;
        while let Some(t) = tkns.get(idx) {
            if t.kind == TokKind::Op {
                match t.text.as_str() {
                    "<" => d += 1,
                    ">" => {
                        d -= 1;
                        if d == 0 {
                            idx += 1;
                            break;
                        }
                    }
                    "{" | "}" | ";" | "(" => return None,
                    _ => {}
                }
            }
            idx += 1;
        }
    }
    if tkns.get(idx).map_or(false, |t| t.is_op("(")) {
        Some(idx)
    } else {
        None
    }
}

/// `(` に対応する `)` の後ろで、最初の `{` の位置を返す(戻り値型などは読み飛ばす)
fn body_brace(tkns: &[Tok], open: usize) -> Option<usize> {
    let mut d = 0usize;
    let mut j = open;
    while let Some(t) = tkns.get(j) {
        if t.kind == TokKind::Op {
            match t.text.as_str() {
                "(" => d += 1,
                ")" => {
                    d = d.checked_sub(1)?;
                    if d == 0 {
                        j += 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        j += 1;
    }
    for k in j..tkns.len().min(j + 40) {
        let t = &tkns[k];
        if t.kind == TokKind::Op {
            match t.text.as_str() {
                "{" => return Some(k),
                "}" | ";" | "=" => return None,
                _ => {}
            }
        }
        if t.kind == TokKind::Keyword || t.kind == TokKind::Preproc {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(r: &ScanResult) -> Vec<(String, SymKind)> {
        r.syms.iter().map(|s| (s.name.clone(), s.kind)).collect()
    }

    #[test]
    fn symbols_and_members() {
        let r = scan(
            "// 点\npub struct P {\n  x: int\n  y: int\n  len(self: P): int {\n    ret self->x\n  }\n}\n\
             impl P { sum(self): int { ret 1 } }\nadd<T>(a: T, b: T): T { ret a }\n",
        );
        let n = names(&r);
        assert!(n.contains(&("P".into(), SymKind::Struct)));
        assert!(n.contains(&("x".into(), SymKind::Field)));
        assert!(n.contains(&("len".into(), SymKind::Method)));
        assert!(n.contains(&("sum".into(), SymKind::Method)));
        assert!(n.contains(&("add".into(), SymKind::Func)));
        // 引数の `self: P` をメンバーと誤認しない
        assert!(!n.iter().any(|(s, k)| s == "self" && *k == SymKind::Field));
        let p = r.syms.iter().find(|s| s.name == "P" && s.kind == SymKind::Struct).unwrap();
        assert_eq!(p.doc, "点");
        assert!(p.public);
        assert_eq!(p.end_line, 7);
        let len = r.syms.iter().find(|s| s.name == "len").unwrap();
        assert_eq!(r.syms[len.container.unwrap()].name, "P");
    }

    #[test]
    fn variant_members() {
        let r = scan("variant V {\n  A\n  B(int)\n}\n");
        let f: Vec<_> = r.syms.iter().filter(|s| s.kind == SymKind::Field).map(|s| s.name.as_str()).collect();
        assert_eq!(f, vec!["A", "B"]);
    }

    #[test]
    fn include_forms() {
        let r = scan(
            "#include \"lib/a.hexl\"\n#include M=\"b.hexl\"\n#include \"c.hexl\"::f\n\
             #include \"d.hexl\"::*\n#include std::\"os/vec.hexl\"\n#include mod::file\n",
        );
        let v = &r.includes;
        assert_eq!(v.len(), 6);
        assert_eq!((v[0].path.as_str(), v[0].module.as_str()), ("lib/a.hexl", "a"));
        assert_eq!(v[1].module, "M");
        assert_eq!(v[2].select, Select::Func("f".into()));
        assert_eq!(v[3].select, Select::Star);
        assert!(v[4].is_std && v[4].module == "vec");
        assert!(v[5].legacy && v[5].path == "mod/file.hexl");
    }

    #[test]
    fn diagnostics() {
        let r = scan("main() {\n  break\n  s = \"x\n");
        let m: Vec<_> = r.diags.iter().map(|d| d.msg.clone()).collect();
        assert!(m.iter().any(|s| s.contains("break")));
        assert!(m.iter().any(|s| s.contains("文字列")));
        assert!(m.iter().any(|s| s.contains("閉じていません")));
        // ループ内の break は問題なし
        let r = scan("main() { loop a > 0 { break } }");
        assert!(r.diags.is_empty(), "{:?}", r.diags);
    }

    #[test]
    fn duplicates_ignore_conditional() {
        let r = scan("f() { }\nf() { }\n");
        assert!(r.diags.iter().any(|d| d.msg.contains("重複")));
        let r = scan("#if #windows\nf() { }\n#else\nf() { }\n#endif\n");
        assert!(r.diags.is_empty(), "{:?}", r.diags);
    }
}
