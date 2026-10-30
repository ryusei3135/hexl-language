//! `#include`先のジェネリクス関数を、呼び出し側のファイルで実体化する。
//!
//! `#include`先のファイルは、IR側が単独で`build`するため、ジェネリクス関数
//! (`pub malloc<T>(..)`)は呼び出されるまで実体ができない。そこで:
//! 1. `parser`の最初に、`#include`先のファイルを字句解析して、公開された
//!    ジェネリクス関数の定義を集める(`collect_included_generics`)。
//! 2. `mod::func<int>(..)`の呼び出しが来たら、型パラメータを置き換えた関数を
//!    呼び出し側のファイルに作り、モジュール名(`mod`)を付ける
//!    (C言語のファイルの関数と同じ扱い)。
//!
//! ## 制限
//! - 関数の本体は呼び出し側のファイルとして解析される。そのため本体の中から
//!   include先の非公開の関数や型を使っていると、解決できない。
//! - include先がさらに`#include`しているファイルのジェネリクスは集めない。
//! - 対応するのは関数のみ(include先の構造体/バリアントのジェネリクスは未対応)。

use super::*;
use crate::compiler::hexl_frontend::include_root;
use std::collections::HashMap;
use std::rc::Rc;

impl Parser {
    /// 最上位にある全ての`#include`を調べ、その先の公開ジェネリクス関数を集める
    pub(super) fn collect_included_generics(&mut self) -> Result<(), err::ErrKind> {
        let Some(tkns) = self.tkns.as_ref() else {
            return crate::err_at!(self.tkn_is_eof(Vec::new()));
        };
        // `#`の位置(最上位の`#include`だけ)
        let mut heads = Vec::<usize>::new();
        let mut depth = 0usize;
        for (i, t) in tkns.iter().enumerate() {
            match &t.tkn {
                lex::Tkn::LBrace => depth += 1,
                lex::Tkn::RBrace => depth = depth.saturating_sub(1),
                lex::Tkn::CompleSyn
                    if depth == 0
                        && matches!(
                            tkns.get(i + 1).map(|t| &t.tkn),
                            Some(lex::Tkn::Name(n)) if n == "include"
                        ) =>
                {
                    heads.push(i)
                }
                _ => {}
            }
        }

        let saved_idx = self.idx;
        let mut paths = Vec::<ModPath>::new();
        for head in heads {
            self.idx = head;
            let node = self.comple_syntax();
            match node {
                Ok(Group2Node::Include(path)) => paths.push(path),
                Ok(_) => {}
                Err(e) => {
                    self.idx = saved_idx;
                    return Err(e);
                }
            }
        }
        self.idx = saved_idx;

        for path in paths {
            self.collect_generics_of_include(&path);
        }
        Ok(())
    }

