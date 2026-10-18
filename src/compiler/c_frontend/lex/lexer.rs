use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub(in crate::compiler::c_frontend) enum Token {
    KeyWord {
        name: String,
    },
    Ident {
        name: String,
    },
    /// 整数定数。suffix は "", "u", "l", "ul", "ll", "ull" など(元の綴り)
    IntLiteral {
        value: u64,
        suffix: String,
    },
    /// 浮動小数点定数。suffix は "", "f", "l"(元の綴り)
    FloatLiteral {
        value: f64,
        suffix: String,
    },
    /// 文字定数。body はクォートを除いた生のテキスト(エスケープ未展開)
    CharLiteral {
        prefix: String,
        body: String,
    },
    /// 文字列リテラル。body はクォートを除いた生のテキスト(エスケープ未展開)
    StringLiteral {
        prefix: String,
        body: String,
    },
    /// パンクチュエータ(ダイグラフは正規形に変換)
    Punct {
        name: &'static str,
    },
    /// `#include <...>` のヘッダ名(山括弧を除いた中身)。#include 直後にだけ生成される
    HeaderName {
        name: String,
    },
    /// 数値として解釈できない pp-number(寛容モードのみ)
    PpNumber {
        text: String,
    },
    /// どのトークンにも当てはまらない1文字(寛容モードのみ)
    Other {
        ch: char,
    },
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::compiler::c_frontend) struct TokenInfo {
    pub token: Token,
    pub line: usize,
    pub col: usize,
    /// ソース上のバイト範囲(プリプロセス後のトークンでは 0)
    pub start: usize,
    pub end: usize,
    /// 論理行の最初のトークンか(行継続は同じ論理行として扱う)
    pub bol: bool,
    /// 直前に空白・改行・コメントがあったか
    pub space_before: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::compiler::c_frontend) struct LexError {
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.msg)
    }
}
impl std::error::Error for LexError {}

const KEYWORDS: &[&str] = &[
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    // C11
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_Bool",
    "_Complex",
    "_Generic",
    "_Imaginary",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
];

/// (綴り, 正規形)。長いものを先に。
const PUNCTS: &[(&str, &str)] = &[
    ("%:%:", "##"),
    ("...", "..."),
    ("<<=", "<<="),
    (">>=", ">>="),
    ("->", "->"),
    ("++", "++"),
    ("--", "--"),
    ("<<", "<<"),
    (">>", ">>"),
    ("<=", "<="),
    (">=", ">="),
    ("==", "=="),
    ("!=", "!="),
    ("&&", "&&"),
    ("||", "||"),
    ("*=", "*="),
    ("/=", "/="),
    ("%=", "%="),
    ("+=", "+="),
    ("-=", "-="),
    ("&=", "&="),
    ("^=", "^="),
    ("|=", "|="),
    ("##", "##"),
    ("<:", "["),
    (":>", "]"),
    ("<%", "{"),
    ("%>", "}"),
    ("%:", "#"),
    ("[", "["),
    ("]", "]"),
    ("(", "("),
    (")", ")"),
    ("{", "{"),
    ("}", "}"),
    (".", "."),
    ("&", "&"),
    ("*", "*"),
    ("+", "+"),
    ("-", "-"),
    ("~", "~"),
    ("!", "!"),
    ("/", "/"),
    ("%", "%"),
    ("<", "<"),
    (">", ">"),
    ("^", "^"),
    ("|", "|"),
    ("?", "?"),
    (":", ":"),
    (";", ";"),
    ("=", "="),
    (",", ","),
    ("#", "#"),
];

pub(in crate::compiler::c_frontend) struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
    /// 寛容モード(プリプロセッサ用)。不正な文字や閉じていないクォートでエラーにせず
    /// Other / PpNumber トークンにする。スキップされる #if 0 ブロック内の
    /// 「it's」のような文字列でも字句解析が失敗しないようにするため。
    lenient: bool,
    saw_newline: bool,
    saw_space: bool,
    /// 0: 通常, 1: 行頭の '#' の直後, 2: '#include' の直後
    dir_state: u8,
}

impl<'a> Lexer<'a> {
    pub(in crate::compiler::c_frontend) fn new(src: &'a str) -> Self {
        Lexer {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
            lenient: false,
            saw_newline: false,
            saw_space: false,
            dir_state: 0,
        }
    }

