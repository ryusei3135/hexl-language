use std::rc::Rc;

use super::*;
use regex::{Captures, Regex};

/// `${...}`を検出する正規表現。
/// `gen_asm_line`は`#asm`ブロックの行ごとに呼ばれるため、毎回
/// `Regex::new`でコンパイルすると無駄なコストがかかる。
/// `OnceLock`で最初の1回だけコンパイルし、以降は使い回す。
// fn inline_var_regex() -> Regex {
//     Regex::new(r"\$\{([^}]+)\}").unwrap()
// }

impl Parser {
    pub(super) fn make_preproc(
        &mut self,
        proc_name: &str,
    ) -> Result<Group2Node, err::ErrKind> {
        let result = match proc_name {
            "include" => Group2Node::Include(self.build_include_path()?),
            // "preserve" => {
            //     println!("{}", proc_name);
            //     panic!();
            // }
            "asm" => self.build_asm_ast()?,
            _ => return crate::err_at!(self.preproc_unexpected_token()),
        };

        Ok(result)
    }

    /// `#include`の後ろを解析する。以下の書き方に対応する:
    /// - `#include Name="mod/file.hexl"`  公開関数を全て`Name::`で取り込む
    /// - `#include "mod/file.hexl"`       公開関数を全て、ファイル名から
    ///                                     生成したモジュール名で取り込む
    ///                                     (`file::func()`)
    /// - `#include "mod/file.hexl"::func` `func`だけをそのまま取り込む
    /// - `#include "mod/file.hexl"::*`    公開関数を全て、そのまま取り込む
    /// - `#include mod::file`             従来通りの書き方(下位互換)
    fn build_include_path(&mut self) -> Result<ModPath, err::ErrKind> {
        // `#include`の位置
        let span = self.build_err_span();
        // `Name "=" ...`の形なら、エイリアス指定として読み取る
        let alias = self.try_take_include_alias()?;

        // match の結果を直接 return する形に統一
        match self.next_tkn(&["string", "name"])? {
            lex::Tkn::Str(literal) => {
                self.finish_literal_include(&literal, alias, Rc::clone(&span))
            }
            // match のガード条件 `if` は Rust らしくて非常に綺麗です！
            lex::Tkn::Name(first_seg) if alias.is_none() => {
                self.build_mod_path_segments(first_seg, Rc::clone(&span))
            }
            _ => {
                return crate::err_at!(self.expected_path_segment());
            }
        }
    }

    /// `Name "=" `の形になっているかどうかを覗き見て判定する。
    /// なっていれば両方のトークンを消費してエイリアス名を返し、
    /// なっていなければ何も消費せず`None`を返す
    fn try_take_include_alias(&mut self) -> Result<Option<String>, err::ErrKind> {
        let is_alias = matches!(self.peek_tkn()?, lex::Tkn::Name(_))
            && matches!(self.peek2_tkn(), Some(lex::Tkn::Equal));

        if !is_alias {
            return Ok(None);
        }

        let lex::Tkn::Name(alias_name) = self.next_tkn(&["name"])? else {
            return crate::err_at!(self.expected_path_segment());
        };
        // `=`を読み飛ばす
        self.next_tkn(&["="])?;
        Ok(Some(alias_name))
    }

    /// `"mod/file.hexl"`(と、それに続く`::func` / `::*`)を解析して
    /// `ModPath`を作る。呼び出された時点で、対象の文字列トークンは
    /// 読み込み済み(`current_tkn`がその文字列を指している)
    fn finish_literal_include(
        &mut self,
        literal: &str,
        alias: Option<String>,
        span: Rc<err::Span>,
    ) -> Result<ModPath, err::ErrKind> {
        // `::`が続いていなければ、ファイル全体をモジュールとして
        // 取り込む(エイリアスが無ければファイル名からモジュール名を
        // 自動生成する)
        if self.peek_tkn()? != lex::Tkn::ModPathTkn {
            return Ok(ModPath::new_literal(
                literal,
                ImportKind::Module(alias),
                Rc::clone(&span),
            ));
        }
        // `::`を読み飛ばす
        self.advance_tkn_checked()?;

        let kind = match self.next_tkn(&["name", "*"])? {
            lex::Tkn::Name(func_name) => ImportKind::Func(func_name),
            lex::Tkn::Mul => ImportKind::Glob,
            _ => {
                return crate::err_at!(self.expected_path_segment());
            }
        };

        Ok(ModPath::new_literal(literal, kind, Rc::clone(&span)))
    }

