use super::*;

// ============================== 公開API ==============================

/// コンパイル済み正規表現。`new` と `replace_all` のみ実装。
pub struct Regex {
    root: Node,
    group_count: usize,
}

impl Regex {
    /// 正規表現パターンをコンパイルする。
    pub fn new(pattern: &str) -> Result<Regex, RegexError> {
        let mut parser = Parser::new(pattern);
        let root = parser.parse_alt()?;
        if parser.pos > parser.chars_len {
            return Err(RegexError(format!(
                "予期しない文字が {} 文字目にあります",
                parser.pos
            )));
        }
        Ok(Regex { root, group_count: parser.group_count })
    }

    /// `chars` の `start` 文字目以降で最初にマッチする位置を探す。
    /// 見つかれば各キャプチャグループの (開始, 終了) 文字インデックスを返す。
    /// 添字 0 が全体マッチに対応する。
    fn find_at(&self, chars: &[char], start: usize) -> Option<Caps> {
        for pos in start..=chars.len() {
            let mut caps: Caps = vec![None; self.group_count + 1];
            let mut k: Box<Cont> = Box::new(|end, _caps: &mut Caps| Some(end));
            if let Some(end) = match_node(&self.root, chars, pos, &mut caps, &mut *k) {
                caps[0] = Some((pos, end));
                return Some(caps);
            }
        }
        None
    }

    /// マッチした箇所をすべて `replacer` に置き換えた新しい文字列を返す。
    /// `replacer` には次の2種類を渡せる:
    ///   - `&str` : `$1`, `$2`, ... / `$$` を使ったテンプレート (`Captures::replace_all` に委譲)
    ///   - `FnMut(&Captures) -> String` のクロージャ : マッチ毎に呼ばれ、戻り値で置き換える
    pub fn replace_all<R: Replacer>(&self, text: &str, mut replacer: R) -> String {
        let chars: Vec<char> = text.chars().collect();
        let char_to_byte = build_char_to_byte(text);

        let mut result = String::new();
        let mut last_end = 0usize; // 文字インデックス
        let mut search_pos = 0usize;

        while search_pos <= chars.len() {
            let spans = match self.find_at(&chars, search_pos) {
                Some(s) => s,
                None => break,
            };
            let (m_start, m_end) = spans[0].unwrap();

            // マッチの直前までの部分をそのままコピー
            result.push_str(&text[char_to_byte[last_end]..char_to_byte[m_start]]);

            // このマッチのキャプチャから置換文字列を組み立てる
            let captures = Captures::new(text, spans);
            result.push_str(&replacer.replace(&captures));

            last_end = m_end;
            search_pos = if m_end > m_start { m_end } else { m_end + 1 };
        }

        result.push_str(&text[char_to_byte[last_end]..]);
        result
    }
}

/// `Regex::replace_all` に渡せる「置換の仕方」を表すトレイト。
/// 次の2つに対して実装されている:
///   - `&str`                      : `Captures::replace_all` によるテンプレート展開
///   - `FnMut(&Captures) -> String` : マッチ毎に呼ばれるクロージャ
pub trait Replacer {
    fn replace(&mut self, caps: &Captures) -> String;
}

impl<'r> Replacer for &'r str {
    fn replace(&mut self, caps: &Captures) -> String {
        caps.replace_all(self)
    }
}

impl<F> Replacer for F
where
    F: FnMut(&Captures) -> String,
{
    fn replace(&mut self, caps: &Captures) -> String {
        self(caps)
    }
}

/// 文字インデックス -> バイトオフセットの対応表を作る (&str のスライスに必要)。
fn build_char_to_byte(text: &str) -> Vec<usize> {
    let mut table = Vec::with_capacity(text.len() + 1);
    let mut byte_pos = 0usize;
    for c in text.chars() {
        table.push(byte_pos);
        byte_pos += c.len_utf8();
    }
    table.push(byte_pos);
    table
}

/// 1回のマッチのキャプチャ結果。`new` と `replace_all` のみ実装。
///
/// 添字 0 は全体マッチ、1以降は `(...)` の出現順に対応する。
/// マッチしなかった任意グループの位置は `None` になる。
pub struct Captures<'t> {
    text: &'t str,
    // 各キャプチャグループの (開始, 終了) 文字インデックス。0番目が全体マッチ。
    spans: Vec<Option<(usize, usize)>>,
    char_to_byte: Vec<usize>,
}

