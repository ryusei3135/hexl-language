use super::{Parser, *};

/// 関数を定義するノードの生成
impl Parser {
    /// この関数を呼び出すときは、次のトークンが`(`で無ければ
    /// いけない
    ///
    /// ## self_name
    /// 構造体/列挙型に定義されたメゾットを解析している場合、その
    /// 構造体/列挙型自身の名前を渡す。引数や戻り値の型に予約語
    /// `Self`が使われたとき、この名前へ解決するために使われる。
    /// トップレベルの関数を解析している場合は`None`を渡す。
    pub(super) fn func_node(
        &mut self,
        func_name: &str,
        is_public: bool,
    ) -> Result<node::Group1Node, err::ErrKind> {
        let arg = match self.next_tkn(&["(", "<"])? {
            lex::Tkn::LParen => self.define_arg_node()?,
            lex::Tkn::LAngleBracket => {
                // トップレベルのジェネリクス関数(`func<T>(..)`)は、
                // `Parser::build_func`が先に処理するので、ここに
                // 来るのはメゾットなどにジェネリクスが使われた場合
                return crate::syntax_err!(
                    self.build_err_span(),
                    err::SyntaxErrKind::NotImplemented {
                        feature: "メゾットのジェネリクス",
                    }
                );
            }
            t => {
                return crate::syntax_err!(
                    self.build_err_span(),
                    err::SyntaxErrKind::ExpectedKind {
                        expected: "( or <",
                        found: t,
                    }
                );
            }
        };

        match self.current_tkn() {
            lex::Tkn::Colon => {
                let ret_ty = self.define_ty_node()?;

                if self.current_tkn() == &lex::Tkn::LBrace {
                    Ok(node::FuncDefine::new(
                        func_name,
                        arg,
                        ret_ty,
                        is_public,
                    ))
                } else {
                    Err(err::ErrKind::NotFoundTkn(Box::new(lex::Tkn::LBrace)))
                }
            }
            // 戻り値の型が指定されていない場合、組み込みの`int`型を
            // デフォルトの戻り値の型として扱う
            lex::Tkn::LBrace => Ok(node::FuncDefine::new(
                func_name,
                arg,
                node::TyNode::Ty("int".to_string()),
                is_public,
            )),
            t => crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::ExpectedKind {
                    expected: ": or `{`",
                    found: t.clone(),
                }
            ),
        }
    }

    /// 引数の定義をするノードを作成する
    /// この関数では、型のあとまでトークンを進めているので、呼び出し元では
    /// current_tknでトークンを判定する
    ///
    /// ## Errors
    /// - `,`を挟まずに次の引数の定義が来た場合(名前や`)`以外のトークン)
    /// - `,`の直前に引数が無い場合(先頭が`,`、または`,,`)
    /// - 引数の後に`,`でも`)`でもないトークンが来た場合
    ///
    /// ## self_name
    /// メゾットの引数を解析している場合、そのメゾットが定義されている
    /// 構造体/列挙型自身の名前を渡す。引数の型に予約語`Self`が使われた
    /// 場合、`node::TyNode::SelfTy(self_name)`へ解決するために使われる
    /// (実際に`Self`が第一引数以外に使われていないかのチェックは、
    /// IRへの変換時に行う)
    fn define_arg_node(&mut self) -> Result<Vec<node::ArgsNode>, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LParen {
            return Err(err::ErrKind::NotFoundTkn(Box::new(lex::Tkn::LParen)));
        }

        let mut args_params = Vec::<node::ArgsNode>::new();
        // 引数の定義を作成可能か
        let mut can_create_param = true;

        loop {
            // ループの先頭に来る時点では、直前の反復で`,`を消費して
            // `can_create_param`が`true`に戻されているはず(そうでなければ
            // 下の`,`の分岐、またはループを抜ける`)`の分岐のいずれかを
            // 通っているはず)なので、ここが`false`になるのはパーサー
            // 自体のバグ
            if can_create_param == false {
                unreachable!("define_arg_node: 引数リストの内部状態が不正です");
            }
            match self.next_tkn(&["name", ")"])? {
                lex::Tkn::Name(name) => {
                    let var_attr: VarMutAttr = self.args_is_mut(&name)?;
                    // 直前の`if !can_create_param`のチェックを通過して
                    // いるため、ここでは常に`true`(同上、パーサー自体の
                    // バグでない限り到達しない)
                    if can_create_param == false {
                        unreachable!("define_arg_node: 引数リストの内部状態が不正です");
                    }
                    self.next_tkn(&[])?;
                    let ty = self.define_ty_node()?;
                    args_params.push(node::ArgsNode {
                        name: name.clone(),
                        ty,
                        var_attr,
                    });
                    can_create_param = false;
                }
                // これが来た場合引数の定義が終了
                lex::Tkn::RParen => {
                    self.next_tkn(&[])?;
                    break;
                }
                t => {
                    return self.args_expr_in_unexpect_tkn(t);
                }
            }

            match self.current_tkn() {
                lex::Tkn::RParen => {
                    self.next_tkn(&[])?;
                    break;
                }
                lex::Tkn::Comma => {
                    if can_create_param == false {
                        can_create_param = true;
                    } else {
                        // `,`の前に引数が無い(先頭が`,`、または`,,`)
                        return crate::syntax_err!(
                            self.build_err_span(),
                            err::SyntaxErrKind::ExpectedKind {
                                expected: "name",
                                found: lex::Tkn::Comma,
                            }
                        );
                    }
                }
                t => {
                    return crate::syntax_err!(
                        self.build_err_span(),
                        err::SyntaxErrKind::ExpectedKind {
                            expected: ", or `)`",
                            found: t.clone(),
                        }
                    );
                }
            }
        }

        Ok(args_params)
    }
}

