use super::*;

impl Parser {
    /// 変数のアドレス取得などのノードを作成
    /// 呼び出し元では、lex::Tkn::LBracket
    ///
    /// `[name`の次が添字の式(数字/変数/`a.a`のようなメンバーなど)の
    /// 場合は、アドレスの取得ではなく配列の要素へのアクセスになる
    /// - `[arr 0]`
    /// - `[arr i else 0]`
    /// - `[arr a.a else 0]`
    pub(in crate::compiler::hexl_frontend::parse::expr) fn get_var_addr_node(
        &mut self,
    ) -> Result<Expr, err::ErrKind> {
        // `[`の位置
        let span = self.build_err_span();
        let next = self.next_tkn_ref(&["name"])?;
        // `[`の位置から見て、`[name->member index]`の形か(`[`から2つ先が`->`、
        // 4つ先が添字の先頭)
        let is_ptr_member_arr = matches!(self.tkn_at(2), Some(lex::Tkn::AccessPtr))
            && self
                .tkn_at(4)
                .map_or(false, |t| Self::is_arr_index_start(&t));
        let name = match next.clone() {
            lex::Tkn::Name(name) => name,
            // `[self->member index]`の`self`は、構造体/バリアント自身の名前
            lex::Tkn::KeyWordSelf if is_ptr_member_arr => {
                let Some(self_name) = self.struct_self_name.clone() else {
                    return crate::err_at!(self.self_outside_of_type());
                };
                self_name
            }
            _ => return crate::err_at!(self.expected_tkn("name", next)),
        };
        // `[name->member index else else_idx]`: ポインタが指す構造体のメンバー(配列)の要素
        if is_ptr_member_arr {
            // `[`から`name`まで進める
            self.next_tkn(&["name"])?;
            return self.ptr_member_array_expr(&name);
        }
        // nameの次に、添字の式が来た場合、それは配列にアクセスする
        // (`[`の次が`name`、その次が添字の先頭)
        if self
            .peek2_tkn()
            .map_or(false, |t| Self::is_arr_index_start(&t))
        {
            return self.ref_array_expr(name);
        }

        let result = self.expr_add(true)?;
        let node = match self.current_tkn().clone() {
            lex::Tkn::LBracket => {
                dbg!(self.current_tkn());
                return crate::err_at!(self.arr_access_unexpected_lbracket());
            }
            // `]`の次が`.`の場合、アドレスの取得ではなく、ポインタが
            // 指す構造体のメンバー/メゾットへのアクセス
            // `[name].member` / `[name].method(..)`
            _ if matches!(self.peek_tkn(), Ok(lex::Tkn::Dot)) => {
                self.next_tkn(&["."])?;
                self.ptr_member_node(&name, None, None)?
            }
            _ => Expr::new(ExprNode::GetAddress(Box::new(result)), span),
        };
        Ok(node)
    }

    /// 現在位置から`n`個先のトークンを、位置を進めずに返す(`n == 0`は現在のトークン)
    #[inline(always)]
    fn tkn_at(&self, n: usize) -> Option<lex::Tkn> {
        self.tkns.as_ref()?.get(self.idx + n).map(|v| v.tkn.clone())
    }

