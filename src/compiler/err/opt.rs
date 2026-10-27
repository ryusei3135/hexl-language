use std::fmt;

/// コマンドライン引数のパース中に発生するエラー
#[derive(Debug, Clone, PartialEq)]
pub enum OptErrs {
    /// 指定されたオプションが存在しない
    NotFoundOpt(String),
    OptFlags,
}

impl fmt::Display for OptErrs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFoundOpt(opt) => {
                write!(f, "{}: オプション`{}`が見つかりません", super::CAT_OPT, opt)
            }
            Self::OptFlags => write!(f, "{}: オプションの指定方法が不正です", super::CAT_OPT),
        }
    }
}

impl std::error::Error for OptErrs {}