/// ジェネリクスの型を入れ子にできる深さの上限。
/// `f<T>`の中で`f<T*>`を呼ぶように、型が増え続ける再帰で
/// 無限にインスタンス化してしまうのを防ぐ
const MAX_GENERIC_DEPTH: usize = 32;

/// ジェネリクス関数(`func<T>(..)`)の解析
///
/// ## 仕様
/// - 定義(`func<T>(arg: T): T { .. }`)を読んだ時点では、ノードを作らない
/// - 呼び出し(`func<int>(10)`)が来るたびに、その型の関数のノードを作る。
///   型の数だけ関数ができる(同じ型での呼び出しが何度あっても1つだけ)
/// - 作った関数の`FuncDefine::temp_ty`に、`<>`の中身(`[int]`)を入れる。
///   呼び出しの`CallInfo::temp_ty`にも同じ値が入るので、この2つを
///   突き合わせて呼び出し先の関数を特定できる
///
/// ## 作り方
/// 定義のトークン列の型パラメータ(`T`)を、呼び出しで指定された型の
/// トークンに置き換えて、通常の関数と同じ方法で解析し直す。
/// 作った関数は`pending_funcs`に貯め、`Parser::parser`の最後に
/// 通常の関数の後ろへ追加する。
impl Parser {
    /// トップレベルのジェネリクス関数の定義をすべて集める。
    ///
    /// 呼び出しが定義より前に書かれていても、その型の関数を作れる
    /// ように、本格的な解析(`parse_loop`)の前に1度だけ呼ぶ
    pub(super) fn collect_generic_funcs(&mut self) -> Result<(), err::ErrKind> {
        let len = self.tkns.as_ref().unwrap().len();
        // `{`の深さ。トップレベル(0)にある`name<`だけが関数の定義
        let mut depth = 0usize;
        let mut i = 0usize;

        while i < len {
            let tkns = self.tkns.as_ref().unwrap();
            let is_generic_head = matches!(tkns[i].tkn, lex::Tkn::Name(_))
                && matches!(
                    tkns.get(i + 1).map(|t| &t.tkn),
                    Some(lex::Tkn::LAngleBracket)
                );

            if matches!(tkns[i].tkn, lex::Tkn::LBrace) {
                depth += 1;
            } else if matches!(tkns[i].tkn, lex::Tkn::RBrace) {
                depth = depth.saturating_sub(1);
            } else if depth == 0 && is_generic_head {
                let (name, generic, end) = self.extract_generic_def(i)?;
                self.generic_funcs.insert(name, generic);
                // 定義の中身は読み飛ばす
                i = end;
            }
            i += 1;
        }
        Ok(())
    }

    /// 現在のトークン(関数名)から始まるジェネリクス関数の定義を、
    /// 本体を閉じる`}`まで読み飛ばす。終了時は`}`を指す
    pub(super) fn skip_generic_func_def(&mut self) -> Result<(), err::ErrKind> {
        let (_, _, end) = self.extract_generic_def(self.idx)?;
        self.idx = end;
        Ok(())
    }

