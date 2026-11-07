//! 変数の定義を名前から引くための木(`VarTree`)と、変数のメタデータ
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum VarType {
    Local(usize), // 変数の値や式のidx
    Param(usize), //これは、左から何番目の引数かを保存
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarLife {
    EndConstract,
    Constracting,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VarMetaData {
    pub attribute: VarType,
    pub size: TyNode,
    pub var_attr: parse::VarMutAttr,
    pub life: VarLife,
}

impl VarMetaData {
    pub(in crate::compiler::ir) fn new(
        attribute: &VarType,
        size: &TyNode,
        var_attr: &parse::VarMutAttr,
    ) -> Self {
        Self {
            attribute: attribute.clone(),
            size: size.clone(),
            var_attr: var_attr.clone(),
            life: VarLife::Constracting,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VarTree {
    hash: HashMap<String, VarMetaData>,
    /// 変数の"現在値"が定数式として分かっている場合に記録しておくための
    /// テーブル(範囲付きポインタなどの静的な範囲チェックで使用する)。
    ///
    /// `checker/tracking.rs`の`record_var_value`から更新され、
    /// `eval_expr`/`is_const_index`が変数を含む式を評価・判定する際に
    /// 参照される。値が実行時にしか分からない式で更新された場合は
    /// `forget_value`でここから取り除かれる
    values: HashMap<String, usize>,
}

/// `must`かどうかは、型に情報がある
impl VarTree {
    pub(in crate::compiler::ir) fn new() -> Self {
        Self {
            hash: HashMap::new(),
            values: HashMap::new(),
        }
    }

    /// ## K
    /// - `usize`の場合 local変数
    /// - 'l' = local
    /// - 'p' = param
    pub(in crate::compiler::ir) fn push<const K: char>(
        &mut self,
        var_name: &str,
        var_index: usize,
        var_ty: &TyNode,
        var_attr: &parse::VarMutAttr,
        span: Rc<Span>,
        mut put_flag: impl FnMut() -> Result<(), err::ErrKind>,
    ) -> Result<(), err::ErrKind> {
        let var = match K {
            'l' => VarType::Local(var_index),
            'p' => VarType::Param(var_index),
            _ => panic!("system err VarTree::AddのKには、`l`か`p`以外入れられません"),
        };
        if let Some(ref mut var_info) = self.hash.get_mut(var_name) {
            let is_constract_ty =
                var_info.size.is_contract_must() || var_info.size.is_contract_of();
            return match var_info.life {
                VarLife::Constracting if is_constract_ty => {
                    crate::GenCompileErr!(
                        ReassignActiveContract,
                        format!("{} 契約中の変数klkj", var_name),
                        span
                    )
                }
                VarLife::Constracting | VarLife::EndConstract => {
                    **var_info = VarMetaData::new(&var, &var_ty, &var_attr);
                    Ok(())
                }
            };
        }
        put_flag()?;
        self.hash.insert(
            var_name.to_owned(),
            VarMetaData::new(&var, &var_ty, &var_attr),
        );
        Ok(())
    }

    pub(in crate::compiler::ir) fn get_ty_name(
        &self,
        var_name: &str,
    ) -> Result<String, err::ErrKind> {
        if let Some(ref var) = self.hash.get(var_name) {
            let r = match &var.size {
                TyNode::Ty(name) => name.to_string(),
                TyNode::Pointer { ty_name, .. } => {
                    match &**ty_name {
                        TyNode::Ty(name) => name,
                        // `Self*`/`Self*mut`のように、`Self`を指す
                        // ポインタ型の場合も、実際の構造体名を返す
                        TyNode::SelfTy(name) => name,
                        _ => panic!(),
                    }
                    .to_string()
                }
                TyNode::SelfTy(name) => name.to_string(),
                // 契約(`must`/`of`)が付いた型は、内側の型の名前を返す
                TyNode::ConstractMust(ty) | TyNode::ConstractOf(ty) => {
                    ty.unwrap_ty().get_ty_str_name()
                }
                // 契約を借りた型(`RefContract`)も、内側の型の名前を返す
                TyNode::RefContract(_) => var.size.get_ty_str_name(),
                t => panic!("{:?}", t),
            };
            Ok(r)
        } else {
            Err(crate::GenUndefErrResult!(
                UndefVar,
                "none var".to_string(),
                Some(var_name.to_owned())
            ))
        }
    }

    #[inline(always)]
    pub(in crate::compiler) fn get_var(&self, name: &str) -> Result<VarMetaData, err::ErrKind> {
        self.hash
            .get(name)
            .ok_or_else(|| {
                crate::GenUndefErrResult!(UndefVar, "none var".to_string(), Some(name.to_owned()))
            })
            .cloned()
    }

    #[inline(always)]
    pub(in crate::compiler::ir) fn is_mut(&self, name: &str) -> parse::VarMutAttr {
        self.hash
            .get(name)
            .expect(format!("var name, {:?}", name).as_str())
            .var_attr
            .clone()
    }

    // /// 既に登録されている変数の型だけを上書きする
    // ///
    // /// 契約(`must`/`of`)が付いた変数は、値の生成自体は
    // /// 内側の型として行うため、登録後に契約付きの型へ戻すのに使う
    // /// (`src/ir/builder/expr_node.rs`の`def_var_node`)
    // pub(in crate::compiler::ir)
    // fn overwrite_ty(&mut self, var_name: &str, ty: &TyNode) {
    //     if let Some(var) = self.hash.get_mut(var_name) {
    //         var.size = ty.clone();
    //     }
    // }

    /// 契約を終わらせる
    pub(in crate::compiler::ir) fn finish_constract_var(
        &mut self,
        var_name: &str,
        span: Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        if let Some(ref mut var) = self.hash.get_mut(var_name) {
            match var.life {
                VarLife::Constracting => {
                    var.life = VarLife::EndConstract;
                }
                VarLife::EndConstract => {
                    return crate::GenCompileErr!(
                        VariableConstractExpired,
                        format!("kkkkklljjbkj {var_name}"),
                        span
                    );
                }
            }
            Ok(())
        } else {
            Err(crate::GenUndefErrResult!(
                UndefVar,
                "none var".to_string(),
                Some(var_name.to_owned())
            ))
        }
    }

    /// 契約を借りる(`RefContract`)引数へ渡す変数の契約が、まだ終了していないことを確認する
    ///
    /// 終了済みの契約は、借りることもできない(二重に終了するのと同じ扱い)。
    /// 契約を持たない変数・契約を借りているだけの変数は、検査の対象外
    pub(in crate::compiler::ir) fn ensure_contract_alive(
        &self,
        var_name: &str,
        span: Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        match self.hash.get(var_name) {
            Some(var) if var.size.is_contract_must() && var.life == VarLife::EndConstract => {
                crate::GenCompileErr!(
                    VariableConstractExpired,
                    format!("{var_name} 終了済みの契約は借りられません"),
                    span
                )
            }
            _ => Ok(()),
        }
    }

    /// 契約を借りているだけ(`RefContract`)の変数の契約を、終了しようとしたときのエラー
    ///
    /// (専用のエラー種別を`CompileErrKind`へ足したら、ここだけ差し替えればよい)
    pub(in crate::compiler::ir) fn ref_contract_cannot_end(
        var_name: &str,
        span: Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        crate::GenCompileErr!(
            VariableConstractExpired,
            format!("{var_name} 借りている契約(RefContract)は、呼び出し先では終了できません"),
            span
        )
    }

    /// 全変数の契約の状態(`life`)を記録する(条件分岐の前に呼ぶ)
    pub(in crate::compiler::ir) fn snapshot_life(&self) -> HashMap<String, VarLife> {
        self.hash
            .iter()
            .map(|(name, var)| (name.clone(), var.life.clone()))
            .collect()
    }

    /// `snapshot_life`で記録した状態へ戻す(分岐の本体を作り終えるたびに呼ぶ)。
    /// 記録後に消えた変数(分岐の中のローカル変数)は無視する
    pub(in crate::compiler::ir) fn restore_life(&mut self, snapshot: &HashMap<String, VarLife>) {
        for (name, life) in snapshot {
            if let Some(var) = self.hash.get_mut(name) {
                var.life = life.clone();
            }
        }
    }

    /// `snapshot`の時点では契約中(`Constracting`)で、今は終了済みの変数の名前
    pub(in crate::compiler::ir) fn ended_since(
        &self,
        snapshot: &HashMap<String, VarLife>,
    ) -> Vec<String> {
        snapshot
            .iter()
            .filter(|(name, life)| {
                **life == VarLife::Constracting
                    && self
                        .hash
                        .get(*name)
                        .is_some_and(|var| var.life == VarLife::EndConstract)
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// 指定された変数を、契約が終了済みの状態にする
    pub(in crate::compiler::ir) fn mark_life_ended(&mut self, names: &[String]) {
        for name in names {
            if let Some(var) = self.hash.get_mut(name) {
                var.life = VarLife::EndConstract;
            }
        }
    }

    /// 指定された変数が`must`の契約を持つかどうか
    #[inline(always)]
    pub(in crate::compiler::ir) fn is_contract_must(&self, var_name: &str) -> bool {
        self.hash
            .get(var_name)
            .is_some_and(|var| var.size.is_contract_must())
    }

    #[inline(always)]
    pub(in crate::compiler::ir) fn get_ty_node(&self, var_name: &str) -> Option<TyNode> {
        self.hash.get(var_name).map(|var| var.size.clone())
    }

    /// 指定された変数が`Self`型、または`Self`を指すポインタ型
    /// (`Self*` / `Self*mut`)かどうかを判定する
    pub(in crate::compiler::ir) fn is_self_ty(&self, var_name: &str) -> bool {
        let mut b = move |v: &VarMetaData| match &v.size {
            TyNode::SelfTy(_) => true,
            TyNode::Pointer { ty_name, .. } => {
                matches!(**ty_name, TyNode::SelfTy(_))
            }
            _ => false,
        };
        self.hash.get(var_name).is_some_and(&mut b)
    }

    #[inline(always)]
    pub(in crate::compiler::ir) fn all_var_is_end_constract(
        &self,
        state: &VarTree,
        span: Rc<Span>,
    ) -> Result<(), err::ErrKind> {
        for (name, var) in &self.hash {
            if state.hash.contains_key(name)
                || !var.size.is_contract_must()
                || var.life != def_tree::VarLife::Constracting
            {
                continue;
            } else {
                return crate::GenCompileErr!(VariableConstractExpired, name, span);
            }
        }
        Ok(())
    }

    pub(in crate::compiler::ir) fn contains_key(&mut self, state: &VarTree) {
        self.hash.retain(|name, _| state.hash.contains_key(name));
    }

    /// `get`と同じだが、登録されていない変数でもpanicせず`None`を返す
    pub(in crate::compiler::ir) fn find(&self, name: &str) -> Option<&VarType> {
        self.hash.get(name).map(|var| &var.attribute)
    }

    /// 指定された変数が引数か、ローカル変数かなどを返す
    pub(in crate::compiler::ir) fn get(&self, name: &str) -> &VarType {
        &self.hash.get(name).expect(name).attribute
    }

    /// 変数の"現在値"が定数式として分かっている場合に記録する
    ///
    /// 範囲付きポインタへの再代入(`a += 10`など)を静的にチェックする際、
    /// 右辺の式が変数自身を参照していても値を評価できるようにするために使う
    pub(in crate::compiler::ir) fn record_value(&mut self, name: &str, value: usize) {
        self.values.insert(name.to_owned(), value);
    }

    /// `record_value`で記録された、変数の現在値を取得する
    ///
    /// まだ一度も定数値として記録されていない変数や、実行時にしか
    /// 値が分からない式で更新された変数は`None`を返す
    pub(in crate::compiler::ir) fn get_value(&self, name: &str) -> Option<usize> {
        self.values.get(name).copied()
    }

    /// 記録済みの変数の値を消す
    ///
    /// 変数が実行時にしか値の分からない式で更新された場合、古い記録が
    /// 残ったまま以降の静的チェックに誤って使われないようにするために呼ぶ
    pub(in crate::compiler::ir) fn forget_value(&mut self, name: &str) {
        self.values.remove(name);
    }
}