    /// ポインタが指す構造体のメンバー(配列)の要素にアクセスするノードを作成する
    /// `[name->member index]` / `[name->member index else else_idx]`
    ///
    /// `[name].member`と同じ`PtrMember`のノードで、`target`が配列の要素
    /// (`RefArray`)になる(`name.[member index]`の`Member`と同じ作り)
    ///
    /// ## 呼び出し時の前提
    /// `current_tkn()`が`name`(`self`は構造体自身の名前に置き換えて渡す)を
    /// 指し、その次が`->`、その次が`member`、その次が添字の先頭である必要がある。
    /// 終了時、`current_tkn()`は`]`を指す。
    pub(in crate::compiler::hexl_frontend::parse) fn ptr_member_array_expr(
        &mut self,
        name: &str,
    ) -> Result<Expr, err::ErrKind> {
        // `name`の位置(`PtrMember`全体の位置)
        let span = self.build_err_span();
        self.next_tkn(&["->"])?;
        let member_tkn = self.next_tkn(&["name"])?;
        let lex::Tkn::Name(member) = member_tkn.clone() else {
            return crate::err_at!(self.ptr_member_name_not_found(member_tkn));
        };
        // `member`の位置
        let member_span = self.build_err_span();
        let index = self.arr_access_index()?;
        let else_idx = self.arr_access_else_idx()?.map(|v| Box::new(v));

        let arr = Expr::new(
            ExprNode::RefArray {
                name: member.clone(),
                dst: Box::new(Expr::new(
                    ExprNode::Var(member),
                    Rc::clone(&member_span),
                )),
                index: Box::new(index),
                else_idx,
            },
            member_span,
        );
        Ok(Expr::new(
            ExprNode::PtrMember {
                name: name.to_string(),
                index: None,
                else_idx: None,
                target: Box::new(arr),
            },
            span,
        ))
    }

    /// ポインタが指す構造体のメンバー(配列)に値を代入するノードを作成する
    /// `[name->member index] = value` / `[name->member index else else_idx] = value`
    ///
    /// ## 呼び出し時の前提
    /// `ptr_member_array_expr`と同じ(`current_tkn()`が`name`を指している)
    pub(in crate::compiler::hexl_frontend::parse) fn make_ptr_member_array_assign_node(
        &mut self,
        name: &str,
    ) -> Result<Group2Node, err::ErrKind> {
        let span = self.build_err_span();
        let dst = self.ptr_member_array_expr(name)?;
        // `]`の次の`=`まで進める
        let eq_tkn = self.next_tkn(&["="])?;
        if !matches!(eq_tkn, lex::Tkn::Equal) {
            return crate::err_at!(self.arr_assign_equal_not_found(eq_tkn));
        }
        let value = self.expr_branch()?;
        Ok(AssignVar::new(name, dst, value, span).wrap_group2())
    }

    /// `[name`の次のトークンが、配列の添字の式の先頭になれるかを判定する
    ///
    /// `[ptr]`(ポインタ参照)や`[ptr + 1]`などと区別するために使う。
    /// `(`/`*`/`[`は、関数呼び出し(`f(..)`)、乗算(`p * 2`)、ポインタ参照と
    /// 区別できないため、添字の先頭には使えない。
    /// そのような式を添字にしたい場合は、先に変数へ代入してから使う
    #[inline(always)]
    pub(in crate::compiler::hexl_frontend::parse) fn is_arr_index_start(tkn: &lex::Tkn) -> bool {
        matches!(
            tkn,
            lex::Tkn::Number(_) | lex::Tkn::Name(_) | lex::Tkn::KeyWordSelf
        )
    }

    /// 配列の要素にアクセスするノードを作成する(値の参照)
    /// `[name index]` / `[name index else else_idx]`
    ///
    /// ## 呼び出し時の前提
    /// `current_tkn()`が`[`を指している必要がある。
    /// 終了時、`current_tkn()`は`]`を指す。
    fn ref_array_expr(&mut self, name: String) -> Result<Expr, err::ErrKind> {
        // `[`の位置
        let span = self.build_err_span();
        // `[`から配列の名前まで進める
        self.next_tkn(&["name"])?;
        let index = self.arr_access_index()?;
        let else_idx = self.arr_access_else_idx()?.map(|v| Box::new(v));

        // `]`の次が`.`の場合、配列の要素ではなく、添字付きポインタが指す
        // 構造体のメンバー/メゾットへのアクセス
        // `[ptr index else else_idx].member`
        if matches!(self.peek_tkn(), Ok(lex::Tkn::Dot)) {
            self.next_tkn(&["."])?;
            return self.ptr_member_node(&name, Some(Box::new(index)), else_idx);
        }

        let dst = Expr::new(ExprNode::Var(name.clone()), Rc::clone(&span));
        Ok(Expr::new(
            ExprNode::RefArray {
                dst: Box::new(dst),
                name,
                index: Box::new(index),
                else_idx,
            },
            span,
        ))
    }

