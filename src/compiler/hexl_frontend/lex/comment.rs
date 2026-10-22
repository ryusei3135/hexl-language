
/// ソースからコメントを取り除く関数
///
/// - `// ...` : 行末までのコメント(改行は残す)
/// - `/* ... */` : ブロックコメント(複数行可、入れ子は不可)
///
/// 行番号(`#line` やトークンのspan)がずれないよう、コメントの中にあった改行は
/// そのまま残す。ブロックコメント自体は空白1つに置き換える
/// (`a/**/b` が `ab` にくっつかず、`a b` の2トークンになる)。
/// 閉じていない `/*` は、ソースの末尾までをコメントとして読み飛ばす。
pub(super) fn strip_comments(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut in_str = false;

    while i < chars.len() {
        let c = chars[i];

        if in_str {
            out.push(c);
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }

        match (c, chars.get(i + 1).copied()) {
            ('"', _) => {
                in_str = true;
                out.push(c);
                i += 1;
            }
            // 行コメント: 改行の手前まで読み飛ばす(改行は次のループで出力される)
            ('/', Some('/')) => {
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            // ブロックコメント: `*/` の後ろまで読み飛ばす
            ('/', Some('*')) => {
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    if chars[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
                // `*/` を読み飛ばす(閉じていなければ末尾で終わる)
                i = (i + 2).min(chars.len());
                out.push(' ');
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::strip_comments;

    #[test]
    fn line_comment_is_removed_and_newline_kept() {
        assert_eq!(strip_comments("a // b\nc"), "a \nc");
    }

    #[test]
    fn line_comment_at_eof() {
        assert_eq!(strip_comments("a // b"), "a ");
    }

    #[test]
    fn block_comment_is_replaced_with_space() {
        assert_eq!(strip_comments("a/* x */b"), "a b");
    }

    #[test]
    fn block_comment_keeps_newlines() {
        assert_eq!(strip_comments("a/* x\ny\nz */b"), "a\n\n b");
    }

    #[test]
    fn slash_star_slash_does_not_close_itself() {
        assert_eq!(strip_comments("a/*/ b */c"), "a c");
    }

    #[test]
    fn line_comment_marker_inside_block_comment_is_ignored() {
        assert_eq!(strip_comments("a/* // */b"), "a b");
    }

    #[test]
    fn block_comment_marker_inside_line_comment_is_ignored() {
        assert_eq!(strip_comments("a // /* \nb"), "a \nb");
    }

    #[test]
    fn comment_markers_inside_string_are_kept() {
        assert_eq!(strip_comments("\"a//b\" \"c/*d*/\""), "\"a//b\" \"c/*d*/\"");
    }

    #[test]
    fn division_is_kept() {
        assert_eq!(strip_comments("a / b /= c"), "a / b /= c");
    }

    #[test]
    fn unterminated_block_comment_runs_to_eof() {
        assert_eq!(strip_comments("a/* b\nc"), "a\n ");
    }
}