    /// 1つの`#include`の先から、公開ジェネリクス関数を集める。
    /// ファイルが読めない/解析できない場合は何もしない
    /// (そのエラーは、IR側の`build`が報告する)
    fn collect_generics_of_include(&mut self, path: &ModPath) {
        // (ファイルのパス, 取り込み方)を、IR側(`include_proc`)と同じ規則で決める
        let full = include_root::resolve_for(path, &path.gen_path());
        let (file, kind): (String, ImportKind) = match &path.kind {
            ImportKind::Auto => {
                if std::path::Path::new(&full).exists() {
                    (full, ImportKind::Module(Some(path.last_segment())))
                } else {
                    let Ok(parent) = path.gen_parent_path() else {
                        return;
                    };
                    let parent_full = include_root::resolve_for(path, &parent);
                    // どちらも無いなら、両方の探した場所を示して止める
                    // (`std::io`と書いて`library/io.hexl`が無い場合など)
                    if !std::path::Path::new(&parent_full).exists() {
                        panic!(
                            "#includeで指定されたファイルが見つかりません: {} \
                            (関数名として`{}`も試しましたが、ファイル{}も見つかりませんでした)",
                            full,
                            path.last_segment(),
                            parent_full
                        );
                    }
                    (parent_full, ImportKind::Func(path.last_segment()))
                }
            }
            ImportKind::Module(alias) => {
                let module = alias.clone().unwrap_or_else(|| {
                    std::path::Path::new(&path.gen_path())
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                        .to_string()
                });
                (full, ImportKind::Module(Some(module)))
            }
            other => (full, other.clone()),
        };
        // C言語のファイルのジェネリクスは無い
        if file.ends_with(".c") || file.ends_with(".h") {
            return;
        }
        // ファイルが無いのに黙って読み飛ばすと、`mod::func<int>(..)`が
        // ジェネリクス関数として認識されず、`<`が予期しないトークンになる
        // という分かりにくい構文エラーになる。IR側(`include_proc`)と同じく、
        // 探した場所を示して止める
        if !std::path::Path::new(&file).exists() {
            panic!(
                "#includeで指定されたファイルが見つかりません: {} (`std`は、hexl本体のあるディレクトリの`library/`を探します)",
                file
            );
        }
        let Ok(content) = std::fs::read_to_string(&file) else {
            return;
        };
        let mut lexer = lex::Lexer::new(&Rc::new(file.clone()));
        if lexer.analy(&content).is_err() {
            return;
        }

        // include先のトークン列で、定義を集める。今のファイルの状態は
        // 必ず元に戻す
        // 最上位の関数名(本体の呼び出しを`mod::f`に直すために使う)
        let mut fn_names = Self::top_level_fn_names(&lexer.gen_tkns);
        let private_fns = Self::private_fn_defs(&lexer.gen_tkns);
        let saved_tkns = self.tkns.replace(lexer.gen_tkns);
        let saved_funcs = std::mem::take(&mut self.generic_funcs);
        let saved_types = std::mem::take(&mut self.generic_types);
        let collected = self
            .collect_generic_types()
            .and_then(|_| self.collect_generic_funcs());
        let found = std::mem::replace(&mut self.generic_funcs, saved_funcs);
        // include先のジェネリクスな型も、`mod::Type<int>`で使えるように残す
        let found_types = std::mem::replace(&mut self.generic_types, saved_types);
        self.tkns = saved_tkns;
        if collected.is_err() {
            return;
        }

        fn_names.extend(found.keys().cloned());
        // 使えるのは公開された関数だけ
        let found: HashMap<String, GenericFunc> =
            found.into_iter().filter(|(_, g)| g.is_public).collect();

        match kind {
            ImportKind::Module(Some(module)) => {
                self.included_private_fns
                    .entry(module.clone())
                    .or_default()
                    .extend(private_fns);
                self.included_fn_names
                    .entry(module.clone())
                    .or_default()
                    .extend(fn_names);
                self.included_generic_types
                    .entry(module.clone())
                    .or_default()
                    .extend(found_types);
                self.included_generics
                    .entry(module)
                    .or_default()
                    .extend(found);
            }
            // `#include "file"::func`: モジュール名を付けず、その関数だけ
            ImportKind::Func(func_name) => {
                if let Some(g) = found.get(&func_name) {
                    self.generic_funcs
                        .entry(func_name.clone())
                        .or_insert_with(|| g.clone());
                }
                if let Some(t) = found_types.get(&func_name) {
                    self.generic_types
                        .entry(func_name)
                        .or_insert_with(|| t.clone());
                }
            }
            // `#include "file"::*`: モジュール名を付けず、全て
            ImportKind::Glob => {
                for (name, g) in found {
                    self.generic_funcs.entry(name).or_insert(g);
                }
                for (name, t) in found_types {
                    self.generic_types.entry(name).or_insert(t);
                }
            }
            ImportKind::Module(None) | ImportKind::Auto => {}
        }
    }

