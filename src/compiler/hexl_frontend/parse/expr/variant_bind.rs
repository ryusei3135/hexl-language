//! `cond`の条件式の中だけで使える、バリアントのパターン束縛
//!
//! ```text
//! variant A { M(int) N }
//!
//! cond M(val) := a {        // `val`の型は`M`の中身(int)から推論
//!     // aが`M`のとき。ここで`val`が使える
//! } | {
//!     // それ以外
//! }
//!
//! cond M(val): int = a { .. } | { .. }   // 型を明示する書き方
//! cond A::M(val) := a { .. } | { .. }     // バリアント型の名前を付ける書き方
//! cond A::M(val) mut:= a { .. } | { .. }  // valを可変にする(`mut`/`const`は`)`と`:`の間)
//! ```
//!
//! `:=`は、`:`と`=`の2つのトークンとして扱う
//! (`a: int = 1`の型の部分を省略した形)。

use super::*;
use crate::compiler::hexl_frontend::parse::VarMutAttr;

impl Parser {
    /// `tkns[start..]`が、`Name ( Name ) :`(または`Name :: Name ( Name ) :`)で
    /// 始まっているかを調べる
    /// (トークンは進めない)。
    ///
    /// `cond f(a) == 1 { .. }`のような通常の関数呼び出しは、
    /// `)`の次が`:`にならないので、区別できる
    pub(in crate::compiler::hexl_frontend::parse) fn is_variant_bind_at(
        &self,
        start: usize,
    ) -> bool {
        let Some(tkns) = self.tkns.as_ref() else {
            return false;
        };
        let tkn_at = |offset: usize| tkns.get(start + offset).map(|t| &t.tkn);
        // `Type::M(val)`のように、型の名前が付いていてもよい
        let off = if matches!(tkn_at(1), Some(lex::Tkn::ModPathTkn)) {
            2
        } else {
            0
        };
        matches!(tkn_at(0), Some(lex::Tkn::Name(_)))
            && matches!(tkn_at(off), Some(lex::Tkn::Name(_)))
            && matches!(tkn_at(off + 1), Some(lex::Tkn::LParen))
            && matches!(tkn_at(off + 2), Some(lex::Tkn::Name(_)))
            && matches!(tkn_at(off + 3), Some(lex::Tkn::RParen))
            // `mut`/`const`は`)`と`:`の間に書く(`e mut: ty = ..`と同じ並び)
            && match tkn_at(off + 4) {
                Some(lex::Tkn::KeyWordMut | lex::Tkn::KeyWordConst) => {
                    matches!(tkn_at(off + 5), Some(lex::Tkn::Colon))
                }
                t => matches!(t, Some(lex::Tkn::Colon)),
            }
    }

    /// 次のトークン(`current_tkn()`の1つ先)から、パターン束縛が始まるか
    #[inline(always)]
    pub(in crate::compiler::hexl_frontend::parse) fn is_variant_bind_ahead(&self) -> bool {
        self.is_variant_bind_at(self.idx + 1)
    }

