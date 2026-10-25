//! C11 プリプロセッサ
//!
//! 対応: #include / #define / #undef / #if / #ifdef / #ifndef / #elif / #else / #endif /
//!       #error / #warning / #pragma(once) / #line、`#` `##` 演算子、可変長引数、
//!       `defined`、`__FILE__` `__LINE__` `__DATE__` `__TIME__` `__COUNTER__`、
//!       `, ## __VA_ARGS__`(GNU 拡張)、`__VA_OPT__`、`_Pragma("...")`、
//!       `__has_include` / `__has_include_next`、`#include_next`、
//!       `__INCLUDE_LEVEL__` `__BASE_FILE__`、組み込みの標準ヘッダ(`builtin_headers.rs`)。
//! マクロ展開は Dave Prosser の hide-set アルゴリズム(C 規格の例どおりに動く)。

use super::builtin_headers::builtin_header;
use crate::compiler::c_frontend::lex;
use crate::compiler::c_frontend::lex::lexer::{Lexer, Token, TokenInfo};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

// ====================================================================
// エラー
// ====================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct PpError {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

impl fmt::Display for PpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}: {}", self.file, self.line, self.col, self.msg)
    }
}
impl std::error::Error for PpError {}

// ====================================================================
// 内部トークン
// ====================================================================

#[derive(Debug, Clone)]
struct PpTok {
    token: Token,
    /// ソース上の綴り(`#` 演算子・`##` 演算子で使う)
    spelling: String,
    line: usize,
    col: usize,
    space: bool,
    bol: bool,
    /// このトークンを再展開してはいけないマクロ名の集合
    hide: HashSet<String>,
}

impl PpTok {
    fn from_spanned(s: TokenInfo, src: &[u8]) -> Self {
        let spelling = String::from_utf8_lossy(&src[s.start..s.end]).into_owned();
        PpTok {
            token: s.token,
            spelling,
            line: s.line,
            col: s.col,
            space: s.space_before,
            bol: s.bol,
            hide: HashSet::new(),
        }
    }

    /// `at` の位置・空白情報を引き継いだ合成トークン
    fn synth(token: Token, spelling: String, at: &PpTok) -> Self {
        PpTok {
            token,
            spelling,
            line: at.line,
            col: at.col,
            space: at.space,
            bol: false,
            hide: HashSet::new(),
        }
    }
}

/// 識別子またはキーワードの名前(マクロ名は `#define inline` のようにキーワードでもよい)
fn name_of(t: &PpTok) -> Option<&str> {
    match &t.token {
        Token::Ident { name } | Token::KeyWord { name } => Some(name.as_str()),
        _ => None,
    }
}

fn is_punct(t: &PpTok, p: &str) -> bool {
    matches!(&t.token, Token::Punct { name } if *name == p)
}

