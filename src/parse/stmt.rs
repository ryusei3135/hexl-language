//! group1は、関数や構造体など
//! group2は変数の定義や条件分岐など

use std::collections::HashMap;

use crate::{models::Body, node::Group2Info};

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum GenFlag {
    Group1,
    Group2,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum StkInfo {
    Generics,
}

/// ジェネリクス関数(`func<T>(..) { .. }`)の定義。
///
/// 定義を読んだ時点ではノードを作らず、トークン列をそのまま保存しておく。
/// 呼び出し(`func<int>(..)`)のたびに、型パラメータを実際の型の
/// トークンに置き換えて解析し直し、その型の関数のノードを作る
/// (`func.rs`の`instantiate_generic_func`)。
#[derive(Clone, Debug, PartialEq)]
pub(super) struct GenericFunc {
    pub(super) is_public: bool,
    /// `<T, U>`の型パラメータの名前
    pub(super) params: Vec<String>,
    /// 関数名から本体を閉じる`}`までのトークン。
    /// ただし`<T, U>`の部分は取り除いてある
    /// (`name(arg: T): T { .. }`という通常の関数と同じ形)
    pub(super) tkns: Vec<lex::LocatedTkn>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parser {
    pub(super) gen_nodes: Vec<node::Group1Node>,
    pub(super) tkns: Option<Vec<lex::LocatedTkn>>,
    pub(super) idx: usize,
    pub(super) scope_counter: usize,
    pub(super) loop_depth: usize,
    pub(super) struct_self_name: Option<String>,
    pub(super) gen_flag: GenFlag,
    pub(super) other_stk: Vec<(String, StkInfo)>, // 処理中の一時データを保存
    /// 定義済みのジェネリクス関数(関数名 -> 定義)
    pub(super) generic_funcs: HashMap<String, GenericFunc>,
    /// 作成済みの(関数名, `<>`の中身)。同じ型での呼び出しが
    /// 何度あっても、その型の関数は1つだけ作るために使う
    pub(super) generated_funcs: Vec<(String, Vec<node::TyNode>)>,
    /// 呼び出された型ごとに作った関数のノード。
    /// 関数の解析中に`gen_nodes`へ積むと、本体の追加先
    /// (`gen_nodes.last_mut()`)がずれてしまうため、
    /// 解析がすべて終わってから`gen_nodes`の後ろに追加する
    pub(super) pending_funcs: Vec<node::Group1Node>,
    /// ジェネリクス関数の中で、さらにジェネリクス関数が
    /// 呼び出されている入れ子の深さ
    pub(super) generic_depth: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            gen_nodes: Vec::new(),
            tkns: None,
            idx: 0,
            scope_counter: 0,
            loop_depth: 0,
            struct_self_name: None,
            gen_flag: GenFlag::Group1,
            other_stk: Vec::new(),
            generic_funcs: HashMap::new(),
            generated_funcs: Vec::new(),
            pending_funcs: Vec::new(),
            generic_depth: 0,
        }
    }

    // 関数の中身などを作成する
    // P は、この関数が公開されるかどうかのbool
    fn build_func<const P: bool>(
        &mut self, 
        func_name: &String
    ) -> Result<(), err::ErrKind> {
        // ジェネリクス関数(`name<T>(..)`)の定義は、呼び出されるまで
        // ノードを作らない。定義は`collect_generic_funcs`で集め済みなので、
        // ここでは本体を閉じる`}`まで読み飛ばすだけ
        if matches!(
            self.next_tkn_ref(vec!["(", "<"])?, 
            lex::Tkn::LAngleBracket
        ) {
            return self.skip_generic_func_def();
        }

        // トップレベルの関数定義なので、`Self`が解決される
        // 構造体/列挙型は存在しない
        let node = self.func_node(&func_name, P)?;
        self.gen_nodes.push(node);

        self.gen_flag = GenFlag::Group2;
        self.scope_counter += 1;
        Ok(())
    }

    pub fn parser(
        &mut self,
        tkns: Vec<lex::LocatedTkn>,
    ) -> Result<&Vec<node::Group1Node>, err::ErrKind> {
        self.tkns = Some(tkns);

        // 呼び出しが定義より前に書かれていても、その型の関数を
        // 作れるように、先にジェネリクス関数の定義を集めておく
        self.collect_generic_funcs()?;
        self.parse_loop()?;

        // 呼び出された型ごとに作った関数を、通常の関数の後ろに追加する
        self.gen_nodes.append(&mut self.pending_funcs);
        Ok(&self.gen_nodes)
    }

    /// `self.tkns`の先頭から順に、関数などのノードを`gen_nodes`へ作る。
    /// ジェネリクス関数の呼び出しで、型ごとの関数を作る際にも
    /// (トークン列を差し替えて)この関数が使われる
    pub(super) fn parse_loop(&mut self) -> Result<(), err::ErrKind> {
        loop {
            match &self.gen_flag {
                GenFlag::Group1 => {
                    match self.current_tkn().clone() {
                        lex::Tkn::KeyWordPub => {
                            self.pub_keyword_node()?;
                        }
                        lex::Tkn::Name(name) => {
                            self.build_func::<false>(&name)?;
                        }
                        lex::Tkn::CompleSyn => {
                            let node = self.comple_syntax()?;
                            self.gen_nodes.push(node.change_group1());
                        }
                        lex::Tkn::KeyWordStruct => {
                            let node = self.struct_node()?;
                            self.gen_nodes.push(node);
                        }
                        lex::Tkn::KeyWordEnum => {
                            let node = self.enum_node()?;
                            self.gen_nodes.push(node);
                        }
                        t => {
                            crate::syntax_err!(
                                self.build_err_span(),
                                err::SyntaxErrKind::UnexpectTknInStmt { found: t }
                            )?;
                        }
                    };
                }
                GenFlag::Group2 => {
                    // 関数の中身が空(`{}`)の場合、`one_line_node`を呼ばずに
                    // ここでスコープを閉じる
                    if matches!(self.current_tkn(), lex::Tkn::RBrace) {
                        self.scope_counter -= 1;
                        if self.next_tkn(vec![])
                            .is_err() 
                        {
                            return Ok(());
                        }

                        self.gen_flag = GenFlag::Group1;
                        continue;
                    }

                    let node: Group2Info = self.one_line_node()?
                        .gen_group_info(&self.build_err_span().line);

                    match self.gen_nodes.last_mut().unwrap() {
                        node::Group1Node::FuncDefine(func) => {
                            func.add(node);
                        }
                        // `Group2`のノードは常に直前に生成された関数定義に
                        // 追加されるはずなので、ここに来るのはパーサー自体の
                        // バグ(利用者の入力に起因するエラーではない)
                        t => unreachable!(
                            "one_line_nodeで生成されたノードの追加先が関数定義ではありません: {:?}",
                            t
                        ),
                    }

                    if matches!(self.current_tkn(), lex::Tkn::RBrace) {
                        self.scope_counter -= 1;
                        if self.next_tkn(vec![]).is_err() {
                            return Ok(());
                        }

                        self.gen_flag = GenFlag::Group1;
                    }
                    continue;
                }
            }
            if self.next_tkn(vec![]).is_err() && self.scope_counter == 0 {
                return Ok(());
            }
        }
        //Ok(())
    }

    pub(super) fn one_line_node(
        &mut self
    ) -> Result<node::Group2Node, err::ErrKind> {
        let node = match self.current_tkn().clone() {
            lex::Tkn::CompleSyn => self.comple_syntax()?,
            lex::Tkn::Name(name) => {
                node::Group2Node::Expr(self.build_scope_node(&name)?)
            }
            // ポインタ/配列にアクセスするノードの作成
            lex::Tkn::LBracket => {
                let tkn = self.next_tkn(vec!["name"])?;
                if let lex::Tkn::Name(name) = tkn.clone() {
                    match self.peek_tkn() {
                        // `name`の次が数字の場合、配列への代入
                        // `[name index] = value`
                        Some(lex::Tkn::Number(_)) => {
                            self.make_array_assign_node(&name)?.wrap_group2()
                        }
                        // それ以外の場合、ポインタへの代入
                        // `[name] = value`
                        _ => {
                            let mut ptr_connect = self.expr_define_var(name.to_string())?;
                            ptr_connect.get_assign_node().name = name.to_string();
                            let dst = ptr_connect.get_assign_node().clone().dst;
                            ptr_connect.get_assign_node().dst =
                                Box::new(node::Expr::ConnectAddr(dst));
                            ptr_connect.wrap_group2()
                        }
                    }
                } else {
                    return crate::syntax_err!(
                        self.build_err_span(),
                        err::SyntaxErrKind::ExpectedKind {
                            expected: "name",
                            found: tkn,
                        }
                    );
                }
            }
            lex::Tkn::KeyWordRet => node::StmtNode::Return(self.expr_add(true)?).wrap(),
            lex::Tkn::KeyWordContinue => self.loop_control_node(true)?,
            lex::Tkn::KeyWordBreak => self.loop_control_node(false)?,
            lex::Tkn::KeyWordLoop => self.make_loop_node()?,
            // 条件分岐
            lex::Tkn::KeyWordCond => {
                let n = self.expr_match()?;
                node::Group2Node::Expr(n)
            }
            lex::Tkn::RBrace => {
                // 空のブロック(`{}`)は呼び出し元(`gen_block_node`や
                // メゾットの本体を解析する処理など)が`one_line_node`を
                // 呼ぶ前に判定するはずなので、ここに来るのは想定外
                panic!("one_line_node: 空のブロックが処理されていません")
            }
            t => {
                return crate::syntax_err!(
                    self.build_err_span(),
                    err::SyntaxErrKind::UnexpectTknInStmt { found: t }
                );
            }
        };
        Ok(node)
    }

    fn loop_control_node(
        &mut self,
        is_continue: bool,
    ) -> Result<node::Group2Node, err::ErrKind> {
        if self.loop_depth == 0 {
            return Err(err::ErrKind::Syntax(err::SyntaxErr {
                kind: err::SyntaxErrKind::UnexpectTknInStmt {
                    found: self.current_tkn().clone(),
                },
                loc: err::ErrLoc::new(
                    self.build_err_span(),
                    "parse::stmt::loop_control_node".to_string(),
                ),
            }));
        }
        self.next_tkn(vec![])?;
        let stmt = if is_continue {
            node::StmtNode::Continue
        } else {
            node::StmtNode::Break
        };
        Ok(stmt.wrap())
    }

    fn pub_keyword_node(
        &mut self
    ) -> Result<(), err::ErrKind> {
        match self.next_tkn(vec!["name", ".."])? {
            lex::Tkn::Name(name) => self.build_func::<true>(&name),
            unexpect_tkn => {
                // 期待したトークンじゃないので、エラー
                crate::syntax_err!(
                    self.build_err_span(),
                    err::SyntaxErrKind::UnexpectTknAfterKeyword {
                        keyword: lex::Tkn::KeyWordPub,
                        expected: vec!["struct", "enum", "name"],
                        found: unexpect_tkn,
                    }
                )
            }
        }
    }

    /// 反復処理のノードを作成する関数
    fn make_loop_node(
        &mut self
    ) -> Result<node::Group2Node, err::ErrKind> {
        // 反復処理の条件式
        let pattern = match self.next_tkn_ref(vec!["{", ".."])? {
            // "{"の場合は条件無し
            lex::Tkn::LBrace => None,
            // 条件式あり
            _ => {
                // 条件に構造体の初期化を使うことはできない
                Some(Box::new(self.expr_cmp(false)?))
            }
        };
        // "{"をスキップ
        if !matches!(self.current_tkn(), lex::Tkn::LBrace) {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: "{",
                    found: self.current_tkn().clone(),
                }
            );
        }
        self.next_tkn(vec!["{"])?;

        self.loop_depth += 1;
        let body_result = self.gen_block_node();
        self.loop_depth -= 1;
        let body = body_result?;
        self.next_tkn(vec!["expr"])?;

        let node = node::Group2Node::Expr(
            node::Expr::Loop { 
                pattern, 
                body 
            }
        );
        Ok(node)
    }

    /// 同じスコープ内のノードを生成
    pub(super) fn gen_block_node(
        &mut self
    ) -> Result<Body, err::ErrKind> {
        let mut block = Body::new();

        // ブロックが空(`{}`)の場合、`one_line_node`を呼ばずに
        // そのまま空のブロックを返す
        if matches!(self.current_tkn(), lex::Tkn::RBrace) {
            return Ok(block);
        }

        loop {
            let node = self.one_line_node()?;
            block.push(
                node.gen_group_info(
                    &self.build_err_span().line
                )
            );
            if matches!(
                self.current_tkn(), 
                lex::Tkn::RBrace
            ) {
                break;
            }
        }
        Ok(block)
    }

    pub(super) fn comple_syntax(
        &mut self
    ) -> Result<node::Group2Node, err::ErrKind> {
        let tkn = self.next_tkn(vec!["name"])?;
        let lex::Tkn::Name(name) = tkn.clone() else {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: "name",
                    found: tkn,
                }
            );
        };
        self.make_preproc(&name)
    }

    /// 配列に値を代入するノードを作成する
    ///
    /// ## 呼び出し時の前提
    /// 呼び出し元(`one_line_node`)で`lex::Tkn::LBracket`の次の
    /// `lex::Tkn::Name(name)`まで読み進めた状態で呼び出す。
    /// つまり`current_tkn()`が`name`を指している必要がある。
    ///
    /// ## 文法のルール
    /// `[name index] = value`
    /// - `[arr 0] = 10`
    ///
    /// ## Errors
    /// `name`の次のトークンが数字(`lex::Tkn::Number`)ではない場合エラー
    fn make_array_assign_node(
        &mut self, 
        name: &String
    ) -> Result<node::Expr, err::ErrKind> {
        // `index`は数字である必要がある。そうでなければエラーを返す
        let tkn = self.next_tkn(vec!["number"])?;
        let lex::Tkn::Number(index) = tkn.clone() else {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: "number",
                    found: tkn,
                }
            );
        };
        self.next_tkn(vec!["]"])?;
        self.next_tkn(vec!["="])?;
        let value = self.expr_branch()?;

        Ok(node::AssignVar::new(
            name,
            node::Expr::RefArray {
                name: name.to_string(),
                dst: Box::new(node::Expr::Var(name.to_string())),
                index: Box::new(node::Expr::Number(index)),
            },
            value,
        ))
    }
}