//! 変数や引数が不変か可変を判定するパーサーのモジュール
use std::rc::Rc;

use super::*;
use crate::compiler::ast::{AssignVar, Expr, ExprNode};

impl Parser {
    /// 変数を定義する際に、可変か不変かを判定する
    /// - 可変の場合、`true`を返す
    #[inline(always)]
    pub(in crate::compiler::hexl_frontend::parse) fn assign_expr_is_mut(
        &mut self,
        name: &str,
    ) -> Result<VarMutAttr, Result<Expr, err::ErrKind>> {
        let attr = match &self.current_tkn() {
            lex::Tkn::KeyWordMut => {
                if let Err(e) = self.advance_tkn_checked() {
                    return Err(Err(e));
                }
                VarMutAttr::Var
            }
            lex::Tkn::KeyWordConst => {
                if let Err(e) = self.advance_tkn_checked() {
                    return Err(Err(e));
                }
                VarMutAttr::Const
            }
            // コロンが来た場合、それは型の定義なので、変数の定義
            lex::Tkn::Colon => VarMutAttr::Invar,
            // `a += 1` のような複合代入は `a = a + 1` の形に展開して
            // 通常の代入ノード(`AssignVar`)にする。
            lex::Tkn::AddEq => {
                return Err(self.compound_assign_node(name, ExprNode::Add));
            }
            lex::Tkn::SubEq => {
                return Err(self.compound_assign_node(name, ExprNode::Sub));
            }
            lex::Tkn::MulEq => {
                return Err(self.compound_assign_node(name, ExprNode::Mul));
            }
            lex::Tkn::DivEq => {
                return Err(self.compound_assign_node(name, ExprNode::Div));
            }
            _ => {
                // define assign var node
                return Err(self.plain_assign_node(name));
            }
        };
        Ok(attr)
    }

    /// 複合代入(`a += 1`など)を、`a = a + 1`の代入ノードにする
    fn compound_assign_node(
        &mut self,
        name: &str,
        op: fn((Box<Expr>, Box<Expr>)) -> ExprNode,
    ) -> Result<Expr, err::ErrKind> {
        // 代入演算子(`+=`など)の位置
        let span = Rc::new(self.build_err_span());
        let value = self.compound_assign_value(name, op, Rc::clone(&span))?;
        let dst = Expr::new(ExprNode::Var(name.to_string()), Rc::clone(&span));
        Ok(AssignVar::new(name, dst, value, Rc::clone(&span)))
    }

    /// 通常の代入(`a = 1`)の代入ノードを作る
    fn plain_assign_node(&mut self, name: &str) -> Result<Expr, err::ErrKind> {
        // 代入の位置(現在のトークン)
        let span = Rc::new(self.build_err_span());
        let value = self.expr_branch()?;
        let dst = Expr::new(ExprNode::Var(name.to_string()), Rc::clone(&span));
        Ok(AssignVar::new(name, dst, value, Rc::clone(&span)))
    }

    /// `a += 1` / `a -= 1` / `a *= 1` / `a /= 1` の右辺を
    /// `a <op> 1` の式に展開する。
    ///
    /// `op`には `ExprNode::Add` / `Sub` / `Mul` / `Div` のような
    /// タプルバリアントのコンストラクタをそのまま渡す。
    /// `span`は、作る式(`a <op> 1`と、その中の`a`)の位置
    #[inline(always)]
    fn compound_assign_value(
        &mut self,
        name: &str,
        op: fn((Box<Expr>, Box<Expr>)) -> ExprNode,
        span: Rc<err::Span>,
    ) -> Result<Expr, err::ErrKind> {
        let var = Expr::new(ExprNode::Var(name.to_string()), span.clone());
        Ok(Expr::new(
            op(Expr::wrap(var, self.expr_branch()?)),
            span,
        ))
    }

    #[inline(always)]
    pub fn args_is_mut(&mut self, _name: &str) -> Result<VarMutAttr, err::ErrKind> {
        // 引数が不変か
        let attr = match self.next_tkn_ref(&["mut"])? {
            lex::Tkn::KeyWordMut => VarMutAttr::Var,
            lex::Tkn::KeyWordConst => VarMutAttr::Const,
            _ => VarMutAttr::Invar,
        };
        if attr != VarMutAttr::Invar {
            self.next_tkn(&["mut"])?;
        }
        Ok(attr)
    }
}
