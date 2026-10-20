use std::rc::Rc;

use crate::parse::node;

use crate::err::{self, Span, ErrKind};

/// コンパイルエラー本体。発生位置(`span`)を伴う
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileErr {
    pub kind: CompileErrKind,
    pub span: Rc<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileErrKind {
    /// 配列の指定しているindexが数字じゃない
    InvalidIndexType {
        val: String,
    },
    /// 配列のindexの型が符号なし整数じゃない
    InvalidIndexUnsigned(String),
    /// 配列の中に使えない式がある
    UnexpectArrIndexExprKind(node::Expr),
    InvalidIndexIsNotConst(String),
    ArrElseIndexNotFound,
    /// 不変の変数に値を代入しようとした
    AssignToImmutableVar(String),
    ArrIndexIsOutBounds(String),
    /// ポインタに指定された範囲と初期値の長さが一致しない
    PointerRangeLengthMismatch {
        range: (usize, usize),
        value_len: usize,
    },
    /// `of`の引数に、`must`ではない値を渡した
    ContractOfRequiresMust {
        fn_name: String,
        param_name: String,
        of_name: String,
    },
    /// `must`の値を、`of`ではない引数に渡した
    ContractMustRequiresOf {
        fn_name: String,
        param_name: String,
        must_name: String,
    },
    /// `must`と`of`の契約の名前が一致していない
    ContractNameMismatch {
        fn_name: String,
        param_name: String,
        must_name: String,
        of_name: String,
    },
    /// `must`の値が、一度も`of`の引数として使われていない
    ContractMustNotUsed {
        fn_name: String,
        var_name: String,
    },
    /// `must`の契約を持つ変数へ再代入しようとした
    AssignToMustVar(String),
    /// `++`/`--`を整数以外の変数に使おうとした
    IncDecRequiresInteger(String),

    VariableConstractExpired(String),
    ReassignActiveContract(String),
}

/// `GenCompileErr!(Kind, msg, span)`
#[macro_export]
macro_rules! GenCompileErr {
    ($kind:ident, $msg:expr, $span:expr) => {
        Err(err::ErrKind::CompileErr(Box::new(CompileErr {
            kind: CompileErrKind::$kind($msg.to_string()),
            span: $span,
        })))
    };
}

/// もじれつじゃないエラーノードを生成する
/// `GenCompileErrNode!(Kind, node, span)`
#[macro_export]
macro_rules! GenCompileErrNode {
    ($kind:ident, $msg:expr, $span:expr) => {
        Err(err::ErrKind::CompileErr(Box::new(CompileErr {
            kind: CompileErrKind::$kind($msg.clone()),
            span: $span,
        })))
    };
}

/// 渡された`CompileErrKind`と`span`から、直接エラーを生成する
/// `GenCompileErr2!(CompileErrKind::Xxx { .. }, span)`
#[macro_export]
macro_rules! GenCompileErr2 {
    ($kind:expr, $span:expr) => {
        Err(err::ErrKind::CompileErr(Box::new(CompileErr {
            kind: $kind,
            span: $span,
        })))
    };
}

impl CompileErr {
    pub fn new(kind: CompileErrKind, span: Rc<Span>) -> Self {
        Self { kind, span }
    }

    /// エラーの発生位置
    pub fn span(&self) -> &Rc<Span> {
        &self.span
    }

    fn err<T>(kind: CompileErrKind, span: Rc<Span>) -> Result<T, err::ErrKind> {
        Err(err::ErrKind::CompileErr(Box::new(Self { kind, span })))
    }

    pub fn assign_to_imm_var(var_name: &str, span: Rc<Span>) -> Result<Self, err::ErrKind> {
        Self::err(
            CompileErrKind::AssignToImmutableVar(var_name.to_string()),
            span,
        )
    }

    pub fn ptr_range_len_mismatch(
        range: (usize, usize),
        value_len: usize,
        span: Rc<Span>,
    ) -> Result<Self, err::ErrKind> {
        Self::err(
            CompileErrKind::PointerRangeLengthMismatch { range, value_len },
            span,
        )
    }

    /// `of`の引数に、`must`ではない値が渡された
    pub fn constract_of_requires_must(
        fn_name: &str,
        param_name: &str,
        of_name: &str,
        span: Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        Self::err(
            CompileErrKind::ContractOfRequiresMust {
                fn_name: fn_name.to_string(),
                param_name: param_name.to_string(),
                of_name: of_name.to_string(),
            },
            span,
        )
    }

    /// `must`の値が、`of`ではない引数に渡された
    pub fn constract_must_requires_of(
        fn_name: &str,
        param_name: &str,
        must_name: &str,
        span: Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        Self::err(
            CompileErrKind::ContractMustRequiresOf {
                fn_name: fn_name.to_string(),
                param_name: param_name.to_string(),
                must_name: must_name.to_string(),
            },
            span,
        )
    }

    /// `must`と`of`の名前が一致していない
    pub fn constract_name_mismatch(
        fn_name: &str,
        param_name: &str,
        must_name: &str,
        of_name: &str,
        span: Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        Self::err(
            CompileErrKind::ContractNameMismatch {
                fn_name: fn_name.to_string(),
                param_name: param_name.to_string(),
                must_name: must_name.to_string(),
                of_name: of_name.to_string(),
            },
            span,
        )
    }

    /// `must`の値が、一度も関数へ渡されていない
    pub fn constract_must_not_used(
        fn_name: &str,
        var_name: &str,
        span: Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        Self::err(
            CompileErrKind::ContractMustNotUsed {
                fn_name: fn_name.to_string(),
                var_name: var_name.to_string(),
            },
            span,
        )
    }

    /// `must`の変数への再代入
    pub fn assign_to_must_var(var_name: &str, span: Rc<Span>) -> Result<(), err::ErrKind> {
        Self::err(CompileErrKind::AssignToMustVar(var_name.to_string()), span)
    }

    pub fn inc_dec_requires_integer(var_name: &str, span: Rc<Span>) -> err::ErrKind {
        err::ErrKind::CompileErr(Box::new(Self {
            kind: CompileErrKind::IncDecRequiresInteger(var_name.to_string()),
            span,
        }))
    }

    /// エラーの内容を表示するための文字列を作る
    ///
    /// 既に`CompileErr`を表示する仕組み(`Display`の実装や
    /// `match`で分岐している関数)がある場合は、この関数は使わずに
    /// そちらへ上の各バリアントの分岐を足してください
    pub fn message(&self) -> String {
        self.kind.message()
    }
}

impl CompileErrKind {
    /// エラーの内容を表示するための文字列を作る
    pub fn message(&self) -> String {
        match self {
            Self::AssignToImmutableVar(var_name) => {
                format!("不変の変数`{var_name}`には代入できません")
            }
            Self::IncDecRequiresInteger(var_name) => {
                format!("`++`/`--`は整数型の変数`{var_name}`にのみ使用できます")
            }
            Self::PointerRangeLengthMismatch { range, value_len } => {
                format!(
                    "ポインタの範囲`{:?}`と初期値の長さ`{}`が一致しません",
                    range, value_len
                )
            }
            Self::ContractOfRequiresMust {
                fn_name,
                param_name,
                of_name,
            } => {
                format!(
                    "関数`{fn_name}`の引数`{param_name}`は`of {of_name}`です\n\
                     `must {of_name}`の値しか渡せません"
                )
            }
            Self::ContractMustRequiresOf {
                fn_name,
                param_name,
                must_name,
            } => {
                format!(
                    "`must {must_name}`の値は、`of {must_name}`の引数にしか渡せません\n\
                     関数`{fn_name}`の引数`{param_name}`には契約がありません"
                )
            }
            Self::ContractNameMismatch {
                fn_name,
                param_name,
                must_name,
                of_name,
            } => {
                format!(
                    "契約の名前が一致しません\n\
                     関数`{fn_name}`の引数`{param_name}`: `of {of_name}`\n\
                     渡された値: `must {must_name}`"
                )
            }
            Self::ContractMustNotUsed { fn_name, var_name } => {
                format!(
                    "`must`の変数`{var_name}`が、関数`{fn_name}`の中で\n\
                     一度も`of`の引数として使われていません"
                )
            }
            Self::AssignToMustVar(var_name) => {
                format!("`must`の契約を持つ変数`{var_name}`には再代入できません")
            }
            Self::VariableConstractExpired(var_name) => {
                format!("契約が終了仕手います{var_name}")
            }
            Self::ReassignActiveContract(var_name) => {
                format!("{var_name}")
            }
            t => panic!("{:?}", t),
        }
    }
}