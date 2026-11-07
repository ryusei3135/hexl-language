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
use crate::compiler::hexl_frontend::parse::generic_scan::{is_type_args_start, is_type_component};

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
        if matches!(
            tkns.get(kw_idx).map(|t| &t.tkn),
            Some(lex::Tkn::KeyWordVariant)
        ) && matches!(tkns.get(name_idx).map(|t| &t.tkn), Some(lex::Tkn::Str(_)))
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
    pub(in crate::compiler::hexl_frontend::parse) fn collect_generic_types(
        &mut self,
    ) -> Result<(), err::ErrKind> {
        let Some(len) = self.tkns.as_ref().map(|tkns| tkns.len()) else {
            return crate::err_at!(self.tkn_is_eof(Vec::new()));
        };
        // `{`の深さ。トップレベル(0)にある`struct`/`variant`だけが定義
        let mut depth = 0usize;
        let mut i = 0usize;

        while i < len {
            let Some(tkn) = self
                .tkns
                .as_ref()
                .and_then(|tkns| tkns.get(i))
                .map(|t| t.tkn.clone())
            else {
                return crate::err_at!(self.tkn_is_eof(Vec::new()));
            };
            match tkn {
                lex::Tkn::LBrace => depth += 1,
                lex::Tkn::RBrace => depth = depth.saturating_sub(1),
                lex::Tkn::KeyWordStruct | lex::Tkn::KeyWordVariant
                    if depth == 0 && self.generic_type_head(i).is_some() =>
                {
                    let (name, mut generic, end) = self.extract_generic_type_def(i)?;
                    // `pub struct Name<T> {..}`か
                    generic.public = i > 0
                        && self
                            .tkns
                            .as_ref()
                            .and_then(|tkns| tkns.get(i - 1))
                            .is_some_and(|t| t.tkn == lex::Tkn::KeyWordPub);
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
    pub(in crate::compiler::hexl_frontend::parse) fn skip_generic_type_def(
        &mut self,
    ) -> Result<bool, err::ErrKind> {
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
        let Some(tkns) = self.tkns.as_ref() else {
            return crate::err_at!(self.tkn_is_eof(Vec::new()));
        };
        // エラーは`generic_def_expected`/`generic_def_eof`が作る
        // (関数のジェネリクスと同じ。位置は指定位置のトークンの場所)
        let expected = |i: usize, expected: &'static str| {
            crate::err_at!(self.generic_def_expected(i, expected))
        };
        let eof = |expected: Vec<&'static str>| crate::err_at!(self.generic_def_eof(expected));

        // ジェネリクスな型の定義(`struct Name<`)でなければ、`kw_idx`の位置でエラー
        let Some(name_idx) = self.generic_type_head(kw_idx) else {
            return expected(kw_idx, "generic type definition");
        };
        let Some(lex::Tkn::Name(name)) = tkns.get(name_idx).map(|t| t.tkn.clone()) else {
            return expected(name_idx, "name");
        };

        // `<T, U>`の型パラメータ(`name_idx + 1`は`<`)。
        // 型パラメータ同士、型自身の名前と同じ名前は使えない
        let (params, gt_idx) = self.read_generic_params(tkns, name_idx + 1, Some(name.as_str()))?;

        // `>`の次は本体を開く`{`
        let body_start = gt_idx + 1;
        match tkns.get(body_start).map(|t| &t.tkn) {
            Some(lex::Tkn::LBrace) => {}
            Some(_) => return expected(body_start, "{"),
            None => return eof(vec!["{"]),
        }

        // 本体を閉じる`}`
        let end = self.find_block_end(tkns, body_start)?;

        // `struct`/`variant ["unsafe"] Name` + `{ .. }` (`<T, U>`は取り除く)
        let mut def_tkns = tkns[kw_idx..=name_idx].to_vec();
        def_tkns.extend_from_slice(&tkns[body_start..=end]);

        Ok((
            name,
            GenericType {
                params,
                tkns: def_tkns,
                name_idx: name_idx - kw_idx,
                // `pub`の有無は、定義の外側のトークンなので呼び出し元
                // (`collect_generic_types`)が設定する
                public: false,
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
        if !is_type_args_start(tkns.get(lt_idx + 1).map(|t| &t.tkn)) {
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
                t if is_type_component(t) => {}
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
    pub(in crate::compiler::hexl_frontend::parse) fn is_generic_type_expr(
        &self,
        name: &str,
        lt_idx: usize,
    ) -> bool {
        if !self.generic_types.contains_key(name) {
            return false;
        }
        let Some(gt_idx) = self.generic_args_end(lt_idx) else {
            return false;
        };
        let Some(tkns) = self.tkns.as_ref() else {
            return false;
        };
        matches!(
            tkns.get(gt_idx + 1).map(|t| &t.tkn),
            Some(lex::Tkn::LBrace | lex::Tkn::ModPathTkn)
        )
    }

    /// `Pair<int, byte>`の型引数を読み込み、その型引数の構造体/バリアントを
    /// (まだ無ければ)作って、その名前(`Pair$int$byte`)を返す。
    /// 呼び出し時の`current_tkn()`は`<`で、終了時は`>`を指す
    ///
    /// ## Errors
    /// `name`が定義済みのジェネリクスな型でない場合
    /// (呼び出し元で`generic_types`を確認しておくこと)
    pub(in crate::compiler::hexl_frontend::parse) fn generic_type_ref(
        &mut self,
        name: &str,
    ) -> Result<String, err::ErrKind> {
        let Some(generic) = self.generic_types.get(name) else {
            return crate::err_at!(self.internal_state_err("defined generic type"));
        };
        let param_count = generic.params.len();

        // 型引数の読み込みは、ジェネリクス関数と共通(`func.rs`)
        let (ty_args, ty_tkns) = self.generic_type_args(param_count)?;
        self.instantiate_generic_type(name, &ty_args, &ty_tkns)
    }

    /// ジェネリクスな型を使う式`Name<int> { .. }` / `Name<int>::Mem`を作成する。
    ///
    /// 呼び出し時の`current_tkn()`は`<`。呼び出し元で`is_generic_type_expr`を
    /// 確認しておくこと。終了時の位置は、通常の構造体の初期化
    /// (`struct_init_node`)や`Name::Mem`(`scope_member_node`)と同じ
    pub(in crate::compiler::hexl_frontend::parse) fn generic_type_expr<const T: bool>(
        &mut self,
        name: &str,
        init_struct: bool,
    ) -> Result<Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LAngleBracket {
            return crate::err_at!(self.fn_unexpect_tkn::<Expr>());
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

    // ===== ジェネリクスな型エイリアス =====

    /// 現在のトークン(`type`)が、ジェネリクスな型エイリアスの定義
    /// (`type Name<`)の先頭か
    pub(in crate::compiler::hexl_frontend::parse) fn is_generic_alias_head(&self) -> bool {
        let Some(tkns) = self.tkns.as_ref() else {
            return false;
        };
        matches!(
            tkns.get(self.idx + 1).map(|t| &t.tkn),
            Some(lex::Tkn::Name(_))
        ) && matches!(
            tkns.get(self.idx + 2).map(|t| &t.tkn),
            Some(lex::Tkn::LAngleBracket)
        )
    }

    /// ジェネリクスな型エイリアス`type Name<T, U> = Pair<T, U>*`を読み込み、
    /// `generic_aliases`に登録する(ノードは作らない)。
    /// 呼び出し時の`current_tkn()`は`type`で、終了時は`=`の右側の型の最後の
    /// トークンを指す(`type_alias_node`と同じ)。
    ///
    /// `=`の右側は、`Name` / `Name<..>`(型引数の中のジェネリクスも可)に、
    /// 0個以上の`*`が付いた形
    pub(in crate::compiler::hexl_frontend::parse) fn generic_alias_def(
        &mut self,
    ) -> Result<(), err::ErrKind> {
        let name_tkn = self.next_tkn(&["name"])?;
        let lex::Tkn::Name(name) = name_tkn.clone() else {
            return crate::err_at!(self.expected_tkn("type alias name", name_tkn));
        };
        // `<`
        self.next_tkn(&["<"])?;

        // `<T, U>`の型パラメータ
        let mut params = Vec::<String>::new();
        loop {
            let param_tkn = self.next_tkn(&["name"])?;
            let lex::Tkn::Name(param) = param_tkn.clone() else {
                return crate::err_at!(self.expected_tkn("type parameter name", param_tkn));
            };
            // 型パラメータ同士、エイリアス自身の名前と同じ名前は使えない
            if params.contains(&param) || param == name {
                return crate::err_at!(self.expected_tkn("unique type parameter name", param_tkn));
            }
            params.push(param);
            match self.next_tkn(&[",", ">"])? {
                lex::Tkn::Comma => continue,
                lex::Tkn::RAngleBracket => break,
                t => return crate::err_at!(self.expected_tkn(", or `>`", t)),
            }
        }

        // `=`
        let eq_tkn = self.next_tkn(&["="])?;
        if !matches!(eq_tkn, lex::Tkn::Equal) {
            return crate::err_at!(self.expected_tkn("=", eq_tkn));
        }

        // `=`の右側の型: `Name` / `Name<..>` / `*`
        let base_tkn = self.next_tkn(&["type name"])?;
        if !matches!(base_tkn, lex::Tkn::Name(_)) {
            return crate::err_at!(self.expected_tkn("type name", base_tkn));
        }
        let start = self.idx;
        if matches!(self.peek_tkn(), Ok(lex::Tkn::LAngleBracket)) {
            self.next_tkn(&["<"])?;
            let Some(gt_idx) = self.generic_args_end(self.idx) else {
                let found = self.current_tkn().clone();
                return crate::err_at!(self.expected_tkn("type arguments", found));
            };
            self.idx = gt_idx;
        }
        while matches!(self.peek_tkn(), Ok(lex::Tkn::Mul)) {
            self.next_tkn(&["*"])?;
        }
        let end = self.idx;

        let Some(tkns) = self
            .tkns
            .as_ref()
            .and_then(|tkns| tkns.get(start..=end))
            .map(|t| t.to_vec())
        else {
            return crate::err_at!(self.internal_state_err("type alias tokens"));
        };
        self.generic_aliases
            .insert(name, GenericAlias { params, tkns });
        Ok(())
    }

    /// `Alias<int, byte>`の型引数を読み込み、エイリアスの型パラメータを
    /// 置き換えた型を返す。
    /// 呼び出し時の`current_tkn()`は`<`で、終了時は`>`を指す
    ///
    /// ## Errors
    /// `name`が定義済みのジェネリクスな型エイリアスでない場合
    /// (呼び出し元で`generic_aliases`を確認しておくこと)
    pub(in crate::compiler::hexl_frontend::parse) fn generic_alias_ref(
        &mut self,
        name: &str,
    ) -> Result<TyNode, err::ErrKind> {
        let Some(alias) = self.generic_aliases.get(name).cloned() else {
            return crate::err_at!(self.internal_state_err("defined generic type alias"));
        };
        // `type A<T> = A<T>`のような循環で、無限に展開しないようにする
        if self.generic_depth >= MAX_GENERIC_DEPTH {
            return crate::err_at!(
                self.fn_not_implemented("型が増え続ける再帰的なジェネリクスな型エイリアス")
            );
        }
        // 型引数の読み込みは、ジェネリクス関数・型と共通
        let (_, ty_tkns) = self.generic_type_args(alias.params.len())?;

        let Some(first) = alias.tkns.first() else {
            return crate::err_at!(self.internal_state_err("type alias tokens"));
        };
        // `substitute_params`は先頭のトークンを置き換えない(関数名/`struct`用)ので、
        // `type Id<T> = T`の先頭の`T`も置き換えられるよう、ダミーを先頭に付ける。
        // ダミーは、型の前の区切り(`<`)と、後ろの区切り(`>`)を兼ねる
        let mut with_delims = Vec::with_capacity(alias.tkns.len() + 2);
        with_delims.push(first.clone());
        with_delims.extend(alias.tkns.iter().cloned());
        let mut tkns = Self::substitute_params(&with_delims, &alias.params, &ty_tkns);
        tkns[0].tkn = lex::Tkn::LAngleBracket;
        let mut close = tkns[0].clone();
        close.tkn = lex::Tkn::RAngleBracket;
        tkns.push(close);

        // 呼び出し元の解析の状態を退避して、置き換えたトークン列だけを型として解析する。
        // エラーでも、必ず状態を元に戻してから返す
        let saved_tkns = self.tkns.replace(tkns);
        let saved_idx = std::mem::replace(&mut self.idx, 0);
        self.generic_depth += 1;
        let result = self.ty_node_after_delim();
        self.generic_depth -= 1;
        self.idx = saved_idx;
        self.tkns = saved_tkns;
        result
    }

    // ===== 型の作成 =====

    /// 型引数を含んだ、作った構造体/バリアントの名前を作る。
    /// `Pair<int, byte>` -> `Pair$int$byte`
    pub(in crate::compiler::hexl_frontend::parse) fn mangle_generic_name(
        name: &str,
        ty_args: &[TyNode],
    ) -> String {
        let mut mangled = name.to_string();
        for ty in ty_args {
            mangled.push('$');
            mangled.push_str(&Self::mangle_ty(ty));
        }
        mangled
    }

    /// 型を、名前に使える文字列にする(`int*` -> `int_ptr`)
    fn mangle_ty(ty: &TyNode) -> String {
        match ty {
            TyNode::Ty(name) | TyNode::SelfTy(name) => name.clone(),
            TyNode::Pointer {
                is_const, ty_name, ..
            } => {
                let suffix = if *is_const { "cptr" } else { "ptr" };
                format!("{}_{}", Self::mangle_ty(ty_name), suffix)
            }
            TyNode::RefTy(inner) => Self::mangle_ty(inner),
            TyNode::Stack { name, len } => format!("{:?}_stack{}", name, len),
            TyNode::Static { name, len } => format!("{:?}_static{}", name, len),
            TyNode::ConstractMust(inner) | TyNode::ConstractOf(inner) => {
                Self::mangle_ty(&inner.unwrap_ty())
            }
            TyNode::RefContract(n) => Self::mangle_ty(&*n),
            TyNode::Auto => panic!(),
        }
    }

    /// 型パラメータを、実際の型のトークンに置き換える。
    /// ジェネリクス関数と、ジェネリクスな構造体/バリアントで共通。
    ///
    /// `x.T`や`mod::T`の`T`は、型ではなくメンバー/パスの名前なので置き換えない
    pub(in crate::compiler::hexl_frontend::parse) fn substitute_params(
        tkns: &[lex::LocatedTkn],
        params: &[String],
        ty_tkns: &[Vec<lex::LocatedTkn>],
    ) -> Vec<lex::LocatedTkn> {
        let mut out = Vec::with_capacity(tkns.len());

        for (i, t) in tkns.iter().enumerate() {
            // 先頭は関数名/`struct`/`variant`なので置き換えない
            let replaceable =
                i > 0 && !matches!(tkns[i - 1].tkn, lex::Tkn::Dot | lex::Tkn::ModPathTkn);
            if let (true, lex::Tkn::Name(n)) = (replaceable, &t.tkn) {
                if let Some(p) = params.iter().position(|p| p == n) {
                    for ty_tkn in &ty_tkns[p] {
                        // 位置情報は、置き換え前の`T`の位置にしておく
                        let mut tkn = ty_tkn.clone();
                        tkn.span = Rc::clone(&t.span);
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
        ty_args: &[TyNode],
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

        let Some(generic) = self.generic_types.get(name).cloned() else {
            return crate::err_at!(self.internal_state_err("defined generic type"));
        };
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
        let mut def = match made.next() {
            Some(def @ (Group1Node::StructDefine(_) | Group1Node::VariantDefine(_))) => def,
            _ => {
                return crate::err_at!(self.internal_state_err("generated struct/variant node"));
            }
        };
        // 定義の`pub`を引き継ぐ(解析し直すトークン列には`pub`が含まれない)
        def.set_public(generic.public);
        self.pending_types.push(def);
        Ok(mangled)
    }

    /// 解析の途中の状態を退避して、`tkns`だけを先頭から解析し、作られた
    /// ノードを返す(トップレベルの定義として解析される)。
    /// エラーでも、必ず状態を元に戻してから返す
    fn parse_tkns_isolated(
        &mut self,
        tkns: Vec<lex::LocatedTkn>,
    ) -> Result<Vec<Group1Node>, err::ErrKind> {
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

        let result = self.parse_loop::<parse_loop_control::ParseLoopPushPool>();

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
mod generic_type_tests;
