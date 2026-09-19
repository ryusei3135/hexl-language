use crate::err;


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileErr {
    /// 不変の変数に値を代入しようとした
    AssignToImmutableVar(String),
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

    VariableConstractExpired(String),
    ReassignActiveContract(String),
}

#[macro_export]
macro_rules! GenCompileErr {
    ($kind:ident, $msg:expr) => {
        Err(
            err::ErrKind::CompileErr(
                CompileErr::$kind($msg.to_string())
            )
        )
    };
}

impl CompileErr {
    pub fn constract_expired(
        var_name: &String
    ) -> Result<Self, err::ErrKind> {
        Err(
            err::ErrKind::CompileErr(
                Self::VariableConstractExpired(var_name.to_string())
            )
        )
    }

    pub fn assign_to_imm_var(
        var_name: &String
    ) -> Result<Self, err::ErrKind> {
        Err(
            err::ErrKind::CompileErr(
                Self::AssignToImmutableVar(var_name.to_string())
            )
        )
    }

    pub fn ptr_range_len_mismatch(
        range: (usize, usize),
        value_len: usize,
    ) -> Result<Self, err::ErrKind> {
        Err(err::ErrKind::CompileErr(
            Self::PointerRangeLengthMismatch { range, value_len },
        ))
    }

    /// `of`の引数に、`must`ではない値が渡された
    pub fn constract_of_requires_must(
        fn_name: &String,
        param_name: &String,
        of_name: &str,
    ) -> Result<(), err::ErrKind> {
        Err(err::ErrKind::CompileErr(Self::ContractOfRequiresMust {
            fn_name: fn_name.to_string(),
            param_name: param_name.to_string(),
            of_name: of_name.to_string(),
        }))
    }

    /// `must`の値が、`of`ではない引数に渡された
    pub fn constract_must_requires_of(
        fn_name: &String,
        param_name: &String,
        must_name: &str,
    ) -> Result<(), err::ErrKind> {
        Err(err::ErrKind::CompileErr(Self::ContractMustRequiresOf {
            fn_name: fn_name.to_string(),
            param_name: param_name.to_string(),
            must_name: must_name.to_string(),
        }))
    }

    /// `must`と`of`の名前が一致していない
    pub fn constract_name_mismatch(
        fn_name: &String,
        param_name: &String,
        must_name: &str,
        of_name: &str,
    ) -> Result<(), err::ErrKind> {
        Err(err::ErrKind::CompileErr(Self::ContractNameMismatch {
            fn_name: fn_name.to_string(),
            param_name: param_name.to_string(),
            must_name: must_name.to_string(),
            of_name: of_name.to_string(),
        }))
    }

    /// `must`の値が、一度も関数へ渡されていない
    pub fn constract_must_not_used(
        fn_name: &String,
        var_name: &String,
    ) -> Result<(), err::ErrKind> {
        Err(err::ErrKind::CompileErr(Self::ContractMustNotUsed {
            fn_name: fn_name.to_string(),
            var_name: var_name.to_string(),
        }))
    }

    /// `must`の変数への再代入
    pub fn assign_to_must_var(
        var_name: &String
    ) -> Result<(), err::ErrKind> {
        Err(err::ErrKind::CompileErr(
            Self::AssignToMustVar(var_name.to_string())
        ))
    }

    /// エラーの内容を表示するための文字列を作る
    ///
    /// 既に`CompileErr`を表示する仕組み(`Display`の実装や
    /// `match`で分岐している関数)がある場合は、この関数は使わずに
    /// そちらへ上の各バリアントの分岐を足してください
    pub fn message(&self) -> String {
        match self {
            Self::AssignToImmutableVar(var_name) => {
                format!("不変の変数`{var_name}`には代入できません")
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
        }
    }
}