    /// プリプロセッサ用の寛容モードの Lexer
    pub(in crate::compiler::c_frontend) fn new_pp(src: &'a str) -> Self {
        let mut l = Self::new(src);
        l.lenient = true;
        l
    }

    pub(in crate::compiler::c_frontend) fn tokenize(mut self) -> Result<Vec<TokenInfo>, LexError> {
        let mut out: Vec<TokenInfo> = Vec::new();
        loop {
            self.saw_space = false;
            self.saw_newline = false;
            self.skip_trivia()?;
            let first = out.is_empty();
            let bol = first || self.saw_newline;
            let space_before = first || self.saw_space || self.saw_newline;
            let (line, col, start) = (self.line, self.col, self.pos);
            if self.pos >= self.src.len() {
                out.push(TokenInfo {
                    token: Token::Eof,
                    line,
                    col,
                    start,
                    end: start,
                    bol: true,
                    space_before: true,
                });
                return Ok(out);
            }
            let token = self.next_token()?;
            self.dir_state = match (&token, self.dir_state) {
                (Token::Punct { name: "#" }, _) if bol => 1,
                (Token::Ident { name }, 1)
                    if name.as_str() == "include" || name.as_str() == "include_next" =>
                {
                    2
                }
                _ => 0,
            };
            out.push(TokenInfo {
                token,
                line,
                col,
                start,
                end: self.pos,
                bol,
                space_before,
            });
        }
    }

    // ---------- 基本操作 ----------

    fn at(&self, i: usize) -> u8 {
        *self.src.get(self.pos + i).unwrap_or(&0)
    }

    fn bump(&mut self) {
        if self.pos >= self.src.len() {
            return;
        }
        if self.src[self.pos] == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.pos += 1;
    }

    fn bump_n(&mut self, n: usize) {
        for _ in 0..n {
            self.bump();
        }
    }

    fn err<T>(&self, msg: impl Into<String>) -> Result<T, LexError> {
        Err(LexError {
            line: self.line,
            col: self.col,
            msg: msg.into(),
        })
    }

    fn text(&self, a: usize, b: usize) -> String {
        String::from_utf8_lossy(&self.src[a..b]).into_owned()
    }

    fn is_ident_start(c: u8) -> bool {
        c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
    }
    fn is_ident_cont(c: u8) -> bool {
        Self::is_ident_start(c) || c.is_ascii_digit()
    }