    /// 構造体の配列メンバーの要素にアクセス、または代入するノードを作成する
    /// - 読み取り: `name.[member index]`
    /// - 代入:     `name.[member index] = value`
    ///
    /// ## 呼び出し時の前提
    /// 呼び出し元(`build_member_node`)で、`.`と`[`を読み飛ばした
    /// 状態で呼び出す。つまり`current_tkn()`が`[`を指している必要がある。
    ///
    /// ## 添字について
    /// 構造体の配列メンバーの添字は、`[arr 0]`のように数字のみ使える
    /// (メンバーの位置はコンパイル時に決まるため)
    ///
    /// ## Errors
    /// `member`の次のトークンが名前(`lex::Tkn::Name`)、または
    /// その次が数字(`lex::Tkn::Number`)ではない場合エラー
    pub(in crate::compiler::hexl_frontend::parse) fn build_member_array_node(
        &mut self,
        name: &str,
    ) -> Result<Expr, err::ErrKind> {
        // `[`の位置(メンバーアクセス全体の位置として使う)
        let span = self.build_err_span();
        let mk = |n: ExprNode| Expr::new(n, Rc::clone(&span));
        let member_tkn = self.next_tkn(&["name"])?;
        let lex::Tkn::Name(member) = member_tkn.clone() else {
            return crate::err_at!(self.arr_member_name_not_found(member_tkn));
        };
        let index_tkn = self.next_tkn(&["number"])?;
        let lex::Tkn::Number(index) = index_tkn.clone() else {
            return crate::err_at!(self.arr_member_index_not_number(index_tkn));
        };
        // 添字の次(`else`か`]`)まで進める
        self.next_tkn(&["]", "else"])?;
        // "]"まで進める(current_tkn()は"]"を指す)
        let else_idx = self.arr_access_else_idx()?.map(|v| Box::new(v));

        let member_node = mk(ExprNode::Member {
            scope: vec![name.to_string()],
            target: Box::new(mk(ExprNode::RefArray {
                name: member.clone(),
                dst: Box::new(mk(ExprNode::Var(member))),
                index: Box::new(mk(ExprNode::Number(index))),
                else_idx,
            })),
        });

        // `a.[c 0] = 10`のように代入の場合、"="の後に続く値を
        // 読み取り、代入のノードとして返す
        if self.next_tkn_ref(&["="])? == lex::Tkn::Equal {
            self.next_tkn(&["="])?;
            let value = self.expr_branch()?;
            return Ok(AssignVar::new(name, member_node, value, span));
        }

        // 代入ではなく値の参照なので、"]"を消費せずに返す
        // (呼び出し元の`expr_value`が続けて読み進める)
        Ok(member_node)
    }