    /// `module::name`が、`#include`先のジェネリクスな型なら、現在のファイルの
    /// `generic_types`へ取り込んで`true`を返す(同名の型が既にあれば、そちらを使う)。
    /// 取り込んだ後は、そのファイルで定義した型と同じ方法(`Name<int>`)で実体化できる。
    ///
    /// 実体化した型は、モジュール名のない名前(`Vec$int`)で、呼び出し側のファイルに作られる
    pub(super) fn import_included_generic_type(&mut self, module: &str, name: &str) -> bool {
        let Some(def) = self
            .included_generic_types
            .get(module)
            .and_then(|types| types.get(name))
            .cloned()
        else {
            return false;
        };
        self.generic_types.entry(name.to_string()).or_insert(def);
        true
    }

    /// 最上位にある関数の名前(`name(`で始まる定義)を集める
    fn top_level_fn_names(tkns: &[lex::LocatedTkn]) -> std::collections::HashSet<String> {
        let mut names = std::collections::HashSet::new();
        let mut depth = 0usize;
        for (i, t) in tkns.iter().enumerate() {
            match &t.tkn {
                lex::Tkn::LBrace => depth += 1,
                lex::Tkn::RBrace => depth = depth.saturating_sub(1),
                lex::Tkn::Name(n)
                    if depth == 0
                        && matches!(tkns.get(i + 1).map(|t| &t.tkn), Some(lex::Tkn::LParen)) =>
                {
                    names.insert(n.clone());
                }
                _ => {}
            }
        }
        names
    }

