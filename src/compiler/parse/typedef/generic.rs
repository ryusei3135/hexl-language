//! 構造体/バリアントのジェネリクス(`struct Name<T> { .. }` /
//! `variant Name<T> { .. }` / `variant "unsafe" Name<T> { .. }`)の解析
//!
//! ジェネリクス関数(`func.rs`)と同じ方法で実装している。
//!
//! ## 仕様
//! - 定義(`struct Pair<T> { a: T b: T }`)を読んだ時点では、ノードを作らない
//! - `Pair<int>`のように型引数を付けて使われるたびに、その型引数の構造体/
//!   バリアントのノードを作る。型引数の数だけ構造体/バリアントができる
//!   (同じ型引数での使用が何度あっても1つだけ)
//! - 作った構造体/バリアントの名前は、型引数を含んだ名前
//!   (`Pair<int>` -> `Pair$int`、`Pair<int, byte>` -> `Pair$int$byte`)にする。
//!   `Pair<int>`の型は`TyNode::Ty("Pair$int")`になり、メゾットの`Self`や
//!   メゾットのモジュール名もこの名前になるので、以降の処理は通常の
//!   構造体/バリアントと同じに扱える
//!
//! ## 使える場所
//! - 型(変数・引数・戻り値・メンバーの型): `a: Pair<int>` / `Pair<int>*`
//! - 構造体の初期化: `Pair<int> { a: 1 b: 2 }`
//! - メゾット/バリアントのメンバー: `Pair<int>::new()` / `Opt<int>::None`
//! - 期待する型から解決できる場合は、`Opt::None` / `Opt::Some(value)`のように
//!   バリアントの型引数を省略できる
//!
//! ## 作り方
//! 定義のトークン列の型パラメータ(`T`)を、型引数のトークンに置き換えて、
//! 通常の構造体/バリアントと同じ方法で解析し直す。
//! 作ったノードは`pending_types`に貯め、`Parser::parser`の最後に
//! 通常のノードの後ろへ追加する。

use super::*;

/// ジェネリクスの型を入れ子にできる深さの上限(`func.rs`と同じ値)。
/// `struct Node<T> { next: Node<T*>* }`のように、型が増え続ける再帰で
/// 無限にインスタンス化してしまうのを防ぐ
const MAX_GENERIC_DEPTH: usize = 32;

impl Parser {
    // ===== 定義の収集 =====

    /// `tkns[kw_idx]`(`struct`か`variant`)が、ジェネリクスな型の定義
    /// (`struct Name<`、`variant Name<`、`variant "unsafe" Name<`)の
    /// 先頭ならば、型の名前のトークンのインデックスを返す
    fn generic_type_head(&self, kw_idx: usize) -> Option<usize> {
        let tkns = self.tkns.as_ref()?;
        let mut name_idx = kw_idx + 1;
        if matches!(tkns.get(kw_idx).map(|t| &t.tkn), Some(lex::Tkn::KeyWordVariant))
            && matches!(tkns.get(name_idx).map(|t| &t.tkn), Some(lex::Tkn::Str(_)))
        {
            name_idx += 1;
        }

        let is_head = matches!(tkns.get(name_idx).map(|t| &t.tkn), Some(lex::Tkn::Name(_)))
            && matches!(
                tkns.get(name_idx + 1).map(|t| &t.tkn),
                Some(lex::Tkn::LAngleBracket)
            );
        is_head.then_some(name_idx)
    }

    /// トップレベルのジェネリクスな構造体/バリアントの定義をすべて集める。
    ///
    /// 使用が定義より前に書かれていても、その型を作れるように、
    /// 本格的な解析(`parse_loop`)の前に1度だけ呼ぶ
    pub(in crate::compiler::parse)
    fn collect_generic_types(&mut self) -> Result<(), err::ErrKind> {
        let len = self.tkns.as_ref().unwrap().len();
        // `{`の深さ。トップレベル(0)にある`struct`/`variant`だけが定義
        let mut depth = 0usize;
        let mut i = 0usize;

        while i < len {
            let tkn = self.tkns.as_ref().unwrap()[i].tkn.clone();
            match tkn {
                lex::Tkn::LBrace => depth += 1,
                lex::Tkn::RBrace => depth = depth.saturating_sub(1),
                lex::Tkn::KeyWordStruct | lex::Tkn::KeyWordVariant
                    if depth == 0 && self.generic_type_head(i).is_some() =>
                {
                    let (name, generic, end) = self.extract_generic_type_def(i)?;
                    self.generic_types.insert(name, generic);
                    // 定義の中身は読み飛ばす
                    i = end;
                }
                _ => {}
            }
            i += 1;
        }
        Ok(())
    }

