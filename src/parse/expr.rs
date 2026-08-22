//! ## 引数
//! 1. init_struct
//!     - これがtrueの場合構造体を初期化するノードを作成できる
//!     - falseの場合は初期化ノードを作成しない

use super::*;

mod cond;
/// このファイルでしか使われないAPIのモジュール
mod value_api;

type ExprResult = Result<node::Expr, err::ErrKind>;

impl Parser {
    pub(super) fn expr_define_var(
        &mut self,
        // 呼び出す前にでた、変数や関数などの名前
        name: String,
    ) -> Result<node::Expr, err::ErrKind> {
        let node = if self
            .next_tkn(vec!["(", "{", ",", "[", ":", "=", "]"])
            .map(|_| true)?
        {
            match &self.current_tkn() {
                lex::Tkn::Dot | lex::Tkn::ModPathTkn => {
                    return self.build_scope_node(&name);
                }
                lex::Tkn::RBrace => {
                    return Ok(node::Expr::Var(name));
                }
                lex::Tkn::LParen => {
                    // 関数の呼び出しノードを生成
                    let n = self.call_func_expr(&name, true);
                    return n;
                }
                lex::Tkn::LBrace => {
                    return self.struct_init_node::<false>(&name);
                }
                lex::Tkn::Comma => {
                    return Ok(node::Expr::Var(name));
                }
                // ポインタ参照
                lex::Tkn::RBracket => {
                    // ポインタ参照なので、次のトークンに進めずに
                    // ノードを返す
                    if !matches!(self.next_tkn_ref(vec!["="])?, lex::Tkn::Equal) {
                        return Ok(node::Expr::Var(name));
                    }
                    self.next_tkn(vec!["="])?;
                    return Ok(node::AssignVar::new(
                        &name,
                        node::Expr::GetAddress(Box::new(node::Expr::Var(name.to_string()))),
                        self.expr_branch()?,
                    ));
                }
                _ => {}
            }

            let ty_node = match &self.current_tkn() {
                // コロンが来た場合、それは型の定義なので、変数の定義
                lex::Tkn::Colon => self.define_ty_node()?,
                _ => {
                    // define assign var node
                    return Ok(node::AssignVar::new(
                        &name,
                        node::Expr::Var(name.to_string()),
                        self.expr_branch()?,
                    ));
                }
            };

            if matches!(self.current_tkn(), lex::Tkn::RBracket) {
                if matches!(self.next_tkn(vec!["="])?, lex::Tkn::Equal) {
                    return Ok(node::DefineVar::new(
                        &name,
                        node::Expr::ConnectAddr(Box::new(self.expr_branch()?)),
                        &ty_node,
                    )
                    .wrap());
                }
            }

            if self.current_tkn() == &lex::Tkn::Equal {
                node::DefineVar::new(&name, self.expr_branch()?, &ty_node).wrap()
            } else {
                panic!(">> :{:?}", self.current_tkn());
            }
        } else {
            let result = err::SyntaxErr::tkn_is_eof(
                self.build_err_span(),
                vec!["(", "{", ",", ":", "]", "="],
            );
            panic!("{:?}", result);
        };

        Ok(node)
    }

    /// 式に代入する物が、構文の式 例(match)かどうかで
    pub(super) fn expr_branch(&mut self) -> Result<node::Expr, err::ErrKind> {
        if matches!(self.next_tkn_ref(vec!["match"])?, lex::Tkn::KeyWordCond) {
            self.next_tkn(vec![])?;
            self.expr_match()
        } else {
            self.expr_cmp(true)
        }
    }

    pub(super) fn expr_cmp(&mut self, init_struct: bool) -> Result<node::Expr, err::ErrKind> {
        let mut left = self.expr_add(init_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::LAngleBracket => {
                    node::Expr::LessThen(node::Expr::wrap(left, self.expr_add(init_struct)?))
                }
                lex::Tkn::RAngleBracket => {
                    node::Expr::GreaterThen(node::Expr::wrap(left, self.expr_add(init_struct)?))
                }
                lex::Tkn::EqEq => {
                    node::Expr::Equal(node::Expr::wrap(left, self.expr_add(init_struct)?))
                }
                lex::Tkn::NotEq => {
                    node::Expr::NotEq(node::Expr::wrap(left, self.expr_add(init_struct)?))
                }
                _ => break,
            };
        }

