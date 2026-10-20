//! group1は、関数や構造体など
//! group2は変数の定義や条件分岐など
pub(in crate::compiler::parse) mod parse_loop_control;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub(in crate::compiler::parse) use parse_loop_control::*;

use crate::compiler::parse::stmt::parse_loop_control::ParseLoopControl;
use crate::err::err_pool;

use crate::compiler::{c_frontend, models::Body, node::Group2Info};
use crate::syntax_err;

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

/// ジェネリクスな構造体/バリアント
/// (`struct Name<T> { .. }` / `variant Name<T> { .. }` / `variant "unsafe" Name<T> { .. }`)の定義。
///
/// 関数と同じく、定義を読んだ時点ではノードを作らず、トークン列をそのまま
/// 保存しておく。`Name<int>`のように使われるたびに、型パラメータを実際の型の
/// トークンに置き換えて解析し直し、その型の構造体/バリアントのノードを作る
/// (`typedef/generic.rs`の`instantiate_generic_type`)。
#[derive(Clone, Debug, PartialEq)]
pub(super) struct GenericType {
    /// `<T, U>`の型パラメータの名前
    pub(super) params: Vec<String>,
    /// `struct`/`variant`から本体を閉じる`}`までのトークン。
    /// ただし`<T, U>`の部分は取り除いてある
    /// (`struct Name { .. }`という通常の定義と同じ形)
    pub(super) tkns: Vec<lex::LocatedTkn>,
    /// `tkns`の中で、型の名前のトークンがある位置
    /// (`struct Name`なら1、`variant "unsafe" Name`なら2)
    pub(super) name_idx: usize,
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
    /// 定義済みのジェネリクスな構造体/バリアント(型名 -> 定義)
    pub(super) generic_types: HashMap<String, GenericType>,
    /// 作成済みの(型名, `<>`の中身)。同じ型引数での使用が
    /// 何度あっても、その型の構造体/バリアントは1つだけ作るために使う
    pub(super) generated_types: Vec<(String, Vec<node::TyNode>)>,
    /// 使われた型引数ごとに作った構造体/バリアントのノード。
    /// `pending_funcs`と同じ理由で、解析がすべて終わってから
    /// `gen_nodes`の後ろに追加する
    pub(super) pending_types: Vec<node::Group1Node>,
    /// `impl Name { .. }`が、`Name`の定義より先に書かれたときに、
    /// 解析済みのメゾットを予約しておく場所(型名 -> メゾット)。
    /// `Name`の`struct`/`variant`の定義が終わったときに、その型へ追加する
    pub(super) reserved_impls: HashMap<String, Vec<node::Group1Node>>,
    pub(super) file_name: Rc<String>,
    /// エラーログをためる
    pub(super) err_pool: Rc<RefCell<err_pool::ErrPool>>,
}