    /// 現在のトークン(`struct`/`variant`)が、ジェネリクスな型の定義なら、
    /// 本体を閉じる`}`まで読み飛ばして`true`を返す(終了時は`}`を指す)。
    /// ジェネリクスでなければ、何もせず`false`を返す
    pub(in crate::compiler::parse)
    fn skip_generic_type_def(&mut self) -> Result<bool, err::ErrKind> {
        if self.generic_type_head(self.idx).is_none() {
            return Ok(false);
        }
        let (_, _, end) = self.extract_generic_type_def(self.idx)?;
        self.idx = end;
        Ok(true)
    }

    /// `tkns[kw_idx]`(`struct`/`variant`)から始まるジェネリクスな型の定義
    /// `struct Name<T, U> { .. }`を切り出す。
    /// 呼び出し前に`generic_type_head`で定義の先頭であることを確認しておくこと
    ///
    /// ## 戻り値
    /// `(型の名前, 定義, 本体を閉じる`}`のインデックス)`
    fn extract_generic_type_def(
        &self,
        kw_idx: usize,
    ) -> Result<(String, GenericType, usize), err::ErrKind> {
        let tkns = self.tkns.as_ref().unwrap();
        // エラーは`generic_def_expected`/`generic_def_eof`が作る
        // (関数のジェネリクスと同じ。位置は指定位置のトークンの場所)
        let expected = |i: usize, expected: &'static str| {
            crate::err_at!(self.generic_def_expected(i, expected))
        };
        let eof = |expected: Vec<&'static str>| crate::err_at!(self.generic_def_eof(expected));

        let name_idx = self
            .generic_type_head(kw_idx)
            .expect("extract_generic_type_def: ジェネリクスな型の定義ではありません");
        let lex::Tkn::Name(name) = tkns[name_idx].tkn.clone() else {
            unreachable!("extract_generic_type_def: 型名のトークンではありません");
        };