        Ok(left)
    }

    pub(super) fn expr_add(&mut self, init_struct: bool) -> ExprResult {
        let mut left = self.expr_mul(init_struct)?;

        // expr_mulですでにトークンを進めているので現在のトークンを参照
        loop {
            left = match self.current_tkn() {
                lex::Tkn::Add => {
                    node::Expr::Add(node::Expr::wrap(left, self.expr_mul(init_struct)?))
                }
                lex::Tkn::Sub => {
                    node::Expr::Sub(node::Expr::wrap(left, self.expr_mul(init_struct)?))
                }
                _ => break,
            };
        }
        Ok(left)
    }

    fn expr_mul(&mut self, init_struct: bool) -> ExprResult {
        let mut left = self.expr_value(init_struct)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::Mul => {
                    node::Expr::Mul(node::Expr::wrap(left, self.expr_value(init_struct)?))
                }
                lex::Tkn::Div => {
                    node::Expr::Div(node::Expr::wrap(left, self.expr_value(init_struct)?))
                }
                _ => break,
            };
        }
        Ok(left)
    }

    fn expr_value(&mut self, init_struct: bool) -> ExprResult {
        // ## 値のトークンが出たら
        // - 呼び出し元で、次のトークンに進めるのでNumberやRParenがきたら終了
        if let lex::Tkn::Name(name) = self.current_tkn().clone() {
            match self.next_tkn(vec![])? {
                // おそらくこれは、条件しきなので変数の名前として返す
                lex::Tkn::LBrace => {
                    return self.gen_name_node::<false>(name, init_struct);
                }
                lex::Tkn::RBrace => {
                    return self.gen_name_node::<false>(name, init_struct);
                }
                // 関数を呼ぶノード
                lex::Tkn::LParen => {}
                t => panic!(" name {:?} {:?}", name, t),
            }
            // スコープを作成
            if self.current_tkn() == &lex::Tkn::Dot {
                return self.build_scope_node(&name);
            }
            // 前回のトークンが名前かつ(なので、関数を呼び出すノードを作成する
            return self.call_func_expr(&name, init_struct);
        }

        // 配列の中の処理は`src/parse/expr_value.rs`にある
        let v = match self.next_tkn(vec!["[", "*", "number", "string", "name", "(", "{"])? {
            // 変数のアドレスを取得するノード
            lex::Tkn::LBracket => self.get_var_addr_node()?,
            // ポインタにアクセス
            lex::Tkn::Mul => node::Expr::ConnectAddr(Box::new(self.expr_value(init_struct)?)),
            lex::Tkn::Number(value) => node::Expr::Number(value),
            lex::Tkn::KeyWordSelf => {
                let self_name = self.struct_self_name.as_ref().unwrap().to_string();
                self.gen_name_node::<true>(self_name, init_struct)?
            }
            lex::Tkn::Str(value) => node::Expr::Str(value),
            lex::Tkn::Name(name) => self.gen_name_node::<false>(name, init_struct)?,
            lex::Tkn::LParen => {
                let result = self.expr_cmp(init_struct)?;

                if self.current_tkn() == &lex::Tkn::LParen {
                    dbg!(self.current_tkn());
                    Err(err::ErrKind::UnexpectedToken)?
                } else {
                    result
                }
            }
            // 配列リテラル: `{100, 100, 100, 100}`
            lex::Tkn::LBrace => self.make_array_node()?,
            t => panic!("expr {:?} {:?}", t, self.peek_tkn()),
        };

        // `call_func_expr`は、自身で`)`を読み飛ばして呼び出し式の
        // 次のトークンまで進めた状態で返ってくる(構造体初期化/配列
        // リテラルが`}`を消費せず呼び出し元に委ねるのとは逆の方式)。
        // そのため、値が関数呼び出し(または関数呼び出しを直接
        // 包んだ`Scope`/`Member`)の場合、現在のトークンはすでに
        // 呼び出し式の「次」を正しく指しており、ここでさらに
        // 読み進めてはいけない
        fn already_positioned_after_call(v: &node::Expr) -> bool {
            match v {
                node::Expr::CallFunc(..) | node::Expr::Scope { .. } => true,
                node::Expr::Member { target, .. } => {
                    matches!(**target, node::Expr::CallFunc(..))
                }
                _ => false,
            }
        }

        if !already_positioned_after_call(&v) {
            match self.current_tkn() {
                lex::Tkn::Name(_)
                | lex::Tkn::Number(_)
                | lex::Tkn::Str(_)
                | lex::Tkn::RParen
                | lex::Tkn::RBracket => {
                    self.next_tkn(vec![])?;
                }
                // `}` は、構造体初期化(`Name { .. }`)や配列リテラル
                // (`{ .. }`)を閉じる場合にのみ、ここで読み飛ばす。
                // `self.b`のようなメンバーアクセスの直後に続く`}`は
                // 呼び出し元のブロック(関数/メゾットの本体など)を
                // 閉じるトークンなので、消費せずそのまま残す
                lex::Tkn::RBrace
                    if matches!(v, node::Expr::InitStruct { .. } | node::Expr::Array(..)) =>
                {
                    self.next_tkn(vec![])?;
                }
                _ => {}
            }
        }

        Ok(v)
    }

    /// 関数を呼び出すノードを作成
    ///
    /// ## Args
    /// - name
    ///     関数の名前
    /// - init_struct
    ///     これがtrueの場合のみ構造体を初期化するノードを作成可能
    ///
    /// ## Panics
    /// 現在のトークンが`lex::Tkn::LParen`でないならpanicする
    ///
    /// ## Safety
    /// この関数が実行される場合、現在のトークンが`lex::Tkn::LParen`
    /// である必要がある
    pub(super) fn call_func_expr(&mut self, name: &String, init_struct: bool) -> ExprResult {
        if !matches!(self.current_tkn(), lex::Tkn::LParen) {
            panic!("call_func_exprを呼び出す際にLParenではない");
        }
        // 引数
        let mut args = Vec::<node::Expr>::new();

        // 関数を呼び出す式に引数がない場合は実行されない
        if !matches!(self.next_tkn_ref(vec!["not `)`"])?, lex::Tkn::RParen) {
            loop {
                // 引数の式を取得
                args.push(self.expr_cmp(init_struct)?);

                match self.current_tkn() {
                    lex::Tkn::Comma => {
                        continue;
                    }
                    lex::Tkn::RParen => {
                        // 関数の最後の部分に来たので、ループを終了する
                        break;
                    }
                    _ => {
                        panic!("{:?}", self.next_tkn_ref(vec![]));
                    }
                }
            }
        } else {
            // 引数がない場合、現在のトークンはまだ`(`のままなので、
            // 引数がある場合のループが`)`を指した状態で抜けるのに
            // 合わせて、ここで`)`まで進めておく
            self.next_tkn(vec![])?;
        }

        // ')'をスキップ
        self.next_tkn(vec![])?;
        Ok(node::Expr::CallFunc(node::CallInfo {
            name: name.clone(),
            args,
        }))
    }
}