impl<'t> Captures<'t> {
    /// マッチ対象のテキスト全体と、各グループの (開始, 終了) 文字インデックスから
    /// `Captures` を組み立てる。`spans[0]` は全体マッチの範囲。
    pub fn new(text: &'t str, spans: Vec<Option<(usize, usize)>>) -> Captures<'t> {
        Captures { char_to_byte: build_char_to_byte(text), text, spans }
    }

    /// 添字 `i` のキャプチャの実際の文字列を返す。
    /// マッチしなかったグループ、または存在しない添字は `None`。
    fn get(&self, i: usize) -> Option<&'t str> {
        let (s, e) = (*self.spans.get(i)?)?;
        Some(&self.text[self.char_to_byte[s]..self.char_to_byte[e]])
    }

    /// `template` 中の `$1`, `$2`, ... を対応するキャプチャの文字列に、
    /// `$$` をリテラルの `$` に置き換えた文字列を返す。
    /// マッチしなかったグループを参照した場合は空文字列に置き換わる。
    pub fn replace_all(&self, template: &str) -> String {
        let chars: Vec<char> = template.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() {
                if chars[i + 1] == '$' {
                    out.push('$');
                    i += 2;
                    continue;
                }
                if unsafe {is_byte_digit(chars[i + 1] as u8) == 1} {
                    let mut j = i + 1;
                    let mut num = String::new();
                    while j < chars.len() && unsafe {is_byte_digit(chars[j] as u8) == 1} {
                        num.push(chars[j]);
                        j += 1;
                    }
                    let idx: usize = num.parse().unwrap();
                    if let Some(s) = self.get(idx) {
                        out.push_str(s);
                    }
                    i = j;
                    continue;
                }
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }
}

/// `caps[1]` のように添字アクセスできるようにする。
/// マッチしなかったグループや範囲外の添字を指定するとパニックする
/// (`regex` クレートの `Captures` と同じ挙動)。
impl<'t> std::ops::Index<usize> for Captures<'t> {
    type Output = str;
    fn index(&self, i: usize) -> &str {
        self.get(i)
            .unwrap_or_else(|| panic!("キャプチャグループ {} はマッチしていません", i))
    }
}

// ============================== 動作確認 ==============================

// fn main() {
//     let re = Regex::new(r"(\w+)@(\w+)\.com").unwrap();
//     println!("{}", re.replace_all("contact: alice@example.com or bob@test.com", "$1 [at] $2 [dot] com"));

//     let re2 = Regex::new(r"a+").unwrap();
//     println!("{}", re2.replace_all("baaaab aab", "X"));

//     let re3 = Regex::new(r"[0-9]{2,4}").unwrap();
//     println!("{}", re3.replace_all("id:1 code:12345 num:9", "#"));

//     let re4 = Regex::new(r"colou?r").unwrap();
//     println!("{}", re4.replace_all("color and colour", "COLOR"));

//     let re5 = Regex::new(r"^\s+|\s+$").unwrap();
//     println!("[{}]", re5.replace_all("   trim me   ", ""));

//     let re6 = Regex::new(r"cat|dog").unwrap();
//     println!("{}", re6.replace_all("I have a cat and a dog", "pet"));

//     // Captures を直接組み立てて使う例
//     // "2026-09-01" のうち "2026-09" の部分だけを1個のグループとして手動で表現
//     let text = "2026-09-01";
//     let spans: Vec<Option<(usize, usize)>> = vec![
//         Some((0, 10)), // 全体
//         Some((0, 7)),  // "2026-09"
//     ];
//     let caps = Captures::new(text, spans);
//     println!("{}", caps.replace_all("month=$1"));

//     // クロージャ版 replace_all + caps[1] インデックスアクセスの例
//     // (質問にあった asm オペランドの置き換えを単純化したもの)
//     let inline_var = Regex::new(r"\{(\w+)\}").unwrap();
//     let value = "mov {dst}, {src}, {dst}";
//     let mut operands: Vec<String> = Vec::new();
//     let mut parse_err: Option<String> = None;

//     let asm = inline_var.replace_all(value, |caps: &Captures| {
//         if parse_err.is_some() {
//             // すでにエラーが起きているので、これ以上解析しても意味が無い
//             return String::new();
//         }

//         let inner = &caps[1];

//         // ここが本来は Parser::parse_asm_operand(inner) に相当する部分
//         if inner.is_empty() {
//             parse_err = Some("empty operand".to_string());
//             return String::new();
//         }

//         let index = operands.len();
//         operands.push(inner.to_string());
//         format!("{{{}}}", index)
//     });

//     println!("asm={:?} operands={:?} err={:?}", asm, operands, parse_err);
// }
