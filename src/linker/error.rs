//! リンカ全体で使うエラー型。

use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, LinkError>;

/// 未定義シンボルの参照 1 件分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndefinedRef {
    /// 未定義だったシンボル名
    pub symbol: String,
    /// そのシンボルを参照していたオブジェクト名
    pub object: String,
}

#[derive(Debug)]
pub enum LinkError {
    /// 入力ファイルが壊れている / ELF・COFF として不正
    Parse { object: String, message: String },
    /// 形式としては正しいが、このリンカが未対応の機能を含む
    Unsupported { object: String, message: String },
    /// 強いシンボルが複数のオブジェクトで定義されている
    DuplicateSymbol {
        name: String,
        first: String,
        second: String,
    },
    /// 解決できなかったシンボル参照 (すべて集めて 1 回で報告する)
    UndefinedSymbols(Vec<UndefinedRef>),
    /// エントリポイントのシンボルが見つからない
    EntryNotFound(String),
    /// リロケーションの値が書き込み先の幅に収まらない
    RelocationOverflow {
        object: String,
        section: String,
        offset: u64,
        reloc: &'static str,
        symbol: String,
        value: i128,
    },
    /// オプションの値が不正
    InvalidOption(String),
    /// ファイル入出力エラー
    Io { path: String, source: io::Error },
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::Parse { object, message } => write!(f, "{object}: {message}"),
            LinkError::Unsupported { object, message } => {
                write!(f, "{object}: unsupported: {message}")
            }
            LinkError::DuplicateSymbol {
                name,
                first,
                second,
            } => write!(
                f,
                "duplicate symbol `{name}': defined in {first} and in {second}"
            ),
            LinkError::UndefinedSymbols(list) => {
                for (i, u) in list.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{}: undefined reference to `{}'", u.object, u.symbol)?;
                }
                Ok(())
            }
            LinkError::EntryNotFound(name) => {
                write!(f, "entry symbol `{name}' is not defined")
            }
            LinkError::RelocationOverflow {
                object,
                section,
                offset,
                reloc,
                symbol,
                value,
            } => write!(
                f,
                "{object}({section}+{offset:#x}): relocation {reloc} against `{symbol}' \
                 overflows (value {value:#x} does not fit)"
            ),
            LinkError::InvalidOption(msg) => write!(f, "invalid option: {msg}"),
            LinkError::Io { path, source } => write!(f, "{path}: {source}"),
        }
    }
}

impl std::error::Error for LinkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LinkError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
