use crate::compiler::hexl_frontend::lex::*;

fn lexer() -> Lexer {
    Lexer::new("<test>")
}

#[test]
fn check_str_tkn() {
    let mut lex = lexer();
    lex.analy(&"\"hello world!!\" name".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::Str("hello world!!".to_string()),
            Tkn::Name("name".to_string())
        ]
    );
}

#[test]
fn check_str_followed_by_symbol() {
    // 文字列の直後に space を挟まず記号が来るケース。
    // 修正前はここで後続が消える/壊れるバグがあった。
    let mut lex = lexer();
    lex.analy(&"\"abc\"+1".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::Str("abc".to_string()),
            Tkn::Add,
            Tkn::Number("1".to_string()),
        ]
    );
}

#[test]
fn check_hex_number() {
    let mut lex = lexer();
    lex.analy(&"0x1F".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(tkns, vec![Tkn::Number("31".to_string())]);
}

#[test]
fn check_invalid_hex_number_errors_instead_of_panicking() {
    let mut lex = lexer();
    let result = lex.analy(&"0xZZ".to_string());
    assert!(result.is_err());
}

#[test]
fn check_two_strings_back_to_back() {
    let mut lex = lexer();
    lex.analy(&"\"abc\"\"def\"".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![Tkn::Str("abc".to_string()), Tkn::Str("def".to_string())]
    );
}

#[test]
fn check_empty_string_literal() {
    let mut lex = lexer();
    lex.analy(&"\"\"".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(tkns, vec![Tkn::Str("".to_string())]);
}

#[test]
fn check_operators_inside_string_are_not_split() {
    // 文字列リテラル内の演算子記号がトークンとして分断されないこと。
    // 修正前は "a+b-c" が Add, Name("b"), Sub, Name("c"), Str("") に
    // 壊れていた。
    let mut lex = lexer();
    lex.analy(&"\"a+b-c\"".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(tkns, vec![Tkn::Str("a+b-c".to_string())]);
}

#[test]
fn check_string_in_middle_of_expression() {
    let mut lex = lexer();
    lex.analy(&"1+\"mid\"+2".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::Number("1".to_string()),
            Tkn::Add,
            Tkn::Str("mid".to_string()),
            Tkn::Add,
            Tkn::Number("2".to_string()),
        ]
    );
}

#[test]
fn check_name_immediately_followed_by_string() {
    let mut lex = lexer();
    lex.analy(&"name\"abc\"".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![Tkn::Name("name".to_string()), Tkn::Str("abc".to_string())]
    );
}

#[test]
fn check_basic_expression_unaffected() {
    let mut lex = lexer();
    lex.analy(&"x = 1 + 2".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::Name("x".to_string()),
            Tkn::Equal,
            Tkn::Number("1".to_string()),
            Tkn::Add,
            Tkn::Number("2".to_string()),
        ]
    );
}

#[test]
fn check_postfix_increment_and_decrement_tokens() {
    let mut lex = lexer();
    lex.analy(&"a++ b-- a + +b a - -b".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::Name("a".to_string()),
            Tkn::Inc,
            Tkn::Name("b".to_string()),
            Tkn::Dec,
            Tkn::Name("a".to_string()),
            Tkn::Add,
            Tkn::Add,
            Tkn::Name("b".to_string()),
            Tkn::Name("a".to_string()),
            Tkn::Sub,
            Tkn::Sub,
            Tkn::Name("b".to_string()),
        ]
    );
}

#[test]
fn check_not_eq_tkn() {
    // `!=` が `Not, Equal` の2トークンに分かれず、
    // `NotEq` 1トークンとして生成されることを確認する。
    let mut lex = lexer();
    lex.analy(&"a != b".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::Name("a".to_string()),
            Tkn::NotEq,
            Tkn::Name("b".to_string()),
        ]
    );
}