    // ---------- 空白・コメント ----------

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            let c = self.at(0);
            match c {
                b'\n' => {
                    self.saw_newline = true;
                    self.bump();
                }
                b' ' | b'\t' | b'\r' | 0x0b | 0x0c => {
                    self.saw_space = true;
                    self.bump();
                }
                // 行継続 (バックスラッシュ + 改行)。論理行は続くので newline 扱いにしない
                b'\\' if self.at(1) == b'\n' => {
                    self.saw_space = true;
                    self.bump_n(2);
                }
                b'\\' if self.at(1) == b'\r' && self.at(2) == b'\n' => {
                    self.saw_space = true;
                    self.bump_n(3);
                }
                b'/' if self.at(1) == b'/' => {
                    self.saw_space = true;
                    while self.pos < self.src.len() && self.at(0) != b'\n' {
                        self.bump();
                    }
                }
                b'/' if self.at(1) == b'*' => {
                    self.saw_space = true;
                    let (l, c0) = (self.line, self.col);
                    self.bump_n(2);
                    loop {
                        if self.pos >= self.src.len() {
                            return Err(LexError {
                                line: l,
                                col: c0,
                                msg: "コメントが閉じられていません".into(),
                            });
                        }
                        if self.at(0) == b'*' && self.at(1) == b'/' {
                            self.bump_n(2);
                            break;
                        }
                        self.bump();
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    // ---------- トークン ----------

    fn next_token(&mut self) -> Result<Token, LexError> {
        let c = self.at(0);

        // #include <...> のヘッダ名
        if self.dir_state == 2 && c == b'<' {
            if let Some(tok) = self.try_header_name() {
                return Ok(tok);
            }
        }

        // 文字列・文字リテラル(プレフィックス付き含む)
        if let Some(plen) = self.literal_prefix_len() {
            let prefix = self.text(self.pos, self.pos + plen);
            self.bump_n(plen);
            return self.lex_quoted(prefix);
        }

        if Self::is_ident_start(c) {
            let start = self.pos;
            while Self::is_ident_cont(self.at(0)) {
                self.bump();
            }
            let name = self.text(start, self.pos);
            return Ok(if KEYWORDS.contains(&name.as_str()) {
                Token::KeyWord { name }
            } else {
                Token::Ident { name }
            });
        }

        if c.is_ascii_digit() || (c == b'.' && self.at(1).is_ascii_digit()) {
            return self.lex_number();
        }

        for (spelling, canon) in PUNCTS {
            if self.src[self.pos..].starts_with(spelling.as_bytes()) {
                self.bump_n(spelling.len());
                return Ok(Token::Punct { name: canon });
            }
        }

        if self.lenient {
            self.bump();
            return Ok(Token::Other { ch: c as char });
        }
        self.err(format!("不正な文字: {:?}", c as char))
    }

    /// `<name>` を同じ行内で探す。閉じ '>' が無ければ None(通常の '<' として扱う)
    fn try_header_name(&mut self) -> Option<Token> {
        let mut i = 1;
        loop {
            let c = self.at(i);
            if c == 0 || c == b'\n' {
                return None;
            }
            if c == b'>' {
                break;
            }
            i += 1;
        }
        let name = self.text(self.pos + 1, self.pos + i);
        self.bump_n(i + 1);
        Some(Token::HeaderName { name })
    }

    /// `"` `'` の直前にある L / u / U / u8 プレフィックスの長さ。
    /// プレフィックス無しのクォートなら 0、リテラルでなければ None。
    fn literal_prefix_len(&self) -> Option<usize> {
        let is_q = |c: u8| c == b'"' || c == b'\'';
        let c = self.at(0);
        if is_q(c) {
            return Some(0);
        }
        match c {
            b'L' | b'U' if is_q(self.at(1)) => Some(1),
            b'u' if is_q(self.at(1)) => Some(1),
            // u8 は文字列のみ(C11)。u8'a' は C23 なので除外
            b'u' if self.at(1) == b'8' && self.at(2) == b'"' => Some(2),
            _ => None,
        }
    }

    fn lex_quoted(&mut self, prefix: String) -> Result<Token, LexError> {
        let save = (self.pos, self.line, self.col);
        match self.lex_quoted_strict(prefix) {
            Err(_) if self.lenient => {
                // 開きクォートだけを Other として切り出し、残りは通常どおり読み直す
                (self.pos, self.line, self.col) = save;
                let q = self.at(0);
                self.bump();
                Ok(Token::Other { ch: q as char })
            }
            r => r,
        }
    }

    fn lex_quoted_strict(&mut self, prefix: String) -> Result<Token, LexError> {
        let q = self.at(0);
        let (l0, c0) = (self.line, self.col);
        self.bump();
        let start = self.pos;
        loop {
            if self.pos >= self.src.len() {
                return Err(LexError {
                    line: l0,
                    col: c0,
                    msg: "リテラルが閉じられていません".into(),
                });
            }
            match self.at(0) {
                b'\n' => {
                    return Err(LexError {
                        line: l0,
                        col: c0,
                        msg: "リテラル内に改行があります".into(),
                    });
                }
                b'\\' => {
                    if self.pos + 1 >= self.src.len() {
                        return Err(LexError {
                            line: l0,
                            col: c0,
                            msg: "リテラルが閉じられていません".into(),
                        });
                    }
                    self.bump_n(2);
                }
                c if c == q => break,
                _ => self.bump(),
            }
        }
        let body = self.text(start, self.pos);
        self.bump(); // 閉じクォート
        if q == b'\'' {
            if body.is_empty() {
                return Err(LexError {
                    line: l0,
                    col: c0,
                    msg: "空の文字定数です".into(),
                });
            }
            Ok(Token::CharLiteral { prefix, body })
        } else {
            Ok(Token::StringLiteral { prefix, body })
        }
    }

    // ---------- 数値 ----------

    fn lex_number(&mut self) -> Result<Token, LexError> {
        let save = (self.pos, self.line, self.col);
        match self.lex_number_strict() {
            Err(_) if self.lenient => {
                // pp-number として読み直す(後段で必要になった時点でエラーにする)
                (self.pos, self.line, self.col) = save;
                let start = self.pos;
                loop {
                    let c = self.at(0);
                    if matches!(c, b'e' | b'E' | b'p' | b'P') && matches!(self.at(1), b'+' | b'-') {
                        self.bump_n(2);
                    } else if c == b'.' || Self::is_ident_cont(c) {
                        self.bump();
                    } else {
                        break;
                    }
                }
                Ok(Token::PpNumber {
                    text: self.text(start, self.pos),
                })
            }
            r => r,
        }
    }

    fn lex_number_strict(&mut self) -> Result<Token, LexError> {
        let (l0, c0) = (self.line, self.col);
        let start = self.pos;
        let mut is_float = false;
        let mut hex = false;

        let digit = |c: u8, hex: bool| {
            if hex {
                c.is_ascii_hexdigit()
            } else {
                c.is_ascii_digit()
            }
        };

        if self.at(0) == b'0'
            && matches!(self.at(1), b'x' | b'X')
            && (self.at(2).is_ascii_hexdigit()
                || (self.at(2) == b'.' && self.at(3).is_ascii_hexdigit()))
        {
            hex = true;
            self.bump_n(2);
        }

        while digit(self.at(0), hex) {
            self.bump();
        }
        if self.at(0) == b'.' {
            is_float = true;
            self.bump();
            while digit(self.at(0), hex) {
                self.bump();
            }
        }
        let (e_lo, e_up) = if hex { (b'p', b'P') } else { (b'e', b'E') };
        if self.at(0) == e_lo || self.at(0) == e_up {
            let has_sign = matches!(self.at(1), b'+' | b'-');
            let d = if has_sign { self.at(2) } else { self.at(1) };
            if d.is_ascii_digit() {
                is_float = true;
                self.bump_n(if has_sign { 2 } else { 1 });
                while self.at(0).is_ascii_digit() {
                    self.bump();
                }
            } else if hex {
                return self.err("16進浮動小数の指数部がありません");
            }
        }

        let body = self.text(start, self.pos);
        if hex && is_float && !body.to_ascii_lowercase().contains('p') {
            return Err(LexError {
                line: l0,
                col: c0,
                msg: "16進浮動小数には p 指数が必要です".into(),
            });
        }

        let sfx_start = self.pos;
        while Self::is_ident_cont(self.at(0)) {
            self.bump();
        }
        let suffix = self.text(sfx_start, self.pos);
        let lower = suffix.to_ascii_lowercase();
        let bad = |what: &str| LexError {
            line: l0,
            col: c0,
            msg: format!("不正な{}サフィックス: {:?}", what, suffix),
        };

        if is_float {
            if !matches!(lower.as_str(), "" | "f" | "l") {
                return Err(bad("浮動小数"));
            }
            let value = if hex {
                parse_hex_float(&body)
            } else {
                body.parse::<f64>().ok()
            }
            .ok_or_else(|| LexError {
                line: l0,
                col: c0,
                msg: format!("不正な浮動小数: {}", body),
            })?;
            Ok(Token::FloatLiteral { value, suffix })
        } else {
            let ok = matches!(
                lower.as_str(),
                "" | "u" | "l" | "ul" | "lu" | "ll" | "ull" | "llu"
            ) && !suffix.contains("lL")
                && !suffix.contains("Ll");
            if !ok {
                return Err(bad("整数"));
            }
            let (digits, radix) = if hex {
                (&body[2..], 16)
            } else if body.len() > 1 && body.starts_with('0') {
                (&body[1..], 8)
            } else {
                (&body[..], 10)
            };
            let value = u64::from_str_radix(digits, radix).map_err(|_| LexError {
                line: l0,
                col: c0,
                msg: format!("不正な整数定数(範囲外または桁が不正): {}", body),
            })?;
            Ok(Token::IntLiteral { value, suffix })
        }
    }
}

/// "0x1.8p3" 形式をパース
fn parse_hex_float(s: &str) -> Option<f64> {
    let s = s[2..].to_ascii_lowercase();
    let (mant, exp) = s.split_once('p')?;
    let exp: i32 = exp.parse().ok()?;
    let (int_part, frac_part) = mant.split_once('.').unwrap_or((mant, ""));
    let mut v = 0.0f64;
    for c in int_part.chars() {
        v = v * 16.0 + c.to_digit(16)? as f64;
    }
    let mut scale = 1.0 / 16.0;
    for c in frac_part.chars() {
        v += c.to_digit(16)? as f64 * scale;
        scale /= 16.0;
    }
    Some(v * 2f64.powi(exp))
}

/// 厳密モードの字句解析(コンパイラ本体向け)
pub(in crate::compiler::c_frontend) fn lex(src: &str) -> Result<Vec<TokenInfo>, LexError> {
    Lexer::new(src).tokenize()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Token> {
        lex(s).unwrap().into_iter().map(|t| t.token).collect()
    }

    #[test]
    fn keywords_and_idents() {
        let t = toks("int _Bool foo _Static_assert");
        assert_eq!(t[0], Token::KeyWord { name: "int".into() });
        assert_eq!(
            t[1],
            Token::KeyWord {
                name: "_Bool".into()
            }
        );
        assert_eq!(t[2], Token::Ident { name: "foo".into() });
        assert_eq!(
            t[3],
            Token::KeyWord {
                name: "_Static_assert".into()
            }
        );
    }

    #[test]
    fn numbers() {
        let t = toks("42 0x1Fu 017 1.5f .5 1e3 0x1.8p1 10UL");
        assert_eq!(
            t[0],
            Token::IntLiteral {
                value: 42,
                suffix: "".into()
            }
        );
        assert_eq!(
            t[1],
            Token::IntLiteral {
                value: 31,
                suffix: "u".into()
            }
        );
        assert_eq!(
            t[2],
            Token::IntLiteral {
                value: 15,
                suffix: "".into()
            }
        );
        assert_eq!(
            t[3],
            Token::FloatLiteral {
                value: 1.5,
                suffix: "f".into()
            }
        );
        assert_eq!(
            t[4],
            Token::FloatLiteral {
                value: 0.5,
                suffix: "".into()
            }
        );
        assert_eq!(
            t[5],
            Token::FloatLiteral {
                value: 1000.0,
                suffix: "".into()
            }
        );
        assert_eq!(
            t[6],
            Token::FloatLiteral {
                value: 3.0,
                suffix: "".into()
            }
        );
        assert_eq!(
            t[7],
            Token::IntLiteral {
                value: 10,
                suffix: "UL".into()
            }
        );
    }

    #[test]
    fn strings_and_chars() {
        let t = toks(r#"u8"hi\n" L'a' '\''"#);
        assert_eq!(
            t[0],
            Token::StringLiteral {
                prefix: "u8".into(),
                body: "hi\\n".into()
            }
        );
        assert_eq!(
            t[1],
            Token::CharLiteral {
                prefix: "L".into(),
                body: "a".into()
            }
        );
        assert_eq!(
            t[2],
            Token::CharLiteral {
                prefix: "".into(),
                body: "\\'".into()
            }
        );
    }

    #[test]
    fn puncts_and_comments() {
        let t = toks("a->b /* c */ <<= // x\n ... <: %:%:");
        assert_eq!(t[1], Token::Punct { name: "->" });
        assert_eq!(t[3], Token::Punct { name: "<<=" });
        assert_eq!(t[4], Token::Punct { name: "..." });
        assert_eq!(t[5], Token::Punct { name: "[" });
        assert_eq!(t[6], Token::Punct { name: "##" });
    }

    #[test]
    fn bol_and_continuation() {
        let t = lex("#define X \\\n  1\nY").unwrap();
        // # define X 1 Y Eof
        assert!(t[0].bol);
        assert!(!t[1].bol && !t[2].bol && !t[3].bol); // 行継続後の 1 は同じ論理行
        assert!(t[4].bol);
    }

    #[test]
    fn header_name_and_lenient() {
        let t = Lexer::new_pp("#include <sys/a-1.2.h>\n#if 0\nit's \"x\n#endif\n")
            .tokenize()
            .unwrap();
        assert_eq!(
            t[2].token,
            Token::HeaderName {
                name: "sys/a-1.2.h".into()
            }
        );
        assert!(
            t.iter()
                .any(|s| matches!(s.token, Token::Other { ch: '\'' }))
        );
    }
}
