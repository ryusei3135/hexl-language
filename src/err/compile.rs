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
}

impl CompileErr {
    pub fn assign_to_imm_var(var_name: &String) -> Result<Self, err::ErrKind> {
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
}