    /// `tkns[name_idx]`(関数名)から始まるジェネリクス関数の定義
    /// `name<T, U>(..): ty { .. }`を切り出す。
    ///
    /// ## 戻り値
    /// `(関数名, 定義, 本体を閉じる`}`のインデックス)`
    fn extract_generic_def(
        &self,
        name_idx: usize,
    ) -> Result<(String, GenericFunc, usize), err::ErrKind> {
        let tkns = self.tkns.as_ref().unwrap();
        // 指定位置のトークンの場所を、エラーの位置にする
        // (範囲外は最後のトークンの位置)
        let span_at = |i: usize| {
            let t = &tkns[i.min(tkns.len() - 1)];
            err::Span::new(t.line, t.pos)
        };
        let expected = |i: usize, expected: &'static str| {
            crate::syntax_err!(
                span_at(i),
                err::SyntaxErrKind::ExpectedKind {
                    expected,
                    found: tkns[i].tkn.clone(),
                }
            )
        };
        let eof = |expected: Vec<&'static str>| {
            crate::syntax_err!(
                span_at(tkns.len()),
                err::SyntaxErrKind::TknIsEof { expected }
            )
        };

        let lex::Tkn::Name(name) = tkns[name_idx].tkn.clone() else {
            unreachable!("extract_generic_def: 関数名のトークンではありません");
        };

        // `<T, U>`の型パラメータ
        let mut params = Vec::<String>::new();
        // `<`を指している
        let mut i = name_idx + 1;
        loop {
            i += 1;
            match tkns.get(i).map(|t| &t.tkn) {
                Some(lex::Tkn::Name(param)) => {
                    if params.contains(param) {
                        return expected(i, "unique type parameter name");
                    }
                    params.push(param.clone());
                }
                Some(_) => return expected(i, "name"),
                None => return eof(vec!["name"]),
            }
            i += 1;
            match tkns.get(i).map(|t| &t.tkn) {
                Some(lex::Tkn::Comma) => continue,
                Some(lex::Tkn::RAngleBracket) => break,
                Some(_) => return expected(i, ", or `>`"),
                None => return eof(vec![",", ">"]),
            }
        }

        // `>`の次から本体を開く`{`までが、引数と戻り値の型
        let head_start = i + 1;
        match tkns.get(head_start).map(|t| &t.tkn) {
            Some(lex::Tkn::LParen) => {}
            Some(_) => return expected(head_start, "("),
            None => return eof(vec!["("]),
        }
        let Some(body_start) = tkns[head_start..]
            .iter()
            .position(|t| t.tkn == lex::Tkn::LBrace)
            .map(|p| head_start + p)
        else {
            return eof(vec!["{"]);
        };

        // 本体を閉じる`}`
        let mut depth = 0usize;
        let mut end = body_start;
        loop {
            match tkns.get(end).map(|t| &t.tkn) {
                Some(lex::Tkn::LBrace) => depth += 1,
                Some(lex::Tkn::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Some(_) => {}
                None => return eof(vec!["}"]),
            }
            end += 1;
        }

        // 関数名 + `(..): ty { .. }` (`<T, U>`は取り除く)
        let mut def_tkns = vec![tkns[name_idx].clone()];
        def_tkns.extend_from_slice(&tkns[head_start..=end]);

        let is_public = name_idx > 0 && tkns[name_idx - 1].tkn == lex::Tkn::KeyWordPub;

        Ok((
            name,
            GenericFunc {
                is_public,
                params,
                tkns: def_tkns,
            },
            end,
        ))
    }

    /// `name<..>`が、ジェネリクス関数の呼び出しか判定する。
    ///
    /// 比較の`a < b`と区別するため、次の両方を満たすときだけ
    /// 呼び出しとみなす。
    /// - `name`が定義済みのジェネリクス関数
    /// - `<`から`>`までが型の並びで、`>`の次が`(`
    ///
    /// ## Args
    /// - lt_idx `<`のトークンのインデックス
    pub(super) fn is_generic_call(&self, name: &str, lt_idx: usize) -> bool {
        if !self.generic_funcs.contains_key(name) {
            return false;
        }
        let tkns = self.tkns.as_ref().unwrap();

        // 型の先頭になれるトークンでなければ、型引数ではない
        // (`f<>(..)`や`f<3>(..)`など)
        if !matches!(
            tkns.get(lt_idx + 1).map(|t| &t.tkn),
            Some(
                lex::Tkn::Name(_)
                    | lex::Tkn::LBracket
                    | lex::Tkn::KeyWordStatic
                    | lex::Tkn::KeyWordSelf
            )
        ) {
            return false;
        }

        let mut k = lt_idx + 1;
        while let Some(t) = tkns.get(k) {
            match &t.tkn {
                lex::Tkn::RAngleBracket => {
                    return matches!(tkns.get(k + 1).map(|t| &t.tkn), Some(lex::Tkn::LParen));
                }
                // 型を構成するトークン
                lex::Tkn::Name(_)
                | lex::Tkn::Comma
                | lex::Tkn::Mul
                | lex::Tkn::KeyWordConst
                | lex::Tkn::KeyWordMut
                | lex::Tkn::KeyWordStatic
                | lex::Tkn::KeyWordSelf
                | lex::Tkn::LBracket
                | lex::Tkn::RBracket
                | lex::Tkn::Number(_) => k += 1,
                _ => return false,
            }
        }
        false
    }

    /// `<int, byte>`のように、ジェネリクス関数を呼び出す際の型引数を読み込む。
    /// 呼び出し時の`current_tkn()`は`<`で、終了時は`>`を指す
    ///
    /// ## Args
    /// - param_count 関数の型パラメータの数。型引数の数がこれと
    ///   違う場合はエラー
    ///
    /// ## 戻り値
    /// `(型のノード, 各型のトークン列)`。トークン列は、型パラメータを
    /// 置き換えるために使う
    fn generic_type_args(
        &mut self,
        param_count: usize,
    ) -> Result<(Vec<node::TyNode>, Vec<Vec<lex::LocatedTkn>>), err::ErrKind> {
        let mut tys = Vec::<node::TyNode>::new();
        let mut ty_tkns = Vec::<Vec<lex::LocatedTkn>>::new();

        loop {
            // 型の先頭のトークン(`<`または`,`の次)
            let start = self.idx + 1;
            let ty = self.ty_node_after_delim()?;
            // 型を読み終えて、次のトークン(`,`か`>`)を指している
            ty_tkns.push(self.tkns.as_ref().unwrap()[start..self.idx].to_vec());
            tys.push(ty);

            match self.current_tkn().clone() {
                lex::Tkn::Comma if tys.len() < param_count => continue,
                lex::Tkn::RAngleBracket if tys.len() == param_count => break,
                // 型引数が多すぎる(`,`)
                lex::Tkn::Comma => {
                    return crate::syntax_err!(
                        self.build_err_span(),
                        err::SyntaxErrKind::ExpectedKind {
                            expected: "`>`",
                            found: lex::Tkn::Comma,
                        }
                    );
                }
                // 型引数が足りない(`>`)
                lex::Tkn::RAngleBracket => {
                    return crate::syntax_err!(
                        self.build_err_span(),
                        err::SyntaxErrKind::ExpectedKind {
                            expected: "`,`",
                            found: lex::Tkn::RAngleBracket,
                        }
                    );
                }
                t => {
                    return crate::syntax_err!(
                        self.build_err_span(),
                        err::SyntaxErrKind::ExpectedKind {
                            expected: ", or `>`",
                            found: t,
                        }
                    );
                }
            }
        }
        Ok((tys, ty_tkns))
    }

    /// ジェネリクス関数を呼び出すノード`name<int>(args)`を作成する。
    /// その型の関数がまだ無ければ、ここで作る。
    ///
    /// 呼び出し時の`current_tkn()`は`<`で、終了時は
    /// `call_func_expr`と同じく、呼び出しの`)`の次を指す
    ///
    /// ## Panics
    /// `name`が定義済みのジェネリクス関数でない、または現在のトークンが
    /// `<`でない場合(呼び出し元で`is_generic_call`を確認しておくこと)
    pub(super) fn generic_call_expr(
        &mut self,
        name: &String,
        ini_struct: bool,
    ) -> Result<node::Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LAngleBracket {
            return self.fn_unexpect_tkn::<node::Expr>();
        }
        let param_count = self
            .generic_funcs
            .get(name)
            .expect("generic_call_expr: 定義されていないジェネリクス関数")
            .params
            .len();

        let (ty_args, ty_tkns) = self.generic_type_args(param_count)?;

        // `>`の次は`(`
        if self.next_tkn(&["("])? != lex::Tkn::LParen {
            return self.not_found_lparen();
        }

        self.instantiate_generic_func(name, &ty_args, &ty_tkns)?;

        let mut call = self.call_func_expr(name, ini_struct)?;
        if let node::Expr::CallFunc(ref mut info) = call {
            info.temp_ty = ty_args;
        }
        Ok(call)
    }

    /// ジェネリクス関数の型パラメータを、実際の型のトークンに置き換える
    fn substitute_ty_params(
        generic: &GenericFunc,
        ty_tkns: &Vec<Vec<lex::LocatedTkn>>,
    ) -> Vec<lex::LocatedTkn> {
        let mut out = Vec::with_capacity(generic.tkns.len());

        for (i, t) in generic.tkns.iter().enumerate() {
            // 先頭は関数名。`x.T`や`mod::T`の`T`は、型ではなく
            // メンバー/パスの名前なので置き換えない
            let replaceable = i > 0
                && !matches!(
                    generic.tkns[i - 1].tkn,
                    lex::Tkn::Dot | lex::Tkn::ModPathTkn
                );
            if let (true, lex::Tkn::Name(n)) = (replaceable, &t.tkn) {
                if let Some(p) = generic.params.iter().position(|p| p == n) {
                    for ty_tkn in &ty_tkns[p] {
                        // 位置情報は、置き換え前の`T`の位置にしておく
                        let mut tkn = ty_tkn.clone();
                        tkn.line = t.line;
                        tkn.pos = t.pos;
                        out.push(tkn);
                    }
                    continue;
                }
            }
            out.push(t.clone());
        }
        out
    }

    /// 型引数`ty_args`で、ジェネリクス関数`name`の関数のノードを作り、
    /// `pending_funcs`に追加する。同じ型引数ですでに作っていれば何もしない。
    ///
    /// 呼び出し時点の解析の状態(トークン列や位置など)は、この関数の
    /// 前後で変わらない
    fn instantiate_generic_func(
        &mut self,
        name: &String,
        ty_args: &Vec<node::TyNode>,
        ty_tkns: &Vec<Vec<lex::LocatedTkn>>,
    ) -> Result<(), err::ErrKind> {
        if self
            .generated_funcs
            .iter()
            .any(|(n, t)| n == name && t == ty_args)
        {
            return Ok(());
        }
        if self.generic_depth >= MAX_GENERIC_DEPTH {
            return crate::syntax_err!(
                self.build_err_span(),
                err::SyntaxErrKind::NotImplemented {
                    feature: "型が増え続ける再帰的なジェネリクス関数",
                }
            );
        }

        let generic = self
            .generic_funcs
            .get(name)
            .expect("instantiate_generic_func: 定義されていないジェネリクス関数")
            .clone();
        // 本体の中で自分自身を呼び出していても、無限に作らないよう
        // 解析を始める前に登録する
        self.generated_funcs.push((name.clone(), ty_args.clone()));

        let tkns = Self::substitute_ty_params(&generic, ty_tkns);

        // 解析の途中の状態を退避して、置き換え後のトークン列だけを
        // 先頭から解析する(トップレベルの関数として解析される)
        let saved_tkns = self.tkns.replace(tkns);
        let saved_idx = std::mem::replace(&mut self.idx, 0);
        let saved_flag = std::mem::replace(&mut self.gen_flag, GenFlag::Group1);
        let saved_scope = std::mem::replace(&mut self.scope_counter, 0);
        let saved_self = self.struct_self_name.take();
        let saved_stk = std::mem::take(&mut self.other_stk);
        let saved_nodes = std::mem::take(&mut self.gen_nodes);
        self.generic_depth += 1;

        let result = self.parse_loop();

        // エラーでも、必ず状態を元に戻してから返す
        self.generic_depth -= 1;
        let made = std::mem::replace(&mut self.gen_nodes, saved_nodes);
        self.other_stk = saved_stk;
        self.struct_self_name = saved_self;
        self.scope_counter = saved_scope;
        self.gen_flag = saved_flag;
        self.idx = saved_idx;
        self.tkns = saved_tkns;
        result?;

        // 置き換え後のトークン列は関数1つ分なので、作られるノードも1つ
        let Some(node::Group1Node::FuncDefine(mut func)) = made.into_iter().next() else {
            unreachable!("instantiate_generic_func: 関数のノードが作られていません");
        };
        func.set_temp_ty(ty_args);
        func.public = generic.is_public;
        self.pending_funcs.push(node::Group1Node::FuncDefine(func));
        Ok(())
    }
}