impl Parser {
    pub fn new(file_name_ref: &Rc<String>, err_pool: Rc<RefCell<err_pool::ErrPool>>) -> Self {
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
            generic_types: HashMap::new(),
            generated_types: Vec::new(),
            pending_types: Vec::new(),
            reserved_impls: HashMap::new(),
            file_name: Rc::clone(&file_name_ref),
            err_pool,
        }
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        let file_name = Rc::new("<test>".to_string());
        Self::new(&file_name, Rc::new(RefCell::new(err_pool::ErrPool::new())))
    }

    #[cfg(test)]
    pub fn parser_for_test(
        &mut self,
        tkns: &[lex::LocatedTkn],
    ) -> Result<&[node::Group1Node], err::ErrKind> {
        self.parser(tkns, &Vec::<String>::new())
    }

    // 関数の中身などを作成する
    // P は、この関数が公開されるかどうかのbool
    fn build_func<const P: bool>(&mut self, func_name: &str) -> Result<(), err::ErrKind> {
        // ジェネリクス関数(`name<T>(..)`)の定義は、呼び出されるまで
        // ノードを作らない。定義は`collect_generic_funcs`で集め済みなので、
        // ここでは本体を閉じる`}`まで読み飛ばすだけ
        if matches!(self.next_tkn_ref(&["(", "<"])?, lex::Tkn::LAngleBracket) {
            return self.skip_generic_func_def();
        }

        // トップレベルの関数定義なので、`Self`が解決される
        // 構造体/列挙型は存在しない
        let node = self.func_node(func_name.to_owned(), P)?;
        self.gen_nodes.push(node);

        self.gen_flag = GenFlag::Group2;
        self.scope_counter += 1;
        Ok(())
    }

    /// c言語のノードも作成
    pub fn parser(
        &mut self,
        tkns: &[lex::LocatedTkn],
        // #include "file.c"と文字列のみで指定されたものを処理
        include_c_list: &[String],
    ) -> Result<&[node::Group1Node], err::ErrKind> {
        use std::{
            fs,
            path::{Path, PathBuf},
        };

        self.tkns = Some(tkns.to_owned());

        for f in include_c_list {
            let content = fs::read_to_string(f).unwrap();
            let n = c_frontend::emit_c_node(&content);
            self.gen_nodes.extend(n);
        }
        // 呼び出しが定義より前に書かれていても、その型の関数を
        // 作れるように、先にジェネリクス関数の定義を集めておく
        // 構造体/バリアントのジェネリクスも同様。関数の定義を集める際に、
        // 引数や戻り値の型`Pair<int>`を関数の定義と間違えないよう、
        // 型の定義を先に集める
        self.collect_generic_types()?;
        self.collect_generic_funcs()?;
        self.parse_loop::<stmt::ParseLoopPushPool>()?;

        // `impl Name { .. }`の`Name`が、最後まで定義されなかった
        if let Some(name) = self.reserved_impls.keys().min().cloned() {
            return crate::err_at!(self.expected_tkn(
                "struct or variant definition for `impl`",
                lex::Tkn::Name(name)
            ));
        }

        // 使われた型引数ごとに作った構造体/バリアント、呼び出された型ごとに
        // 作った関数を、通常のノードの後ろに追加する
        self.gen_nodes.append(&mut self.pending_types);
        self.gen_nodes.append(&mut self.pending_funcs);
        Ok(&self.gen_nodes)
    }

    /// `self.tkns`の先頭から順に、関数などのノードを`gen_nodes`へ作る。
    /// ジェネリクス関数の呼び出しで、型ごとの関数を作る際にも
    /// (トークン列を差し替えて)この関数が使われる
    pub(super) fn parse_loop<C: ParseLoopControl>(&mut self) -> Result<(), err::ErrKind> {
        loop {
            match &self.gen_flag {
                GenFlag::Group1 => {
                    match self.current_tkn().clone() {
                        lex::Tkn::KeyWordPub => {
                            let binding = self.pub_keyword_node();
                            C::handle(&self.err_pool, binding)?;
                        }
                        lex::Tkn::Name(name) => {
                            let binding = self.build_func::<false>(&name);
                            C::handle(&self.err_pool, binding)?;
                        }
                        lex::Tkn::CompleSyn => {
                            let syntax_result = self.comple_syntax();
                            let node =
                                C::handle_ty::<node::Group2Node>(&self.err_pool, syntax_result)?;
                            let result = node.change_group1();
                            self.gen_nodes
                                .push(C::handle_ty::<node::Group1Node>(&self.err_pool, result)?);
                        }
                        lex::Tkn::KeyWordStruct => {
                            // ジェネリクス(`struct Name<T> {..}`)は、使われるまで
                            // ノードを作らないので、読み飛ばす
                            let binding = self.skip_generic_type_def();
                            if !C::handle_ty::<bool>(&self.err_pool, binding)? {
                                let struct_node = self.struct_node();
                                let node =
                                    C::handle_ty::<node::Group1Node>(&self.err_pool, struct_node)?;
                                self.gen_nodes.push(node);
                            }
                        }
                        lex::Tkn::KeyWordVariant => {
                            let binding = self.skip_generic_type_def();
                            if !C::handle_ty::<bool>(&self.err_pool, binding)? {
                                let binding = self.variant_node();
                                let node =
                                    C::handle_ty::<node::Group1Node>(&self.err_pool, binding)?;
                                self.gen_nodes.push(node);
                            }
                        }
                        // `impl Name { func() {..} }`
                        // (終了時は`impl`を閉じる`}`を指す。`struct`と同じ)
                        lex::Tkn::KeyWordImpl => {
                            let impl_node = self.impl_node();
                            C::handle(&self.err_pool, impl_node)?;
                        }
                        t => {
                            return crate::err_at!(self.unexpect_tkn_in_stmt(t));
                        }
                    };
                }
                GenFlag::Group2 => {
                    // 関数の中身が空(`{}`)の場合、`one_line_node`を呼ばずに
                    // ここでスコープを閉じる
                    if matches!(self.current_tkn(), lex::Tkn::RBrace) {
                        self.scope_counter -= 1;
                        if self.next_tkn(&[]).is_err() {
                            return Ok(());
                        }

                        self.gen_flag = GenFlag::Group1;
                        continue;
                    }

                    // 文の先頭の位置
                    let span = self.build_err_span();
                    let line_node = self.one_line_node();
                    let node: Group2Info =
                        C::handle_ty::<node::Group2Node>(&self.err_pool, line_node)?
                            .gen_group_info(self.build_err_span().line, span);

                    // `Group2`のノードは常に直前に生成された関数定義に
                    // 追加されるはずなので、そうでない場合はパーサー自体の
                    // バグ(利用者の入力に起因するエラーではない)
                    let Some(node::Group1Node::FuncDefine(func)) = self.gen_nodes.last_mut() else {
                        return crate::err_at!(
                            self.internal_state_err("function definition to add the node to")
                        );
                    };
                    func.add(node);

                    if matches!(self.current_tkn(), lex::Tkn::RBrace) {
                        self.scope_counter -= 1;
                        if self.next_tkn(&[]).is_err() {
                            return Ok(());
                        }

                        self.gen_flag = GenFlag::Group1;
                    }
                    continue;
                }
            }
            if self.next_tkn(&[]).is_err() && self.scope_counter == 0 {
                return Ok(());
            }
        }
        //Ok(())
    }

    pub(super) fn one_line_node(&mut self) -> Result<node::Group2Node, err::ErrKind> {
        let node = match self.current_tkn().clone() {
            lex::Tkn::CompleSyn => self.comple_syntax()?,
            lex::Tkn::Name(name) => {
                // `M(val) := a`は`cond`の条件式の中でしか使えない
                if self.is_variant_bind_at(self.idx) {
                    return crate::err_at!(self.variant_bind_outside_cond());
                }
                node::Group2Node::Expr(self.build_scope_node(&name)?)
            }
            // ポインタ/配列にアクセスするノードの作成
            lex::Tkn::LBracket => {
                let tkn = self.next_tkn(&["name"])?;
                if let lex::Tkn::Name(name) = tkn.clone() {
                    match self.peek_tkn() {
                        // `name`の次が添字の式(数字/変数/`a.a`など)の場合、
                        // 配列への代入
                        // `[name index] = value` / `[name index else 0] = value`
                        // (`]`の次が`.`の場合は、添字付きポインタのメンバーへのアクセス
                        // `[name index else 0].member = value`)
                        Ok(ref next) if Self::is_arr_index_start(next) => {
                            // expr/arr_access.rsで定義
                            self.make_array_assign_node(&name)?
                        }
                        // それ以外の場合、ポインタへの代入、または
                        // ポインタが指す構造体のメンバー/メゾットへのアクセス
                        // `[name] = value` / `[name].member = value` / `[name].method(..)`
                        _ => self.ptr_stmt_node(&name)?,
                    }
                } else {
                    return crate::err_at!(self.stmt_name_not_found_after_lbracket(tkn));
                }
            }
            lex::Tkn::KeyWordRet => node::StmtNode::Return(self.expr_branch()?).wrap(),
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
                return crate::err_at!(self.unexpect_tkn_in_stmt(lex::Tkn::RBrace));
            }
            t => {
                return crate::err_at!(self.unexpect_tkn_in_stmt(t));
            }
        };
        Ok(node)
    }

    fn loop_control_node(&mut self, is_continue: bool) -> Result<node::Group2Node, err::ErrKind> {
        if self.loop_depth == 0 {
            return crate::err_at!(self.loop_control_outside_loop());
        }
        self.next_tkn(&[])?;
        let stmt = if is_continue {
            node::StmtNode::Continue
        } else {
            node::StmtNode::Break(self.build_err_span())
        };
        Ok(stmt.wrap())
    }

    fn pub_keyword_node(&mut self) -> Result<(), err::ErrKind> {
        match self.next_tkn(&["name", ".."])? {
            lex::Tkn::Name(name) => self.build_func::<true>(&name),
            unexpect_tkn => {
                // 期待したトークンじゃないので、エラー
                crate::err_at!(self.unexpect_tkn_after_pub(unexpect_tkn))
            }
        }
    }

    /// 反復処理のノードを作成する関数
    fn make_loop_node(&mut self) -> Result<node::Group2Node, err::ErrKind> {
        // `loop`の位置
        let span = self.build_err_span();
        // 反復処理の条件式
        let pattern = match self.next_tkn_ref(&["{", ".."])? {
            // "{"の場合は条件無し
            lex::Tkn::LBrace => None,
            // 条件式あり
            _ => {
                // `M(val) := a`は`cond`の条件式の中でしか使えない
                if self.is_variant_bind_ahead() {
                    self.next_tkn(&[])?;
                    return crate::err_at!(self.variant_bind_outside_cond());
                }
                // 条件に構造体の初期化を使うことはできない
                Some(Box::new(self.expr_or(false)?))
            }
        };
        // "{"をスキップ
        if !matches!(self.current_tkn(), lex::Tkn::LBrace) {
            return crate::err_at!(self.loop_lbrace_not_found());
        }
        self.next_tkn(&["{"])?;

        self.loop_depth += 1;
        let body_result = self.gen_block_node();
        self.loop_depth -= 1;
        let body = body_result?;
        self.next_tkn(&["expr"])?;

        let node = node::Group2Node::Expr(node::Expr::new(
            node::ExprNode::Loop { pattern, body },
            span,
        ));
        Ok(node)
    }

    /// 同じスコープ内のノードを生成
    pub(super) fn gen_block_node(&mut self) -> Result<Body, err::ErrKind> {
        let mut block = Body::new();

        // ブロックが空(`{}`)の場合、`one_line_node`を呼ばずに
        // そのまま空のブロックを返す
        if matches!(self.current_tkn(), lex::Tkn::RBrace) {
            return Ok(block);
        }

        loop {
            // 文の先頭の位置
            let span = self.build_err_span();
            let node = self.one_line_node()?;
            block.push(node.gen_group_info(self.build_err_span().line, span));
            if matches!(self.current_tkn(), lex::Tkn::RBrace) {
                break;
            }
        }
        Ok(block)
    }

    pub(super) fn comple_syntax(&mut self) -> Result<node::Group2Node, err::ErrKind> {
        let tkn = self.next_tkn(&["name"])?;
        let lex::Tkn::Name(name) = tkn.clone() else {
            return crate::err_at!(self.preproc_name_not_found(tkn));
        };
        self.make_preproc(&name)
    }

    /// ポインタが指す構造体のメンバー/メゾットへアクセスする文のノードを作成する
    /// - `[name].member = value` / `[name].method(..)`
    /// - `[name index else else_idx].member = value` (`index`/`else_idx`は配列と同じ)
    ///
    /// ## 呼び出し時の前提
    /// `current_tkn()`が`]`を指し、その次が`.`である必要がある
    pub(in crate::compiler::parse) fn ptr_member_stmt_node(
        &mut self,
        name: &str,
        index: Option<Box<node::Expr>>,
        else_idx: Option<Box<node::Expr>>,
    ) -> Result<node::Group2Node, err::ErrKind> {
        self.next_tkn(&["."])?;
        let member_node = self.ptr_member_node(name, index, else_idx)?;

        // メゾット呼び出しの場合、`ptr_member_node`の時点ですでに
        // 呼び出し式の次のトークンまで進んでいるので、そのまま式として返す
        if matches!(&member_node.node, node::ExprNode::PtrMember { target, .. } if matches!(target.node, node::ExprNode::CallFunc(..)))
        {
            return Ok(member_node.wrap_group2());
        }

        // フィールドアクセスの場合、続く`=`の有無で代入か参照かを判断する
        // `[name].member = value`
        if matches!(self.peek_tkn(), Ok(lex::Tkn::Equal)) {
            self.next_tkn(&["="])?;
            let value = self.expr_branch()?;
            // 代入の位置は、代入先(メンバー)の位置
            let span = member_node.span.clone();
            Ok(node::AssignVar::new(name, member_node, value, span).wrap_group2())
        } else {
            Ok(member_node.wrap_group2())
        }
    }

    /// `[name]`から始まる文のノードを作成する
    ///
    /// ## 対応する構文
    /// (添字付き`[name index else 0].member`は`make_array_assign_node`が処理する)
    /// - `[name] = value`             : ポインタが指す値へ代入
    /// - `[name].member = value`      : ポインタが指す構造体のメンバーへ代入
    /// - `[name].method(..)`          : ポインタが指す構造体のメゾットを呼び出す
    ///
    /// ## 呼び出し時の前提
    /// 呼び出し元(`one_line_node`)で`lex::Tkn::LBracket`の次の
    /// `lex::Tkn::Name(name)`まで読み進めた状態で呼び出す。
    /// つまり`current_tkn()`が`name`を指している必要がある。
    fn ptr_stmt_node(&mut self, name: &String) -> Result<node::Group2Node, err::ErrKind> {
        // `[name]`の`name`の位置
        let span = self.build_err_span();
        // `]`まで進める
        self.next_tkn(&["]"])?;

        // `]`の次が`.`の場合、ポインタが指す構造体のメンバー/メゾットへアクセスする
        if matches!(self.peek_tkn(), Ok(lex::Tkn::Dot)) {
            return self.ptr_member_stmt_node(name, None, None);
        }

        // 既存通り: ポインタが指す値そのものへの代入 `[name] = value`
        self.next_tkn(&["="])?;
        let value = self.expr_branch()?;
        let mk = |n: node::ExprNode| node::Expr::new(n, Rc::clone(&span));
        let var = mk(node::ExprNode::Var(name.to_string()));
        let addr = mk(node::ExprNode::GetAddress(Box::new(var)));
        let dst = mk(node::ExprNode::ConnectAddr(Box::new(addr)));
        Ok(node::AssignVar::new(name, dst, value, span).wrap_group2())
    }
}