#[test]
fn check_ret_keyword() {
    let mut lex = lexer();
    lex.analy(&"ret 5".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(tkns, vec![Tkn::KeyWordRet, Tkn::Number("5".to_string())]);
}

#[test]
fn check_self_keyword() {
    let mut lex = lexer();
    lex.analy(&"Self".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(tkns, vec![Tkn::KeyWordSelf]);
}

#[test]
fn check_bracket_tkns() {
    let mut lex = lexer();
    lex.analy(&"[ty 4]".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::LBracket,
            Tkn::Name("ty".to_string()),
            Tkn::Number("4".to_string()),
            Tkn::RBracket,
        ]
    );
}

#[test]
fn check_preprocessor_define_and_use() {
    // `#define A 10` で登録した値が、行頭・行中どちらの `#A` でも
    // 展開されることを確認する。
    let mut lex = lexer();
    lex.analy(&"#define A 10\nret #A + 1\n#A".to_string())
        .unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::KeyWordRet,
            Tkn::Number("10".to_string()),
            Tkn::Add,
            Tkn::Number("1".to_string()),
            Tkn::Number("10".to_string()),
        ]
    );
}

#[test]
fn check_hash_without_name_is_complesyn() {
    // `#` の直後が識別子でない場合はプリプロセッサとして扱わず、
    // 従来どおり `Tkn::CompleSyn` になることを確認する。
    let mut lex = lexer();
    lex.analy(&"ret 1 #".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::KeyWordRet,
            Tkn::Number("1".to_string()),
            Tkn::CompleSyn,
        ]
    );
}

#[test]
fn check_asm_directive_is_not_treated_as_undefined_macro() {
    let mut lex = lexer();
    lex.analy(&"#asm(gas) { \"mov ${a}\" }".to_string())
        .unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::CompleSyn,
            Tkn::Name("asm".to_string()),
            Tkn::LParen,
            Tkn::Name("gas".to_string()),
            Tkn::RParen,
            Tkn::LBrace,
            Tkn::Str("mov ${a}".to_string()),
            Tkn::RBrace,
        ]
    );
}

#[test]
#[should_panic]
fn check_preprocessor_undefined_symbol_panics() {
    // 仕様: 登録されていない名前の `#NAME` は panic する。
    let mut lex = lexer();
    let _ = lex.analy(&"#UNDEFINED".to_string());
}

#[test]
fn check_preprocessor_if_defined_takes_if_branch() {
    let mut lex = lexer();
    lex.analy(&"#define A 10\n#if #A\nret 1\n#else\nret 2\n#endif".to_string())
        .unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(tkns, vec![Tkn::KeyWordRet, Tkn::Number("1".to_string())]);
}

#[test]
fn check_preprocessor_if_undefined_takes_else_branch() {
    let mut lex = lexer();
    lex.analy(&"#if #A\nret 1\n#else\nret 2\n#endif".to_string())
        .unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(tkns, vec![Tkn::KeyWordRet, Tkn::Number("2".to_string())]);
}

#[test]
fn check_preprocessor_if_without_else() {
    let mut lex = lexer();
    lex.analy(&"#if #A\nret 1\n#endif\nret 2".to_string())
        .unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    // `A` は未定義なので `#if`〜`#endif` はまるごと消える
    assert_eq!(tkns, vec![Tkn::KeyWordRet, Tkn::Number("2".to_string())]);
}

#[test]
fn check_preprocessor_line_builtin() {
    let mut lex = lexer();
    lex.analy(&"ret 1\nret #line".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::KeyWordRet,
            Tkn::Number("1".to_string()),
            Tkn::KeyWordRet,
            Tkn::Number("2".to_string()),
        ]
    );
}

#[test]
fn check_preprocessor_define_fn_macro() {
    // `#define Name(arg, arg2, ..) BODY` で、仮引数を実引数に置き換えて展開する
    let mut lex = lexer();
    lex.analy(&"#define SET(a, b) (a = b + 1)\n#SET(x, 2)".to_string())
        .unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::LParen,
            Tkn::Name("x".to_string()),
            Tkn::Equal,
            Tkn::Number("2".to_string()),
            Tkn::Add,
            Tkn::Number("1".to_string()),
            Tkn::RParen,
        ]
    );
}