    /// `M(val) := value` / `M(val): ty = value`のノードを作成する。
    ///
    /// 呼び出し時の`current_tkn()`は、式の1つ手前のトークン
    /// (`cond`や、アームの1つ前の`{`/`}`)。
    /// `is_variant_bind_ahead()`が`true`のときだけ呼び出す。
    /// 終了時は、`value`の次のトークン(`{`や`=>`)を指す
    pub(in crate::compiler::hexl_frontend::parse) fn expr_variant_bind(
        &mut self,
    ) -> Result<Expr, err::ErrKind> {
        // 条件式の中なので、構造体を初期化する式は使えない
        const STRUCT_NOT_INIT: bool = false;

        // M または Type::M
        let lex::Tkn::Name(first) = self.next_tkn(&["name"])? else {
            return crate::err_at!(
                self.variant_bind_unexpected("name", self.current_tkn().clone())
            );
        };
        // 束縛の位置(`M`か`Type`の位置)
        let span = self.build_err_span();
        let (variant_ty, variant) = if self.peek_tkn()? == lex::Tkn::ModPathTkn {
            // `::`
            self.next_tkn(&["::"])?;
            let lex::Tkn::Name(member) = self.next_tkn(&["name"])? else {
                return crate::err_at!(
                    self.variant_bind_unexpected("name", self.current_tkn().clone())
                );
            };
            (Some(first), member)
        } else {
            (None, first)
        };
        // (
        if self.next_tkn(&["("])? != lex::Tkn::LParen {
            return crate::err_at!(self.variant_bind_unexpected("(", self.current_tkn().clone()));
        }
        // val
        let lex::Tkn::Name(bind) = self.next_tkn(&["name"])? else {
            return crate::err_at!(
                self.variant_bind_unexpected("name", self.current_tkn().clone())
            );
        };
        // )
        if self.next_tkn(&[")"])? != lex::Tkn::RParen {
            return crate::err_at!(self.variant_bind_unexpected(")", self.current_tkn().clone()));
        }
        // `mut` / `const`(省略すると不変)
        let var_attr = match self.next_tkn_ref(&["mut", ":"])? {
            lex::Tkn::KeyWordMut => {
                self.next_tkn(&["mut"])?;
                VarMutAttr::Var
            }
            lex::Tkn::KeyWordConst => {
                self.next_tkn(&["const"])?;
                VarMutAttr::Const
            }
            _ => VarMutAttr::Invar,
        };
        // :
        if self.next_tkn(&[":"])? != lex::Tkn::Colon {
            return crate::err_at!(self.variant_bind_unexpected(":", self.current_tkn().clone()));
        }

        let ty = if self.peek_tkn()? == lex::Tkn::Equal {
            // `:=` 型推論。現在のトークンを`=`にする
            self.next_tkn(&["="])?;
            None
        } else {
            // `: ty =` 型を明示。終了時は型の次のトークンを指す
            let ty = self.define_ty_node()?;
            if self.current_tkn() != &lex::Tkn::Equal {
                return crate::err_at!(self.unexpect_tkn_in_expr());
            }
            Some(ty)
        };

        // 判定する対象の式。比較式(`a == b`)はバリアントの値ではないので、
        // 四則演算までの式を受け付ける
        let value = self.expr_add(STRUCT_NOT_INIT)?;

        Ok(VariantBind::new(&variant, &bind, ty, value)
            .with_variant_ty(variant_ty)
            .with_var_attr(var_attr)
            .wrap(span))
    }
}

#[cfg(test)]
mod variant_bind_tests {
    use crate::compiler::{
        ast::{self, *},
        err,
        hexl_frontend::lex,
        hexl_frontend::parse,
    };

    /// テスト用の位置。`Expr`の`PartialEq`は`span`を比較しないので、何でもよい
    fn sp() -> std::rc::Rc<err::Span> {
        std::rc::Rc::new(err::Span::new(0, 0, String::new()))
    }

    /// 位置を付けて`Expr`にする
    fn ex(n: ExprNode) -> Expr {
        Expr::new(n, sp())
    }