fn join_spelling(ts: &[PpTok]) -> String {
    let mut s = String::new();
    for (i, t) in ts.iter().enumerate() {
        if i > 0 && t.space {
            s.push(' ');
        }
        s.push_str(&t.spelling);
    }
    s
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ====================================================================
// マクロ定義
// ====================================================================

#[derive(Debug, Clone)]
struct Macro {
    /// None なら object-like。可変長の場合、最後の要素が可変引数の名前
    params: Option<Vec<String>>,
    variadic: bool,
    body: Vec<PpTok>,
}

fn same_def(a: &Macro, b: &Macro) -> bool {
    a.params == b.params
        && a.variadic == b.variadic
        && a.body.len() == b.body.len()
        && a.body
            .iter()
            .zip(&b.body)
            .all(|(x, y)| x.spelling == y.spelling && x.space == y.space)
}

fn param_index(mac: &Macro, t: &PpTok) -> Option<usize> {
    let ps = mac.params.as_ref()?;
    let n = name_of(t)?;
    ps.iter().position(|p| p == n)
}

/// `#x` : 引数トークン列を文字列リテラルにする
fn stringize(arg: &[PpTok], at: &PpTok) -> PpTok {
    let mut s = String::new();
    for (k, t) in arg.iter().enumerate() {
        if k > 0 && t.space {
            s.push(' ');
        }
        match &t.token {
            Token::StringLiteral { .. } | Token::CharLiteral { .. } => {
                s.push_str(&escape_str(&t.spelling))
            }
            _ => s.push_str(&t.spelling),
        }
    }
    PpTok::synth(
        Token::StringLiteral {
            prefix: String::new(),
            body: s.clone(),
        },
        format!("\"{}\"", s),
        at,
    )
}

// ====================================================================
// 条件ディレクティブのスタック
// ====================================================================

struct Cond {
    parent_active: bool,
    active: bool,
    taken: bool,
    seen_else: bool,
    line: usize,
}

struct Found {
    name: String,
    key: String,
    text: String,
    dir: PathBuf,
}

// ====================================================================
// プリプロセッサ本体
// ====================================================================

#[derive(Debug)]
pub struct Preprocessor {
    macros: HashMap<String, Macro>,
    include_dirs: Vec<PathBuf>,
    system_dirs: Vec<PathBuf>,
    virtual_files: HashMap<String, String>,
    pragma_once: HashSet<String>,
    depth: usize,
    counter: u64,
    cur_file: String,
    base_file: String,
    cur_key: String,
    cur_dir: PathBuf,
    line_delta: i64,
    warnings: Vec<String>,
    pragmas: Vec<String>,
}

impl Default for Preprocessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Preprocessor {
    pub(in crate::compiler::c_frontend) fn new() -> Self {
        let mut p = Preprocessor {
            macros: HashMap::new(),
            include_dirs: Vec::new(),
            system_dirs: Vec::new(),
            virtual_files: HashMap::new(),
            pragma_once: HashSet::new(),
            depth: 0,
            counter: 0,
            cur_file: String::new(),
            base_file: String::new(),
            cur_key: String::new(),
            cur_dir: PathBuf::new(),
            line_delta: 0,
            warnings: Vec::new(),
            pragmas: Vec::new(),
        };
        for (n, v) in [
            ("__STDC__", "1"),
            ("__STDC_VERSION__", "201112L"),
            ("__STDC_HOSTED__", "1"),
            ("__STDC_UTF_16__", "1"),
            ("__STDC_UTF_32__", "1"),
        ] {
            p.define(n, v).expect("builtin macro");
        }
        let (date, time) = build_date_time();
        p.define("__DATE__", &format!("\"{}\"", date))
            .expect("builtin macro");
        p.define("__TIME__", &format!("\"{}\"", time))
            .expect("builtin macro");
        p
    }

    /// `-I dir`(`#include "..."` と `<...>` の両方で探す)
    pub(in crate::compiler::c_frontend) fn add_include_dir(&mut self, dir: impl Into<PathBuf>) {
        self.include_dirs.push(dir.into());
    }

    /// システムヘッダ用ディレクトリ(`-I` の後に探す)
    pub(in crate::compiler::c_frontend) fn add_system_include_dir(
        &mut self,
        dir: impl Into<PathBuf>,
    ) {
        self.system_dirs.push(dir.into());
    }

    /// メモリ上のヘッダを登録する(ファイルより優先。テストや組み込みヘッダ用)
    pub(in crate::compiler::c_frontend) fn add_virtual_file(&mut self, name: &str, contents: &str) {
        self.virtual_files
            .insert(name.to_string(), contents.to_string());
    }

    /// `-DNAME=VALUE` 相当。`define("F(x)", "x+1")` で関数形式も定義できる
    pub(in crate::compiler::c_frontend) fn define(
        &mut self,
        name: &str,
        value: &str,
    ) -> Result<(), PpError> {
        let src = format!("{} {}", name, value);
        let cmd = |e: lex::LexError| PpError {
            file: "<command-line>".into(),
            line: e.line,
            col: e.col,
            msg: e.msg,
        };
        let spanned = Lexer::new_pp(&src).tokenize().map_err(cmd)?;
        let ts: Vec<PpTok> = spanned
            .into_iter()
            .filter(|t| !matches!(t.token, Token::Eof))
            .map(|s| PpTok::from_spanned(s, src.as_bytes()))
            .collect();
        let Some(first) = ts.first() else {
            return Err(PpError {
                file: "<command-line>".into(),
                line: 1,
                col: 1,
                msg: "マクロ名がありません".into(),
            });
        };
        let (n, m) = self.parse_define(first, &ts)?;
        self.macros.insert(n, m);
        Ok(())
    }

    /// `-UNAME` 相当
    pub(in crate::compiler::c_frontend) fn undef(&mut self, name: &str) {
        self.macros.remove(name);
    }

    pub(in crate::compiler::c_frontend::lex) fn is_defined(&self, name: &str) -> bool {
        self.macros.contains_key(name)
            || matches!(
                name,
                "__LINE__"
                    | "__FILE__"
                    | "__COUNTER__"
                    | "__INCLUDE_LEVEL__"
                    | "__BASE_FILE__"
                    | "__has_include"
                    | "__has_include_next"
            )
    }

    /// `#warning` やマクロ再定義などの警告
    pub(in crate::compiler::c_frontend) fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// 解釈しなかった `#pragma` の本文
    pub(in crate::compiler::c_frontend) fn pragmas(&self) -> &[String] {
        &self.pragmas
    }

    // ----------------------------------------------------------------
    // エントリポイント
    // ----------------------------------------------------------------

    /// ソース文字列をプリプロセスする。戻り値は末尾に Eof を含む。
    /// 注意: マクロ定義は呼び出しをまたいで残る(翻訳単位ごとに新しい Preprocessor を使うこと)。
    pub(in crate::compiler::c_frontend) fn preprocess(
        &mut self,
        file_name: &str,
        src: &str,
    ) -> Result<Vec<TokenInfo>, PpError> {
        self.run_main(file_name, file_name, src)
    }

    pub(in crate::compiler::c_frontend) fn preprocess_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Vec<TokenInfo>, PpError> {
        let p = path.as_ref();
        let name = p.to_string_lossy().into_owned();
        let src = std::fs::read_to_string(p).map_err(|e| PpError {
            file: name.clone(),
            line: 0,
            col: 0,
            msg: format!("読み込めません: {}", e),
        })?;
        let key = std::fs::canonicalize(p)
            .map(|c| c.to_string_lossy().into_owned())
            .unwrap_or_else(|_| name.clone());
        self.run_main(&name, &key, &src)
    }

    fn run_main(&mut self, name: &str, key: &str, src: &str) -> Result<Vec<TokenInfo>, PpError> {
        let dir = Path::new(name)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        self.base_file = name.to_string();
        let mut out = Vec::new();
        self.process_file(name, key, dir, src, &mut out)?;

        let mut res = Vec::with_capacity(out.len() + 1);
        for t in out {
            let bad = match &t.token {
                Token::Other { ch } => Some(format!("不正な文字: {:?}", ch)),
                Token::PpNumber { text } => Some(format!("不正な数値トークン: {}", text)),
                Token::HeaderName { name } => Some(format!("不正なヘッダ名: <{}>", name)),
                _ => None,
            };
            if let Some(msg) = bad {
                return Err(PpError {
                    file: name.to_string(),
                    line: t.line,
                    col: t.col,
                    msg,
                });
            }
            res.push(TokenInfo {
                token: t.token,
                line: t.line,
                col: t.col,
                start: 0,
                end: 0,
                bol: t.bol,
                space_before: t.space,
            });
        }
        res.push(TokenInfo {
            token: Token::Eof,
            line: 0,
            col: 0,
            start: 0,
            end: 0,
            bol: true,
            space_before: true,
        });
        Ok(res)
    }

    fn err(&self, t: &PpTok, msg: impl Into<String>) -> PpError {
        PpError {
            file: self.cur_file.clone(),
            line: t.line,
            col: t.col,
            msg: msg.into(),
        }
    }

    // ----------------------------------------------------------------
    // ファイル処理
    // ----------------------------------------------------------------

    fn process_file(
        &mut self,
        name: &str,
        key: &str,
        dir: PathBuf,
        src: &str,
        out: &mut Vec<PpTok>,
    ) -> Result<(), PpError> {
        if self.depth >= 200 {
            return Err(PpError {
                file: name.to_string(),
                line: 1,
                col: 1,
                msg: "#include のネストが深すぎます".into(),
            });
        }
        let tokens = Lexer::new_pp(src).tokenize().map_err(|e| PpError {
            file: name.to_string(),
            line: e.line,
            col: e.col,
            msg: e.msg,
        })?;
        let bytes = src.as_bytes();
        let toks: Vec<PpTok> = tokens
            .into_iter()
            .map(|s| PpTok::from_spanned(s, bytes))
            .collect();

        let saved_file = std::mem::replace(&mut self.cur_file, name.to_string());
        let saved_key = std::mem::replace(&mut self.cur_key, key.to_string());
        let saved_dir = std::mem::replace(&mut self.cur_dir, dir);
        let saved_delta = std::mem::replace(&mut self.line_delta, 0);
        self.depth += 1;

        let r = self.run_file(&toks, out);

        self.depth -= 1;
        self.cur_file = saved_file;
        self.cur_key = saved_key;
        self.cur_dir = saved_dir;
        self.line_delta = saved_delta;
        r
    }

    fn run_file(&mut self, toks: &[PpTok], out: &mut Vec<PpTok>) -> Result<(), PpError> {
        let mut conds: Vec<Cond> = Vec::new();
        let mut text: Vec<PpTok> = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            if matches!(t.token, Token::Eof) {
                break;
            }
            if t.bol && is_punct(t, "#") {
                let mut j = i + 1;
                while j < toks.len() && !toks[j].bol && !matches!(toks[j].token, Token::Eof) {
                    j += 1;
                }
                // ディレクティブの前までのテキストは、その時点のマクロ定義で展開する
                self.flush_text(&mut text, out)?;
                self.directive(t, &toks[i + 1..j], &mut conds, out)?;
                i = j;
                continue;
            }
            if conds.last().map_or(true, |c| c.active) {
                text.push(t.clone());
            }
            i += 1;
        }
        self.flush_text(&mut text, out)?;
        if let Some(c) = conds.last() {
            return Err(PpError {
                file: self.cur_file.clone(),
                line: c.line,
                col: 1,
                msg: "#if に対応する #endif がありません".into(),
            });
        }
        Ok(())
    }

    fn flush_text(&mut self, text: &mut Vec<PpTok>, out: &mut Vec<PpTok>) -> Result<(), PpError> {
        if text.is_empty() {
            return Ok(());
        }
        let expanded = self.expand(std::mem::take(text))?;
        out.extend(expanded);
        Ok(())
    }

    // ----------------------------------------------------------------
    // ディレクティブ
    // ----------------------------------------------------------------

    fn directive(
        &mut self,
        hash: &PpTok,
        line: &[PpTok],
        conds: &mut Vec<Cond>,
        out: &mut Vec<PpTok>,
    ) -> Result<(), PpError> {
        let Some(first) = line.first() else {
            return Ok(()); // 空ディレクティブ `#`
        };
        let name = name_of(first).unwrap_or("").to_string();
        let rest = &line[1..];
        let active = conds.last().map_or(true, |c| c.active);

        match name.as_str() {
            "if" | "ifdef" | "ifndef" => {
                if !active {
                    // スキップ中のネストは評価せず、else/elif でも有効にならないようにする
                    conds.push(Cond {
                        parent_active: false,
                        active: false,
                        taken: true,
                        seen_else: false,
                        line: hash.line,
                    });
                    return Ok(());
                }
                let v = if name == "if" {
                    self.eval_if(hash, rest)?
                } else {
                    let Some(m) = rest.first().and_then(name_of) else {
                        return Err(self.err(hash, "マクロ名がありません"));
                    };
                    self.is_defined(m) == (name == "ifdef")
                };
                conds.push(Cond {
                    parent_active: true,
                    active: v,
                    taken: v,
                    seen_else: false,
                    line: hash.line,
                });
                Ok(())
            }
            "elif" => {
                let Some(c) = conds.last_mut() else {
                    return Err(self.err(hash, "#elif に対応する #if がありません"));
                };
                if c.seen_else {
                    return Err(self.err(hash, "#else の後に #elif があります"));
                }
                if !c.parent_active || c.taken {
                    c.active = false;
                } else {
                    let v = self.eval_if(hash, rest)?;
                    c.active = v;
                    c.taken = v;
                }
                Ok(())
            }
            "else" => {
                let Some(c) = conds.last_mut() else {
                    return Err(self.err(hash, "#else に対応する #if がありません"));
                };
                if c.seen_else {
                    return Err(self.err(hash, "#else が重複しています"));
                }
                c.seen_else = true;
                c.active = c.parent_active && !c.taken;
                c.taken = true;
                Ok(())
            }
            "endif" => {
                if conds.pop().is_none() {
                    return Err(self.err(hash, "#endif に対応する #if がありません"));
                }
                Ok(())
            }
            _ if !active => Ok(()),
            "define" => {
                let (n, m) = self.parse_define(hash, rest)?;
                self.add_macro(hash, n, m);
                Ok(())
            }
            "undef" => {
                let Some(n) = rest.first().and_then(name_of) else {
                    return Err(self.err(hash, "マクロ名がありません"));
                };
                self.macros.remove(n);
                Ok(())
            }
            "include" => self.do_include(hash, rest, false, out),
            "include_next" => self.do_include(hash, rest, true, out),
            "error" => Err(self.err(hash, format!("#error {}", join_spelling(rest)))),
            "warning" => {
                self.warnings.push(format!(
                    "{}:{}: warning: {}",
                    self.cur_file,
                    hash.line,
                    join_spelling(rest)
                ));
                Ok(())
            }
            "pragma" => {
                self.handle_pragma(rest);
                Ok(())
            }
            "line" => {
                let end_line = line.last().map_or(hash.line, |t| t.line);
                self.do_line(hash, end_line, rest)
            }
            "ident" | "sccs" => Ok(()),
            _ => Err(self.err(first, format!("不明なディレクティブ: #{}", name))),
        }
    }

    fn add_macro(&mut self, at: &PpTok, name: String, m: Macro) {
        if let Some(old) = self.macros.get(&name) {
            if !same_def(old, &m) {
                self.warnings.push(format!(
                    "{}:{}: warning: マクロ {} が再定義されました",
                    self.cur_file, at.line, name
                ));
            }
        }
        self.macros.insert(name, m);
    }

    fn parse_define(&self, hash: &PpTok, rest: &[PpTok]) -> Result<(String, Macro), PpError> {
        let Some(nt) = rest.first() else {
            return Err(self.err(hash, "マクロ名がありません"));
        };
        let Some(name) = name_of(nt) else {
            return Err(self.err(nt, "マクロ名は識別子でなければなりません"));
        };
        let name = name.to_string();
        if name == "defined" {
            return Err(self.err(nt, "defined はマクロ名にできません"));
        }

        let mut idx = 1;
        let mut params: Option<Vec<String>> = None;
        let mut variadic = false;

        // 名前の直後(空白なし)に '(' があれば関数形式
        if rest.get(1).map_or(false, |t| is_punct(t, "(") && !t.space) {
            let mut ps: Vec<String> = Vec::new();
            let mut expect_param = true;
            idx = 2;
            loop {
                let Some(t) = rest.get(idx) else {
                    return Err(self.err(nt, "仮引数リストが閉じられていません"));
                };
                idx += 1;
                if is_punct(t, ")") {
                    if expect_param && !ps.is_empty() {
                        return Err(self.err(t, "仮引数が不正です"));
                    }
                    break;
                }
                if !expect_param {
                    if is_punct(t, ",") {
                        expect_param = true;
                        continue;
                    }
                    return Err(self.err(t, "仮引数リストが不正です"));
                }
                if is_punct(t, "...") {
                    ps.push("__VA_ARGS__".to_string());
                    variadic = true;
                    if !rest.get(idx).map_or(false, |n| is_punct(n, ")")) {
                        return Err(self.err(t, "'...' の後には ')' が必要です"));
                    }
                    expect_param = false;
                    continue;
                }
                let Some(n) = name_of(t) else {
                    return Err(self.err(t, "仮引数は識別子でなければなりません"));
                };
                if ps.iter().any(|p| p == n) {
                    return Err(self.err(t, format!("仮引数 {} が重複しています", n)));
                }
                ps.push(n.to_string());
                expect_param = false;
                // GNU 形式 `args...`
                if rest.get(idx).map_or(false, |n| is_punct(n, "...")) {
                    variadic = true;
                    idx += 1;
                    if !rest.get(idx).map_or(false, |n| is_punct(n, ")")) {
                        return Err(self.err(t, "'...' の後には ')' が必要です"));
                    }
                }
            }
            params = Some(ps);
        }

        let mut body: Vec<PpTok> = rest[idx..].to_vec();
        for t in body.iter_mut() {
            t.bol = false;
            t.hide.clear();
        }
        if let Some(f) = body.first_mut() {
            f.space = false;
        }

        let mac = Macro {
            params,
            variadic,
            body,
        };
        // 仕様どおりの検証
        if let Some(l) = mac.body.last() {
            if is_punct(l, "##") {
                return Err(self.err(l, "'##' がマクロ本体の末尾にあります"));
            }
        }
        if let Some(f) = mac.body.first() {
            if is_punct(f, "##") {
                return Err(self.err(f, "'##' がマクロ本体の先頭にあります"));
            }
        }
        if mac.params.is_some() {
            for (i, t) in mac.body.iter().enumerate() {
                if is_punct(t, "#")
                    && !mac
                        .body
                        .get(i + 1)
                        .map_or(false, |n| param_index(&mac, n).is_some())
                {
                    return Err(self.err(t, "'#' の後にはマクロ仮引数が必要です"));
                }
            }
        }
        Ok((name, mac))
    }

    // ----------------------------------------------------------------
    // #include / #line
    // ----------------------------------------------------------------

    /// `#pragma` の本文を処理する。`once` はここで消費し、それ以外は `pragmas()` に積む
    fn handle_pragma(&mut self, rest: &[PpTok]) {
        if rest.first().and_then(name_of) == Some("once") {
            self.pragma_once.insert(self.cur_key.clone());
        } else {
            self.pragmas.push(join_spelling(rest));
        }
    }

    /// `#include` / `#include_next` / `__has_include` の引数を (名前, 山括弧か) にする。
    /// `#include MACRO` の場合は展開してから解釈する。書式が不正なら Err(メッセージ)
    fn include_target(&mut self, rest: &[PpTok]) -> Result<(String, bool), String> {
        match rest.first().map(|t| &t.token) {
            Some(Token::HeaderName { name }) => Ok((name.clone(), true)),
            Some(Token::StringLiteral { prefix, body }) if prefix.is_empty() => {
                Ok((body.clone(), false))
            }
            _ => {
                let ex = self.expand(rest.to_vec()).map_err(|e| e.msg)?;
                match ex.first().map(|t| &t.token) {
                    Some(Token::StringLiteral { prefix, body }) if prefix.is_empty() => {
                        Ok((body.clone(), false))
                    }
                    Some(Token::Punct { name: "<" }) => {
                        let mut s = String::new();
                        for (k, t) in ex.iter().enumerate().skip(1) {
                            if is_punct(t, ">") {
                                return Ok((s, true));
                            }
                            if k > 1 && t.space {
                                s.push(' ');
                            }
                            s.push_str(&t.spelling);
                        }
                        Err("#include の '>' がありません".to_string())
                    }
                    _ => Err("#include の書式が不正です".to_string()),
                }
            }
        }
    }

    fn do_include(
        &mut self,
        hash: &PpTok,
        rest: &[PpTok],
        next: bool,
        out: &mut Vec<PpTok>,
    ) -> Result<(), PpError> {
        let (name, angle) = self.include_target(rest).map_err(|m| self.err(hash, m))?;
        let Some(f) = self.find_include(&name, angle, next) else {
            return Err(self.err(
                hash,
                format!("インクルードファイルが見つかりません: {}", name),
            ));
        };
        if self.pragma_once.contains(&f.key) {
            return Ok(());
        }
        self.process_file(&f.name, &f.key, f.dir, &f.text, out)
    }

    /// `next` が真なら `#include_next`: 現在のファイルが見つかった検索ディレクトリより後ろだけを探す
    fn find_include(&self, name: &str, angle: bool, next: bool) -> Option<Found> {
        if !next {
            if let Some(text) = self.virtual_files.get(name) {
                return Some(Found {
                    name: name.to_string(),
                    key: format!("virt:{}", name),
                    text: text.clone(),
                    dir: PathBuf::new(),
                });
            }
        }
        let search: Vec<&PathBuf> = self
            .include_dirs
            .iter()
            .chain(self.system_dirs.iter())
            .collect();
        let mut start = 0;
        if next {
            // 現在のファイルのあるディレクトリが検索リストにあれば、その次から探す
            if let Some(i) = search.iter().position(|d| **d == self.cur_dir) {
                start = i + 1;
            }
        }
        let mut cands: Vec<PathBuf> = Vec::new();
        if !angle && !next {
            cands.push(self.cur_dir.join(name));
        }
        for d in &search[start..] {
            cands.push(d.join(name));
        }
        for p in cands {
            let s = p.to_string_lossy().into_owned();
            if let Some(text) = self.virtual_files.get(&s) {
                return Some(Found {
                    name: s.clone(),
                    key: format!("virt:{}", s),
                    text: text.clone(),
                    dir: p.parent().map(Path::to_path_buf).unwrap_or_default(),
                });
            }
            if let Ok(text) = std::fs::read_to_string(&p) {
                let key = std::fs::canonicalize(&p)
                    .map(|c| c.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| s.clone());
                return Some(Found {
                    name: s,
                    key,
                    text,
                    dir: p.parent().map(Path::to_path_buf).unwrap_or_default(),
                });
            }
        }
        // 最後に組み込みの標準ヘッダ(利用者の `-I` が優先される)
        if angle || !next {
            if let Some(text) = builtin_header(name) {
                return Some(Found {
                    name: format!("<{}>", name),
                    key: format!("builtin:{}", name),
                    text: text.to_string(),
                    dir: PathBuf::new(),
                });
            }
        }
        None
    }

    fn do_line(&mut self, hash: &PpTok, end_line: usize, rest: &[PpTok]) -> Result<(), PpError> {
        let ex = self.expand(rest.to_vec())?;
        let n = match ex.first().map(|t| &t.token) {
            Some(Token::IntLiteral { value, .. }) => *value,
            _ => return Err(self.err(hash, "#line の行番号が不正です")),
        };
        if let Some(Token::StringLiteral { prefix, body }) = ex.get(1).map(|t| &t.token) {
            if prefix.is_empty() {
                self.cur_file = body.clone();
            }
        }
        // 次の行が n 行目になるように補正する
        self.line_delta = n as i64 - (end_line as i64 + 1);
        Ok(())
    }

    // ----------------------------------------------------------------
    // #if の式評価
    // ----------------------------------------------------------------

    fn eval_if(&mut self, hash: &PpTok, rest: &[PpTok]) -> Result<bool, PpError> {
        // 1. `defined X` / `defined(X)` を 0/1 に置換(マクロ展開より先に行う)
        let mut toks: Vec<PpTok> = Vec::new();
        let mut i = 0;
        while i < rest.len() {
            let t = &rest[i];
            if matches!(
                name_of(t),
                Some("__has_include") | Some("__has_include_next")
            ) {
                let next = name_of(t) == Some("__has_include_next");
                if !rest.get(i + 1).map_or(false, |n| is_punct(n, "(")) {
                    return Err(self.err(t, "__has_include の後に '(' がありません"));
                }
                // 対応する ')' を探す
                let mut depth = 0usize;
                let mut close = None;
                for (k, x) in rest.iter().enumerate().skip(i + 1) {
                    if is_punct(x, "(") {
                        depth += 1;
                    } else if is_punct(x, ")") {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(k);
                            break;
                        }
                    }
                }
                let Some(close) = close else {
                    return Err(self.err(t, "__has_include( に対応する ')' がありません"));
                };
                let inner = &rest[i + 2..close];
                // `#if` の中では `<a.h>` は HeaderName にならないので、`<` ... `>` を自前で組み立てる
                let target = match inner.first().map(|x| &x.token) {
                    Some(Token::Punct { name: "<" }) => {
                        let mut s = String::new();
                        let mut ok = false;
                        for (k, x) in inner.iter().enumerate().skip(1) {
                            if is_punct(x, ">") {
                                ok = true;
                                break;
                            }
                            if k > 1 && x.space {
                                s.push(' ');
                            }
                            s.push_str(&x.spelling);
                        }
                        if !ok {
                            return Err(self.err(t, "__has_include の '>' がありません"));
                        }
                        (s, true)
                    }
                    _ => self.include_target(inner).map_err(|m| self.err(t, m))?,
                };
                let v = self.find_include(&target.0, target.1, next).is_some() as u64;
                toks.push(PpTok::synth(
                    Token::IntLiteral {
                        value: v,
                        suffix: String::new(),
                    },
                    v.to_string(),
                    t,
                ));
                i = close + 1;
                continue;
            }
            if name_of(t) == Some("defined") {
                let paren = rest.get(i + 1).map_or(false, |n| is_punct(n, "("));
                let name_idx = if paren { i + 2 } else { i + 1 };
                let Some(nm) = rest.get(name_idx).and_then(name_of) else {
                    return Err(self.err(t, "defined の後にマクロ名がありません"));
                };
                let mut next = name_idx + 1;
                if paren {
                    if !rest.get(next).map_or(false, |n| is_punct(n, ")")) {
                        return Err(self.err(t, "defined( に対応する ')' がありません"));
                    }
                    next += 1;
                }
                let v = self.is_defined(nm) as u64;
                toks.push(PpTok::synth(
                    Token::IntLiteral {
                        value: v,
                        suffix: String::new(),
                    },
                    v.to_string(),
                    t,
                ));
                i = next;
            } else {
                toks.push(t.clone());
                i += 1;
            }
        }
        // 2. マクロ展開
        let toks = self.expand(toks)?;
        if toks.is_empty() {
            return Err(self.err(hash, "#if の式がありません"));
        }
        // 3. 評価(残った識別子は 0)
        let mut ev = Ev {
            toks: &toks,
            pos: 0,
        };
        let v = ev.ternary(true).map_err(|m| self.err(hash, m))?;
        if ev.pos != toks.len() {
            return Err(self.err(hash, "#if の式が不正です"));
        }
        Ok(v.v != 0)
    }

    // ----------------------------------------------------------------
    // マクロ展開 (Prosser のアルゴリズム)
    // ----------------------------------------------------------------

    fn dynamic(&mut self, name: &str, t: &PpTok) -> Option<PpTok> {
        match name {
            "__LINE__" => {
                let n = (t.line as i64 + self.line_delta).max(0) as u64;
                Some(PpTok::synth(
                    Token::IntLiteral {
                        value: n,
                        suffix: String::new(),
                    },
                    n.to_string(),
                    t,
                ))
            }
            "__FILE__" => {
                let esc = escape_str(&self.cur_file);
                Some(PpTok::synth(
                    Token::StringLiteral {
                        prefix: String::new(),
                        body: esc.clone(),
                    },
                    format!("\"{}\"", esc),
                    t,
                ))
            }
            "__INCLUDE_LEVEL__" => {
                let n = self.depth.saturating_sub(1) as u64;
                Some(PpTok::synth(
                    Token::IntLiteral {
                        value: n,
                        suffix: String::new(),
                    },
                    n.to_string(),
                    t,
                ))
            }
            "__BASE_FILE__" => {
                let esc = escape_str(&self.base_file);
                Some(PpTok::synth(
                    Token::StringLiteral {
                        prefix: String::new(),
                        body: esc.clone(),
                    },
                    format!("\"{}\"", esc),
                    t,
                ))
            }
            "__COUNTER__" => {
                let n = self.counter;
                self.counter += 1;
                Some(PpTok::synth(
                    Token::IntLiteral {
                        value: n,
                        suffix: String::new(),
                    },
                    n.to_string(),
                    t,
                ))
            }
            _ => None,
        }
    }

    fn expand(&mut self, input: Vec<PpTok>) -> Result<Vec<PpTok>, PpError> {
        let mut q: VecDeque<PpTok> = input.into();
        let mut out: Vec<PpTok> = Vec::new();
        while let Some(t) = q.pop_front() {
            let Some(name) = name_of(&t).map(str::to_string) else {
                out.push(t);
                continue;
            };
            if t.hide.contains(&name) {
                out.push(t);
                continue;
            }
            if let Some(rep) = self.dynamic(&name, &t) {
                out.push(rep);
                continue;
            }
            // `_Pragma("...")` : #pragma と同じに扱い、トークンは出力しない
            if name == "_Pragma" {
                let ok = q.front().map_or(false, |n| is_punct(n, "("))
                    && matches!(
                        q.get(1).map(|n| &n.token),
                        Some(Token::StringLiteral { .. })
                    )
                    && q.get(2).map_or(false, |n| is_punct(n, ")"));
                if !ok {
                    return Err(self.err(&t, "_Pragma の後には (\"文字列\") が必要です"));
                }
                q.pop_front();
                let lit = q.pop_front().unwrap();
                q.pop_front();
                let Token::StringLiteral { body, .. } = &lit.token else {
                    unreachable!()
                };
                // destringize: `\\` -> `\`, `\"` -> `"`
                let text = body.replace("\\\"", "\"").replace("\\\\", "\\");
                let sub = Lexer::new_pp(&text)
                    .tokenize()
                    .map_err(|e| self.err(&t, e.msg))?;
                let toks: Vec<PpTok> = sub
                    .into_iter()
                    .filter(|x| !matches!(x.token, Token::Eof))
                    .map(|x| PpTok::from_spanned(x, text.as_bytes()))
                    .collect();
                self.handle_pragma(&toks);
                continue;
            }
            let Some(mac) = self.macros.get(&name).cloned() else {
                out.push(t);
                continue;
            };

            let (args, hs) = match &mac.params {
                None => {
                    let mut h = t.hide.clone();
                    h.insert(name.clone());
                    (Vec::new(), h)
                }
                Some(params) => {
                    // 関数形式マクロは直後が '(' のときだけ呼び出しになる
                    if !q.front().map_or(false, |n| is_punct(n, "(")) {
                        out.push(t);
                        continue;
                    }
                    q.pop_front();
                    let np = params.len();
                    let (mut args, close) = self.collect_args(&mut q, &t, np, mac.variadic)?;
                    if np == 0 {
                        if args.len() == 1 && args[0].is_empty() {
                            args.clear();
                        } else {
                            return Err(self.err(&t, format!("マクロ {} は引数を取りません", name)));
                        }
                    } else {
                        if mac.variadic && args.len() == np - 1 {
                            args.push(Vec::new());
                        }
                        if args.len() != np {
                            return Err(self.err(
                                &t,
                                format!(
                                    "マクロ {} の引数の数が違います (期待 {}, 実際 {})",
                                    name,
                                    np,
                                    args.len()
                                ),
                            ));
                        }
                    }
                    let mut h: HashSet<String> =
                        t.hide.intersection(&close.hide).cloned().collect();
                    h.insert(name.clone());
                    (args, h)
                }
            };

            let body = self.subst(&mac, &args, &hs, &t)?;
            for tok in body.into_iter().rev() {
                q.push_front(tok);
            }
        }
        Ok(out)
    }

    /// '(' の直後から対応する ')' までを引数ごとに分ける。(引数列, ')' トークン)
    fn collect_args(
        &self,
        q: &mut VecDeque<PpTok>,
        name_tok: &PpTok,
        nparams: usize,
        variadic: bool,
    ) -> Result<(Vec<Vec<PpTok>>, PpTok), PpError> {
        let mut args: Vec<Vec<PpTok>> = vec![Vec::new()];
        let mut depth = 0usize;
        loop {
            let Some(t) = q.pop_front() else {
                return Err(self.err(name_tok, "マクロ呼び出しの ')' がありません"));
            };
            if is_punct(&t, "(") {
                depth += 1;
            } else if is_punct(&t, ")") {
                if depth == 0 {
                    return Ok((args, t));
                }
                depth -= 1;
            } else if is_punct(&t, ",") && depth == 0 && !(variadic && args.len() == nparams) {
                args.push(Vec::new());
                continue;
            }
            args.last_mut().unwrap().push(t);
        }
    }

    fn subst(
        &mut self,
        mac: &Macro,
        args: &[Vec<PpTok>],
        hs: &HashSet<String>,
        site: &PpTok,
    ) -> Result<Vec<PpTok>, PpError> {
        let resolved = self.resolve_va_opt(mac, args)?;
        let body = &resolved;
        let nparams = mac.params.as_ref().map_or(0, |v| v.len());
        let mut os: Vec<PpTok> = Vec::new();
        let mut i = 0;
        while i < body.len() {
            let t = &body[i];

            // `# param`
            if mac.params.is_some() && is_punct(t, "#") {
                if let Some(p) = body.get(i + 1).and_then(|n| param_index(mac, n)) {
                    os.push(stringize(&args[p], site));
                    i += 2;
                    continue;
                }
            }

            // `## rhs`
            if is_punct(t, "##") {
                if let Some(next) = body.get(i + 1) {
                    if let Some(p) = param_index(mac, next) {
                        let a = &args[p];
                        let is_va = mac.variadic && p + 1 == nparams;
                        if is_va && os.last().map_or(false, |l| is_punct(l, ",")) {
                            // GNU 拡張 `, ## __VA_ARGS__` : 空なら ',' ごと消す
                            if a.is_empty() {
                                os.pop();
                            } else {
                                os.extend(a.iter().cloned());
                            }
                        } else if !a.is_empty() {
                            self.glue(&mut os, a)?;
                        }
                    } else {
                        self.glue(&mut os, std::slice::from_ref(next))?;
                    }
                    i += 2;
                    continue;
                }
            }

            if let Some(p) = param_index(mac, t) {
                // `param ## ...` : 引数は展開せずそのまま使う
                if body.get(i + 1).map_or(false, |n| is_punct(n, "##")) {
                    if args[p].is_empty() {
                        if let Some(p2) = body.get(i + 2).and_then(|n| param_index(mac, n)) {
                            os.extend(args[p2].iter().cloned());
                            i += 3;
                        } else {
                            i += 2;
                        }
                    } else {
                        os.extend(args[p].iter().cloned());
                        i += 1;
                    }
                    continue;
                }
                // 通常の仮引数: 完全にマクロ展開した引数で置き換える
                let ex = self.expand(args[p].clone())?;
                os.extend(ex);
                i += 1;
                continue;
            }

            let mut c = t.clone();
            c.line = site.line;
            c.col = site.col;
            os.push(c);
            i += 1;
        }

        for t in os.iter_mut() {
            t.hide.extend(hs.iter().cloned());
            t.bol = false;
        }
        if let Some(f) = os.first_mut() {
            f.space = site.space;
        }
        Ok(os)
    }

    /// 可変引数マクロの本体にある `__VA_OPT__(...)` を解決する。
    /// 可変引数が空でなければ括弧の中身に置き換え、空なら取り除く。
    fn resolve_va_opt(&self, mac: &Macro, args: &[Vec<PpTok>]) -> Result<Vec<PpTok>, PpError> {
        let body = &mac.body;
        if !mac.variadic || !body.iter().any(|t| name_of(t) == Some("__VA_OPT__")) {
            return Ok(body.clone());
        }
        let has_va = args.last().map_or(false, |a| !a.is_empty());
        let mut out: Vec<PpTok> = Vec::with_capacity(body.len());
        let mut i = 0;
        while i < body.len() {
            let t = &body[i];
            if name_of(t) != Some("__VA_OPT__") {
                out.push(t.clone());
                i += 1;
                continue;
            }
            if !body.get(i + 1).map_or(false, |n| is_punct(n, "(")) {
                return Err(self.err(t, "__VA_OPT__ の後に '(' がありません"));
            }
            let mut depth = 0usize;
            let mut close = None;
            for (k, x) in body.iter().enumerate().skip(i + 1) {
                if is_punct(x, "(") {
                    depth += 1;
                } else if is_punct(x, ")") {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(k);
                        break;
                    }
                }
            }
            let Some(close) = close else {
                return Err(self.err(t, "__VA_OPT__( に対応する ')' がありません"));
            };
            if has_va {
                let mut inner: Vec<PpTok> = body[i + 2..close].to_vec();
                if let Some(f) = inner.first_mut() {
                    f.space = t.space;
                }
                out.extend(inner);
            }
            i = close + 1;
        }
        Ok(out)
    }

    /// `os` の末尾トークンと `rs` の先頭トークンを連結(`##`)
    fn glue(&self, os: &mut Vec<PpTok>, rs: &[PpTok]) -> Result<(), PpError> {
        let Some(first) = rs.first() else {
            return Ok(());
        };
        match os.pop() {
            None => os.extend(rs.iter().cloned()),
            Some(l) => {
                let pasted = self.paste(&l, first)?;
                os.push(pasted);
                os.extend(rs[1..].iter().cloned());
            }
        }
        Ok(())
    }

    fn paste(&self, l: &PpTok, r: &PpTok) -> Result<PpTok, PpError> {
        let text = format!("{}{}", l.spelling, r.spelling);
        let toks = Lexer::new_pp(&text)
            .tokenize()
            .map_err(|e| self.err(l, e.msg))?;
        let valid = toks.len() == 2
            && !matches!(toks[0].token, Token::Other { .. } | Token::PpNumber { .. });
        if !valid {
            return Err(self.err(
                l,
                format!(
                    "'{}' と '{}' の連結は有効なトークンになりません",
                    l.spelling, r.spelling
                ),
            ));
        }
        let hide = l.hide.intersection(&r.hide).cloned().collect();
        Ok(PpTok {
            token: toks[0].token.clone(),
            spelling: text,
            line: l.line,
            col: l.col,
            space: l.space,
            bol: false,
            hide,
        })
    }
}