#[test]
fn check_preprocessor_fn_macro_arg_with_comma_in_parens() {
    // 括弧の中の`,`は実引数の区切りにならない
    let mut lex = lexer();
    lex.analy(&"#define ADD(a, b) a + b\nret #ADD(f(1, 2), 3)".to_string())
        .unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::KeyWordRet,
            Tkn::Name("f".to_string()),
            Tkn::LParen,
            Tkn::Number("1".to_string()),
            Tkn::Comma,
            Tkn::Number("2".to_string()),
            Tkn::RParen,
            Tkn::Add,
            Tkn::Number("3".to_string()),
        ]
    );
}

#[test]
#[should_panic]
fn check_preprocessor_fn_macro_wrong_arg_count_panics() {
    let mut lex = lexer();
    let _ = lex.analy(&"#define F(a, b) a\n#F(1)".to_string());
}

#[test]
fn check_preprocessor_multiline_define() {
    // `{{ ... }}` で囲むと、`#define`の本体を複数行で書ける
    let mut lex = lexer();
    lex.analy(&"#define N {{\n  expr = 10\n  expr2 = 10\n}}\n#N".to_string())
        .unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::Name("expr".to_string()),
            Tkn::Equal,
            Tkn::Number("10".to_string()),
            Tkn::Name("expr2".to_string()),
            Tkn::Equal,
            Tkn::Number("10".to_string()),
        ]
    );
}

#[test]
fn check_preprocessor_brace_fn_macro() {
    // `#define M{a, b} BODY`は`#M { x, y }`と呼び出す(空白は省略もできる)
    let src = "#define SET{a, b} a = b + 1\n#SET { x, 2 }\n#SET{y, 3}";
    assert_eq!(
        lex_tkns(src),
        vec![
            Tkn::Name("x".to_string()),
            Tkn::Equal,
            Tkn::Number("2".to_string()),
            Tkn::Add,
            Tkn::Number("1".to_string()),
            Tkn::Name("y".to_string()),
            Tkn::Equal,
            Tkn::Number("3".to_string()),
            Tkn::Add,
            Tkn::Number("1".to_string()),
        ]
    );
}

#[test]
fn check_preprocessor_brace_fn_macro_multiline_body() {
    // `#define M{} {{ ... }}`: 引数なしで、本体が複数行
    let src = "#define M{} {{\n  expr = 10\n  expr2 = 10\n}}\n#M {}";
    assert_eq!(
        lex_tkns(src),
        vec![
            Tkn::Name("expr".to_string()),
            Tkn::Equal,
            Tkn::Number("10".to_string()),
            Tkn::Name("expr2".to_string()),
            Tkn::Equal,
            Tkn::Number("10".to_string()),
        ]
    );
}

#[test]
fn check_preprocessor_brace_fn_macro_multiline_with_params() {
    // 引数は`{}`の中の`,`で区切り、括弧の中の`,`は区切りにならない
    let src = "#define ADD{a, b} {{\n  a + b\n}}\nret #ADD { f(1, 2), 3 }";
    assert_eq!(
        lex_tkns(src),
        vec![
            Tkn::KeyWordRet,
            Tkn::Name("f".to_string()),
            Tkn::LParen,
            Tkn::Number("1".to_string()),
            Tkn::Comma,
            Tkn::Number("2".to_string()),
            Tkn::RParen,
            Tkn::Add,
            Tkn::Number("3".to_string()),
        ]
    );
}

#[test]
#[should_panic]
fn check_preprocessor_brace_fn_macro_without_braces_panics() {
    let mut lex = lexer();
    let _ = lex.analy(&"#define M{a} a\n#M(1)".to_string());
}

