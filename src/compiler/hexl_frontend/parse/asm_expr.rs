//! inlineアセンブラ(`#asm(...) { "..." }`)の中に出てくる
//! `${...}` の内側を解析するための、専用の式パーサー
//!
//! ## なぜ専用のパーサーが必要か
//! `${x}` や `${x.y}` のような内容は、字句解析すると
//! それだけで完結した(後ろに何も続かない)トークン列になる。
//! これに対し、既存の`one_line_node`/`build_scope_node`などの
//! 文の解析APIは「この式の後には必ずまだトークンが続く」
//! (`;`や`}`など)という前提で先読み(`next_tkn_ref`)している。
//! そのため`${...}`の内容をそれらにそのまま渡すと、
//! 先読みした瞬間にトークンが無く(EOF)、エラーとして
//! 伝播してしまう。
use super::*;
use crate::compiler::ast::{Expr, ExprNode};
use std::rc::Rc;

impl Parser {
    /// `${...}` の内側の文字列から式のノードを作成する
    ///
    /// ## 引数
    /// - src `${}`の中に書かれていた文字列(例: `"x.y"`, `"*p"`)
    pub(super) fn parse_asm_operand(&self, src: &str) -> Result<Expr, err::ErrKind> {
        let mut lexer = lex::Lexer::new();
        // 字句解析のエラーは、`${}`の中の文字列が不正ということ
        lexer
            .analy(&src.to_string())
            .map_err(|_| err::ErrKind::UnexpectedToken)?;

        if lexer.gen_tkns.is_empty() {
            return crate::err_at!(self.empty_asm_operand());
        }

        let mut parser = Parser::new(&Rc::clone(&self.file_name), Rc::clone(&self.err_pool));
        parser.tkns = Some(lexer.gen_tkns);

        // `${}`の中の文字列だけを解析する別のパーサーなので、その中の位置は
        // 元のソースの位置にならない。そのため、すべてのノードに
        // この`${}`を含む`#asm`の位置(`self`の現在の位置)を使う
        let span = Rc::new(self.build_err_span());
        let node = parser.asm_operand_expr_at(&span)?;

        // 式の解析が終わった後もまだトークンが残っている場合は
        // `${...}`の中に、式として解析できない文字列が
        // 混ざっていることになるのでエラーにする
        if parser.peek_tkn().is_ok() {
            return crate::err_at!(parser.unexpected_trailing_tkn_in_asm_operand());
        }

        Ok(node)
    }

    /// 加算・減算を含む式のエントリーポイント
    /// ノードの位置には、このパーサーの現在の位置を使う
    pub(super) fn asm_operand_expr(&mut self) -> Result<Expr, err::ErrKind> {
        let span = Rc::new(self.build_err_span());
        self.asm_operand_expr_at(&span)
    }

    /// `asm_operand_expr`と同じだが、ノードの位置を`span`で指定する
    pub(super) fn asm_operand_expr_at(
        &mut self,
        span: &Rc<err::Span>,
    ) -> Result<Expr, err::ErrKind> {
        self.asm_operand_add(span)
    }

    /// 二項演算のノードを作る
    fn asm_binary(
        ctor: fn((Box<Expr>, Box<Expr>)) -> ExprNode,
        left: Expr,
        right: Expr,
        span: &Rc<err::Span>,
    ) -> Expr {
        Expr::new(ctor(Expr::wrap(left, right)), span.clone())
    }

