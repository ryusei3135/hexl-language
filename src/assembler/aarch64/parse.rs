//! aarch64 アセンブリの字句解析 + 構文解析
//!
//! gas (GNU as) の aarch64 構文を元にしているが、x64 版の `emitter_x64` と
//! 見た目を揃えるため、次の2点だけ gas と違う。
//!
//! | 項目 | gas | このアセンブラ |
//! |---|---|---|
//! | レジスタ | `x0` / `w1` / `sp` | `%x0` / `%w1` / `%sp` |
//! | 即値 | `#8` | `$8` |
//!
//! メモリオペランドは gas と同じ `[...]` 形式。
//!
//! ```text
//! [%x0]                     ベースのみ
//! [%x0, $8]                 ベース + 即値オフセット
//! [%x0, $-16]!              プリインデックス (ベースを更新してからアクセス)
//! [%x0], $16                ポストインデックス (アクセスしてからベースを更新)
//! [%x0, %x1]                ベース + レジスタ
//! [%x0, %x1, lsl $3]        ベース + (レジスタ << 3)
//! [%x0, %w1, sxtw $2]       ベース + (符号拡張したW << 2)
//! [%x0, :lo12:sym]          シンボルの下位12bit (`adrp` とセットで使う)
//! ```
//!
//! コメントは `//` (行末まで) / `/* ... */` / `#` (行末まで)。
//! `;` は文の区切り (改行と同じ)。

use super::reg::Reg;

// ============================================================
// Lexer
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    Register(String),
    Number(i64),
    Float(f64),
    Str(String),

    Comma,
    Colon,
    Dollar,
    LBracket,
    RBracket,
    Bang,
    Plus,
    Minus,

    NewLine,
    Eof,
}

pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    rest: &'a str,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            rest: input,
        }
    }

    fn peek2(&self) -> Option<char> {
        let mut it = self.chars.clone();
        it.next();
        it.next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.next();

        if let Some(c) = c {
            self.rest = &self.rest[c.len_utf8()..];
        }

        c
    }

    fn number(&mut self) -> Result<Token, String> {
        // 0x.. / 0b..
        if self.chars.peek() == Some(&'0') {
            let radix = match self.peek2() {
                Some('x' | 'X') => Some(16),
                Some('b' | 'B') => Some(2),
                _ => None,
            };

            if let Some(radix) = radix {
                // `0x` の直後が数字のときだけ (`0b` が識別子の一部にならないように)
                let mut it = self.chars.clone();
                it.next();
                it.next();

                if matches!(it.next(), Some(c) if c.is_digit(radix)) {
                    self.advance();
                    self.advance();

                    let mut value: u64 = 0;

                    while let Some(d) = self.chars.peek().and_then(|c| c.to_digit(radix)) {
                        value = value.wrapping_mul(radix as u64).wrapping_add(d as u64);
                        self.advance();
                    }

                    return Ok(Token::Number(value as i64));
                }
            }
        }

        let mut text = String::new();

        while let Some(&c) = self.chars.peek() {
            if c.is_ascii_digit() {
                text.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let mut is_float = false;

        // `.` の直後が数字のときだけ小数
        if self.chars.peek() == Some(&'.') && matches!(self.peek2(), Some(c) if c.is_ascii_digit())
        {
            is_float = true;
            text.push('.');
            self.advance();

            while let Some(&c) = self.chars.peek() {
                if c.is_ascii_digit() {
                    text.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if is_float {
            return match text.parse::<f64>() {
                Ok(f) if f.is_finite() => Ok(Token::Float(f)),
                _ => Err(format!("invalid float literal: {}", text)),
            };
        }

        if let Ok(n) = text.parse::<i64>() {
            Ok(Token::Number(n))
        } else if let Ok(n) = text.parse::<u64>() {
            // i64 に収まらない 64bit の値は2の補数で表す
            Ok(Token::Number(n as i64))
        } else {
            Err(format!("integer literal out of range: {}", text))
        }
    }

    fn string_literal(&mut self) -> Result<String, String> {
        self.advance(); // opening quote

        let mut value = String::new();

        loop {
            match self.advance() {
                None => return Err("unterminated string literal".to_string()),
                Some('"') => break,
                Some('\\') => match self.advance() {
                    None => {
                        return Err("unterminated string literal (trailing backslash)".to_string());
                    }
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('r') => value.push('\r'),
                    Some('0') => value.push('\0'),
                    Some(other) => value.push(other),
                },
                Some(c) => value.push(c),
            }
        }

        Ok(value)
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        loop {
            let c = match self.chars.peek().copied() {
                None => {
                    tokens.push(Token::Eof);
                    break;
                }
                Some(c) => c,
            };

            match c {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' | ';' => {
                    self.advance();
                    tokens.push(Token::NewLine);
                }
                // 行コメント
                '#' => {
                    while !matches!(self.chars.peek(), None | Some('\n')) {
                        self.advance();
                    }
                }
                '/' => {
                    match self.peek2() {
                        Some('/') => {
                            while !matches!(self.chars.peek(), None | Some('\n')) {
                                self.advance();
                            }
                        }
                        Some('*') => {
                            self.advance();
                            self.advance();

                            loop {
                                match self.advance() {
                                    None => return Err("unterminated block comment".to_string()),
                                    Some('*') if self.chars.peek() == Some(&'/') => {
                                        self.advance();
                                        break;
                                    }
                                    Some(_) => {}
                                }
                            }
                        }
                        _ => return Err("unexpected character: '/'".to_string()),
                    };
                }
                ',' => {
                    self.advance();
                    tokens.push(Token::Comma);
                }
                ':' => {
                    self.advance();
                    tokens.push(Token::Colon);
                }
                '$' => {
                    self.advance();
                    tokens.push(Token::Dollar);
                }
                '[' => {
                    self.advance();
                    tokens.push(Token::LBracket);
                }
                ']' => {
                    self.advance();
                    tokens.push(Token::RBracket);
                }
                '!' => {
                    self.advance();
                    tokens.push(Token::Bang);
                }
                '+' => {
                    self.advance();
                    tokens.push(Token::Plus);
                }
                '-' => {
                    self.advance();
                    tokens.push(Token::Minus);
                }
                '"' => {
                    let s = self.string_literal()?;
                    tokens.push(Token::Str(s));
                }
                '%' => {
                    self.advance();

                    let mut name = String::from("%");

                    while let Some(&c) = self.chars.peek() {
                        if c.is_ascii_alphanumeric() {
                            name.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }

                    tokens.push(Token::Register(name));
                }
                c if c.is_ascii_digit() => {
                    let t = self.number()?;
                    tokens.push(t);
                }
                c if c.is_ascii_alphabetic() || c == '_' || c == '.' => {
                    let mut name = String::new();

                    while let Some(&c) = self.chars.peek() {
                        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                            name.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }

                    tokens.push(Token::Ident(name));
                }
                other => return Err(format!("unexpected character: {:?}", other)),
            }
        }

        let _ = self.rest;

        Ok(tokens)
    }
}

// ============================================================
// AST
// ============================================================

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Label(String),
    /// `.text` / `.quad 1, foo` など (名前は先頭の `.` 付き)
    Directive(String, Vec<Operand>),
    Instruction(Instruction),
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub mnemonic: String,
    pub operands: Vec<Operand>,
}

#[derive(Debug, Clone)]
pub enum Operand {
    Register(Reg),
    Immediate(i64),
    /// `.float 1.5` などディレクティブ引数のみ
    Float(f64),
    /// ラベル参照 (`b loop` など)。条件名 (`eq`) もここに入る
    Symbol(String),
    Str(String),
    /// `:lo12:sym`
    Lo12(String),
    /// `lsl $3` / `sxtw` / `uxtx $2` (名前, 量)
    Shift(String, Option<i64>),
    Memory(MemoryOperand),
}

#[derive(Debug, Clone)]
pub struct MemoryOperand {
    pub base: Reg,
    pub offset: MemOffset,
    pub mode: AddrMode,
}

#[derive(Debug, Clone)]
pub enum MemOffset {
    /// `[%x0]`
    None,
    /// `[%x0, $8]`
    Imm(i64),
    /// `[%x0, %x1]` / `[%x0, %x1, lsl $3]` (レジスタ, 拡張/シフト名, 量)
    Reg(Reg, Option<(String, Option<i64>)>),
    /// `[%x0, :lo12:sym]`
    Lo12(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrMode {
    Offset,
    PreIndex,
    PostIndex,
}

const SHIFT_NAMES: &[&str] = &[
    "lsl", "lsr", "asr", "ror", "uxtb", "uxth", "uxtw", "uxtx", "sxtb", "sxth", "sxtw", "sxtx",
];

// ============================================================
// Parser
// ============================================================

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(mut tokens: Vec<Token>) -> Self {
        if !matches!(tokens.last(), Some(Token::Eof)) {
            tokens.push(Token::Eof);
        }

        Self { tokens, pos: 0 }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();

        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }

        token
    }

    fn consume(&mut self, expected: &Token) -> Result<(), String> {
        if self.current() == expected {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", expected, self.current()))
        }
    }

    pub fn parse(&mut self) -> Result<Program, String> {
        let mut statements = Vec::new();

        while !matches!(self.current(), Token::Eof) {
            if matches!(self.current(), Token::NewLine) {
                self.advance();
                continue;
            }

            statements.push(self.parse_statement()?);

            if matches!(self.current(), Token::NewLine) {
                self.advance();
            }
        }

        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        let name = match self.advance() {
            Token::Ident(name) => name,
            token => return Err(format!("expected identifier, got {:?}", token)),
        };

        if matches!(self.current(), Token::Colon) {
            self.advance();
            return Ok(Statement::Label(name));
        }

        let is_directive = name.starts_with('.');
        let mut operands = Vec::new();

        if !matches!(self.current(), Token::NewLine | Token::Eof) {
            loop {
                if is_directive {
                    operands.push(self.parse_directive_operand()?);
                } else {
                    operands.push(self.parse_operand()?);
                }

                if matches!(self.current(), Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        if !matches!(self.current(), Token::NewLine | Token::Eof) {
            return Err(format!(
                "unexpected token after operands of `{}`: {:?}",
                name,
                self.current()
            ));
        }

        if is_directive {
            Ok(Statement::Directive(name, operands))
        } else {
            Ok(Statement::Instruction(Instruction {
                mnemonic: name.to_ascii_lowercase(),
                operands,
            }))
        }
    }

    // ディレクティブの引数: 数値 / 小数 / 文字列 / シンボル
    fn parse_directive_operand(&mut self) -> Result<Operand, String> {
        match self.advance() {
            Token::Str(s) => Ok(Operand::Str(s)),
            Token::Dollar => self.parse_directive_operand(),
            Token::Minus => match self.advance() {
                Token::Number(n) => Ok(Operand::Immediate(n.wrapping_neg())),
                Token::Float(f) => Ok(Operand::Float(-f)),
                token => Err(format!("expected number, got {:?}", token)),
            },
            Token::Plus => match self.advance() {
                Token::Number(n) => Ok(Operand::Immediate(n)),
                Token::Float(f) => Ok(Operand::Float(f)),
                token => Err(format!("expected number, got {:?}", token)),
            },
            Token::Number(n) => Ok(Operand::Immediate(n)),
            Token::Float(f) => Ok(Operand::Float(f)),
            Token::Ident(name) => Ok(Operand::Symbol(name)),
            token => Err(format!("invalid directive operand: {:?}", token)),
        }
    }

    fn parse_operand(&mut self) -> Result<Operand, String> {
        match self.current().clone() {
            Token::Register(name) => {
                self.advance();
                Ok(Operand::Register(parse_register(&name)?))
            }
            Token::Dollar => {
                self.advance();
                Ok(Operand::Immediate(self.parse_signed_number()?))
            }
            Token::Colon => {
                let sym = self.parse_lo12()?;
                Ok(Operand::Lo12(sym))
            }
            Token::LBracket => self.parse_memory(),
            Token::Ident(name) => {
                self.advance();

                if SHIFT_NAMES.contains(&name.to_ascii_lowercase().as_str()) {
                    let amount = if matches!(self.current(), Token::Dollar) {
                        self.advance();
                        Some(self.parse_signed_number()?)
                    } else {
                        None
                    };

                    Ok(Operand::Shift(name.to_ascii_lowercase(), amount))
                } else {
                    Ok(Operand::Symbol(name))
                }
            }
            token => Err(format!("invalid operand: {:?}", token)),
        }
    }

    fn parse_signed_number(&mut self) -> Result<i64, String> {
        let negative = match self.current() {
            Token::Minus => {
                self.advance();
                true
            }
            Token::Plus => {
                self.advance();
                false
            }
            _ => false,
        };

        match self.advance() {
            Token::Number(n) => Ok(if negative { n.wrapping_neg() } else { n }),
            token => Err(format!("expected number, got {:?}", token)),
        }
    }

    // `:lo12:sym`
    fn parse_lo12(&mut self) -> Result<String, String> {
        self.consume(&Token::Colon)?;

        match self.advance() {
            Token::Ident(kind) if kind.eq_ignore_ascii_case("lo12") => {}
            token => return Err(format!("expected `lo12` after ':', got {:?}", token)),
        }

        self.consume(&Token::Colon)?;

        match self.advance() {
            Token::Ident(sym) => Ok(sym),
            token => Err(format!("expected symbol after `:lo12:`, got {:?}", token)),
        }
    }

    fn parse_memory(&mut self) -> Result<Operand, String> {
        self.consume(&Token::LBracket)?;

        let base = match self.advance() {
            Token::Register(name) => parse_register(&name)?,
            token => return Err(format!("expected base register, got {:?}", token)),
        };

        let mut offset = MemOffset::None;

        if matches!(self.current(), Token::Comma) {
            self.advance();

            offset = match self.current().clone() {
                Token::Dollar => {
                    self.advance();
                    MemOffset::Imm(self.parse_signed_number()?)
                }
                Token::Colon => MemOffset::Lo12(self.parse_lo12()?),
                Token::Register(name) => {
                    self.advance();
                    let index = parse_register(&name)?;

                    let mut ext = None;

                    if matches!(self.current(), Token::Comma) {
                        self.advance();

                        match self.advance() {
                            Token::Ident(kind)
                                if SHIFT_NAMES.contains(&kind.to_ascii_lowercase().as_str()) =>
                            {
                                let amount = if matches!(self.current(), Token::Dollar) {
                                    self.advance();
                                    Some(self.parse_signed_number()?)
                                } else {
                                    None
                                };

                                ext = Some((kind.to_ascii_lowercase(), amount));
                            }
                            token => {
                                return Err(format!(
                                    "expected shift/extend in memory operand, got {:?}",
                                    token
                                ));
                            }
                        }
                    }

                    MemOffset::Reg(index, ext)
                }
                token => return Err(format!("invalid memory offset: {:?}", token)),
            };
        }

        self.consume(&Token::RBracket)?;

        let mut mode = AddrMode::Offset;

        if matches!(self.current(), Token::Bang) {
            self.advance();
            mode = AddrMode::PreIndex;
        } else if matches!(self.current(), Token::Comma) {
            // `[%x0], $16` : ポストインデックス (メモリオペランドは常に最後なので曖昧にならない)
            self.advance();

            match self.advance() {
                Token::Dollar => {
                    let n = self.parse_signed_number()?;
                    offset = MemOffset::Imm(n);
                    mode = AddrMode::PostIndex;
                }
                token => return Err(format!("expected `$imm` for post-index, got {:?}", token)),
            }
        }

        if mode == AddrMode::PreIndex && !matches!(offset, MemOffset::Imm(_)) {
            return Err("pre-index requires an immediate offset".to_string());
        }

        Ok(Operand::Memory(MemoryOperand { base, offset, mode }))
    }
}

fn parse_register(name: &str) -> Result<Reg, String> {
    Reg::from_name(&name.to_ascii_lowercase()).ok_or_else(|| format!("unknown register: {}", name))
}
