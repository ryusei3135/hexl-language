use std::str::Chars;

// ============================================================
// Lexer
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),

    Register(String),

    Number(i64),

    Str(String),

    Comma,
    Colon,

    Dollar,  // $
    Percent, // %

    LParen, // (
    RParen, // )

    Plus,  // +
    Minus, // -
    Star,  // *

    NewLine,

    Eof,
}

pub struct Lexer<'a> {
    input: Chars<'a>,
    current: Option<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut chars = input.chars();

        Self {
            current: chars.next(),
            input: chars,
        }
    }

    fn advance(&mut self) {
        self.current = self.input.next();
    }

    fn skip_spaces(&mut self) {
        while matches!(self.current, Some(' ' | '\t' | '\r')) {
            self.advance();
        }
    }

    fn number(&mut self) -> i64 {
        let mut value = String::new();

        while let Some(c) = self.current {
            if c.is_ascii_digit() {
                value.push(c);
                self.advance();
            } else {
                break;
            }
        }

        value.parse().unwrap()
    }

    fn identifier(&mut self) -> String {
        let mut value = String::new();

        while let Some(c) = self.current {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                value.push(c);
                self.advance();
            } else {
                break;
            }
        }

        value
    }

    // 文字列リテラル ("...") を読み取る。
    //
    // 呼び出し時点で self.current は開きの `"` を指している。
    // ループの各分岐は必ず self.advance() を呼ぶか break/panic するので、
    // 入力が尽きても current の値が変わらないまま回り続けることはない
    // (未終端の文字列や末尾の `\` は panic で確実に止める)。
    fn string_literal(&mut self) -> String {
        // opening quote
        self.advance();

        let mut value = String::new();

        loop {
            match self.current {
                None => {
                    panic!("unterminated string literal");
                }

                Some('"') => {
                    // closing quote
                    self.advance();
                    break;
                }

                Some('\\') => {
                    // backslash
                    self.advance();

                    match self.current {
                        None => {
                            panic!("unterminated string literal (trailing backslash)");
                        }
                        Some('n') => {
                            value.push('\n');
                            self.advance();
                        }
                        Some('t') => {
                            value.push('\t');
                            self.advance();
                        }
                        Some('r') => {
                            value.push('\r');
                            self.advance();
                        }
                        Some('0') => {
                            value.push('\0');
                            self.advance();
                        }
                        Some('\\') => {
                            value.push('\\');
                            self.advance();
                        }
                        Some('"') => {
                            value.push('"');
                            self.advance();
                        }
                        Some('\'') => {
                            value.push('\'');
                            self.advance();
                        }
                        Some(other) => {
                            // 未知のエスケープはそのまま文字として扱う
                            value.push(other);
                            self.advance();
                        }
                    }
                }

                Some(c) => {
                    value.push(c);
                    self.advance();
                }
            }
        }

        value
    }

    fn register(&mut self) -> String {
        // %
        self.advance();

        let mut value = String::from("%");

        while let Some(c) = self.current {
            if c.is_ascii_alphanumeric() {
                value.push(c);
                self.advance();
            } else {
                break;
            }
        }

        value
    }

    pub fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        loop {
            self.skip_spaces();

            let token = match self.current {
                None => Token::Eof,
                Some('\n') => {
                    self.advance();
                    Token::NewLine
                }
                Some(',') => {
                    self.advance();
                    Token::Comma
                }
                Some(':') => {
                    self.advance();
                    Token::Colon
                }
                Some('$') => {
                    self.advance();
                    Token::Dollar
                }
                Some('%') => Token::Register(self.register()),
                Some('"') => Token::Str(self.string_literal()),
                Some('(') => {
                    self.advance();
                    Token::LParen
                }
                Some(')') => {
                    self.advance();
                    Token::RParen
                }
                Some('+') => {
                    self.advance();
                    Token::Plus
                }
                Some('-') => {
                    self.advance();
                    Token::Minus
                }
                Some('*') => {
                    self.advance();
                    Token::Star
                }
                Some(c) if c.is_ascii_digit() => Token::Number(self.number()),
                Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '.' => {
                    Token::Ident(self.identifier())
                }
                Some(c) => {
                    panic!("unexpected character: {}", c);
                }
            };

            let end = token == Token::Eof;

            tokens.push(token);

            if end {
                break;
            }
        }

        tokens
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
    Instruction(Instruction),
    Label(String),
    Directive(String, Vec<String>),
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub mnemonic: String,
    pub operands: Vec<Operand>,
}

// ============================================================
// Operand
// ============================================================

#[derive(Debug, Clone)]
pub enum Operand {
    Register(String),

    Immediate(i64),

    Symbol(String),

    Str(String),

    Memory(MemoryOperand),
}

#[derive(Debug, Clone)]
pub struct MemoryOperand {
    pub displacement: i64,

    pub base: Option<String>,

    pub index: Option<String>,

    pub scale: u8,

    pub symbol: Option<String>,
}