    fn asm_operand_add(&mut self, span: &Rc<err::Span>) -> Result<Expr, err::ErrKind> {
        let mut left = self.asm_operand_mul(span)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::Add => {
                    self.advance_tkn();
                    Self::asm_binary(ExprNode::Add, left, self.asm_operand_mul(span)?, span)
                }
                lex::Tkn::Sub => {
                    self.advance_tkn();
                    Self::asm_binary(ExprNode::Sub, left, self.asm_operand_mul(span)?, span)
                }
                _ => break,
            };
        }

        Ok(left)
    }

    fn asm_operand_mul(&mut self, span: &Rc<err::Span>) -> Result<Expr, err::ErrKind> {
        let mut left = self.asm_operand_unary(span)?;

        loop {
            left = match self.current_tkn() {
                lex::Tkn::Mul => {
                    self.advance_tkn();
                    Self::asm_binary(
                        ExprNode::Mul,
                        left,
                        self.asm_operand_unary(span)?,
                        span,
                    )
                }
                lex::Tkn::Div => {
                    self.advance_tkn();
                    Self::asm_binary(
                        ExprNode::Div,
                        left,
                        self.asm_operand_unary(span)?,
                        span,
                    )
                }
                _ => break,
            };
        }

        Ok(left)
    }

    /// ポインタの参照(`*p`)とアドレス取得(`[p]`)を含む単項式
    fn asm_operand_unary(&mut self, span: &Rc<err::Span>) -> Result<Expr, err::ErrKind> {
        match self.current_tkn().clone() {
            // `*p` ポインタ`p`が指す値を読み取る
            lex::Tkn::Mul => {
                self.advance_tkn();
                Ok(Expr::new(
                    ExprNode::ConnectAddr(Box::new(self.asm_operand_unary(span)?)),
                    span.clone(),
                ))
            }
            // `[p]` 変数`p`のアドレスを取得する
            lex::Tkn::LBracket => {
                self.advance_tkn();
                let inner = self.asm_operand_add(span)?;

                if !matches!(self.current_tkn(), lex::Tkn::RBracket) {
                    return crate::err_at!(self.expected_rbracket_in_asm_operand());
                }
                self.advance_tkn();

                Ok(Expr::new(
                    ExprNode::GetAddress(Box::new(inner)),
                    span.clone(),
                ))
            }
            _ => self.asm_operand_primary(span),
        }
    }

    fn asm_operand_primary(&mut self, span: &Rc<err::Span>) -> Result<Expr, err::ErrKind> {
        match self.current_tkn().clone() {
            lex::Tkn::Number(value) => {
                self.advance_tkn();
                Ok(Expr::new(ExprNode::Number(value), span.clone()))
            }
            lex::Tkn::Str(value) => {
                self.advance_tkn();
                Ok(Expr::new(ExprNode::Str(value), span.clone()))
            }
            lex::Tkn::Name(name) => {
                self.advance_tkn();
                self.asm_operand_name_tail(name, span)
            }
            lex::Tkn::LParen => {
                self.advance_tkn();
                let inner = self.asm_operand_add(span)?;

                if !matches!(self.current_tkn(), lex::Tkn::RParen) {
                    return crate::err_at!(self.expected_rparen_in_asm_operand());
                }
                self.advance_tkn();

                Ok(inner)
            }
            _ => {
                return crate::err_at!(self.unexpected_tkn_in_asm_operand());
            }
        }
    }

    /// 変数名の後に続く、構造体のメンバーアクセス(`.field`)を解析する
    /// - `x`   -> 通常の変数の参照
    /// - `x.y` -> 構造体`x`のメンバー`y`への参照
    fn asm_operand_name_tail(
        &mut self,
        name: String,
        span: &Rc<err::Span>,
    ) -> Result<Expr, err::ErrKind> {
        if matches!(self.current_tkn(), lex::Tkn::Dot) {
            self.advance_tkn_checked()?;

            let lex::Tkn::Name(field) = self.current_tkn().clone() else {
                return crate::err_at!(self.expected_member_name_in_asm_operand());
            };
            self.advance_tkn_checked()?;

            Ok(Expr::new(
                ExprNode::Member {
                    scope: vec![name],
                    target: Box::new(Expr::new(ExprNode::Var(field), span.clone())),
                },
                span.clone(),
            ))
        } else {
            Ok(Expr::new(ExprNode::Var(name), span.clone()))
        }
    }
}