    /// 従来通りの書き方(`mod::file`)を解析する。
    /// `first_seg`は既に読み込み済みの最初のセグメント
    fn build_mod_path_segments(
        &mut self,
        first_seg: String,
        span: Rc<err::Span>,
    ) -> Result<ModPath, err::ErrKind> {
        let mut mod_path = ModPath::new_segments(Rc::clone(&span));
        mod_path.add_path(&first_seg)?;

        while self.peek_tkn()? == lex::Tkn::ModPathTkn {
            // `::`を読み飛ばす
            // whileでトークンが存在することは確認済み
            self.advance_tkn_checked()?;
            match self.next_tkn(&["name"])? {
                lex::Tkn::Name(name) => mod_path.add_path(&name)?,
                lex::Tkn::Str(val) => mod_path.add_path(&val)?,
                _ => {
                    return crate::err_at!(self.expected_path_segment());
                }
            }
        }

        Ok(mod_path)
    }

    #[inline(always)]
    fn get_asm_name(&mut self) -> Result<String, err::ErrKind> {
        if let lex::Tkn::Name(asm_name) = self.next_tkn(&["name"])? {
            Ok(asm_name)
        } else {
            crate::err_at!(self.not_found_asm_name())
        }
    }
    /// ## 戻り値
    /// - Ok inlineアセンブラの名前
    fn build_asm_ast(&mut self) -> Result<Group2Node, err::ErrKind> {
        // #asm(...)なので、(以外が来たらエラー
        if self.next_tkn(&["not `(`"])? != lex::Tkn::LParen {
            return crate::err_at!(self.expected_lparen_after_asm());
        }
        let asm_name = self.get_asm_name()?;

        // #asm(...)なので、(以外が来たらエラー
        if self.next_tkn(&[")"])? != lex::Tkn::RParen {
            return crate::err_at!(self.expected_rparen_after_asm());
        }
        let nodes = self.gen_asm_preproc()?;
        Ok(Group2Node::CompleSyntax((asm_name, nodes)))
    }

    /// アセンブリ言語のプロプロセッサの
    /// 中身(アセンブリ言語本体)を生成する関数
    fn gen_asm_preproc(&mut self) -> Result<Vec<InlineAsm>, err::ErrKind> {
        let mut nodes = Vec::<InlineAsm>::new();

        if self.next_tkn(&["{"])? == lex::Tkn::LBrace {
            let _ = self.next_tkn(&[])?;
            loop {
                match self.current_tkn().clone() {
                    lex::Tkn::Str(value) => {
                        nodes.push(self.gen_asm_line(&value)?);
                    }
                    lex::Tkn::RBrace => {
                        self.next_tkn(&[])?;
                        break;
                    }
                    t => {
                        return crate::err_at!(self.expected_tkn("string or `}`", t));
                    }
                }
                self.next_tkn(&[])?;
            }
            return Ok(nodes);
        }

        crate::err_at!(self.preproc_unexpected_token())
    }

    /// inlineアセンブラの1行分の文字列から`InlineAsm`を作成する。
    ///
    /// `${...}`は出現順に`{0}`, `{1}`, ...のプレースホルダーへ置き換え、
    /// 中に書かれていた式(変数・構造体のメンバー・ポインタなど、
    /// 普通の式として書けるもの)は`operands`に出現順で積んでいく。
    /// 同じ行に複数の`${...}`があっても、すべて取り込む
    /// (以前の実装は最後の1つしか保持できなかった)。
    fn gen_asm_line(&mut self, value: &str) -> Result<InlineAsm, err::ErrKind> {
        // この行(文字列)の位置
        let span = self.build_err_span();
        // 正規表現は定数なので失敗しないが、失敗した場合も`Err`で返す
        let inline_var = Regex::new(r"\$\{([^}]+)\}").map_err(|_| err::ErrKind::UnexpectedToken)?;

        let mut operands = Vec::<Expr>::new();
        let mut parse_err: Option<err::ErrKind> = None;

        let asm = inline_var.replace_all(value, |caps: &Captures| {
            if parse_err.is_some() {
                return String::new();
            }

            let inner = &caps[1];

            match self.parse_asm_operand(inner) {
                Ok(expr) => {
                    let index = operands.len();
                    operands.push(expr);
                    format!("{{{}}}", index)
                }
                Err(e) => {
                    parse_err = Some(e);
                    String::new()
                }
            }
        });

        if let Some(e) = parse_err {
            return Err(e);
        }

        Ok(InlineAsm {
            asm: asm.to_string(),
            operands,
            span: Rc::clone(&span),
        })
    }
}

#[cfg(test)]
mod inline_asm_tests {
    use crate::compiler::{err, lex, parse};

    /// テスト用の位置。`Expr`の`PartialEq`は`span`を比較しないので、何でもよい
    fn sp() -> std::rc::Rc<err::Span> {
        std::rc::Rc::new(err::Span::new(&0, &0))
    }

    /// 位置を付けて`Expr`にする
    fn ex(n: ExprNode) -> Expr {
        Expr::new(n, sp())
    }

