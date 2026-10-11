use std::collections::HashMap;

/// `#define NAME VALUE` の右辺として実際に登録される値。
/// 今後 `define` 以外のプリプロセッサ命令が増える前提のため、
/// 素の `String` ではなく列挙体として持たせている。
/// (※ `crate::compiler::models` に既に同名の型がある場合はそちらに合わせて削除してください)
#[derive(Clone, Debug, PartialEq)]
pub(in crate::compiler::lex) 
enum ReplaceVal {
    Str(String),
}

pub(in crate::compiler::lex) 
enum PreprocKind {
    Define(ReplaceVal),
}

pub(in crate::compiler::lex) 
struct Preprocessor {
    tables: HashMap<String, PreprocKind>,
}

impl Preprocessor {
    pub(in crate::compiler::lex) 
    fn new() -> Self {
        Self {
            tables: HashMap::new(),
        }
    }

    /// 名前がまだ登録されていなければ追加する。
    /// 仕様には書かれていないが、同名の再定義は無視する
    /// (元の `add` の `is_none()` チェックを踏襲)。
    pub(in crate::compiler::lex) 
    fn add(&mut self, name: &str, preproc: PreprocKind) {
        if self.tables.get(name).is_none() {
            self.tables.insert(name.to_string(), preproc);
        }
    }

    /// `#NAME` を実際の値に展開する。
    /// 仕様どおり、登録されていない名前が来たら panic する。
    pub(in crate::compiler::lex) 
    fn resolve(&self, name: &str) -> &ReplaceVal {
        match self.tables.get(name) {
            Some(PreprocKind::Define(val)) => &val,
            None => panic!("undefined preprocessor symbol: `{}`", name),
        }
    }

    /// `#if #NAME` 用: panicせずに「定義済みかどうか」だけを調べる。
    pub(in crate::compiler::lex) 
    fn is_defined(&self, name: &str) -> bool {
        self.tables.get(name).is_some()
    }

    /// プリプロセッサを処理し、展開済みのソースを作る。
    ///
    /// 対応する行頭ディレクティブ:
    /// - `#define NAME VALUE`: `Preprocessor` に登録する
    /// - `#if #NAME` / `#else` / `#endif`: `NAME` が定義済みなら `#if`〜`#else`間を、
    ///   未定義なら `#else`〜`#endif`間を有効にする(`#else`は省略可、入れ子可)
    pub(in crate::compiler::lex) 
    fn preprocess(&mut self, content: &str) -> Option<String> {
        /// `#if` / `#else` の入れ子1段分の状態。
        struct CondFrame {
            /// 外側のスコープ自体が有効かどうか
            parent_active: bool,
            /// `#if #NAME` の条件(NAMEが定義済みか)
            condition: bool,
            /// すでに `#else` を通過したか
            in_else: bool,
        }
        impl CondFrame {
            fn active(&self) -> bool {
                self.parent_active && (self.condition != self.in_else)
            }
        }
        fn is_active(stack: &[CondFrame]) -> bool {
            stack.last().map(CondFrame::active).unwrap_or(true)
        }

        let chars: Vec<char> = content.chars().collect();
        let mut output = String::new();
        let mut i = 0;
        // 現在位置までの行内が空白だけ(＝まだ行頭)かどうか
        let mut at_line_start = true;
        // 元ソース上で、今どの行を処理しているか(`#line`用)
        let mut current_line: usize = 1;
        let mut cond_stack: Vec<CondFrame> = Vec::new();

        while i < chars.len() {
            let c = chars[i];

            if c == '#' {
                let name_start = i + 1;
                let mut j = name_start;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }

                if j > name_start {
                    let name: String = chars[name_start..j].iter().collect();

                    if at_line_start {
                        let mut line_end = j;
                        while line_end < chars.len() && chars[line_end] != '\n' {
                            line_end += 1;
                        }
                        let rest_of_line: String = chars[j..line_end].iter().collect();
                        let active_here = is_active(&cond_stack);

                        match name.as_str() {
                            "if" => {
                                // `#if #A` のように、条件は `#NAME` の形で書く
                                let target =
                                    rest_of_line.trim().trim_start_matches('#').to_string();
                                cond_stack.push(CondFrame {
                                    parent_active: active_here,
                                    condition: self.is_defined(&target),
                                    in_else: false,
                                });
                                i = line_end;
                                continue;
                            }
                            "else" => {
                                if let Some(frame) = cond_stack.last_mut() {
                                    frame.in_else = true;
                                }
                                i = line_end;
                                continue;
                            }
                            "endif" => {
                                cond_stack.pop();
                                i = line_end;
                                continue;
                            }
                            _ => {
                                if active_here {
                                    if let Some((target, kind)) =
                                        sort_preproc(&name, &rest_of_line)
                                    {
                                        // `#define ...` 行: テーブルに登録
                                        self.add(target, kind);
                                        i = line_end;
                                        continue;
                                    }
                                }
                            }
                        }
                    }

                    // 行頭ディレクティブでなければ、インライン展開として扱う
                    if is_active(&cond_stack) {
                        if name == "asm" || name == "include" {
                            // `#asm(...)` は構文として残し、後段のパーサーが
                            // 直接解釈する。ここで未定義マクロ扱いにしてはならない。
                            output.push('#');
                            output.push_str(&name);
                        } else if name == "line" {
                            output.push_str(&current_line.to_string());
                        } else {
                            let ReplaceVal::Str(value) = self.resolve(&name);
                            output.push_str(&value);
                        }
                    }
                    i = j;
                    at_line_start = false;
                    continue;
                }
            }

            if is_active(&cond_stack) {
                output.push(c);
            }
            if c == '\n' {
                current_line += 1;
            }
            at_line_start = c == '\n' || (at_line_start && c.is_whitespace());
            i += 1;
        }

        Some(output)
    }

}

/// `#` の直後に来た `Name` トークン(例: "define")を見て、
/// 改行までにスタックした残りの文字列をどう解釈するかを決める。
///
/// 仕様の手順:
/// 1. `#` を見つける
/// 2. 直後に `Name` が来たらフラグを立てる
/// 3. 改行が来るまで文字列をスタック
/// 5. 2で集めた名前を match で条件分岐する (命令が今後増える前提)
/// 6. 3の値と一緒にテーブルに置く
///
/// `name` が未知の命令なら `None` を返す。
pub(in crate::compiler::lex) 
fn sort_preproc<'a>(
    name: &str,
    rest_of_line: &'a str,
) -> Option<(&'a str, PreprocKind)> {
    match name {
        "define" => {
            // `A 10` のような形式を「登録する名前」と「値」に分ける
            let mut parts = rest_of_line.trim().splitn(2, char::is_whitespace);
            let target_name = parts.next()?;
            let value = parts.next().unwrap_or("").trim().to_string();
            Some((target_name, PreprocKind::Define(ReplaceVal::Str(value))))
        }
        _ => None,
    }
}