    /// 配列に値を代入するノードを作成する
    ///
    /// ## 呼び出し時の前提
    /// 呼び出し元(`one_line_node`)で`lex::Tkn::LBracket`の次の
    /// `lex::Tkn::Name(name)`まで読み進めた状態で呼び出す。
    /// つまり`current_tkn()`が`name`を指している必要がある。
    ///
    /// ## 文法のルール
    /// `[name index] = value` / `[name index else else_idx] = value`
    /// ポインタの場合は`[name index else else_idx].member = value`も使える
    /// - `[arr 0] = 10`
    /// - `[arr i else 0] = 10`
    /// - `[arr a.a else 0] = 10`
    ///
    /// `index`は式(数字/変数/メンバー/四則演算)。`const`や即値ではない
    /// 添字には`else`が必要になる(`src/ir/checker/access_mem.rs`で検査する)
    ///
    /// ## Errors
    /// 添字の式の次が`else`/`]`ではない場合、`]`の次が`=`ではない場合エラー
    pub(in crate::compiler::hexl_frontend::parse) fn make_array_assign_node(
        &mut self,
        name: &str,
    ) -> Result<Group2Node, err::ErrKind> {
        // 配列の名前の位置
        let span = self.build_err_span();
        let index = self.arr_access_index()?;
        let else_idx = self.arr_access_else_idx()?.map(|v| Box::new(v));

        // `]`の次が`.`の場合、配列ではなく、添字付きポインタが指す
        // 構造体のメンバー/メゾットへのアクセス(`stmt.rs`で定義)
        // `[ptr index else else_idx].member = value` / `[ptr index].method(..)`
        if matches!(self.peek_tkn(), Ok(lex::Tkn::Dot)) {
            return self.ptr_member_stmt_node(name, Some(Box::new(index)), else_idx);
        }

        // `]`の次の`=`まで進める
        let eq_tkn = self.next_tkn(&["="])?;
        if !matches!(eq_tkn, lex::Tkn::Equal) {
            return crate::err_at!(self.arr_assign_equal_not_found(eq_tkn));
        }
        let value = self.expr_branch()?;

        let dst = Expr::new(ExprNode::Var(name.to_string()), Rc::clone(&span));
        let arr = Expr::new(
            ExprNode::RefArray {
                name: name.to_string(),
                dst: Box::new(dst),
                index: Box::new(index),
                else_idx,
            },
            Rc::clone(&span),
        );
        Ok(AssignVar::new(name, arr, value, span).wrap_group2())
    }

    /// `[arr index ..]`の添字(`index`)の式を読み取る
    ///
    /// ## 呼び出し時の前提
    /// `current_tkn()`が配列の名前を指している必要がある
    /// (式は、その次のトークンから始まる)。
    /// 終了時、`current_tkn()`は式の次のトークン(`else`か`]`)を指す。
    #[inline(always)]
    fn arr_access_index(&mut self) -> Result<Expr, err::ErrKind> {
        // `a.a`のような構造体のメンバーも添字にできるように、
        // `true`(メンバーアクセス/構造体の初期化を許可)で読み取る
        self.expr_add(true)
    }

    /// [arr index else ..]の`else`以降の構文を処理する。
    ///
    /// ## 呼び出し時の前提
    /// `current_tkn()`が添字の式の次(`else`か`]`)を指している必要がある。
    /// 終了時、`current_tkn()`は`]`を指す。
    ///
    /// ## Errors
    /// 添字の式の次が`else`/`]`ではない場合、`else`の式の次が`]`ではない場合
    #[inline(always)]
    fn arr_access_else_idx(&mut self) -> Result<Option<Expr>, err::ErrKind> {
        let else_idx = match self.current_tkn().clone() {
            lex::Tkn::KeyWordElse => self.expr_add(false)?,
            lex::Tkn::RBracket => return Ok(None),
            found => {
                return crate::err_at!(self.arr_else_or_rbracket_not_found(found));
            }
        };
        // "]"まで進める(current_tkn()は"]"を指す)
        if !matches!(self.current_tkn(), lex::Tkn::RBracket) {
            return crate::err_at!(self.arr_rbracket_not_found());
        }
        Ok(Some(else_idx))
    }
}

#[cfg(test)]
mod test_access_arr_node {
    use super::*;
    use crate::compiler::hexl_frontend::parse;

    /// テスト用の位置。`Expr`の`PartialEq`は`span`を比較しないので、何でもよい
    fn sp() -> std::rc::Rc<err::Span> {
        std::rc::Rc::new(err::Span::new(0, 0, String::new(), &std::rc::Rc::new("<test>".to_string())))
    }

    /// 位置を付けて`Expr`にする
    fn ex(n: ExprNode) -> Expr {
        Expr::new(n, sp())
    }

    fn gen_nodes(content: &str) -> Vec<lex::LocatedTkn> {
        let mut lexer = lex::Lexer::new("<test>");
        lexer.analy(&content.to_string()).unwrap();
        lexer.gen_tkns.clone()
    }

