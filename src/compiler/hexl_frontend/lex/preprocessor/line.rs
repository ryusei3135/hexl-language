//! `#line`(組み込み): 元のソースの今の行番号に置き換わる

/// `#line`の名前
pub(super) const NAME: &str = "line";

/// `#line`が置き換わる文字列(元ソース上の行番号)
pub(super) fn line_text(line: usize) -> String {
    line.to_string()
}
