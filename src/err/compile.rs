use crate::err;


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileErr {
    /// 不変の変数に値を代入しようとした
    AssignToImmutableVar(String),
}

impl CompileErr {
    pub fn assign_to_imm_var(var_name: &String) -> Result<Self, err::ErrKind> {
        Err(
            err::ErrKind::CompileErr(
                Self::AssignToImmutableVar(var_name.to_string())
            )
        )
    }
}