// ============================================================
// Parser
// ============================================================

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
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

            token => {
                return Err(format!("expected identifier, got {:?}", token));
            }
        };

        // label:
        if matches!(self.current(), Token::Colon) {
            self.advance();

            return Ok(Statement::Label(name));
        }

        // instruction / directive
        //
        // `.align 4` や `.long 30` のように、ディレクティブの引数は
        // 括弧を伴わない裸の数値・文字列・シンボルを取る。これは
        // 「裸の数値は必ず `N(base,...)` の形のメモリオペランドである」
        // という通常命令向けの parse_operand の前提と衝突するため、
        // `.` で始まるディレクティブは専用のパーサに振り分ける。
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

        Ok(Statement::Instruction(Instruction {
            mnemonic: name,
            operands,
        }))
    }

    // ディレクティブの引数用オペランドパーサ。
    // 数値やシンボルを、メモリオペランド構文の括弧なしで受け付ける。
    fn parse_directive_operand(&mut self) -> Result<Operand, String> {
        match self.current() {
            Token::Str(_) => {
                if let Token::Str(value) = self.advance() {
                    Ok(Operand::Str(value))
                } else {
                    unreachable!()
                }
            }

            // `$30` のような書き方も許容する
            Token::Dollar => {
                self.advance();
                self.parse_directive_operand()
            }

            Token::Minus => {
                self.advance();

                match self.advance() {
                    Token::Number(n) => Ok(Operand::Immediate(-n)),
                    token => Err(format!("expected number, got {:?}", token)),
                }
            }

            Token::Plus => {
                self.advance();

                match self.advance() {
                    Token::Number(n) => Ok(Operand::Immediate(n)),
                    token => Err(format!("expected number, got {:?}", token)),
                }
            }

            Token::Number(_) => {
                if let Token::Number(n) = self.advance() {
                    Ok(Operand::Immediate(n))
                } else {
                    unreachable!()
                }
            }

            Token::Ident(_) => {
                if let Token::Ident(name) = self.advance() {
                    Ok(Operand::Symbol(name))
                } else {
                    unreachable!()
                }
            }

            token => Err(format!("invalid directive operand: {:?}", token)),
        }
    }

    fn parse_operand(&mut self) -> Result<Operand, String> {
        match self.current() {
            Token::Register(_) => {
                if let Token::Register(reg) = self.advance() {
                    Ok(Operand::Register(reg))
                } else {
                    unreachable!()
                }
            }
            Token::Dollar => {
                self.advance();
                self.parse_immediate()
            }
            // `foo` (シンボル単体) と `foo(%rip)` / `foo(%rax,%rcx,8)`
            // (シンボル+メモリオペランド) はどちらも識別子で始まるため、
            // 判定を parse_memory 側に一本化する。parse_memory は
            // 識別子の直後が `(` でなければそのまま Operand::Symbol を
            // 返すので、シンボル単体の挙動は変わらない。
            Token::Number(_) | Token::Minus | Token::Plus | Token::LParen | Token::Ident(_) => {
                self.parse_memory()
            }
            Token::Str(_) => {
                if let Token::Str(value) = self.advance() {
                    Ok(Operand::Str(value))
                } else {
                    unreachable!()
                }
            }

            token => Err(format!("invalid operand: {:?}", token)),
        }
    }

    fn parse_immediate(&mut self) -> Result<Operand, String> {
        let sign = match self.current() {
            Token::Minus => {
                self.advance();
                -1
            }

            Token::Plus => {
                self.advance();
                1
            }

            _ => 1,
        };

        match self.advance() {
            Token::Number(n) => Ok(Operand::Immediate(n * sign)),

            Token::Ident(name) => {
                // 将来 relocation 用に拡張可能
                Ok(Operand::Symbol(name))
            }

            token => Err(format!("expected immediate, got {:?}", token)),
        }
    }

    fn parse_memory(&mut self) -> Result<Operand, String> {
        let mut displacement = 0;

        // displacement
        match self.current() {
            Token::Number(n) => {
                displacement = *n;
                self.advance();
            }

            Token::Minus => {
                self.advance();

                match self.advance() {
                    Token::Number(n) => {
                        displacement = -n;
                    }

                    token => {
                        return Err(format!("expected number, got {:?}", token));
                    }
                }
            }

            Token::Plus => {
                self.advance();

                match self.advance() {
                    Token::Number(n) => {
                        displacement = n;
                    }

                    token => {
                        return Err(format!("expected number, got {:?}", token));
                    }
                }
            }

            _ => {}
        }

        // symbol displacement (e.g. `symbol(%rip)`, `symbol(%rax,%rcx,8)`)
        let mut symbol = None;

        if let Token::Ident(name) = self.current() {
            let name = name.clone();
            self.advance();

            if !matches!(self.current(), Token::LParen) {
                return Ok(Operand::Symbol(name));
            }

            symbol = Some(name);
        }

        // register単体
        if !matches!(self.current(), Token::LParen) {
            return Err(format!(
                "expected '(' for memory operand, got {:?}",
                self.current()
            ));
        }

        self.advance();

        let mut base = None;
        let mut index = None;
        let mut scale = 1;

        // base
        if let Token::Register(reg) = self.current() {
            base = Some(reg.clone());
            self.advance();
        }

        // ,
        if matches!(self.current(), Token::Comma) {
            self.advance();

            // index
            if let Token::Register(reg) = self.current() {
                index = Some(reg.clone());
                self.advance();
            } else {
                return Err("expected index register".into());
            }

            // ,
            if matches!(self.current(), Token::Comma) {
                self.advance();

                match self.advance() {
                    Token::Number(n) => {
                        if ![1, 2, 4, 8].contains(&n) {
                            return Err("invalid x86 scale".into());
                        }

                        scale = n as u8;
                    }

                    token => {
                        return Err(format!("expected scale, got {:?}", token));
                    }
                }
            }
        }

        self.consume(&Token::RParen)?;

        Ok(Operand::Memory(MemoryOperand {
            displacement,
            base,
            index,
            scale,
            symbol,
        }))
    }
}