#[test]
#[should_panic]
fn check_preprocessor_brace_fn_macro_wrong_arg_count_panics() {
    let mut lex = lexer();
    let _ = lex.analy(&"#define M{a, b} a\n#M { 1 }".to_string());
}

#[test]
fn check_call_with_args() {
    let mut lex = lexer();
    lex.analy(&"f(1,2)".to_string()).unwrap();
    let tkns: Vec<Tkn> = lex.gen_tkns.into_iter().map(|t| t.tkn).collect();
    assert_eq!(
        tkns,
        vec![
            Tkn::Name("f".to_string()),
            Tkn::LParen,
            Tkn::Number("1".to_string()),
            Tkn::Comma,
            Tkn::Number("2".to_string()),
            Tkn::RParen,
        ]
    );
}

fn lex_tkns(src: &str) -> Vec<Tkn> {
    let mut lex = lexer();
    lex.analy(&src.to_string()).unwrap();
    lex.gen_tkns.into_iter().map(|t| t.tkn).collect()
}

#[test]
fn check_float_literal() {
    assert_eq!(lex_tkns("3.14"), vec![Tkn::Float("3.14".to_string())]);
    assert_eq!(lex_tkns("0.5"), vec![Tkn::Float("0.5".to_string())]);
}

#[test]
fn check_float_in_expression() {
    assert_eq!(
        lex_tkns("a = 0.5 + 10.25"),
        vec![
            Tkn::Name("a".to_string()),
            Tkn::Equal,
            Tkn::Float("0.5".to_string()),
            Tkn::Add,
            Tkn::Float("10.25".to_string()),
        ]
    );
}

#[test]
fn check_float_in_call_args() {
    assert_eq!(
        lex_tkns("f(1.5, 2)"),
        vec![
            Tkn::Name("f".to_string()),
            Tkn::LParen,
            Tkn::Float("1.5".to_string()),
            Tkn::Comma,
            Tkn::Number("2".to_string()),
            Tkn::RParen,
        ]
    );
}

#[test]
fn check_range_is_not_float() {
    assert_eq!(
        lex_tkns("1..2"),
        vec![
            Tkn::Number("1".to_string()),
            Tkn::RangeTkn,
            Tkn::Number("2".to_string()),
        ]
    );
}

#[test]
fn check_member_access_is_not_float() {
    assert_eq!(
        lex_tkns("a.b"),
        vec![
            Tkn::Name("a".to_string()),
            Tkn::Dot,
            Tkn::Name("b".to_string())
        ]
    );
    // 数字の後ろの `.` が数字で続かないなら小数にしない
    assert_eq!(
        lex_tkns("3.foo"),
        vec![
            Tkn::Number("3".to_string()),
            Tkn::Dot,
            Tkn::Name("foo".to_string())
        ]
    );
    // `.` の直後の数字列は、その手前の `.` と合わせて小数にしない
    assert_eq!(
        lex_tkns("a.0.1"),
        vec![
            Tkn::Name("a".to_string()),
            Tkn::Dot,
            Tkn::Number("0".to_string()),
            Tkn::Dot,
            Tkn::Number("1".to_string()),
        ]
    );
}

#[test]
fn check_second_dot_ends_float() {
    assert_eq!(
        lex_tkns("1.2.3"),
        vec![
            Tkn::Float("1.2".to_string()),
            Tkn::Dot,
            Tkn::Number("3".to_string()),
        ]
    );
}

#[test]
fn check_hex_is_not_float() {
    assert_eq!(lex_tkns("0x1F"), vec![Tkn::Number("31".to_string())]);
}

#[test]
fn check_float_with_trailing_letters_errors() {
    let mut lex = lexer();
    assert!(lex.analy(&"3.14x".to_string()).is_err());
}

#[test]
fn check_bitwise_operator_tokens() {
    assert_eq!(lex_tkns("& ^ |"), vec![Tkn::And, Tkn::Xor, Tkn::Or]);
}