// ====================================================================
// #if 用の整数定数式
// ====================================================================

#[derive(Clone, Copy)]
struct Val {
    v: i64,
    u: bool,
}

struct Ev<'a> {
    toks: &'a [PpTok],
    pos: usize,
}

type R<T> = Result<T, String>;

fn prec(op: &str) -> Option<u8> {
    Some(match op {
        "||" => 1,
        "&&" => 2,
        "|" => 3,
        "^" => 4,
        "&" => 5,
        "==" | "!=" => 6,
        "<" | ">" | "<=" | ">=" => 7,
        "<<" | ">>" => 8,
        "+" | "-" => 9,
        "*" | "/" | "%" => 10,
        _ => return None,
    })
}

impl<'a> Ev<'a> {
    fn peek_punct(&self) -> Option<&'static str> {
        match self.toks.get(self.pos).map(|t| &t.token) {
            Some(Token::Punct { name }) => Some(*name),
            _ => None,
        }
    }

    fn eat(&mut self, p: &str) -> bool {
        if self.peek_punct() == Some(p) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// `live` が false の部分(短絡で評価されない側)ではゼロ除算をエラーにしない
    fn ternary(&mut self, live: bool) -> R<Val> {
        let c = self.binary(1, live)?;
        if self.eat("?") {
            let cond = c.v != 0;
            let a = self.ternary(live && cond)?;
            if !self.eat(":") {
                return Err("条件式に ':' がありません".into());
            }
            let b = self.ternary(live && !cond)?;
            let r = if cond { a } else { b };
            Ok(Val {
                v: r.v,
                u: a.u || b.u,
            })
        } else {
            Ok(c)
        }
    }

    fn binary(&mut self, min: u8, live: bool) -> R<Val> {
        let mut lhs = self.unary(live)?;
        loop {
            let Some(op) = self.peek_punct() else { break };
            let p = match prec(op) {
                Some(p) if p >= min => p,
                _ => break,
            };
            self.pos += 1;
            let rlive = match op {
                "&&" => live && lhs.v != 0,
                "||" => live && lhs.v == 0,
                _ => live,
            };
            let rhs = self.binary(p + 1, rlive)?;
            lhs = apply(op, lhs, rhs, live)?;
        }
        Ok(lhs)
    }

    fn unary(&mut self, live: bool) -> R<Val> {
        if self.eat("+") {
            return self.unary(live);
        }
        if self.eat("-") {
            let x = self.unary(live)?;
            return Ok(Val {
                v: x.v.wrapping_neg(),
                u: x.u,
            });
        }
        if self.eat("~") {
            let x = self.unary(live)?;
            return Ok(Val { v: !x.v, u: x.u });
        }
        if self.eat("!") {
            let x = self.unary(live)?;
            return Ok(Val {
                v: (x.v == 0) as i64,
                u: false,
            });
        }
        self.primary(live)
    }

    fn primary(&mut self, live: bool) -> R<Val> {
        let Some(t) = self.toks.get(self.pos) else {
            return Err("式が途中で終わっています".into());
        };
        self.pos += 1;
        match &t.token {
            Token::IntLiteral { value, suffix } => Ok(Val {
                v: *value as i64,
                u: *value > i64::MAX as u64 || suffix.to_ascii_lowercase().contains('u'),
            }),
            Token::CharLiteral { prefix, body } => char_value(prefix, body)
                .map(|v| Val { v, u: false })
                .ok_or_else(|| format!("不正な文字定数: {}", t.spelling)),
            // マクロ展開後に残った識別子・キーワードは 0
            Token::Ident { .. } | Token::KeyWord { .. } => Ok(Val { v: 0, u: false }),
            Token::Punct { name: "(" } => {
                let v = self.ternary(live)?;
                if !self.eat(")") {
                    return Err("')' がありません".into());
                }
                Ok(v)
            }
            Token::FloatLiteral { .. } => Err("浮動小数点定数は #if で使えません".into()),
            Token::StringLiteral { .. } => Err("文字列リテラルは #if で使えません".into()),
            _ => Err(format!("#if の式が不正です: '{}'", t.spelling)),
        }
    }
}

