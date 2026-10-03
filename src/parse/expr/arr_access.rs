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
    pub(in crate::parse::expr) 
    fn get_var_addr_node(&mut self) -> Result<node::Expr, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn_ref(&["name"])? else {
            panic!()
        };
        // nameの次に、添字の式が来た場合、それは配列にアクセスする
        // (`[`の次が`name`、その次が添字の先頭)
        if self.peek2_tkn().map_or(false, |t| Self::is_arr_index_start(&t)) {
            return self.ref_array_expr(name);
        }

        let result = self.expr_add(true)?;
        let node = match self.current_tkn().clone() {
            lex::Tkn::LBracket => {
                dbg!(self.current_tkn());
                Err(err::ErrKind::UnexpectedToken)?
            }
            // `]`の次が`.`の場合、アドレスの取得ではなく、ポインタが
            // 指す構造体のメンバー/メゾットへのアクセス
            // `[name].member` / `[name].method(..)`
            _ if matches!(self.peek_tkn(), Ok(lex::Tkn::Dot)) => {
                self.next_tkn(&["."])?;
                self.ptr_member_node(&name)?
            }
            _ => node::Expr::GetAddress(Box::new(result)),
        };
        Ok(node)
    }

    /// `[name`の次のトークンが、配列の添字の式の先頭になれるかを判定する
    ///
    /// `[ptr]`(ポインタ参照)や`[ptr + 1]`などと区別するために使う。
    /// `(`/`*`/`[`は、関数呼び出し(`f(..)`)、乗算(`p * 2`)、ポインタ参照と
    /// 区別できないため、添字の先頭には使えない。
    /// そのような式を添字にしたい場合は、先に変数へ代入してから使う
    #[inline(always)]
    pub(in crate::parse)
    fn is_arr_index_start(tkn: &lex::Tkn) -> bool {
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
    fn ref_array_expr(&mut self, name: String) -> Result<node::Expr, err::ErrKind> {
        // `[`から配列の名前まで進める
        self.next_tkn(&["name"])?;
        let index = self.arr_access_index()?;
        let else_idx = self.arr_access_else_idx()?.map(|v| Box::new(v));

        Ok(node::Expr::RefArray {
            dst: Box::new(node::Expr::Var(name.clone())),
            name,
            index: Box::new(index),
            else_idx,
        })
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
    pub(in crate::parse)
    fn build_member_array_node(&mut self, name: &str) -> Result<node::Expr, err::ErrKind> {
        let member_tkn = self.next_tkn(&["name"])?;
        let lex::Tkn::Name(member) = member_tkn.clone() else {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: "name",
                    found: member_tkn,
                }
            );
        };
        let index_tkn = self.next_tkn(&["number"])?;
        let lex::Tkn::Number(index) = index_tkn.clone() else {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: "number",
                    found: index_tkn,
                }
            );
        };
        // 添字の次(`else`か`]`)まで進める
        self.next_tkn(&["]", "else"])?;
        // "]"まで進める(current_tkn()は"]"を指す)
        let else_idx = self.arr_access_else_idx()?.map(|v| Box::new(v));

        let member_node = node::Expr::Member {
            scope: vec![name.to_string()],
            target: Box::new(node::Expr::RefArray {
                name: member.clone(),
                dst: Box::new(node::Expr::Var(member)),
                index: Box::new(node::Expr::Number(index)),
                else_idx,
            }),
        };

        // `a.[c 0] = 10`のように代入の場合、"="の後に続く値を
        // 読み取り、代入のノードとして返す
        if self.next_tkn_ref(&["="])? == lex::Tkn::Equal {
            self.next_tkn(&["="])?;
            let value = self.expr_branch()?;
            return Ok(node::AssignVar::new(name, member_node, value));
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
    /// - `[arr 0] = 10`
    /// - `[arr i else 0] = 10`
    /// - `[arr a.a else 0] = 10`
    ///
    /// `index`は式(数字/変数/メンバー/四則演算)。`const`や即値ではない
    /// 添字には`else`が必要になる(`src/ir/checker/access_mem.rs`で検査する)
    ///
    /// ## Errors
    /// 添字の式の次が`else`/`]`ではない場合、`]`の次が`=`ではない場合エラー
    pub(in crate::parse)
    fn make_array_assign_node(&mut self, name: &str) -> Result<node::Expr, err::ErrKind> {
        let index = self.arr_access_index()?;
        let else_idx = self.arr_access_else_idx()?.map(|v| Box::new(v));

        // `]`の次の`=`まで進める
        let eq_tkn = self.next_tkn(&["="])?;
        if !matches!(eq_tkn, lex::Tkn::Equal) {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: "=",
                    found: eq_tkn,
                }
            );
        }
        let value = self.expr_branch()?;

        Ok(node::AssignVar::new(
            name,
            node::Expr::RefArray {
                name: name.to_string(),
                dst: Box::new(node::Expr::Var(name.to_string())),
                index: Box::new(index),
                else_idx,
            },
            value,
        ))
    }

    /// `[arr index ..]`の添字(`index`)の式を読み取る
    ///
    /// ## 呼び出し時の前提
    /// `current_tkn()`が配列の名前を指している必要がある
    /// (式は、その次のトークンから始まる)。
    /// 終了時、`current_tkn()`は式の次のトークン(`else`か`]`)を指す。
    #[inline(always)]
    fn arr_access_index(&mut self) -> Result<node::Expr, err::ErrKind> {
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
    fn arr_access_else_idx(&mut self) -> Result<Option<node::Expr>, err::ErrKind> {
        let else_idx = match self.current_tkn().clone() {
            lex::Tkn::KeyWordElse => self.expr_add(false)?,
            lex::Tkn::RBracket => return Ok(None),
            found => {
                return crate::syntax_err!(
                    self.build_err_span(),
                    err::SyntaxErrKind::ExpectedKind {
                        expected: "`else` or `]`",
                        found,
                    }
                );
            }
        };
        // "]"まで進める(current_tkn()は"]"を指す)
        if !matches!(self.current_tkn(), lex::Tkn::RBracket) {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: "]",
                    found: self.current_tkn().clone(),
                }
            );
        }
        Ok(Some(else_idx))
    }
}