#[cfg(test)]
mod generic_tests {
    use crate::{lex, node, parse};

    fn try_build(src: &str) -> Result<Vec<node::Group1Node>, ()> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&src.to_string()).unwrap();
        let mut p = parse::Parser::new();
        p.parser(lexer.gen_tkns.clone())
            .map(|nodes| nodes.clone())
            .map_err(|_| ())
    }

    fn build(src: &str) -> Vec<node::Group1Node> {
        try_build(src).expect("parse failed")
    }

    fn ty(name: &str) -> node::TyNode {
        node::TyNode::Ty(name.to_string())
    }

    fn funcs(nodes: &Vec<node::Group1Node>) -> Vec<&node::FuncDefine> {
        nodes
            .iter()
            .map(|n| match n {
                node::Group1Node::FuncDefine(f) => f,
                t => panic!("関数ではありません: {:?}", t),
            })
            .collect()
    }

    /// `name`で、`<>`の中身が`temp_ty`の関数を探す
    fn find<'a>(
        nodes: &'a Vec<node::Group1Node>,
        name: &str,
        temp_ty: Vec<node::TyNode>,
    ) -> &'a node::FuncDefine {
        funcs(nodes)
            .into_iter()
            .find(|f| f.name == name && f.temp_ty == temp_ty)
            .unwrap_or_else(|| panic!("{}<{:?}>が無い: {:?}", name, temp_ty, nodes))
    }

    fn call(name: &str, temp_ty: Vec<node::TyNode>, args: Vec<node::Expr>) -> node::Expr {
        node::Expr::CallFunc(node::CallInfo {
            name: name.to_string(),
            temp_ty,
            args,
        })
    }

    // ---- 仕様のサンプル ----

    #[test]
    fn spec_example_makes_one_func_per_type() {
        let nodes = build(
            "
            func<T>(arg: T): T {
                var: T = 10
                ret var
            }
            main(): int {
                a: int = func<int>(10)
                b: byte = func<byte>(10)
                ret a
            }
            ",
        );
        // main + `func<int>` + `func<byte>`(型の数だけ作られる)
        assert_eq!(nodes.len(), 3);

        // 定義そのもの(`func<T>`)はノードにならない
        assert!(
            funcs(&nodes)
                .iter()
                .all(|f| f.name != "func" || !f.temp_ty.is_empty())
        );

        for t in ["int", "byte"] {
            let f = find(&nodes, "func", vec![ty(t)]);
            assert_eq!(f.params.len(), 1);
            assert_eq!(f.params[0].name, "arg");
            // `T`が実際の型に置き換わっている
            assert_eq!(f.params[0].ty, ty(t));
            assert_eq!(f.ret_ty, ty(t));
            assert_eq!(f.body.len(), 2);
            assert_eq!(
                f.body[0].get_node(),
                node::gen_var_node("var", "10", t, 0).get_node()
            );
            assert_eq!(
                f.body[1].get_node(),
                &node::StmtNode::Return(node::Expr::Var("var".to_string())).wrap()
            );
            assert!(!f.public);
        }
    }

    #[test]
    fn call_node_has_temp_ty() {
        let nodes = build(
            "
            func<T>(arg: T): T { ret arg }
            main(): int {
                var: int = func<int>(10)
                var2: byte = func<byte>(var)
            }
            ",
        );
        let main = find(&nodes, "main", vec![]);
        let node::Group2Node::Expr(node::Expr::DefVar(v)) = main.body[0].get_node() else {
            panic!("{:?}", main.body[0]);
        };
        assert_eq!(
            *v.value,
            call(
                "func",
                vec![ty("int")],
                vec![node::Expr::Number("10".to_string())]
            )
        );
        let node::Group2Node::Expr(node::Expr::DefVar(v)) = main.body[1].get_node() else {
            panic!("{:?}", main.body[1]);
        };
        assert_eq!(
            *v.value,
            call(
                "func",
                vec![ty("byte")],
                vec![node::Expr::Var("var".to_string())]
            )
        );
    }

    // ---- 作られる関数の数 ----

    #[test]
    fn same_type_is_made_only_once() {
        let nodes = build(
            "
            func<T>(arg: T): T { ret arg }
            main(): int {
                a: int = func<int>(1)
                b: int = func<int>(2)
                c: int = func<int>(3)
            }
            ",
        );
        assert_eq!(nodes.len(), 2);
        find(&nodes, "func", vec![ty("int")]);
    }

    #[test]
    fn uncalled_generic_func_makes_no_node() {
        let nodes = build(
            "
            func<T>(arg: T): T { ret arg }
            main(): int { ret 0 }
            ",
        );
        assert_eq!(nodes.len(), 1);
        find(&nodes, "main", vec![]);
    }

    #[test]
    fn call_before_definition() {
        let nodes = build(
            "
            main(): int {
                a: int = func<int>(1)
                ret a
            }
            func<T>(arg: T): T { ret arg }
            ",
        );
        assert_eq!(nodes.len(), 2);
        find(&nodes, "func", vec![ty("int")]);
    }

    #[test]
    fn statement_level_and_ret_call() {
        let nodes = build(
            "
            func<T>(arg: T): T { ret arg }
            main(): int {
                func<int>(10)
                ret func<byte>(1)
            }
            ",
        );
        assert_eq!(nodes.len(), 3);
        let main = find(&nodes, "main", vec![]);
        assert_eq!(
            main.body[0].get_node(),
            &call(
                "func",
                vec![ty("int")],
                vec![node::Expr::Number("10".to_string())]
            )
            .wrap_group2()
        );
        assert_eq!(
            main.body[1].get_node(),
            &node::StmtNode::Return(call(
                "func",
                vec![ty("byte")],
                vec![node::Expr::Number("1".to_string())]
            ))
            .wrap()
        );
        find(&nodes, "func", vec![ty("int")]);
        find(&nodes, "func", vec![ty("byte")]);
    }

    // ---- 定義の形 ----

    #[test]
    fn public_generic_func_keeps_public() {
        let nodes = build(
            "
            pub func<T>(arg: T): T { ret arg }
            main(): int { func<int>(1) }
            ",
        );
        assert!(find(&nodes, "func", vec![ty("int")]).public);
        assert!(!find(&nodes, "main", vec![]).public);
    }

    #[test]
    fn multiple_type_params() {
        let nodes = build(
            "
            first<A, B>(a: A, b: B): A { ret a }
            main(): int {
                x: int = first<int, byte>(1, 2)
                y: int = first<byte, int>(1, 2)
            }
            ",
        );
        assert_eq!(nodes.len(), 3);
        let f = find(&nodes, "first", vec![ty("int"), ty("byte")]);
        assert_eq!(f.params[0].ty, ty("int"));
        assert_eq!(f.params[1].ty, ty("byte"));
        assert_eq!(f.ret_ty, ty("int"));
        // 型の順番が違えば、別の関数
        let f = find(&nodes, "first", vec![ty("byte"), ty("int")]);
        assert_eq!(f.params[0].ty, ty("byte"));
        assert_eq!(f.params[1].ty, ty("int"));
        assert_eq!(f.ret_ty, ty("byte"));
    }

    #[test]
    fn pointer_type_arg() {
        let nodes = build(
            "
            func<T>(arg: T): T { ret arg }
            main(): int { func<int*>(1) }
            ",
        );
        let ptr = node::TyNode::Pointer {
            is_const: false,
            ty_name: Box::new(ty("int")),
            range: None,
        };
        let f = find(&nodes, "func", vec![ptr.clone()]);
        assert_eq!(f.params[0].ty, ptr);
        assert_eq!(f.ret_ty, ptr);
    }

    #[test]
    fn omitted_ret_ty_stays_default() {
        let nodes = build(
            "
            func<T>(arg: T) { ret 0 }
            main(): int { func<byte>(1) }
            ",
        );
        let f = find(&nodes, "func", vec![ty("byte")]);
        assert_eq!(f.ret_ty, ty("int"));
        assert_eq!(f.params[0].ty, ty("byte"));
    }

    #[test]
    fn member_named_like_type_param_is_not_replaced() {
        // `p.T`の`T`はメンバーの名前なので、型パラメータではない
        // (トークン列の置き換えを直接確認する)
        let lex_tkns = |src: &str| {
            let mut lexer = lex::Lexer::new();
            lexer.analy(&src.to_string()).unwrap();
            lexer.gen_tkns
        };
        let mut p = parse::Parser::new();
        p.tkns = Some(lex_tkns("func<T>(p: Point): T { ret p.T }"));
        p.collect_generic_funcs().unwrap();
        let generic = p.generic_funcs.get("func").unwrap().clone();

        let replaced = parse::Parser::substitute_ty_params(&generic, &vec![lex_tkns("int")]);
        let name = |s: &str| lex::Tkn::Name(s.to_string());
        assert_eq!(
            replaced.iter().map(|t| t.tkn.clone()).collect::<Vec<_>>(),
            vec![
                name("func"),
                lex::Tkn::LParen,
                name("p"),
                lex::Tkn::Colon,
                name("Point"),
                lex::Tkn::RParen,
                lex::Tkn::Colon,
                // 戻り値の型の`T`は置き換わる
                name("int"),
                lex::Tkn::LBrace,
                lex::Tkn::KeyWordRet,
                name("p"),
                lex::Tkn::Dot,
                // メンバーの`T`はそのまま
                name("T"),
                lex::Tkn::RBrace,
            ]
        );
    }

    // ---- ジェネリクス関数から、ジェネリクス関数を呼ぶ ----

    #[test]
    fn generic_calls_generic() {
        let nodes = build(
            "
            outer<T>(a: T): T { ret inner<T>(a) }
            inner<U>(b: U): U { ret b }
            main(): int { outer<byte>(1) }
            ",
        );
        assert_eq!(nodes.len(), 3);
        let outer = find(&nodes, "outer", vec![ty("byte")]);
        assert_eq!(
            outer.body[0].get_node(),
            &node::StmtNode::Return(call(
                "inner",
                vec![ty("byte")],
                vec![node::Expr::Var("a".to_string())]
            ))
            .wrap()
        );
        let inner = find(&nodes, "inner", vec![ty("byte")]);
        assert_eq!(inner.params[0].ty, ty("byte"));
    }

    #[test]
    fn recursive_generic_func_terminates() {
        let nodes = build(
            "
            f<T>(n: T): T { ret f<T>(n) }
            main(): int { f<int>(1) }
            ",
        );
        assert_eq!(nodes.len(), 2);
        find(&nodes, "f", vec![ty("int")]);
    }

    #[test]
    fn growing_recursion_is_an_error_not_a_hang() {
        // `f<T>`が`f<T*>`を呼ぶと、型が無限に増える
        assert!(
            try_build(
                "
            f<T>(n: T): T { ret f<T*>(n) }
            main(): int { f<int>(1) }
            "
            )
            .is_err()
        );
    }

    // ---- 既存の構文への影響 ----

    #[test]
    fn comparison_is_not_a_generic_call() {
        // `func`がジェネリクス関数でも、`<..>(`の形でなければ比較
        let nodes = build(
            "
            func<T>(arg: T): T { ret arg }
            main(): int {
                a: int = 1
                b: int = a < 3
                c: int = func < 3
            }
            ",
        );
        // 呼び出しが無いので、`func`の関数は作られない
        assert_eq!(nodes.len(), 1);
        let main = find(&nodes, "main", vec![]);
        let node::Group2Node::Expr(node::Expr::DefVar(v)) = main.body[1].get_node() else {
            panic!("{:?}", main.body[1]);
        };
        assert_eq!(
            *v.value,
            node::Expr::LessThen((
                Box::new(node::Expr::Var("a".to_string())),
                Box::new(node::Expr::Number("3".to_string())),
            ))
        );
    }

    #[test]
    fn normal_func_is_unchanged() {
        let nodes = build("main(): int { ret 0 }");
        let f = find(&nodes, "main", vec![]);
        assert!(f.temp_ty.is_empty());
    }

    // ---- エラー ----

    #[test]
    fn wrong_number_of_type_args_is_an_error() {
        // 多い
        assert!(
            try_build(
                "
            func<T>(arg: T): T { ret arg }
            main(): int { func<int, byte>(1) }
            "
            )
            .is_err()
        );
        // 少ない
        assert!(
            try_build(
                "
            two<A, B>(a: A, b: B): A { ret a }
            main(): int { two<int>(1, 2) }
            "
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_definition_is_an_error() {
        // 型パラメータが空
        assert!(try_build("func<>(a: int): int { ret a } main(): int { ret 0 }").is_err());
        // 型パラメータの重複
        assert!(try_build("func<T, T>(a: T): T { ret a } main(): int { ret 0 }").is_err());
        // `>`が閉じていない
        assert!(try_build("func<T(a: T): T { ret a } main(): int { ret 0 }").is_err());
        // 本体が閉じていない
        assert!(try_build("func<T>(a: T): T { ret a").is_err());
    }
}