    fn gen_main_body(content: &str) -> Result<Vec<Group2Info>, ()> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        let tkns = lexer.gen_tkns.clone();
        let mut p = parse::Parser::new_for_test();
        let nodes = p.parser_for_test(&tkns).map_err(|_| ())?;
        for n in nodes {
            if let Group1Node::FuncDefine(func) = n {
                if func.name == "main" {
                    return Ok(func.body.clone());
                }
            }
        }
        panic!("main is not found");
    }

    /// `main`の最初の文の`Match`のアームの条件式を取り出す
    fn first_arm_patterns(body: &[Group2Info]) -> (Option<Box<Expr>>, Vec<Expr>, bool) {
        let Group2Node::Expr(Expr {
            node:
                ExprNode::Match {
                    pattern,
                    arms,
                    arm_else,
                },
            ..
        }) = body[0].get_node()
        else {
            panic!("not match");
        };
        (
            pattern.clone(),
            arms.iter().map(|a| (*a.pattern).clone()).collect(),
            arm_else.is_some(),
        )
    }

    #[test]
    fn check_bind_infer() {
        // `:=` 型推論
        let body = gen_main_body(
            "
            variant A { M(int) N }
            main(): int {
              cond M(val) := a {
                x: int = 1
              } else {
                y: int = 2
              }
            }
            ",
        )
        .expect("node is err");
        let (target, patterns, has_else) = first_arm_patterns(&body);
        assert_eq!(target, None);
        assert_eq!(
            patterns,
            vec![VariantBind::new("M", "val", None, ex(ExprNode::Var("a".to_string()))).wrap(sp())]
        );
        assert!(has_else);
    }

    #[test]
    fn check_bind_with_ty() {
        // `: ty =` 型を明示
        let body = gen_main_body(
            "
            variant A { M(int) N }
            main(): int {
              cond M(val): int = a {
                x: int = 1
              } else {
                y: int = 2
              }
            }
            ",
        )
        .expect("node is err");
        let (_, patterns, _) = first_arm_patterns(&body);
        assert_eq!(
            patterns,
            vec![
                VariantBind::new(
                    "M",
                    "val",
                    Some(TyNode::Ty("int".to_string())),
                    ex(ExprNode::Var("a".to_string()))
                )
                .wrap(sp())
            ]
        );
    }

    #[test]
    fn check_bind_in_arm_without_target() {
        // 対象の式がない`cond { .. }`のアームでも使える
        let body = gen_main_body(
            "
            variant A { M(int) N }
            main(): int {
              cond {
                M(val) := a => {
                  x: int = 1
                }
                else => {
                  y: int = 2
                }
              }
            }
            ",
        )
        .expect("node is err");
        let (target, patterns, has_else) = first_arm_patterns(&body);
        assert_eq!(target, None);
        assert_eq!(
            patterns,
            vec![VariantBind::new("M", "val", None, ex(ExprNode::Var("a".to_string()))).wrap(sp())]
        );
        assert!(has_else);
    }

    #[test]
    fn check_function_call_condition_is_not_bind() {
        // `f(a) == 1`は、パターン束縛ではなく通常の関数呼び出しの比較
        let body = gen_main_body(
            "
            main() {
              cond f(a) == 1 {
                x: int = 1
              } else {
                y: int = 2
              }
            }
            ",
        )
        .expect("node is err");
        let (_, patterns, _) = first_arm_patterns(&body);
        assert!(matches!(patterns[0].node, ExprNode::Equal(_)));
    }

    #[test]
    fn check_bind_outside_cond_is_err() {
        // `cond`の条件式の外では使えない
        assert!(gen_main_body("main(): int { M(val) := a }").is_err());
        assert!(gen_main_body("main(): int { loop M(val) := a { break } }").is_err());
    }

    #[test]
    fn check_bind_in_arm_with_target_is_err() {
        // `cond a { .. }`のアームは値の比較なので使えない
        assert!(
            gen_main_body(
                "
            main(): int {
              cond a {
                M(val) := b => {
                  x: int = 1
                }
                else => {
                  y: int = 2
                }
              }
            }
            "
            )
            .is_err()
        );
    }

    #[test]
    fn check_bind_with_variant_ty_name() {
        // `A::M(val) := a` のように、バリアント型の名前を付けられる
        let body = gen_main_body(
            "
            variant A { M(int) N }
            main(): int {
              cond A::M(val) := a {
                x: int = 1
              } else {
                y: int = 2
              }
            }
            ",
        )
        .expect("node is err");
        let (_, patterns, _) = first_arm_patterns(&body);
        assert_eq!(
            patterns,
            vec![
                VariantBind::new("M", "val", None, ex(ExprNode::Var("a".to_string())))
                    .with_variant_ty(Some("A".to_string()))
                    .wrap(sp())
            ]
        );
    }

    #[test]
    fn check_bind_mut() {
        // `val`を可変にする: `M(val) mut:= a` / `M(val) mut: int = a`
        for src in ["A::M(val) mut:= a", "A::M(val) mut: int = a"] {
            let body = gen_main_body(&format!(
                "
                variant A {{ M(int) N }}
                main(): int {{
                  cond {} {{
                    val += 1
                  }} else {{
                    y: int = 2
                  }}
                }}
                ",
                src
            ))
            .expect("node is err");
            let (_, patterns, _) = first_arm_patterns(&body);
            let ExprNode::VariantBind(bind) = &patterns[0].node else {
                panic!("not bind");
            };
            assert_eq!(bind.var_attr, parse::VarMutAttr::Var);
        }
    }
}