    /// 公開されていない(`pub`が付かない)最上位の通常の関数の定義を集める。
    /// 定義は、関数名から本体を閉じる`}`までのトークン
    fn private_fn_defs(tkns: &[lex::LocatedTkn]) -> HashMap<String, GenericFunc> {
        let mut defs = HashMap::new();
        let mut depth = 0usize;
        let mut i = 0usize;
        while i < tkns.len() {
            match &tkns[i].tkn {
                lex::Tkn::LBrace => depth += 1,
                lex::Tkn::RBrace => depth = depth.saturating_sub(1),
                lex::Tkn::Name(n)
                    if depth == 0
                        && matches!(tkns.get(i + 1).map(|t| &t.tkn), Some(lex::Tkn::LParen))
                        && (i == 0 || tkns[i - 1].tkn != lex::Tkn::KeyWordPub) =>
                {
                    // 本体を開く`{`と、閉じる`}`を探す
                    let Some(open) = tkns[i..]
                        .iter()
                        .position(|t| t.tkn == lex::Tkn::LBrace)
                        .map(|p| i + p)
                    else {
                        break;
                    };
                    let mut d = 0usize;
                    let mut end = open;
                    while end < tkns.len() {
                        match tkns[end].tkn {
                            lex::Tkn::LBrace => d += 1,
                            lex::Tkn::RBrace => {
                                d -= 1;
                                if d == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        end += 1;
                    }
                    if end >= tkns.len() {
                        break;
                    }
                    defs.insert(
                        n.clone(),
                        GenericFunc {
                            is_public: false,
                            params: Vec::new(),
                            tkns: tkns[i..=end].to_vec(),
                        },
                    );
                    i = end;
                }
                _ => {}
            }
            i += 1;
        }
        defs
    }

    /// 実体化する本体(`mod::f(..)`に書き換え済み)から呼ばれている、
    /// 公開されていない関数を、呼び出し側のファイルに複製する
    pub(super) fn instantiate_private_callees(
        &mut self,
        module: &str,
        tkns: &[lex::LocatedTkn],
    ) -> Result<(), err::ErrKind> {
        let Some(privs) = self.included_private_fns.get(module) else {
            return Ok(());
        };
        let mut called = Vec::<(String, GenericFunc)>::new();
        for w in tkns.windows(3) {
            if let (lex::Tkn::Name(m), lex::Tkn::ModPathTkn, lex::Tkn::Name(n)) =
                (&w[0].tkn, &w[1].tkn, &w[2].tkn)
            {
                if m == module && !called.iter().any(|(c, _)| c == n) {
                    if let Some(def) = privs.get(n) {
                        called.push((n.clone(), def.clone()));
                    }
                }
            }
        }
        for (n, def) in called {
            self.instantiate_generic_def(&n, Some(module), def, &[], &[])?;
        }
        Ok(())
    }

    /// 実体化する本体のトークン列の中で、`#include`先の関数の呼び出し
    /// `f(..)` / `f<T>(..)`を、`mod::f(..)`に書き換える。
    /// 先頭(定義の関数名)と、`.`/`::`の直後の名前は書き換えない
    pub(super) fn qualify_included_calls(
        &self,
        module: &str,
        tkns: Vec<lex::LocatedTkn>,
    ) -> Vec<lex::LocatedTkn> {
        let generics = self.included_generics.get(module);
        let fns = self.included_fn_names.get(module);
        let mut out = Vec::with_capacity(tkns.len());
        for (i, t) in tkns.iter().enumerate() {
            let target = match &t.tkn {
                lex::Tkn::Name(n)
                    if i > 0
                        && !matches!(tkns[i - 1].tkn, lex::Tkn::Dot | lex::Tkn::ModPathTkn)
                        && (fns.is_some_and(|f| f.contains(n))
                            || generics.is_some_and(|g| g.contains_key(n))) =>
                {
                    matches!(
                        tkns.get(i + 1).map(|t| &t.tkn),
                        Some(lex::Tkn::LParen | lex::Tkn::LAngleBracket)
                    )
                }
                _ => false,
            };
            if target {
                out.push(lex::LocatedTkn {
                    tkn: lex::Tkn::Name(module.to_string()),
                    span: Rc::clone(&t.span),
                });
                out.push(lex::LocatedTkn {
                    tkn: lex::Tkn::ModPathTkn,
                    span: Rc::clone(&t.span),
                });
            }
            out.push(t.clone());
        }
        out
    }

    /// `module::name<..>(`が、`#include`先のジェネリクス関数の呼び出しか判定する。
    ///
    /// ## Args
    /// - lt_idx `<`のトークンのインデックス
    pub(super) fn is_included_generic_call(&self, module: &str, name: &str, lt_idx: usize) -> bool {
        self.included_generics
            .get(module)
            .is_some_and(|m| m.contains_key(name))
            && matches!(
                self.tkns
                    .as_ref()
                    .and_then(|t| t.get(lt_idx))
                    .map(|t| &t.tkn),
                Some(lex::Tkn::LAngleBracket)
            )
            && self.is_generic_args_then_lparen(lt_idx)
    }

    /// `#include`先のジェネリクス関数を呼び出すノード(`name<int>(args)`の部分)を
    /// 作成する。`mod::`は呼び出し元で`Scope`として包む。
    ///
    /// 呼び出し時の`current_tkn()`は`<`で、終了時は`generic_call_expr`と同じく、
    /// 呼び出しの`)`の次を指す
    pub(super) fn included_generic_call_expr(
        &mut self,
        module: &str,
        name: &str,
        ini_struct: bool,
    ) -> Result<Expr, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LAngleBracket {
            return crate::err_at!(self.fn_unexpect_tkn::<Expr>());
        }
        let Some(generic) = self
            .included_generics
            .get(module)
            .and_then(|m| m.get(name))
            .cloned()
        else {
            return crate::err_at!(self.internal_state_err("included generic function"));
        };

        let (ty_args, ty_tkns) = self.generic_type_args(generic.params.len())?;

        // `>`の次は`(`
        if self.next_tkn(&["("])? != lex::Tkn::LParen {
            return crate::err_at!(self.not_found_lparen());
        }

        self.instantiate_generic_def(name, Some(module), generic, &ty_args, &ty_tkns)?;

        let mut call = self.call_func_expr(name, ini_struct)?;
        if let ExprNode::CallFunc(ref mut info) = call.node {
            info.temp_ty = ty_args;
        }
        Ok(call)
    }
}