    /// テスト用に、関数の中に`#asm(...)`ブロックを1つ持つ
    /// プログラムを解析し、`InlineAsm`の一覧を取り出す
    fn gen_inline_asm(asm_body: &str) -> Vec<InlineAsm> {
        let src = format!("main(): b1 {{ #asm(gas) {{ {} }} }}", asm_body);

        let mut lexer = lex::Lexer::new();
        lexer.analy(&src.to_string()).unwrap();

        let mut p = parse::Parser::new_for_test();
        let nodes = p.parser_for_test(&lexer.gen_tkns).expect("parse failed");

        let Group1Node::FuncDefine(func) = &nodes[0] else {
            panic!("not a func define")
        };

        let Group2Node::CompleSyntax((name, lines)) = &func.body[0].get_node() else {
            panic!("not an inline asm node: {:?}", func.body[0])
        };
        assert_eq!(name, "gas");
        lines.clone()
    }

    #[test]
    fn plain_var_operand() {
        let lines = gen_inline_asm(r#""mov ${a}, ${b}""#);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].asm, "mov {0}, {1}");
        assert_eq!(
            lines[0].operands,
            vec![
                ex(ExprNode::Var("a".to_string())),
                ex(ExprNode::Var("b".to_string())),
            ]
        );
    }

    #[test]
    fn struct_member_operand() {
        // 構造体のメンバーへのアクセスが、通常の式と同様に使える
        let lines = gen_inline_asm(r#""mov ${p.x}, ${p.y}""#);
        assert_eq!(lines[0].asm, "mov {0}, {1}");
        assert_eq!(
            lines[0].operands,
            vec![
                ex(ExprNode::Member {
                    scope: vec!["p".to_string()],
                    target: Box::new(ex(ExprNode::Var("x".to_string()))),
                }),
                ex(ExprNode::Member {
                    scope: vec!["p".to_string()],
                    target: Box::new(ex(ExprNode::Var("y".to_string()))),
                }),
            ]
        );
    }

    #[test]
    fn pointer_deref_and_address_of_operand() {
        // ポインタの参照(`*p`)とアドレス取得(`[p]`)が、通常の式と同様に使える
        let lines = gen_inline_asm(r#""mov ${*p}, ${[p]}""#);
        assert_eq!(lines[0].asm, "mov {0}, {1}");
        assert_eq!(
            lines[0].operands,
            vec![
                ex(ExprNode::ConnectAddr(Box::new(ex(
                    ExprNode::Var("p".to_string())
                )))),
                ex(ExprNode::GetAddress(Box::new(ex(
                    ExprNode::Var("p".to_string())
                )))),
            ]
        );
    }

    #[test]
    fn arithmetic_operand() {
        let lines = gen_inline_asm(r#""mov ${a + 1}, ${p.x * 2}""#);
        assert_eq!(lines[0].asm, "mov {0}, {1}");
        assert_eq!(
            lines[0].operands,
            vec![
                ex(ExprNode::Add((
                    Box::new(ex(ExprNode::Var("a".to_string()))),
                    Box::new(ex(ExprNode::Number("1".to_string()))),
                ))),
                ex(ExprNode::Mul((
                    Box::new(ex(ExprNode::Member {
                        scope: vec!["p".to_string()],
                        target: Box::new(ex(ExprNode::Var("x".to_string()))),
                    })),
                    Box::new(ex(ExprNode::Number("2".to_string()))),
                ))),
            ]
        );
    }

    #[test]
    fn multiple_lines_each_keep_their_own_operands() {
        let lines = gen_inline_asm(r#""mov ${a}, ${b}" "add ${c}, ${d}""#);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].asm, "mov {0}, {1}");
        assert_eq!(lines[1].asm, "add {0}, {1}");
        assert_eq!(
            lines[1].operands,
            vec![
                ex(ExprNode::Var("c".to_string())),
                ex(ExprNode::Var("d".to_string())),
            ]
        );
    }

    #[test]
    fn line_without_any_placeholder_keeps_empty_operands() {
        let lines = gen_inline_asm(r#""nop""#);
        assert_eq!(lines[0].asm, "nop");
        assert!(lines[0].operands.is_empty());
    }

    #[test]
    fn asm_block_does_not_truncate_following_statements() {
        let src = "main(): b1 { #asm(gas) { \"mov ${a}\" } ret a }";

        let mut lexer = lex::Lexer::new();
        lexer.analy(&src.to_string()).unwrap();

        let mut p = parse::Parser::new_for_test();
        let nodes = p.parser_for_test(&lexer.gen_tkns).expect("parse failed");

        let Group1Node::FuncDefine(func) = &nodes[0] else {
            panic!("not a func define")
        };

        assert_eq!(func.body.len(), 2);
        assert!(matches!(
            func.body[1].get_node(),
            Group2Node::Stmt(StmtNode::Return(_))
        ));
    }
}