#[cfg(test)]
mod test_access_arr_node {
    use super::*;
    use crate::parse;

    fn gen_nodes(content: &str) -> Vec<lex::LocatedTkn> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&content.to_string()).unwrap();
        lexer.gen_tkns.clone()
    }

    /// `main(): int { <stmt> }`の、1行目のノードを返す
    fn first_stmt(stmt: &str) -> node::Group2Node {
        let mut p = parse::Parser::new();
        let tkns = gen_nodes(&format!("main(): int {{ {} }}", stmt));
        let node::Group1Node::FuncDefine(ref node) = p.parser(&tkns).expect("node is err")[0] else {
            panic!("not func");
        };
        node.body[0].get_node().clone()
    }

    fn num(n: &str) -> node::Expr {
        node::Expr::Number(n.to_string())
    }

    fn var(name: &str) -> node::Expr {
        node::Expr::Var(name.to_string())
    }

    /// `[name index else else_idx]`のノード
    fn ref_arr(name: &str, index: node::Expr, else_idx: Option<node::Expr>) -> node::Expr {
        node::Expr::RefArray {
            name: name.to_string(),
            dst: Box::new(var(name)),
            index: Box::new(index),
            else_idx: else_idx.map(Box::new),
        }
    }

    #[test]
    fn check_arr_node() {
        assert_eq!(
            first_stmt("[arr 0 else 10] = 10"),
            node::AssignVar::new("arr", ref_arr("arr", num("0"), Some(num("10"))), num("10"))
                .wrap_group2()
        );
    }

    #[test]
    fn check_arr_node_without_else() {
        assert_eq!(
            first_stmt("[arr 0] = 10"),
            node::AssignVar::new("arr", ref_arr("arr", num("0"), None), num("10")).wrap_group2()
        );
    }

    /// `[arr a.a else 0] = 0`: 添字が構造体のメンバー
    #[test]
    fn check_arr_node_member_index() {
        let index = node::Expr::Member {
            scope: vec!["a".to_string()],
            target: Box::new(var("a")),
        };
        assert_eq!(
            first_stmt("[arr a.a else 0] = 0"),
            node::AssignVar::new("arr", ref_arr("arr", index, Some(num("0"))), num("0"))
                .wrap_group2()
        );
    }

    #[test]
    fn check_arr_node_var_index() {
        assert_eq!(
            first_stmt("[arr i else 0] = 1"),
            node::AssignVar::new("arr", ref_arr("arr", var("i"), Some(num("0"))), num("1"))
                .wrap_group2()
        );
    }

    #[test]
    fn check_arr_node_calc_index() {
        let index = node::Expr::Add((Box::new(var("i")), Box::new(num("1"))));
        assert_eq!(
            first_stmt("[arr i + 1 else 0] = 1"),
            node::AssignVar::new("arr", ref_arr("arr", index, Some(num("0"))), num("1"))
                .wrap_group2()
        );
    }

    /// 代入ではなく、式として値を読み取る
    #[test]
    fn check_arr_node_as_value() {
        let index = node::Expr::Member {
            scope: vec!["a".to_string()],
            target: Box::new(var("a")),
        };
        assert_eq!(
            first_stmt("x: int = [arr a.a else 0]"),
            node::Expr::DefVar(node::DefineVar {
                name: "x".to_string(),
                value: Box::new(ref_arr("arr", index, Some(num("0")))),
                ty: node::TyNode::Ty("int".to_string()),
                var_attr: parse::VarMutAttr::Invar,
            })
            .wrap_group2()
        );
    }

    /// 添字がない場合や、`[ptr]`は今まで通りポインタの参照になる
    #[test]
    fn check_ptr_is_not_arr_node() {
        assert_eq!(
            first_stmt("a: int* = [b]"),
            node::Expr::DefVar(node::DefineVar {
                name: "a".to_string(),
                value: Box::new(node::Expr::GetAddress(Box::new(var("b")))),
                ty: node::TyNode::Pointer {
                    is_const: false,
                    ty_name: Box::new(node::TyNode::Ty("int".to_string())),
                    range: None,
                },
                var_attr: parse::VarMutAttr::Invar,
            })
            .wrap_group2()
        );
    }

    /// `name.[member index]`: 構造体の配列メンバー
    #[test]
    fn check_method_arr_node() {
        assert_eq!(
            first_stmt("me.[arr 0 else 10] = 10"),
            node::AssignVar::new(
                "me",
                node::Expr::Member {
                    scope: vec!["me".to_string()],
                    target: Box::new(ref_arr("arr", num("0"), Some(num("10")))),
                },
                num("10"),
            )
            .wrap_group2()
        );
    }
}