    /// `main(): int { <stmt> }`の、1行目のノードを返す
    fn first_stmt(stmt: &str) -> Group2Node {
        let mut p = parse::Parser::new_for_test();
        let tkns = gen_nodes(&format!("main(): int {{ {} }}", stmt));
        let Group1Node::FuncDefine(ref node) = p.parser_for_test(&tkns).expect("node is err")[0]
        else {
            panic!("not func");
        };
        node.body[0].get_node().clone()
    }

    fn num(n: &str) -> Expr {
        ex(ExprNode::Number(n.to_string()))
    }

    fn var(name: &str) -> Expr {
        ex(ExprNode::Var(name.to_string()))
    }

    /// `[name index else else_idx]`のノード
    fn ref_arr(name: &str, index: Expr, else_idx: Option<Expr>) -> Expr {
        ex(ExprNode::RefArray {
            name: name.to_string(),
            dst: Box::new(var(name)),
            index: Box::new(index),
            else_idx: else_idx.map(Box::new),
        })
    }

    #[test]
    fn check_arr_node() {
        assert_eq!(
            first_stmt("[arr 0 else 10] = 10"),
            AssignVar::new(
                "arr",
                ref_arr("arr", num("0"), Some(num("10"))),
                num("10"),
                sp()
            )
            .wrap_group2()
        );
    }

    #[test]
    fn check_arr_node_without_else() {
        assert_eq!(
            first_stmt("[arr 0] = 10"),
            AssignVar::new("arr", ref_arr("arr", num("0"), None), num("10"), sp()).wrap_group2()
        );
    }

    /// `[arr a.a else 0] = 0`: 添字が構造体のメンバー
    #[test]
    fn check_arr_node_member_index() {
        let index = ex(ExprNode::Member {
            scope: vec!["a".to_string()],
            target: Box::new(var("a")),
        });
        assert_eq!(
            first_stmt("[arr a.a else 0] = 0"),
            AssignVar::new("arr", ref_arr("arr", index, Some(num("0"))), num("0"), sp())
                .wrap_group2()
        );
    }

    #[test]
    fn check_arr_node_var_index() {
        assert_eq!(
            first_stmt("[arr i else 0] = 1"),
            AssignVar::new(
                "arr",
                ref_arr("arr", var("i"), Some(num("0"))),
                num("1"),
                sp()
            )
            .wrap_group2()
        );
    }

    #[test]
    fn check_arr_node_calc_index() {
        let index = ex(ExprNode::Add((Box::new(var("i")), Box::new(num("1")))));
        assert_eq!(
            first_stmt("[arr i + 1 else 0] = 1"),
            AssignVar::new("arr", ref_arr("arr", index, Some(num("0"))), num("1"), sp())
                .wrap_group2()
        );
    }

