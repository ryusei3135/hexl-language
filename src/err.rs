use crate::lex;

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub line: usize,
    pos: usize,
}

impl Span {
    pub fn new(line: &usize, pos: &usize) -> Self {
        Self {
            line: *line,
            pos: *pos,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SystemErr {
    FlagNotFound,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UnenclosedScope {
    Scope,
    Syntax(lex::Tkn),
}

/// 構文エラーを管理する
#[derive(Clone, Debug, PartialEq)]
pub enum SyntaxErr {
    Expected(char),
    Unexpected(lex::Tkn),
    DoubleTokenErr(lex::Tkn),
    UnenclosedScope(UnenclosedScope),
    UnexpectedTkn {
        /// 期待されたのに無かったトークン
        found: lex::Tkn,
        /// 期待したトークン
        expected: lex::Tkn,
        syntax: lex::Tkn,
    },
    UnexpectedEOF {
        expected: Vec<String>,
    },
    UnexpectedTokenAfterKeyword {
        /// 直前にあった予約語（例: "let", "fn", "if"）
        keyword: lex::Tkn,
        /// 本来期待されていたトークンの説明（例: "an identifier", "an expression"）
        expected: Vec<String>,
        /// 実際に解析された不正なトークン
        found: lex::Tkn,
    },
}

impl SyntaxErr {
    /// {}でスコープが閉じられていないときのエラー
    pub fn unenclosed_scope(span: Span, target: Option<lex::Tkn>) -> ErrKind {
        ErrKind::SyntaxErr {
            span,
            kind: Self::UnenclosedScope(
                target
                    .map(|v| UnenclosedScope::Syntax(v))
                    // Noneの場合はScopeになる
                    .or(Some(UnenclosedScope::Scope))
                    .unwrap(),
            ),
        }
    }

    pub fn unexpected_tkn(
        span: Span,
        found: lex::Tkn,
        expected: lex::Tkn,
        // どの構文でのエラーか
        syntax: lex::Tkn,
    ) -> ErrKind {
        ErrKind::SyntaxErr {
            span,
            kind: Self::UnexpectedTkn {
                found,
                expected,
                syntax,
            },
        }
    }
    /// 任意のトークンを期待したのに、トークンが
    /// 終了したとき
    pub fn tkn_is_eof(span: Span, expected: Vec<&'static str>) -> Result<lex::Tkn, ErrKind> {
        let node = ErrKind::SyntaxErr {
            span,
            kind: Self::UnexpectedEOF {
                expected: expected.into_iter().map(String::from).collect(),
            },
        };
        Err(node)
    }
    /// 期待したトークンと違うものが来た時のエラー
    pub fn unexpect_tkn_after_keyword(
        span: Span,
        keyword: lex::Tkn,
        expected: Vec<&str>,
        found: &lex::Tkn,
    ) -> Result<(), ErrKind> {
        let node = ErrKind::SyntaxErr {
            span,
            kind: Self::UnexpectedTokenAfterKeyword {
                keyword,
                expected: expected.into_iter().map(String::from).collect(),
                found: found.clone(),
            },
        };
        Err(node)
    }
}

/// プリプロセッサ関係のエラーコード
#[derive(Clone, Debug, PartialEq)]
pub enum PreprocErrs {
    // #asm
    ExpectedLParenAfterAsm,
    ExpectedRParenAfterAsm,
    NotFoundAsmName,

    // #asm の `${...}` 内の式
    EmptyAsmOperand,
    UnexpectedTokenInAsmOperand,
    ExpectedMemberNameInAsmOperand,
    ExpectedRParenInAsmOperand,
    ExpectedRBracketInAsmOperand,
    UnexpectedTrailingTokenInAsmOperand,

    // #include
    ExpectedPathSegment,
}

impl PreprocErrs {
    pub fn build(self, span: Span) -> ErrKind {
        ErrKind::PreprocErr { kind: self, span }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LexErr {
    ThisNumIsInvalid,
}

impl LexErr {
    pub fn fmt(self, line: &usize, pos: &usize) -> ErrKind {
        ErrKind::LexErrs {
            kind: self,
            line: line.clone(),
            pos: pos.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ErrKind {
    EndTkn,
    OptErr,
    SystemErr(SystemErr),
    UnexpectedToken,
    NotFoundTkn(lex::Tkn),
    SyntaxErr {
        /// エラーが発生したソースコード上の位置（行・列など）
        span: Span,
        kind: SyntaxErr,
    },
    PreprocErr {
        kind: PreprocErrs,
        span: Span,
    },
    MissingTknAfter(Option<lex::Tkn>),
    LexErrs {
        kind: LexErr,
        line: usize,
        pos: usize,
    },
}

impl ErrKind {
    /// エラーのバリアントを`Result`のエラーで包む
    pub fn wrap_in_err(self) -> Result<(), Self> {
        Err(self)
    }

    pub fn lex_err(&self) {
        if let Self::LexErrs { kind, line, pos } = self {
            match &kind {
                LexErr::ThisNumIsInvalid => {
                    println!("this num is invalid");
                    println!("line `{}`, pos `{}`", line, pos);
                }
            }
        }
    }
}