fn apply(op: &str, l: Val, r: Val, live: bool) -> R<Val> {
    let u = l.u || r.u;
    let b = |x: bool| Val {
        v: x as i64,
        u: false,
    };
    let cmp = |f: fn(std::cmp::Ordering) -> bool| {
        let o = if u {
            (l.v as u64).cmp(&(r.v as u64))
        } else {
            l.v.cmp(&r.v)
        };
        b(f(o))
    };
    Ok(match op {
        "||" => b(l.v != 0 || r.v != 0),
        "&&" => b(l.v != 0 && r.v != 0),
        "==" => b(l.v == r.v),
        "!=" => b(l.v != r.v),
        "<" => cmp(|o| o.is_lt()),
        ">" => cmp(|o| o.is_gt()),
        "<=" => cmp(|o| o.is_le()),
        ">=" => cmp(|o| o.is_ge()),
        "<<" => Val {
            v: if (0..64).contains(&r.v) {
                l.v.wrapping_shl(r.v as u32)
            } else {
                0
            },
            u: l.u,
        },
        ">>" => Val {
            v: if (0..64).contains(&r.v) {
                if l.u {
                    ((l.v as u64) >> r.v) as i64
                } else {
                    l.v >> r.v
                }
            } else if l.u || l.v >= 0 {
                0
            } else {
                -1
            },
            u: l.u,
        },
        "+" => Val {
            v: l.v.wrapping_add(r.v),
            u,
        },
        "-" => Val {
            v: l.v.wrapping_sub(r.v),
            u,
        },
        "*" => Val {
            v: l.v.wrapping_mul(r.v),
            u,
        },
        "/" | "%" => {
            if r.v == 0 {
                if live {
                    return Err("#if 内でゼロによる除算".into());
                }
                Val { v: 0, u }
            } else if u {
                let (a, c) = (l.v as u64, r.v as u64);
                Val {
                    v: (if op == "/" { a / c } else { a % c }) as i64,
                    u,
                }
            } else {
                Val {
                    v: if op == "/" {
                        l.v.wrapping_div(r.v)
                    } else {
                        l.v.wrapping_rem(r.v)
                    },
                    u,
                }
            }
        }
        "&" => Val { v: l.v & r.v, u },
        "|" => Val { v: l.v | r.v, u },
        "^" => Val { v: l.v ^ r.v, u },
        _ => return Err(format!("未対応の演算子: {}", op)),
    })
}

