use super::*;
use regex::{Captures, Regex};

impl AsmEmitter {
    /// アセンブリ言語のフォーマットをする際に、フォーマット
    /// ファイル側に書いてあるレジスタを埋め込む処理を実行
    pub fn replace_insert_fmt_reg(
        &self,
        val: &str,
        size: &types::Size,
    ) -> Result<String, err::ErrKind> {
        let insert_reg = Regex::new(r"%\{([^}]+)\}")
            .map_err(|e| err::ErrKind::asm_emit_invalid_reg_placeholder(e.to_string()))?;
        let mut parse_err: Option<err::ErrKind> = None;
        let replaced = insert_reg
            .replace_all(val, |caps: &Captures| {
                if parse_err.is_some() {
                    // すでにエラーが起きているので、これ以上解析しても意味が無い
                    return String::new();
                }

                let inner = &caps[1];
                match inner.parse::<usize>() {
                    Ok(reg_num) => format!("{}", self.asm_fmt.get_fmt_reg(reg_num, size)?),
                    Err(_) => {
                        parse_err = Some(err::ErrKind::asm_emit_invalid_reg_placeholder(
                            inner.to_string(),
                        ));
                        String::new()
                    }
                }
            })
            .to_string();
        match parse_err {
            Some(e) => Err(e),
            None => Ok(replaced),
        }
    }

    /// レジスタの「番号」だけを一時的に埋め込むための、目印付きの
    /// プレースホルダー文字列を作る。
    ///
    /// 例: `Self::insert_fmt_reg_placeholder(&3)` -> `"{{reg:3}}"`
    ///
    /// このプレースホルダーは、まだ実際のレジスタ名(サイズ込みの
    /// `%eax`のような文字列)へ展開されておらず、後で
    /// [`Self::replace_insert_fmt_reg`]によって展開される。
    pub fn insert_fmt_reg_placeholder(reg_num: usize) -> String {
        format!("%{{{}}}", reg_num)
    }
}