fn ret_nums(os: &str, src: &str) -> Vec<String> {
    let mut lex = Lexer::with_os(os, "<test>");
    lex.analy(&src.to_string()).unwrap();
    lex.gen_tkns
        .into_iter()
        .filter_map(|t| match t.tkn {
            Tkn::Number(n) => Some(n),
            _ => None,
        })
        .collect()
}

const OS_SRC: &str = "#if #windows\nret 1\n#elif #linux\nret 2\n#else\nret 3\n#endif";

#[test]
fn check_preprocessor_os_windows() {
    assert_eq!(ret_nums("windows", OS_SRC), vec!["1"]);
}

#[test]
fn check_preprocessor_os_linux() {
    assert_eq!(ret_nums("linux", OS_SRC), vec!["2"]);
}

#[test]
fn check_preprocessor_os_unknown_takes_else() {
    assert_eq!(ret_nums("macos", OS_SRC), vec!["3"]);
}

#[test]
fn check_preprocessor_os_symbol_expands() {
    assert_eq!(
        ret_nums("linux", "#define A 5\n#if #linux\nret #A\n#endif"),
        vec!["5"]
    );
}

#[test]
fn check_preprocessor_elif_first_match_only() {
    // 複数の `#elif` が当てはまっても、最初の枝だけが有効
    let src = "#define A 1\n#define B 1\n#if #X\nret 1\n#elif #A\nret 2\n#elif #B\nret 3\n#else\nret 4\n#endif";
    assert_eq!(ret_nums("linux", src), vec!["2"]);
}

#[test]
fn check_preprocessor_elif_skipped_when_if_taken() {
    let src = "#define A 1\n#if #A\nret 1\n#elif #A\nret 2\n#endif";
    assert_eq!(ret_nums("linux", src), vec!["1"]);
}

#[test]
fn check_preprocessor_elif_without_else_none_taken() {
    let src = "#if #X\nret 1\n#elif #Y\nret 2\n#endif\nret 9";
    assert_eq!(ret_nums("linux", src), vec!["9"]);
}

#[test]
fn check_preprocessor_elif_nested_in_inactive_parent() {
    // 外側が無効なら、内側の `#elif` / `#else` が当てはまっても有効にならない
    let src =
        "#if #windows\n#if #X\nret 1\n#elif #linux\nret 2\n#else\nret 3\n#endif\n#endif\nret 9";
    assert_eq!(ret_nums("linux", src), vec!["9"]);
}

#[test]
fn check_preprocessor_conditionals_preserve_line_numbers() {
    let src = "#if #windows\nret 1\n#elif #linux\nret #line\n#else\nret 3\n#endif\nret #line";
    let mut lex = Lexer::with_os("linux", "<test>");
    lex.analy(src).unwrap();
    let nums: Vec<(String, usize)> = lex
        .gen_tkns
        .iter()
        .filter_map(|t| match &t.tkn {
            Tkn::Number(n) => Some((n.clone(), t.span.line)),
            _ => None,
        })
        .collect();
    assert_eq!(nums, vec![("4".to_string(), 4), ("8".to_string(), 8)]);
}

#[test]
#[should_panic]
fn check_preprocessor_elif_after_else_panics() {
    let _ = ret_nums("linux", "#if #X\n#else\n#elif #linux\n#endif");
}

#[test]
fn check_include_plain_is_not_inlined() {
    // `.hexl`は展開せず、`#include`のまま後段(ROOT基準+pubのみ)に任せる。
    // ファイルが無くてもここでは読まない
    let mut lex = Lexer::with_os("linux", "<test>");
    lex.analy(&"ret 0\n#include \"dir/a.hexl\"\nret 2".to_string())
        .unwrap();
    let includes = lex
        .gen_tkns
        .iter()
        .filter(|t| t.tkn == Tkn::Name("include".to_string()))
        .count();
    assert_eq!(includes, 1);
}