fn decode_escapes(body: &str) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    let mut it = body.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c as u32);
            continue;
        }
        let e = it.next()?;
        let v = match e {
            'n' => 10,
            't' => 9,
            'r' => 13,
            'a' => 7,
            'b' => 8,
            'f' => 12,
            'v' => 11,
            'e' => 27,
            '\\' | '\'' | '"' | '?' => e as u32,
            '0'..='7' => {
                let mut v = e.to_digit(8)?;
                for _ in 0..2 {
                    match it.peek().and_then(|d| d.to_digit(8)) {
                        Some(d) => {
                            v = v * 8 + d;
                            it.next();
                        }
                        None => break,
                    }
                }
                v
            }
            'x' => {
                let mut v = 0u32;
                let mut n = 0;
                while let Some(d) = it.peek().and_then(|d| d.to_digit(16)) {
                    v = v.wrapping_mul(16).wrapping_add(d);
                    it.next();
                    n += 1;
                }
                if n == 0 {
                    return None;
                }
                v
            }
            'u' | 'U' => {
                let len = if e == 'u' { 4 } else { 8 };
                let mut v = 0u32;
                for _ in 0..len {
                    v = v.wrapping_mul(16).wrapping_add(it.next()?.to_digit(16)?);
                }
                v
            }
            _ => return None,
        };
        out.push(v);
    }
    Some(out)
}

