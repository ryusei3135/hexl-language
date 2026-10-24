//! C言語の構文木(CAST)。パーサはまずこれを作り、`lower.rs`が`node`へ変換する。
//! `node`に無い情報(typedef・前置/後置の違い・`for`など)もここでは失わない。

#[derive(Clone, Debug, PartialEq)]
pub enum CType {
    /// `int` / `unsigned long` / 構造体・共用体・列挙型のタグ名(typedefは展開済み)
    Named { name: String, konst: bool },
    /// ポインタ。`konst`はポインタ自身が`const`(`T *const`)
    Ptr { inner: Box<CType>, konst: bool },
    Array {
        inner: Box<CType>,
        len: Option<usize>,
    },
    Func {
        ret: Box<CType>,
        params: Vec<CParam>,
        variadic: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CParam {
    pub name: Option<String>,
    pub ty: CType,
}

impl CType {
    pub fn named(name: &str) -> CType {
        CType::Named {
            name: name.to_string(),
            konst: false,
        }
    }

    /// 最上位の`const`を立てる
    pub fn with_const(mut self) -> CType {
        match &mut self {
            CType::Named { konst, .. } | CType::Ptr { konst, .. } => *konst = true,
            _ => {}
        }
        self
    }

    /// 最上位が`const`か
    pub fn is_const(&self) -> bool {
        match self {
            CType::Named { konst, .. } | CType::Ptr { konst, .. } => *konst,
            _ => false,
        }
    }

    /// 組み込み型・ポインタ・配列の`(大きさ, 境界)`(バイト、LP64)
    ///
    /// 構造体・共用体・列挙型(タグ名)は定義が要るので`None`
    /// (`Lowerer::size_align`が定義を見て計算する)。`void`・関数型・大きさの無い配列も`None`
    pub fn prim_size_align(&self) -> Option<(u64, u64)> {
        match self {
            CType::Named { name, .. } => {
                let n = match name.as_str() {
                    "char" | "signed char" | "unsigned char" | "bool" => 1,
                    "short" | "unsigned short" => 2,
                    "int" | "unsigned int" | "float" => 4,
                    "long" | "unsigned long" | "long long" | "unsigned long long" | "double" => 8,
                    "long double" => 16,
                    _ => return None,
                };
                Some((n, n))
            }
            CType::Ptr { .. } => Some((8, 8)),
            CType::Array { inner, len: Some(n) } => {
                let (size, align) = inner.prim_size_align()?;
                Some((size.checked_mul(*n as u64)?, align))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Plus,
    Neg,
    Not,
    BitNot,
    Deref,
    Addr,
    PreInc,
    PreDec,
    PostInc,
    PostDec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Shl,
    Shr,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitXor,
    BitOr,
    LogAnd,
    LogOr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CExpr {
    pub line: usize,
    pub kind: CExprKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CExprKind {
    Ident(String),
    /// 列挙定数(パース時に解決済み): 列挙型名, メンバー名
    EnumConst {
        ename: String,
        variant: String,
    },
    /// 10進に直した整数(サフィックスは捨てる)
    Int(String),
    Float(String),
    /// 文字定数の値
    Char(i64),
    /// エスケープ展開済みの文字列
    Str(String),
    Unary(UnOp, Box<CExpr>),
    Binary(BinOp, Box<CExpr>, Box<CExpr>),
    /// `a = b`(`None`) / `a += b`(`Some(Add)`)
    Assign(Option<BinOp>, Box<CExpr>, Box<CExpr>),
    Cond(Box<CExpr>, Box<CExpr>, Box<CExpr>),
    Call(Box<CExpr>, Vec<CExpr>),
    Index(Box<CExpr>, Box<CExpr>),
    /// `a.b`(`arrow == false`) / `a->b`(`arrow == true`)
    Member(Box<CExpr>, String, bool),
    Cast(CType, Box<CExpr>),
    SizeofType(CType),
    SizeofExpr(Box<CExpr>),
    Comma(Vec<CExpr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Designator {
    Field(String),
    Index(i64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CInit {
    Expr(CExpr),
    List(Vec<(Vec<Designator>, CInit)>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    None,
    Static,
    Extern,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CDecl {
    pub line: usize,
    pub name: String,
    pub ty: CType,
    pub storage: Storage,
    pub init: Option<CInit>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CStmt {
    pub line: usize,
    pub kind: CStmtKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CStmtKind {
    Decl(Vec<CDecl>),
    Expr(Option<CExpr>),
    Block(Vec<CStmt>),
    If(CExpr, Box<CStmt>, Option<Box<CStmt>>),
    Switch(CExpr, Box<CStmt>),
    While(CExpr, Box<CStmt>),
    DoWhile(Box<CStmt>, CExpr),
    For {
        init: Option<Box<CStmt>>,
        cond: Option<CExpr>,
        step: Option<CExpr>,
        body: Box<CStmt>,
    },
    Return(Option<CExpr>),
    Break,
    Continue,
    Goto(String),
    /// `name:`(本体は後ろの兄弟ステートメント)
    Label(String),
    /// `case expr:`(本体は後ろの兄弟ステートメント)
    Case(CExpr),
    /// `default:`
    Default,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CField {
    pub name: String,
    pub ty: CType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CDef {
    Struct {
        name: String,
        fields: Vec<CField>,
    },
    Union {
        name: String,
        fields: Vec<CField>,
    },
    /// (メンバー名, 値)
    Enum {
        name: String,
        members: Vec<(String, i64)>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CFunc {
    pub line: usize,
    pub name: String,
    pub ret: CType,
    pub params: Vec<CParam>,
    pub variadic: bool,
    pub is_static: bool,
    pub body: Vec<CStmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CItem {
    Def(CDef),
    Func(CFunc),
    /// 本体の無い関数宣言(プロトタイプ)。nodeに対応するものが無いので読み飛ばす
    Proto {
        name: String,
    },
    /// `extern`の変数宣言。読み飛ばす
    ExternVar {
        name: String,
    },
    /// 本体のあるグローバル変数(nodeに対応するものが無い)
    Global(CDecl),
}