#[test]
fn check_include_with_alias_is_left_for_parser() {
    // `"file"::io` と `io="file"` は読み込まず、`#include` のまま残る
    let mut lex = Lexer::with_os("linux", "<test>");
    lex.analy(&"#include \"x.hexl\"::io\n#include io=\"x.hexl\"".to_string())
        .unwrap();
    let names = lex
        .gen_tkns
        .iter()
        .filter(|t| t.tkn == Tkn::Name("include".to_string()))
        .count();
    assert_eq!(names, 2);
}

#[test]
fn check_include_c_file_only_saves_name() {
    // `.c` / `.h` は読み込まず(ファイルが無くてもよい)、名前だけが保存される。
    // ソースには何も展開されず、後ろのトークンの行番号もずれない
    let mut lex = Lexer::with_os("linux", "<test>");
    lex.analy(&"ret 1\n#include \"a.c\"\n#include \"b.h\"\nret 2".to_string())
        .unwrap();
    assert_eq!(lex.include_c_file_list, vec!["a.c", "b.h"]);
    let nums: Vec<(String, usize)> = lex
        .gen_tkns
        .iter()
        .filter_map(|t| match &t.tkn {
            Tkn::Number(n) => Some((n.clone(), t.span.line)),
            _ => None,
        })
        .collect();
    assert_eq!(nums, vec![("1".to_string(), 1), ("2".to_string(), 4)]);
}

#[test]
fn check_include_c_file_duplicate_is_saved_once() {
    let mut lex = Lexer::with_os("linux", "<test>");
    lex.analy(&"#include \"a.c\"\n#include \"a.c\"".to_string())
        .unwrap();
    assert_eq!(lex.include_c_file_list, vec!["a.c"]);
}

#[test]
fn check_include_c_file_in_inactive_branch_is_skipped() {
    let mut lex = Lexer::with_os("linux", "<test>");
    lex.analy(
        &"#if #windows\n#include \"w.c\"\n#endif\n#if #linux\n#include \"l.c\"\n#endif"
            .to_string(),
    )
    .unwrap();
    assert_eq!(lex.include_c_file_list, vec!["l.c"]);
}

fn tkns_of(src: &str) -> Vec<Tkn> {
    let mut lex = lexer();
    lex.analy(&src.to_string()).unwrap();
    lex.gen_tkns.into_iter().map(|t| t.tkn).collect()
}

#[test]
fn check_line_comment() {
    assert_eq!(
        tkns_of("x = 1 // ここはコメント\ny = 2"),
        tkns_of("x = 1\ny = 2")
    );
}

#[test]
fn check_block_comment() {
    assert_eq!(tkns_of("x = /* 1 + */ 2"), tkns_of("x = 2"));
}

#[test]
fn check_multiline_block_comment() {
    assert_eq!(
        tkns_of("x = 1\n/* a\nb\nc */\ny = 2"),
        tkns_of("x = 1\ny = 2")
    );
}

#[test]
fn check_comment_only_source() {
    assert_eq!(tkns_of("// a\n/* b */"), vec![]);
}

#[test]
fn check_block_comment_separates_tokens() {
    assert_eq!(
        tkns_of("a/**/b"),
        vec![Tkn::Name("a".to_string()), Tkn::Name("b".to_string())]
    );
}

#[test]
fn check_comment_markers_in_string_are_not_comments() {
    assert_eq!(
        tkns_of("\"a//b\" \"c/*d*/\""),
        vec![Tkn::Str("a//b".to_string()), Tkn::Str("c/*d*/".to_string())]
    );
}

#[test]
fn check_div_and_div_eq_still_work() {
    assert_eq!(
        tkns_of("a / b /= c"),
        vec![
            Tkn::Name("a".to_string()),
            Tkn::Div,
            Tkn::Name("b".to_string()),
            Tkn::DivEq,
            Tkn::Name("c".to_string()),
        ]
    );
}

#[test]
fn check_comment_after_define_and_if() {
    assert_eq!(
        tkns_of("#define A 10 // 値\n#if #A // 条件\nret #A\n#endif"),
        tkns_of("ret 10")
    );
}