    /// 代入ではなく、式として値を読み取る
    #[test]
    fn check_arr_node_as_value() {
        let index = ex(ExprNode::Member {
            scope: vec!["a".to_string()],
            target: Box::new(var("a")),
        });
        assert_eq!(
            first_stmt("x: int = [arr a.a else 0]"),
            ex(ExprNode::DefVar(DefineVar {
                name: "x".to_string(),
                value: Box::new(ref_arr("arr", index, Some(num("0")))),
                ty: TyNode::Ty("int".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    /// 添字がない場合や、`[ptr]`は今まで通りポインタの参照になる
    #[test]
    fn check_ptr_is_not_arr_node() {
        assert_eq!(
            first_stmt("a: int* = [b]"),
            ex(ExprNode::DefVar(DefineVar {
                name: "a".to_string(),
                value: Box::new(ex(ExprNode::GetAddress(Box::new(var("b"))))),
                ty: TyNode::Pointer {
                    is_const: false,
                    ty_name: Box::new(TyNode::Ty("int".to_string())),
                    range: None,
                },
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    /// `[ptr index else else_idx].member`のノード
    fn ptr_member(index: Expr, else_idx: Option<Expr>, target: Expr) -> Expr {
        ex(ExprNode::PtrMember {
            name: "ptr".to_string(),
            index: Some(Box::new(index)),
            else_idx: else_idx.map(Box::new),
            target: Box::new(target),
        })
    }

    /// `[ptr 1 else 0].name = 10`: 添字付きポインタのメンバーへ代入
    #[test]
    fn check_ptr_member_index_assign() {
        assert_eq!(
            first_stmt("[ptr 1 else 0].name = 10"),
            AssignVar::new(
                "ptr",
                ptr_member(num("1"), Some(num("0")), var("name")),
                num("10"),
                sp()
            )
            .wrap_group2()
        );
    }

    /// `x: int = [ptr i else 0].name`: 添字付きポインタのメンバーを読み取る
    #[test]
    fn check_ptr_member_index_value() {
        assert_eq!(
            first_stmt("x: int = [ptr i else 0].name"),
            ex(ExprNode::DefVar(DefineVar {
                name: "x".to_string(),
                value: Box::new(ptr_member(var("i"), Some(num("0")), var("name"))),
                ty: TyNode::Ty("int".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    /// `[ptr 1].name = 10`: `else`なしでも書ける(`const`の添字)
    #[test]
    fn check_ptr_member_index_without_else() {
        assert_eq!(
            first_stmt("[ptr 1].name = 10"),
            AssignVar::new(
                "ptr",
                ptr_member(num("1"), None, var("name")),
                num("10"),
                sp()
            )
            .wrap_group2()
        );
    }

    /// `[ptr 1 else 0].f()`: 添字付きポインタのメゾット呼び出し
    #[test]
    fn check_ptr_member_index_call() {
        let call = ex(ExprNode::CallFunc(CallInfo {
            name: "f".to_string(),
            temp_ty: Vec::new(),
            args: Vec::new(),
        }));
        assert_eq!(
            first_stmt("[ptr 1 else 0].f()"),
            ptr_member(num("1"), Some(num("0")), call).wrap_group2()
        );
    }

    /// `[ptr->val i else 0]`のノード
    fn ptr_member_arr(index: Expr, else_idx: Option<Expr>) -> Expr {
        ptr_member_target(ref_arr("val", index, else_idx))
    }

    fn ptr_member_target(target: Expr) -> Expr {
        ex(ExprNode::PtrMember {
            name: "ptr".to_string(),
            index: None,
            else_idx: None,
            target: Box::new(target),
        })
    }

    /// `[ptr->val i else 0] = 10`: ポインタが指す構造体の配列メンバーへ代入
    #[test]
    fn check_ptr_arrow_member_arr_assign() {
        assert_eq!(
            first_stmt("[ptr->val i else 0] = 10"),
            AssignVar::new(
                "ptr",
                ptr_member_arr(var("i"), Some(num("0"))),
                num("10"),
                sp()
            )
            .wrap_group2()
        );
    }

    /// `[ptr->val l] = v`: `else`なしでも書ける
    #[test]
    fn check_ptr_arrow_member_arr_assign_without_else() {
        assert_eq!(
            first_stmt("[ptr->val l] = v"),
            AssignVar::new("ptr", ptr_member_arr(var("l"), None), var("v"), sp()).wrap_group2()
        );
    }

    /// `x: int = [ptr->val i else 0]`: 値の読み取り
    #[test]
    fn check_ptr_arrow_member_arr_value() {
        assert_eq!(
            first_stmt("x: int = [ptr->val i else 0]"),
            ex(ExprNode::DefVar(DefineVar {
                name: "x".to_string(),
                value: Box::new(ptr_member_arr(var("i"), Some(num("0")))),
                ty: TyNode::Ty("int".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            }))
            .wrap_group2()
        );
    }

    /// `name.[member index]`: 構造体の配列メンバー
    #[test]
    fn check_method_arr_node() {
        assert_eq!(
            first_stmt("me.[arr 0 else 10] = 10"),
            AssignVar::new(
                "me",
                ex(ExprNode::Member {
                    scope: vec!["me".to_string()],
                    target: Box::new(ref_arr("arr", num("0"), Some(num("10")))),
                }),
                num("10"),
                sp()
            )
            .wrap_group2()
        );
    }
}