        // `<T, U>`の型パラメータ
        let mut params = Vec::<String>::new();
        // `<`を指している
        let mut i = name_idx + 1;
        loop {
            i += 1;
            match tkns.get(i).map(|t| &t.tkn) {
                Some(lex::Tkn::Name(param)) => {
                    // 型パラメータ同士、型自身の名前と同じ名前は使えない
                    if params.contains(param) || param == &name {
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

        // `>`の次は本体を開く`{`
        let body_start = i + 1;
        match tkns.get(body_start).map(|t| &t.tkn) {
            Some(lex::Tkn::LBrace) => {}
            Some(_) => return expected(body_start, "{"),
            None => return eof(vec!["{"]),
        }

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

        // `struct`/`variant ["unsafe"] Name` + `{ .. }` (`<T, U>`は取り除く)
        let mut def_tkns = tkns[kw_idx..=name_idx].to_vec();
        def_tkns.extend_from_slice(&tkns[body_start..=end]);

        Ok((
            name,
            GenericType {
                params,
                tkns: def_tkns,
                name_idx: name_idx - kw_idx,
            },
            end,
        ))
    }

    // ===== 使用の判定・型引数の読み込み =====

    /// `tkns[lt_idx]`の`<`から始まる型引数の並びを閉じる、`>`のインデックスを返す。
    /// `<`から`>`までが型を構成するトークンだけでなければ`None`
    /// (`Pair<Box<int>, int>`のように、型引数の中のジェネリクスも読める)
    fn generic_args_end(&self, lt_idx: usize) -> Option<usize> {
        let tkns = self.tkns.as_ref()?;

        // 型の先頭になれるトークンでなければ、型引数ではない
        if !matches!(
            tkns.get(lt_idx + 1).map(|t| &t.tkn),
            Some(
                lex::Tkn::Name(_)
                    | lex::Tkn::LBracket
                    | lex::Tkn::KeyWordStatic
                    | lex::Tkn::KeyWordSelf
            )
        ) {
            return None;
        }

        // 型引数の中の`<`の深さ
        let mut depth = 0usize;
        for (k, t) in tkns.iter().enumerate().skip(lt_idx + 1) {
            match &t.tkn {
                lex::Tkn::LAngleBracket => depth += 1,
                lex::Tkn::RAngleBracket => {
                    if depth == 0 {
                        return Some(k);
                    }
                    depth -= 1;
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
                | lex::Tkn::Number(_) => {}
                _ => return None,
            }
        }
        None
    }

    /// 式の中の`name<..>`が、ジェネリクスな構造体/バリアントの初期化
    /// (`Name<int> { .. }`)か、メンバー/メゾットへのアクセス
    /// (`Name<int>::Mem`)か判定する。
    ///
    /// 比較の`a < b`と区別するため、次の両方を満たすときだけ
    /// そうだとみなす。
    /// - `name`が定義済みのジェネリクスな型
    /// - `<`から`>`までが型の並びで、`>`の次が`{`か`::`
    ///
    /// ## Args
    /// - lt_idx `<`のトークンのインデックス
    pub(in crate::compiler::parse)
    fn is_generic_type_expr(&self, name: &str, lt_idx: usize) -> bool {
        if !self.generic_types.contains_key(name) {
            return false;
        }
        let Some(gt_idx) = self.generic_args_end(lt_idx) else {
            return false;
        };
        let tkns = self.tkns.as_ref().unwrap();
        matches!(
            tkns.get(gt_idx + 1).map(|t| &t.tkn),
            Some(lex::Tkn::LBrace | lex::Tkn::ModPathTkn)
        )
    }

    /// `Pair<int, byte>`の型引数を読み込み、その型引数の構造体/バリアントを
    /// (まだ無ければ)作って、その名前(`Pair$int$byte`)を返す。
    /// 呼び出し時の`current_tkn()`は`<`で、終了時は`>`を指す
    ///
    /// ## Panics
    /// `name`が定義済みのジェネリクスな型でない場合
    /// (呼び出し元で`generic_types`を確認しておくこと)
    pub(in crate::compiler::parse)
    fn generic_type_ref(&mut self, name: &str) -> Result<String, err::ErrKind> {
        let param_count = self
            .generic_types
            .get(name)
            .expect("generic_type_ref: 定義されていないジェネリクスな型")
            .params
            .len();

        // 型引数の読み込みは、ジェネリクス関数と共通(`func.rs`)
        let (ty_args, ty_tkns) = self.generic_type_args(param_count)?;
        self.instantiate_generic_type(name, &ty_args, &ty_tkns)
    }

    /// ジェネリクスな型を使う式`Name<int> { .. }` / `Name<int>::Mem`を作成する。
    ///
    /// 呼び出し時の`current_tkn()`は`<`。呼び出し元で`is_generic_type_expr`を
    /// 確認しておくこと。終了時の位置は、通常の構造体の初期化
    /// (`struct_init_node`)や`Name::Mem`(`scope_member_node`)と同じ
    pub(in crate::compiler::parse)
    fn generic_type_expr<const T: bool>(
        &mut self,
        name: &str,
        init_struct: bool,
    ) -> Result<node::Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LAngleBracket {
            return crate::err_at!(self.fn_unexpect_tkn::<node::Expr>());
        }
        let generic_name = self.generic_type_ref(name)?;

        match self.next_tkn_ref(&["{", "::"])? {
            // 構造体の初期化: `Name<int> { .. }`
            lex::Tkn::LBrace => {
                self.next_tkn(&["{"])?;
                self.struct_init_node::<T>(&generic_name)
            }
            // メゾットの呼び出し・バリアントのメンバー: `Name<int>::Mem`
            lex::Tkn::ModPathTkn => self.scope_member_node(generic_name, init_struct),
            _ => crate::err_at!(self.unexpect_tkn_in_expr()),
        }
    }

    // ===== 型の作成 =====

    /// 型引数を含んだ、作った構造体/バリアントの名前を作る。
    /// `Pair<int, byte>` -> `Pair$int$byte`
    pub(in crate::compiler::parse)
    fn mangle_generic_name(name: &str, ty_args: &[node::TyNode]) -> String {
        let mut mangled = name.to_string();
        for ty in ty_args {
            mangled.push('$');
            mangled.push_str(&Self::mangle_ty(ty));
        }
        mangled
    }

    /// 型を、名前に使える文字列にする(`int*` -> `int_ptr`)
    fn mangle_ty(ty: &node::TyNode) -> String {
        match ty {
            node::TyNode::Ty(name) | node::TyNode::SelfTy(name) => name.clone(),
            node::TyNode::Pointer { is_const, ty_name, .. } => {
                let suffix = if *is_const { "cptr" } else { "ptr" };
                format!("{}_{}", Self::mangle_ty(ty_name), suffix)
            }
            node::TyNode::RefTy(inner) => Self::mangle_ty(inner),
            node::TyNode::Stack { name, len } => format!("{}_stack{}", name, len),
            node::TyNode::Static { name, len } => format!("{}_static{}", name, len),
            node::TyNode::ConstractMust(inner) | node::TyNode::ConstractOf(inner) => {
                Self::mangle_ty(&inner.unwrap_ty())
            }
        }
    }

    /// 型パラメータを、実際の型のトークンに置き換える。
    /// ジェネリクス関数と、ジェネリクスな構造体/バリアントで共通。
    ///
    /// `x.T`や`mod::T`の`T`は、型ではなくメンバー/パスの名前なので置き換えない
    pub(in crate::compiler::parse)
    fn substitute_params(
        tkns: &[lex::LocatedTkn],
        params: &[String],
        ty_tkns: &[Vec<lex::LocatedTkn>],
    ) -> Vec<lex::LocatedTkn> {
        let mut out = Vec::with_capacity(tkns.len());

        for (i, t) in tkns.iter().enumerate() {
            // 先頭は関数名/`struct`/`variant`なので置き換えない
            let replaceable = i > 0
                && !matches!(
                    tkns[i - 1].tkn,
                    lex::Tkn::Dot | lex::Tkn::ModPathTkn
                );
            if let (true, lex::Tkn::Name(n)) = (replaceable, &t.tkn) {
                if let Some(p) = params.iter().position(|p| p == n) {
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

    /// 型引数`ty_args`で、ジェネリクスな型`name`の構造体/バリアントのノードを
    /// 作り、`pending_types`に追加する。作った型の名前を返す。
    /// 同じ型引数ですでに作っていれば、何も作らず名前だけ返す。
    ///
    /// 呼び出し時点の解析の状態(トークン列や位置など)は、この関数の
    /// 前後で変わらない
    fn instantiate_generic_type(
        &mut self,
        name: &str,
        ty_args: &[node::TyNode],
        ty_tkns: &[Vec<lex::LocatedTkn>],
    ) -> Result<String, err::ErrKind> {
        let mangled = Self::mangle_generic_name(name, ty_args);

        if self
            .generated_types
            .iter()
            .any(|(n, t)| n == name && t == ty_args)
        {
            return Ok(mangled);
        }
        if self.generic_depth >= MAX_GENERIC_DEPTH {
            return crate::err_at!(
                self.fn_not_implemented("型が増え続ける再帰的なジェネリクスな型")
            );
        }

        let generic = self
            .generic_types
            .get(name)
            .expect("instantiate_generic_type: 定義されていないジェネリクスな型")
            .clone();
        // `next: Node<T>*`のように、定義の中で自分自身を使っていても、
        // 無限に作らないよう、解析を始める前に登録する
        self.generated_types
            .push((name.to_string(), ty_args.to_vec()));

        let mut tkns = Self::substitute_params(&generic.tkns, &generic.params, ty_tkns);
        // 型の名前を、型引数を含んだ名前に置き換える
        // (名前より前のトークンは`struct`/`variant ["unsafe"]`なので、
        // 置き換えでトークンの位置はずれない)
        tkns[generic.name_idx].tkn = lex::Tkn::Name(mangled.clone());

        let mut made = self.parse_tkns_isolated(tkns)?.into_iter();

        // 置き換え後のトークン列は型1つ分なので、作られるノードも1つ
        let def = match made.next() {
            Some(
                def @ (node::Group1Node::StructDefine(_) | node::Group1Node::VariantDefine(_)),
            ) => def,
            t => unreachable!(
                "instantiate_generic_type: 構造体/バリアントのノードが作られていません: {:?}",
                t
            ),
        };
        self.pending_types.push(def);
        Ok(mangled)
    }

    /// 解析の途中の状態を退避して、`tkns`だけを先頭から解析し、作られた
    /// ノードを返す(トップレベルの定義として解析される)。
    /// エラーでも、必ず状態を元に戻してから返す
    fn parse_tkns_isolated(
        &mut self,
        tkns: Vec<lex::LocatedTkn>,
    ) -> Result<Vec<node::Group1Node>, err::ErrKind> {
        let saved_tkns = self.tkns.replace(tkns);
        let saved_idx = std::mem::replace(&mut self.idx, 0);
        let saved_flag = std::mem::replace(&mut self.gen_flag, GenFlag::Group1);
        let saved_scope = std::mem::replace(&mut self.scope_counter, 0);
        // 型の使用が`loop`の中にあっても、メゾットの中の`break`などが
        // その`loop`の中だと誤解されないようにする
        let saved_loop = std::mem::replace(&mut self.loop_depth, 0);
        let saved_self = self.struct_self_name.take();
        let saved_stk = std::mem::take(&mut self.other_stk);
        let saved_nodes = std::mem::take(&mut self.gen_nodes);
        self.generic_depth += 1;

        let result = self.parse_loop();

        self.generic_depth -= 1;
        let made = std::mem::replace(&mut self.gen_nodes, saved_nodes);
        self.other_stk = saved_stk;
        self.struct_self_name = saved_self;
        self.loop_depth = saved_loop;
        self.scope_counter = saved_scope;
        self.gen_flag = saved_flag;
        self.idx = saved_idx;
        self.tkns = saved_tkns;
        result?;

        Ok(made)
    }
}

#[cfg(test)]
mod generic_type_tests {
    use crate::compiler::node::Field;
    use crate::compiler::{lex, node, parse};

    fn try_build(src: &str) -> Result<Vec<node::Group1Node>, ()> {
        let mut lexer = lex::Lexer::new();
        lexer.analy(&src.to_string()).unwrap();
        let mut p = parse::Parser::new();
        p.parser(&lexer.gen_tkns.clone())
            .map(|nodes| nodes.to_vec())
            .map_err(|_| ())
    }

    fn build(src: &str) -> Vec<node::Group1Node> {
        try_build(src).expect("parse failed")
    }

    fn ty(name: &str) -> node::TyNode {
        node::TyNode::Ty(name.to_string())
    }

    fn structs(nodes: &Vec<node::Group1Node>) -> Vec<&node::StructDefine> {
        nodes
            .iter()
            .filter_map(|n| match n {
                node::Group1Node::StructDefine(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    fn variants(nodes: &Vec<node::Group1Node>) -> Vec<&node::VariantDefine> {
        nodes
            .iter()
            .filter_map(|n| match n {
                node::Group1Node::VariantDefine(v) => Some(v),
                _ => None,
            })
            .collect()
    }

    fn func<'a>(nodes: &'a Vec<node::Group1Node>, name: &str) -> &'a node::FuncDefine {
        nodes
            .iter()
            .find_map(|n| match n {
                node::Group1Node::FuncDefine(f) if f.name == name => Some(f),
                _ => None,
            })
            .unwrap_or_else(|| panic!("関数{}が無い: {:?}", name, nodes))
    }

    // ---- 構造体 ----

    #[test]
    fn struct_makes_one_struct_per_type() {
        let nodes = build(
            "
            struct Pair<T> { a: T b: T }
            main(): int {
                x: Pair<int> = Pair<int> { a: 1 b: 2 }
                y: Pair<byte> = Pair<byte> { a: 1 b: 2 }
                z: Pair<int> = Pair<int> { a: 3 b: 4 }
                ret 0
            }
            ",
        );
        // main + `Pair<int>` + `Pair<byte>`(型引数の数だけ作られる。
        // `Pair<int>`は2回使っても1つだけ)
        assert_eq!(nodes.len(), 3);
        // 定義そのもの(`Pair<T>`)はノードにならない
        assert!(structs(&nodes).iter().all(|s| s.name != "Pair"));

        let ss = structs(&nodes);
        for t in ["int", "byte"] {
            let name = format!("Pair${}", t);
            let s = ss.iter().find(|s| s.name == name).expect("型が無い");
            assert_eq!(
                node::Group1Node::StructDefine((*s).clone()),
                node::StructDefine::new(
                    name,
                    vec![
                        node::StructField::new("a".to_string(), ty(t)),
                        node::StructField::new("b".to_string(), ty(t)),
                    ],
                    Vec::new()
                )
            );
        }
    }

    #[test]
    fn struct_used_before_definition() {
        // 使用が定義より前でも作れる
        let nodes = build(
            "
            main(p: Pair<int>): int { ret 0 }
            struct Pair<T> { a: T }
            ",
        );
        assert_eq!(structs(&nodes)[0].name, "Pair$int");
    }

    #[test]
    fn uncalled_generic_struct_makes_no_node() {
        let nodes = build(
            "
            struct Pair<T> { a: T b: T }
            main(): int { ret 0 }
            ",
        );
        assert_eq!(nodes.len(), 1);
        assert_eq!(func(&nodes, "main").name, "main");
    }

    #[test]
    fn type_in_params_and_ret_ty() {
        let nodes = build(
            "
            struct Pair<T> { a: T b: T }
            f(p: Pair<int>): Pair<int> { ret p }
            ",
        );
        let f = func(&nodes, "f");
        assert_eq!(f.params[0].ty, ty("Pair$int"));
        assert_eq!(f.ret_ty, ty("Pair$int"));
        // 2回使っても、型は1つだけ
        assert_eq!(structs(&nodes).len(), 1);
    }

    #[test]
    fn multiple_type_params() {
        let nodes = build(
            "
            struct Pair<T, U> { a: T b: U }
            f(p: Pair<int, byte>): int { ret 0 }
            ",
        );
        let s = structs(&nodes)[0];
        assert_eq!(s.name, "Pair$int$byte");
        assert_eq!(
            s.fields,
            vec![
                node::StructField::new("a".to_string(), ty("int")),
                node::StructField::new("b".to_string(), ty("byte")),
            ]
        );
    }

    #[test]
    fn methods_resolve_self_to_instance_name() {
        let nodes = build(
            "
            struct Box<T> {
                v: T
                new(): Self {}
                func(self: Self, param: T): T { ret 0 }
            }
            f(b: Box<int>): int { ret 0 }
            ",
        );
        let s = structs(&nodes)[0];
        assert_eq!(s.name, "Box$int");

        let node::Group1Node::FuncDefine(new_func) = &s.methods[0] else {
            panic!();
        };
        assert_eq!(new_func.ret_ty, node::TyNode::SelfTy("Box$int".to_string()));

        let node::Group1Node::FuncDefine(method) = &s.methods[1] else {
            panic!();
        };
        assert_eq!(method.params[0].ty, node::TyNode::SelfTy("Box$int".to_string()));
        assert_eq!(method.params[1].ty, ty("int"));
        assert_eq!(method.ret_ty, ty("int"));
    }

    #[test]
    fn recursive_struct_terminates() {
        // 定義の中で自分自身(`Node<T>`)を使っていても、無限に作らない
        let nodes = build(
            "
            struct Node<T> { v: T next: Node<T>* }
            f(n: Node<int>*): int { ret 0 }
            ",
        );
        assert_eq!(structs(&nodes).len(), 1);
        let s = structs(&nodes)[0];
        assert_eq!(s.name, "Node$int");
        let node::TyNode::Pointer { ty_name, .. } = &s.fields[1].ty else {
            panic!("{:?}", s.fields[1]);
        };
        assert_eq!(**ty_name, ty("Node$int"));
    }

    #[test]
    fn nested_generic_type_arg() {
        // `>>`と続けず、`> >`と書く
        let nodes = build(
            "
            struct Box<T> { v: T }
            struct Pair<T> { a: T }
            f(p: Pair<Box<int> >): int { ret 0 }
            ",
        );
        let ss = structs(&nodes);
        assert!(ss.iter().any(|s| s.name == "Box$int"));
        let pair = ss.iter().find(|s| s.name == "Pair$Box$int").expect("型が無い");
        assert_eq!(pair.fields[0].ty, ty("Box$int"));
    }

    // ---- バリアント ----

    #[test]
    fn variant_makes_one_variant_per_type() {
        let nodes = build(
            "
            variant Opt<T> { Some(T) None }
            main(): int {
                a: Opt<int> = Opt<int>::None
                b: Opt<byte> = Opt<byte>::None
                ret 0
            }
            ",
        );
        assert_eq!(nodes.len(), 3);
        let vs = variants(&nodes);
        for t in ["int", "byte"] {
            let name = format!("Opt${}", t);
            let v = vs.iter().find(|v| v.name == name).expect("型が無い");
            assert_eq!(
                v.fields,
                vec![
                    node::VariantField::new("Some".to_string(), ty(t)),
                    node::VariantField::typeless_new("None".to_string()),
                ]
            );
        }
    }

    #[test]
    fn variant_member_access_uses_instance_name() {
        let nodes = build(
            "
            variant Opt<T> { Some(T) None }
            main(): int {
                a: Opt<int> = Opt<int>::None
                ret 0
            }
            ",
        );
        let main = func(&nodes, "main");
        let node::Group2Node::Expr(node::Expr::DefVar(v)) = main.body[0].get_node() else {
            panic!("{:?}", main.body[0]);
        };
        assert_eq!(
            *v.value,
            node::Expr::EnumVariant {
                name: "Opt$int".to_string(),
                variant: "None".to_string(),
            }
        );
    }

    #[test]
    fn generic_variant_constructor_is_init_variant_node() {
        let nodes = build(
            "
            variant Option<T> { Some(T) None }
            main(): int {
                value: int = 10
                o: Option<int> = Option::Some(value)
                ret 0
            }
            ",
        );
        let main = func(&nodes, "main");
        let node::Group2Node::Expr(node::Expr::DefVar(var)) = main.body[1].get_node() else {
            panic!("{:?}", main.body[1]);
        };
        assert_eq!(
            *var.value,
            node::Expr::InitVariant {
                is_self: false,
                name: "Option".to_string(),
                fields: [(
                    "Some".to_string(),
                    Box::new(node::Expr::Var("value".to_string()))
                )]
                .into_iter()
                .collect(),
            }
        );
    }

    #[test]
    fn unsafe_variant() {
        let nodes = build(
            "
            variant \"unsafe\" Raw<T> { a: T b: i64 }
            f(r: Raw<int>): int { ret 0 }
            ",
        );
        let v = variants(&nodes)[0];
        assert_eq!(v.name, "Raw$int");
        assert_eq!(
            v.fields,
            vec![
                node::VariantField::unsafe_new("a".to_string(), ty("int")),
                node::VariantField::unsafe_new("b".to_string(), ty("i64")),
            ]
        );
        assert!(!v.is_tagged());
    }

    // ---- 関数のジェネリクスとの組み合わせ ----

    #[test]
    fn generic_func_uses_generic_type() {
        let nodes = build(
            "
            struct Pair<T> { a: T }
            f<T>(p: Pair<T>): T { ret 0 }
            main(): int {
                x: Pair<int> = Pair<int> { a: 1 }
                ret f<int>(x)
            }
            ",
        );
        let generated = nodes
            .iter()
            .find_map(|n| match n {
                node::Group1Node::FuncDefine(f) if f.name == "f" => Some(f),
                _ => None,
            })
            .expect("f<int>が無い");
        assert_eq!(generated.params[0].ty, ty("Pair$int"));
        assert_eq!(structs(&nodes).len(), 1);
    }

    // ---- エラー ----

    #[test]
    fn too_many_type_args_is_err() {
        assert!(try_build(
            "
            struct Pair<T> { a: T }
            f(p: Pair<int, int>): int { ret 0 }
            "
        )
        .is_err());
    }

    #[test]
    fn too_few_type_args_is_err() {
        assert!(try_build(
            "
            struct Pair<T, U> { a: T b: U }
            f(p: Pair<int>): int { ret 0 }
            "
        )
        .is_err());
    }

    #[test]
    fn duplicate_type_param_is_err() {
        assert!(try_build("struct Pair<T, T> { a: T }").is_err());
    }

    #[test]
    fn non_generic_type_with_type_args_is_err() {
        assert!(try_build(
            "
            struct Plain { a: int }
            f(p: Plain<int>): int { ret 0 }
            "
        )
        .is_err());
    }

    #[test]
    fn infinitely_growing_type_is_err() {
        // `Node<T>`の中で`Node<T*>`を使うと、型が増え続ける
        assert!(try_build(
            "
            struct Node<T> { next: Node<T*> }
            f(n: Node<int>): int { ret 0 }
            "
        )
        .is_err());
    }

    // ---- 型名 ----

    #[test]
    fn mangle_names() {
        assert_eq!(
            parse::Parser::mangle_generic_name("Pair", &[ty("int"), ty("byte")]),
            "Pair$int$byte"
        );
        assert_eq!(
            parse::Parser::mangle_generic_name(
                "Box",
                &[node::TyNode::Pointer {
                    is_const: false,
                    ty_name: Box::new(ty("int")),
                    range: None,
                }]
            ),
            "Box$int_ptr"
        );
    }
}