/// #if 内の文字定数の値(通常の char は符号付き、複数文字は GCC と同様に詰める)
fn char_value(prefix: &str, body: &str) -> Option<i64> {
    let units = decode_escapes(body)?;
    if units.is_empty() {
        return None;
    }
    if prefix.is_empty() {
        let mut v: i64 = 0;
        for u in &units {
            v = (v << 8) | (*u as i64 & 0xff);
        }
        Some(if units.len() == 1 {
            (v as u8 as i8) as i64
        } else {
            v as i32 as i64
        })
    } else {
        Some(units[0] as i64)
    }
}

// ====================================================================
// __DATE__ / __TIME__ (UTC)
// ====================================================================

fn build_date_time() -> (String, String) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // days -> (y, m, d)  (Howard Hinnant の civil_from_days)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    (
        format!("{} {:2} {}", MONTHS[(m - 1) as usize], d, y),
        format!("{:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60),
    )
}

// ====================================================================
// テスト
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::c_frontend::lex::lexer::lex;

    fn toks_of(v: Vec<TokenInfo>) -> Vec<Token> {
        v.into_iter()
            .map(|t| t.token)
            .filter(|t| *t != Token::Eof)
            .collect()
    }

    fn pp(src: &str) -> Vec<Token> {
        let mut p = Preprocessor::new();
        toks_of(p.preprocess("test.c", src).unwrap())
    }

    fn lexed(src: &str) -> Vec<Token> {
        toks_of(lex(src).unwrap())
    }

    fn same(src: &str, expected: &str) {
        assert_eq!(pp(src), lexed(expected), "\nsrc:\n{}", src);
    }

    #[test]
    fn object_like_and_undef() {
        same("#define N 10\nint a[N];\n#undef N\nN", "int a[10]; N");
        same("#define EMPTY\nint EMPTY x;", "int x;");
        same(
            "#define inline __inline\ninline int f();",
            "__inline int f();",
        );
    }

    #[test]
    fn function_like() {
        same(
            "#define SQ(x) ((x)*(x))\nSQ(1+2) SQ(SQ(3))",
            "((1+2)*(1+2)) ((((3)*(3)))*(((3)*(3))))",
        );
        same("#define F(a, b) a + b\nF(1, (2, 3))", "1 + (2, 3)");
        // 関数形式マクロ名が '(' なしで現れた場合はそのまま
        same("#define F(x) x\nint F; F (7)", "int F; 7");
    }

    #[test]
    fn multiline_define() {
        same(
            "#define ADD(a, b) \\\n    ((a) + \\\n     (b))\nADD(1, 2)",
            "((1) + (2))",
        );
    }

    #[test]
    fn self_reference_is_not_expanded_again() {
        same("#define foo foo + 1\nfoo", "foo + 1");
        same("#define A B\n#define B A\nA B", "A B");
    }

    #[test]
    fn stringify_and_paste() {
        same(
            "#define STR(x) #x\n#define XSTR(x) STR(x)\n#define N 42\nSTR(N) XSTR(N) STR(a   +  b)",
            "\"N\" \"42\" \"a + b\"",
        );
        same(
            "#define CAT(a,b) a##b\nCAT(foo, 12) CAT(1,2) CAT(,x) CAT(x,)",
            "foo12 12 x x",
        );
        same(
            r#"#define S(x) #x
S("a\n" 'b')"#,
            r#""\"a\\n\" 'b'""#,
        );
    }

    #[test]
    fn variadic() {
        same(
            "#define LOG(fmt, ...) printf(fmt, __VA_ARGS__)\nLOG(\"a\", 1, 2)",
            "printf(\"a\", 1, 2)",
        );
        same(
            "#define LOG(fmt, ...) printf(fmt, ##__VA_ARGS__)\nLOG(\"a\"); LOG(\"a\", 1);",
            "printf(\"a\"); printf(\"a\", 1);",
        );
        same("#define V(args...) f(args)\nV(1,2,3)", "f(1,2,3)");
    }

    #[test]
    fn c11_standard_example_3() {
        let src = r#"
#define x 3
#define f(a) f(x * (a))
#undef x
#define x 2
#define g f
#define z z[0]
#define h g(~
#define m(a) a(w)
#define w 0,1
#define t(a) a
#define p() int
#define q(x) x
#define r(x,y) x ## y
#define str(x) # x
f(y+1) + f(f(z)) % t(t(g)(0) + t)(1);
g(x+(3,4)-w) | h 5) & m
(f)^m(m);
p() i[q()] = { q(1), r(2,3), r(4,), r(,5), r(,) };
char c[2][6] = { str(hello), str() };
"#;
        let expected = r#"
