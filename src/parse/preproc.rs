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
        proc_name: &String,
    ) -> Result<node::Group2Node, err::ErrKind> {
        let result = match proc_name.as_str() {
            "include" => {
                node::Group2Node::Include(
                    self.build_include_path()?)
            }
            "preserve" => {
                println!("{}", proc_name);
                panic!();
            }
            "asm" => self.build_asm_ast()?,
            _ => panic!(),
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
    fn build_include_path(
        &mut self
    ) -> Result<node::ModPath, err::ErrKind> {
        // `Name "=" ...`の形なら、エイリアス指定として読み取る
        let alias = self.try_take_include_alias()?;

        match self.next_tkn(vec!["string", "name"])? {
            lex::Tkn::Str(literal) => {
                self.finish_literal_include(literal, alias)
            }
            // エイリアスが指定されている場合、パスは必ず
            // 文字列リテラル(`Literal`形式)でなければならない
            lex::Tkn::Name(first_seg) if alias.is_none() => {
                self.build_mod_path_segments(first_seg)
            }
            _ => {
                crate::preproc_err!(self, ExpectedPathSegment);
            }
        }
    }

    /// `Name "=" `の形になっているかどうかを覗き見て判定する。
    /// なっていれば両方のトークンを消費してエイリアス名を返し、
    /// なっていなければ何も消費せず`None`を返す
    fn try_take_include_alias(
        &mut self
    ) -> Result<Option<String>, err::ErrKind> {
        let is_alias = matches!(
            self.peek_tkn(), 
            Some(lex::Tkn::Name(_))
        ) && matches!(
            self.peek2_tkn(), 
            Some(lex::Tkn::Equal)
        );

        if !is_alias {
            return Ok(None);
        }

        let lex::Tkn::Name(alias_name) = self.next_tkn(vec!["name"])? else {
            unreachable!();
        };
        // `=`を読み飛ばす
        self.next_tkn(vec!["="])?;
        Ok(Some(alias_name))
    }

    /// `"mod/file.hexl"`(と、それに続く`::func` / `::*`)を解析して
    /// `ModPath`を作る。呼び出された時点で、対象の文字列トークンは
    /// 読み込み済み(`current_tkn`がその文字列を指している)
    fn finish_literal_include(
        &mut self,
        literal: String,
        alias: Option<String>,
    ) -> Result<node::ModPath, err::ErrKind> {
        // `::`が続いていなければ、ファイル全体をモジュールとして
        // 取り込む(エイリアスが無ければファイル名からモジュール名を
        // 自動生成する)
        if !matches!(self.peek_tkn(), Some(lex::Tkn::ModPathTkn)) {
            return Ok(node::ModPath::new_literal(
                literal,
                node::ImportKind::Module(alias),
            ));
        }
        // `::`を読み飛ばす
        self.next_tkn(vec![])?;

        let kind = match self.next_tkn(vec!["name", "*"])? {
            lex::Tkn::Name(func_name) => node::ImportKind::Func(func_name),
            lex::Tkn::Mul => node::ImportKind::Glob,
            _ => {
                crate::preproc_err!(self, ExpectedPathSegment);
            }
        };

        Ok(node::ModPath::new_literal(literal, kind))
    }

    /// 従来通りの書き方(`mod::file`)を解析する。
    /// `first_seg`は既に読み込み済みの最初のセグメント
    fn build_mod_path_segments(
        &mut self,
        first_seg: String,
    ) -> Result<node::ModPath, err::ErrKind> {
        let mut mod_path = node::ModPath::new_segments();
        mod_path.add_path(&first_seg);

        while matches!(self.peek_tkn(), Some(lex::Tkn::ModPathTkn)) {
            // `::`を読み飛ばす
            self.next_tkn(vec![])?;
            match self.next_tkn(vec!["name"])? {
                lex::Tkn::Name(name) => mod_path.add_path(&name),
                lex::Tkn::Str(val) => mod_path.add_path(&val),
                _ => {
                    crate::preproc_err!(self, ExpectedPathSegment);
                }
            }
        }

        Ok(mod_path)
    }

    #[inline(always)]
    fn get_asm_name(
        &mut self
    ) -> Result<String, err::ErrKind> {
        if let lex::Tkn::Name(asm_name) = self.next_tkn(vec!["name"])? 
        {
            Ok(asm_name)
        } else {
            crate::preproc_err!(self, NotFoundAsmName);
        }
    }
    /// ## 戻り値
    /// - Ok inlineアセンブラの名前
    fn build_asm_ast(
        &mut self
    ) -> Result<node::Group2Node, err::ErrKind> {
        
        // #asm(...)なので、(以外が来たらエラー
        if !matches!(
            self.next_tkn(vec!["not `(`"])?, 
            lex::Tkn::LParen
    ) {
            crate::preproc_err!(self, ExpectedLParenAfterAsm);
        }
        let asm_name = self.get_asm_name()?;

        // #asm(...)なので、(以外が来たらエラー
        if !matches!(self.next_tkn(vec![")"])?, lex::Tkn::RParen) {
            crate::preproc_err!(self, ExpectedRParenAfterAsm);
        }
        let nodes = self.gen_asm_preproc()?;
        Ok(node::Group2Node::CompleSyntax((asm_name, nodes)))
    }

    /// アセンブリ言語のプロプロセッサの
    /// 中身(アセンブリ言語本体)を生成する関数
    fn gen_asm_preproc(
        &mut self
    ) -> Result<Vec<node::InlineAsm>, err::ErrKind> {
        let mut nodes = Vec::<node::InlineAsm>::new();

        if matches!(
            self.next_tkn(vec!["{"])?, 
            lex::Tkn::LBrace
        ) {
            let _ = self.next_tkn(vec![])?;
            loop {
                match self
                    .current_tkn()
                    .clone()
                {
                    lex::Tkn::Str(value) => {
                        nodes.push(self.gen_asm_line(&value)?);
                    }
                    lex::Tkn::RBrace => {
                        self.next_tkn(vec![])?;
                        break;
                    }
                    t => {
                        panic!("{:?}", t);
                    }
                }
                self.next_tkn(vec![])?;
            }
            return Ok(nodes);
        }

        Err(err::ErrKind::UnexpectedToken)
    }

    /// inlineアセンブラの1行分の文字列から`node::InlineAsm`を作成する。
    ///
    /// `${...}`は出現順に`{0}`, `{1}`, ...のプレースホルダーへ置き換え、
    /// 中に書かれていた式(変数・構造体のメンバー・ポインタなど、
    /// 普通の式として書けるもの)は`operands`に出現順で積んでいく。
    /// 同じ行に複数の`${...}`があっても、すべて取り込む
    /// (以前の実装は最後の1つしか保持できなかった)。
    fn gen_asm_line(
        &mut self, 
        value: &String
    ) -> Result<node::InlineAsm, err::ErrKind> {
        let inline_var = Regex::new(r"\$\{([^}]+)\}")
            .unwrap(); // 一度だけコンパイルして使い回してOK

        let mut operands = Vec::<node::Expr>::new();
        let mut parse_err: Option<err::ErrKind> = None;

        let asm = inline_var
            .replace_all(
                value,
                |caps: &Captures| {
                    if parse_err.is_some() {
                        return String::new();
                    }

                    let inner = &caps[1];

                    match Parser::parse_asm_operand(inner) {
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
                }
            );

        if let Some(e) = parse_err {
            return Err(e);
        }

        Ok(node::InlineAsm { asm, operands })
    }
}

#[cfg(test)]
mod inline_asm_tests {
    use crate::{lex, node, parse};

    /// テスト用に、関数の中に`#asm(...)`ブロックを1つ持つ
    /// プログラムを解析し、`InlineAsm`の一覧を取り出す
    fn gen_inline_asm(asm_body: &str) -> Vec<node::InlineAsm> {
        let src = format!(
            "main(): b1 {{ #asm(gas) {{ {} }} }}", 
            asm_body
        );

        let mut lexer = lex::Lexer::new();
        lexer
            .analy(&src.to_string())
            .unwrap();

        let mut p = parse::Parser::new();
        let nodes = p.parser(lexer.gen_tkns)
            .expect("parse failed");

        let node::Group1Node::FuncDefine(func) = &nodes[0] else {
            panic!("not a func define")
        };

        let node::Group2Node::CompleSyntax(
            (name, lines)
        ) = &func.body[0].get_node() else {
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
                node::Expr::Var("a".to_string()),
                node::Expr::Var("b".to_string()),
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
                node::Expr::Member {
                    scope: vec!["p".to_string()],
                    target: Box::new(node::Expr::Var("x".to_string())),
                },
                node::Expr::Member {
                    scope: vec!["p".to_string()],
                    target: Box::new(node::Expr::Var("y".to_string())),
                },
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
                node::Expr::ConnectAddr(Box::new(node::Expr::Var("p".to_string()))),
                node::Expr::GetAddress(Box::new(node::Expr::Var("p".to_string()))),
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
                node::Expr::Add((
                    Box::new(node::Expr::Var("a".to_string())),
                    Box::new(node::Expr::Number("1".to_string())),
                )),
                node::Expr::Mul((
                    Box::new(node::Expr::Member {
                        scope: vec!["p".to_string()],
                        target: Box::new(node::Expr::Var("x".to_string())),
                    }),
                    Box::new(node::Expr::Number("2".to_string())),
                )),
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
                node::Expr::Var("c".to_string()),
                node::Expr::Var("d".to_string()),
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

        let mut p = parse::Parser::new();
        let nodes = p.parser(lexer.gen_tkns).expect("parse failed");

        let node::Group1Node::FuncDefine(func) = &nodes[0] else {
            panic!("not a func define")
        };

        assert_eq!(func.body.len(), 2);
        assert!(matches!(
            func.body[1].get_node(),
            node::Group2Node::Stmt(node::StmtNode::Return(_))
        ));
    }
}