#[cfg(test)]
mod expr_tests {
    use crate::{
        lex,
        node::{self, *},
        parse,
    };

    fn gen_nodes(content: &str) -> Vec<lex::LocatedTkn> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        lexer.gen_tkns.clone()
    }

    #[test]
    fn check_get_address_var() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): b1 { a: int* = [b] }");
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            &node.body[0],
            &node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::GetAddress(Box::new(node::Expr::Var(
                    "b".to_string()
                )))),
                ty: node::TyNode::Pointer {
                    is_const: false,
                    ty_name: Box::new(node::TyNode::Ty("int".to_string()))
                }
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_call_func_node() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): b1 { a(10, a) }");
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            &node.body[0],
            &node::Expr::CallFunc(node::CallInfo {
                name: "a".to_string(),
                args: vec![
                    node::Expr::Number("10".to_string()),
                    node::Expr::Var("a".to_string()),
                ]
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_stack_var_single() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): int { a: [int] = 100 }");
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0],
            node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::Number("100".to_string())),
                ty: node::TyNode::Stack {
                    name: "int".to_string(),
                    len: 1
                },
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_stack_var_array() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): int { a: [int 4] = {100, 100, 100, 100} }");
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0],
            node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::Array(vec![
                    node::Expr::Number("100".to_string()),
                    node::Expr::Number("100".to_string()),
                    node::Expr::Number("100".to_string()),
                    node::Expr::Number("100".to_string()),
                ])),
                ty: node::TyNode::Stack {
                    name: "int".to_string(),
                    len: 4
                },
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_static_var_single() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): int { a: static[int] = 100 }");
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0],
            node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::Number("100".to_string())),
                ty: node::TyNode::Static {
                    name: "int".to_string(),
                    len: 1
                },
            })
            .wrap_group2()
        );
    }

    #[test]
    fn check_match_expr() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(
            "
            main(): int {
              cond {
                10 == 10 => {
                  hh: b1 = 100
                }
                | => {
                  a: b1 = 10
                }
              }
            }
            ",
        );
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0],
            node::Expr::Match {
                pattern: None,
                arms: vec![node::MatchArm {
                    pattern: Box::new(wrap_eq_expr_cmp("10", "10")),
                    body: vec![gen_var_node("hh", "100", "b1"),],
                }],
                arm_else: Some(vec![gen_var_node("a", "10", "b1"),]),
            }
            .wrap_group2()
        );
    }

    #[test]
    fn check_match_expr_bool() {
        // 1. Booleanを与えるパターン (単純なif/else)
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(
            "
            main(): int {
              cond a == 10 {
                hh: int = 100
              } | {
                a: int = 10
              }
            }
            ",
        );
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0],
            node::Expr::Match {
                pattern: None,
                arms: vec![node::MatchArm {
                    pattern: Box::new(node::Expr::Equal((
                        Box::new(node::Expr::Var("a".to_string())),
                        Box::new(node::Expr::Number("10".to_string())),
                    ))),
                    body: vec![gen_var_node("hh", "100", "int"),],
                }],
                arm_else: Some(vec![gen_var_node("a", "10", "int"),]),
            }
            .wrap_group2()
        );
    }

    #[test]
    fn check_match_expr_value() {
        // 2. 値を与えるパターン (他の言語のswitch/matchに相当)
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(
            "
            main(): int {
              cond a {
                10 => {
                  hh: int = 100
                }
                20 => {
                  hh: int = 200
                }
                | => {
                  a: int = 10
                }
              }
            }
            ",
        );
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0],
            node::Expr::Match {
                pattern: Some(Box::new(node::Expr::Var("a".to_string()))),
                arms: vec![
                    node::MatchArm {
                        pattern: Box::new(node::Expr::Number("10".to_string())),
                        body: vec![gen_var_node("hh", "100", "int"),],
                    },
                    node::MatchArm {
                        pattern: Box::new(node::Expr::Number("20".to_string())),
                        body: vec![gen_var_node("hh", "200", "int"),],
                    }
                ],
                arm_else: Some(vec![gen_var_node("a", "10", "int"),]),
            }
            .wrap_group2()
        );
    }

    #[test]
    fn check_enum_variant_expr() {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes("main(): int { a: Color = Color::Green }");
        let node::Group1Node::FuncDefine(ref node) = p.parser(tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        assert_eq!(
            node.body[0],
            node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::EnumVariant {
                    name: "Color".to_string(),
                    variant: "Green".to_string(),
                }),
                ty: node::TyNode::Ty("Color".to_string()),
            })
            .wrap_group2()
        );
    }
}