f(2 * (y+1)) + f(2 * (f(2 * (z[0])))) % f(2 * (0)) + t(1);
f(2 * (2+(3,4)-0,1)) | f(2 * (~ 5)) & f(2 * (0,1))^m(0,1);
int i[] = { 1, 23, 4, 5, };
char c[2][6] = { "hello", "" };
"#;
        same(src, expected);
    }

    #[test]
    fn conditionals() {
        same(
            "#define V 3\n#if V > 2 && (V * 2 == 6) && !defined(UNDEF) && defined V\nyes\n#elif 1\nno\n#else\nno2\n#endif",
            "yes",
        );
        same("#if 0\nA\n#elif 0\nB\n#else\nC\n#endif", "C");
        same("#if 0\n#if 1\nA\n#else\nB\n#endif\n#else\nC\n#endif", "C");
        same("#ifdef X\nA\n#endif\n#ifndef X\nB\n#endif", "B");
        // 評価されない側のゼロ除算はエラーにならない
        same("#if 1 || (1/0)\nok\n#endif", "ok");
        same(
            "#if (2 || 3) == 1 && -1 < 0 && -1 > 0u && 'a' == 97 && '\\n' == 10 && 0x10 == 16\nok\n#endif",
            "ok",
        );
        same("#if 1 ? 0 : 1\nA\n#else\nB\n#endif", "B");
        // 未定義識別子は 0
        same("#if FOO == 0 && !BAR\nok\n#endif", "ok");
    }

    #[test]
    fn skipped_groups_tolerate_garbage() {
        same(
            "#if 0\nit's \"broken\n#error nope\n1e+ @ `\n#else\nok\n#endif",
            "ok",
        );
    }

    #[test]
    fn errors() {
        let mut p = Preprocessor::new();
        let e = p.preprocess("t.c", "#error stop here").unwrap_err();
        assert!(e.msg.contains("stop here"), "{}", e);
        assert!(Preprocessor::new().preprocess("t.c", "#if 1\n").is_err());
        assert!(Preprocessor::new().preprocess("t.c", "#endif\n").is_err());
        assert!(Preprocessor::new().preprocess("t.c", "#else\n").is_err());
        assert!(
            Preprocessor::new()
                .preprocess("t.c", "#if 1/0\n#endif\n")
                .is_err()
        );
        assert!(
            Preprocessor::new()
                .preprocess("t.c", "#include \"nope.h\"\n")
                .is_err()
        );
        assert!(Preprocessor::new().preprocess("t.c", "#bogus\n").is_err());
        assert!(
            Preprocessor::new()
                .preprocess("t.c", "#define F(x) x\nF(1,2)\n")
                .is_err()
        );
        assert!(
            Preprocessor::new()
                .preprocess("t.c", "#define C(a,b) a##b\nC(+,-)\n")
                .is_err()
        );
    }

    #[test]
    fn includes() {
        let mut p = Preprocessor::new();
        p.add_virtual_file("a.h", "#pragma once\n#define A 1\nint a;\n");
        p.add_virtual_file("b.h", "#ifndef B_H\n#define B_H\nint b;\n#endif\n");
        p.add_virtual_file("stdio.h", "int printf(void);\n");
        p.add_virtual_file("inner.h", "#include \"a.h\"\nint inner;\n");
        let src = "#include \"a.h\"\n#include \"a.h\"\n#include \"b.h\"\n#include \"b.h\"\n\
                   #include <stdio.h>\n#define HDR \"inner.h\"\n#include HDR\nA";
        let out = toks_of(p.preprocess("main.c", src).unwrap());
        assert_eq!(out, lexed("int a; int b; int printf(void); int inner; 1"));
    }

    #[test]
    fn include_macro_angle() {
        let mut p = Preprocessor::new();
        p.add_virtual_file("sys/x.h", "int x;\n");
        let src = "#define H <sys/x.h>\n#include H\n";
        let out = toks_of(p.preprocess("main.c", src).unwrap());
        assert_eq!(out, lexed("int x;"));
    }

    #[test]
    fn predefined_and_line() {
        assert_eq!(
            pp("a\n__LINE__"),
            vec![
                Token::Ident { name: "a".into() },
                Token::IntLiteral {
                    value: 2,
                    suffix: "".into()
                }
            ]
        );
        assert_eq!(pp("#line 100\n__LINE__"), lexed("100"));
        assert_eq!(
            pp("__FILE__"),
            vec![Token::StringLiteral {
                prefix: "".into(),
                body: "test.c".into()
            }]
        );
        assert_eq!(pp("__COUNTER__ __COUNTER__"), lexed("0 1"));
        assert_eq!(pp("__STDC_VERSION__"), lexed("201112L"));
        assert_eq!(pp("#define L __LINE__\n\nL"), lexed("3"));
    }

    #[test]
    fn command_line_defines_and_warnings() {
        let mut p = Preprocessor::new();
        p.define("DEBUG", "1").unwrap();
        p.define("ADD(a,b)", "((a)+(b))").unwrap();
        let out = toks_of(
            p.preprocess(
                "t.c",
                "#if DEBUG\nADD(1,2)\n#endif\n#warning hi\n#define DEBUG 2\n",
            )
            .unwrap(),
        );
        assert_eq!(out, lexed("((1)+(2))"));
        assert_eq!(p.warnings().len(), 2);
    }

    #[test]
    fn directive_only_at_line_start() {
        // 行頭以外の # はディレクティブではない(エラーにもならない場合のみ確認)
        same("#define EMPTY\nEMPTY\n  # define X 5\nX", "5");
        same("/* c */ #define Y 6\nY", "6");
    }

    #[test]
    fn va_opt() {
        same(
            "#define F(a, ...) f(a __VA_OPT__(,) __VA_ARGS__)\nF(1) F(1, 2, 3)",
            "f(1) f(1 , 2, 3)",
        );
        same(
            "#define G(...) g(0 __VA_OPT__(, __VA_ARGS__))\nG() G(7)",
            "g(0) g(0, 7)",
        );
        assert!(
            Preprocessor::new()
                .preprocess("t.c", "#define H(...) __VA_OPT__ x\n")
                .is_ok()
        );
        assert!(
            Preprocessor::new()
                .preprocess("t.c", "#define H(...) __VA_OPT__ x\nH(1)\n")
                .is_err()
        );
    }

    #[test]
    fn pragma_operator() {
        let mut p = Preprocessor::new();
        let out = toks_of(
            p.preprocess("t.c", "int _Pragma(\"pack(1)\") x;\n#pragma foo bar\n")
                .unwrap(),
        );
        assert_eq!(out, lexed("int x;"));
        assert_eq!(p.pragmas(), ["pack(1)", "foo bar"]);
        // マクロの中でも使える。once は#pragma onceと同じ
        let mut p = Preprocessor::new();
        p.add_virtual_file("o.h", "_Pragma(\"once\")\nint o;\n");
        let out = toks_of(
            p.preprocess("t.c", "#include \"o.h\"\n#include \"o.h\"\n")
                .unwrap(),
        );
        assert_eq!(out, lexed("int o;"));
        assert!(Preprocessor::new().preprocess("t.c", "_Pragma(1)").is_err());
    }

    #[test]
    fn has_include() {
        let mut p = Preprocessor::new();
        p.add_virtual_file("here.h", "");
        let src = "#if __has_include(\"here.h\") && __has_include(<stdint.h>) && !__has_include(<nope.h>)\nyes\n#else\nno\n#endif\n\
                   #ifdef __has_include\ndef\n#endif\n";
        assert_eq!(toks_of(p.preprocess("t.c", src).unwrap()), lexed("yes def"));
    }

    #[test]
    fn include_level_and_base_file() {
        let mut p = Preprocessor::new();
        p.add_virtual_file("i.h", "__INCLUDE_LEVEL__ __FILE__ __BASE_FILE__\n");
        let out = toks_of(
            p.preprocess("main.c", "__INCLUDE_LEVEL__\n#include \"i.h\"\n")
                .unwrap(),
        );
        assert_eq!(out, lexed("0 1 \"i.h\" \"main.c\""));
    }

    #[test]
    fn include_next() {
        let dir = std::env::temp_dir().join(format!("pp_next_{}", std::process::id()));
        let (a, b) = (dir.join("a"), dir.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("h.h"), "int first;\n#include_next <h.h>\n").unwrap();
        std::fs::write(b.join("h.h"), "int second;\n").unwrap();
        let mut p = Preprocessor::new();
        p.add_include_dir(&a);
        p.add_include_dir(&b);
        let out = toks_of(p.preprocess("main.c", "#include <h.h>\n").unwrap());
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(out, lexed("int first; int second;"));
    }

    #[test]
    fn builtin_headers() {
        let src = "#include <stdbool.h>\n#include <stdint.h>\n#include <limits.h>\n#include <stddef.h>\n\
                   bool b = true; uint8_t u = UINT8_MAX; int m = INT_MAX; void *n = NULL;";
        let out = pp(src);
        assert!(out.contains(&Token::KeyWord {
            name: "_Bool".into()
        }));
        assert!(
            !out.iter()
                .any(|t| matches!(t, Token::Ident { name } if name == "true" || name == "NULL"))
        );
        // 二重インクルードしても typedef は一度だけ
        let src2 = "#include <stddef.h>\n#include <stddef.h>\nsize_t n;";
        let n = pp(src2)
            .iter()
            .filter(|t| matches!(t, Token::Ident { name } if name == "size_t"))
            .count();
        assert_eq!(n, 2); // typedef と変数宣言
        // 利用者のヘッダが組み込みより優先される
        let mut p = Preprocessor::new();
        p.add_include_dir("/nonexistent");
        p.add_virtual_file("/nonexistent/stdbool.h", "int mine;");
        assert_eq!(
            toks_of(p.preprocess("t.c", "#include <stdbool.h>").unwrap()),
            lexed("int mine;")
        );
        // 未知のヘッダはエラーのまま
        assert!(
            Preprocessor::new()
                .preprocess("t.c", "#include <nonexistent.h>")
                .is_err()
        );
    }
}
