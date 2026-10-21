use std::rc::Rc;

use crate::parse::node;

use crate::compiler::err;

/// コンパイルエラー本体。発生位置(`span`)を伴う
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileErr {
    pub kind: CompileErrKind,
    pub span: Rc<err::Span>,
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
    pub fn new(kind: CompileErrKind, span: Rc<err::Span>) -> Self {
        Self { kind, span }
    }

    /// エラーの発生位置
    pub fn span(&self) -> &Rc<err::Span> {
        &self.span
    }

    fn err<T>(kind: CompileErrKind, span: Rc<err::Span>) -> Result<T, err::ErrKind> {
        Err(err::ErrKind::CompileErr(Box::new(Self { kind, span })))
    }

    pub fn assign_to_imm_var(var_name: &str, span: Rc<err::Span>) -> Result<Self, err::ErrKind> {
        Self::err(
            CompileErrKind::AssignToImmutableVar(var_name.to_string()),
            span,
        )
    }

    pub fn ptr_range_len_mismatch(
        range: (usize, usize),
        value_len: usize,
        span: Rc<err::Span>,
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
        span: Rc<err::Span>,
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
        span: Rc<err::Span>,
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
        span: Rc<err::Span>,
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
        span: Rc<err::Span>,
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
    pub fn assign_to_must_var(var_name: &str, span: Rc<err::Span>) -> Result<(), err::ErrKind> {
        Self::err(CompileErrKind::AssignToMustVar(var_name.to_string()), span)
    }

    pub fn inc_dec_requires_integer(var_name: &str, span: Rc<err::Span>) -> err::ErrKind {
        err::ErrKind::CompileErr(Box::new(Self {
            kind: CompileErrKind::IncDecRequiresInteger(var_name.to_string()),
            span,
        }))
    }

    /// Rust風のエラー表示を作る
    ///
    /// ```text
    /// error[E0003]: 不変の変数`x`には代入できません
    ///   --> <span>
    ///    = note: ...
    ///    = help: ...
    /// ```
    ///
    /// `err::Span`の中身(行・列)に依存しないよう、位置は`Debug`表示で出す。
    /// `Span`に行/列/ソースがあるなら、`location`をそこから作る形に差し替えると
    /// ソースの抜粋(`3 | x = 1`と`^^^`)まで出せる
    pub fn message(&self) -> String {
        let location = format!("{:?}", self.span);
        self.kind.render(&location)
    }
}

impl CompileErrKind {
    /// エラーコード(`E0001`形式)
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidIndexType { .. } => "E0001",
            Self::InvalidIndexUnsigned(..) => "E0002",
            Self::UnexpectArrIndexExprKind(..) => "E0003",
            Self::InvalidIndexIsNotConst(..) => "E0004",
            Self::ArrElseIndexNotFound => "E0005",
            Self::AssignToImmutableVar(..) => "E0006",
            Self::ArrIndexIsOutBounds(..) => "E0007",
            Self::PointerRangeLengthMismatch { .. } => "E0008",
            Self::ContractOfRequiresMust { .. } => "E0009",
            Self::ContractMustRequiresOf { .. } => "E0010",
            Self::ContractNameMismatch { .. } => "E0011",
            Self::ContractMustNotUsed { .. } => "E0012",
            Self::AssignToMustVar(..) => "E0013",
            Self::IncDecRequiresInteger(..) => "E0014",
            Self::VariableConstractExpired(..) => "E0015",
            Self::ReassignActiveContract(..) => "E0016",
        }
    }

    /// 1行の見出し(`error[..]: `の後ろに出る文)
    pub fn title(&self) -> String {
        match self {
            Self::InvalidIndexType { val } => {
                format!("配列の添字`{val}`が数値ではありません")
            }
            Self::InvalidIndexUnsigned(name) => {
                format!("配列の添字`{name}`が符号なし整数ではありません")
            }
            Self::UnexpectArrIndexExprKind(_) => "配列の添字に使えない式があります".to_string(),
            Self::InvalidIndexIsNotConst(name) => {
                format!("配列の添字`{name}`が`const`ではありません")
            }
            Self::ArrElseIndexNotFound => {
                "添字が可変な配列アクセスに`else`の添字がありません".to_string()
            }
            Self::AssignToImmutableVar(name) => {
                format!("不変の変数`{name}`には代入できません")
            }
            Self::ArrIndexIsOutBounds(detail) => {
                format!("配列の添字が範囲外です: {detail}")
            }
            Self::PointerRangeLengthMismatch { range, value_len } => {
                format!(
                    "ポインタの範囲`{}..{}`に対して、値`{}`が範囲外です",
                    range.0, range.1, value_len
                )
            }
            Self::ContractOfRequiresMust {
                fn_name,
                param_name,
                of_name,
            } => format!(
                "関数`{fn_name}`の引数`{param_name}`(`of {of_name}`)に、`must {of_name}`ではない値が渡されました"
            ),
            Self::ContractMustRequiresOf {
                fn_name,
                param_name,
                must_name,
            } => format!(
                "`must {must_name}`の値が、`of`ではない引数`{param_name}`(関数`{fn_name}`)に渡されました"
            ),
            Self::ContractNameMismatch {
                fn_name,
                param_name,
                must_name,
                of_name,
            } => format!(
                "契約の名前が一致しません(関数`{fn_name}`の引数`{param_name}`)\n\
                 期待: `of {of_name}` / 渡された値: `must {must_name}`"
            ),
            Self::ContractMustNotUsed { fn_name, var_name } => {
                format!("`must`の変数`{var_name}`が、関数`{fn_name}`の中で一度も使われていません")
            }
            Self::AssignToMustVar(name) => {
                format!("`must`の契約を持つ変数`{name}`には再代入できません")
            }
            Self::IncDecRequiresInteger(name) => {
                format!("`++`/`--`を整数型ではない変数`{name}`に使っています")
            }
            Self::VariableConstractExpired(name) => {
                format!("変数`{name}`の契約は既に終了しています")
            }
            Self::ReassignActiveContract(name) => {
                format!("契約が有効なままの変数`{name}`に再代入しています")
            }
        }
    }

    /// `= note:`に出す補足(原因の説明)
    pub fn note(&self) -> Option<String> {
        Some(match self {
            Self::InvalidIndexType { .. } => {
                "添字には数値(または数値型の変数)だけを使えます".into()
            }
            Self::InvalidIndexUnsigned(..) => {
                "添字の型は`u8`/`u16`/`u32`/`u64`のいずれかである必要があります".into()
            }
            Self::UnexpectArrIndexExprKind(expr) => format!(
                "関数呼び出し・メンバーアクセス・比較などは、値を静的に決められないため使えません\n   = note: 該当する式: {expr:?}"
            ),
            Self::InvalidIndexIsNotConst(..) => {
                "可変な添字は、`else`の添字を付けると実行時に範囲を検査します".into()
            }
            Self::ArrElseIndexNotFound => {
                "添字が`const`/即値ではないため、範囲外のときに使う添字が必要です".into()
            }
            Self::AssignToImmutableVar(..) => "この変数は`mut`なしで定義されています".into(),
            Self::PointerRangeLengthMismatch { .. } => {
                "有効な範囲は`開始..終了`で、終了は含まれません".into()
            }
            Self::ContractOfRequiresMust { of_name, .. } => {
                format!("`of {of_name}`の引数には、`must {of_name}`の契約を持つ値しか渡せません")
            }
            Self::ContractMustRequiresOf { must_name, .. } => {
                format!(
                    "`must {must_name}`の値は、`of {must_name}`の引数に渡して契約を果たす必要があります"
                )
            }
            Self::AssignToMustVar(..) => "`must`の変数は一度だけ`of`の引数として使えます".into(),
            Self::VariableConstractExpired(..) => {
                "契約は`of`の引数として渡された時点で終了します".into()
            }
            _ => return None,
        })
    }

    /// `= help:`に出す修正案
    pub fn help(&self) -> Option<String> {
        Some(match self {
            Self::InvalidIndexUnsigned(name) => {
                format!("`{name}`を符号なし整数型(例: `u64`)で定義してください")
            }
            Self::InvalidIndexIsNotConst(..) => {
                "`arr[index else 0]`のように`else`の添字を追加してください".into()
            }
            Self::ArrElseIndexNotFound => {
                "`[arr i else 0]`のように、範囲内の`const`/即値を`else`に指定してください".into()
            }
            Self::AssignToImmutableVar(name) => {
                format!("`{name}`を`{name} mut: 型 = 値`のように`mut`で定義してください")
            }
            Self::ContractOfRequiresMust { of_name, .. } => {
                format!("`must {of_name}`の契約を持つ変数を渡してください")
            }
            Self::ContractMustRequiresOf { must_name, .. } => {
                format!("`of {must_name}`の引数へ渡してください")
            }
            Self::ContractMustNotUsed { var_name, .. } => {
                format!("`{var_name}`を`of`の引数として関数へ渡してください")
            }
            Self::IncDecRequiresInteger(..) => "整数型の変数に使ってください".into(),
            _ => return None,
        })
    }

    /// Rust風の表示を組み立てる。`location`は発生位置の文字列
    pub fn render(&self, location: &str) -> String {
        let mut out = format!(
            "error[{}]: {}\n  --> {}",
            self.code(),
            self.title(),
            location
        );
        if let Some(note) = self.note() {
            out.push_str(&format!("\n   = note: {note}"));
        }
        if let Some(help) = self.help() {
            out.push_str(&format!("\n   = help: {help}"));
        }
        out
    }

    /// 見出しだけの文字列
    pub fn message(&self) -> String {
        self.title()
    }
}
