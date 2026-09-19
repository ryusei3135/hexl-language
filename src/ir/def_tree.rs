use crate::{
    err::{
        compile::{
            CompileErr
        }
    }
};

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
    pub size: node::TyNode,
    pub is_mut: bool,
    pub life: VarLife,
}

impl VarMetaData {
    pub fn new(
        attribute: &VarType, 
        size: &node::TyNode,
        is_mut: &bool   
    ) -> Self {
        Self {
            attribute: attribute.clone(),
            size: size.clone(),
            is_mut: *is_mut,
            life: VarLife::Constracting,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VarTree {
    pub hash: HashMap<String, VarMetaData>,
}

impl VarTree {
    pub fn new() -> Self {
        Self {
            hash: HashMap::new(),
        }
    }

    /// ## K
    /// - `usize`の場合 local変数
    /// - 'l' = local
    /// - 'p' = param
    pub fn push<const K: char>(
        &mut self,
        var_name: &String,
        var_index: &usize,
        var_ty: &node::TyNode,
        is_mut: &bool,
    ) -> Result<(), err::ErrKind> {
        let var = match K {
            'l' => VarType::Local(*var_index),
            'p' => VarType::Param(*var_index),
            _ => panic!("system err VarTree::AddのKには、`l`か`p`以外入れられません"),
        };
        if let Some(ref var_info) = self.hash
            .get(var_name) 
        {
            return match var_info.life {
                VarLife::Constracting => {
                    crate::GenCompileErr!(
                        ReassignActiveContract, 
                        format!("{} 契約中の変数klkj", var_name)
                    )
                }
                VarLife::EndConstract => {
                    *self.hash
                        .get_mut(var_name)
                        .unwrap()
                        = VarMetaData::new(
                            &var, 
                            &var_ty, 
                            &is_mut
                        );
                    Ok(())
                }
            };
        }
        self.hash
            .insert(
                var_name.clone(), 
                VarMetaData::new(&var, &var_ty, &is_mut)
            );
        Ok(())
    }

    pub fn get_ty_name(
        &self, 
        name: &String
    ) -> String {
        match &self.hash.get(name).unwrap().size {
            node::TyNode::Ty(name) => name.to_string(),
            node::TyNode::Pointer { ty_name, .. } => {
                match &**ty_name {
                    node::TyNode::Ty(name) => name.to_string(),
                    // `Self*`/`Self*mut`のように、`Self`を指す
                    // ポインタ型の場合も、実際の構造体名を返す
                    node::TyNode::SelfTy(name) => name.to_string(),
                    _ => panic!(),
                }
            }
            node::TyNode::SelfTy(name) => name.to_string(),
            // 契約(`must`/`of`)が付いた型は、内側の型の名前を返す
            node::TyNode::ConstractMust(ty) 
            | node::TyNode::ConstractOf(ty) => {
                ty.unwrap_ty().get_ty_str_name()
            }
            t => panic!("{:?}", t),
        }
    }

    #[inline(always)]
    pub fn is_mut(&self, name: &String) -> bool {
        self.hash.get(name).unwrap().is_mut
    }

    /// 既に登録されている変数の型だけを上書きする
    ///
    /// 契約(`must`/`of`)が付いた変数は、値の生成自体は
    /// 内側の型として行うため、登録後に契約付きの型へ戻すのに使う
    /// (`src/ir/builder/expr_node.rs`の`def_var_node`)
    pub fn overwrite_ty(
        &mut self, 
        var_name: &String, 
        ty: &node::TyNode
    ) {
        if let Some(var) = self.hash.get_mut(var_name) {
            var.size = ty.clone();
        }
    }

    /// 契約を終わらせる
    pub(in crate::ir) fn finish_constract_var(
        &mut self,
        var_name: &String,
    ) -> Result<(), err::ErrKind> {
        let life: &VarLife = &self.hash
            .get(var_name)
            .unwrap()
            .life;
        match life {
            VarLife::Constracting => {
                self.hash
                    .get_mut(var_name)
                    .unwrap().life = VarLife::EndConstract;
            }
            VarLife::EndConstract => {
                return crate::GenCompileErr!(
                    VariableConstractExpired, 
                    format!("kkkkklljjbkj {var_name}")
                );
            }
        }
        Ok(())
    }

    /// 指定された変数が`must`の契約を持つかどうか
    #[inline(always)]
    pub fn is_constract_must(&self, var_name: &String) -> bool {
        self.hash
            .get(var_name)
            .is_some_and(|var| var.size.is_constract_must())
    }

    #[inline(always)]
    pub fn get_ty_node(
        &self, 
        var_name: &String
    ) -> Option<node::TyNode> {
        self.hash
            .get(var_name)
            .map(|var| var.size.clone())
    }

    /// 指定された変数が`Self`型、または`Self`を指すポインタ型
    /// (`Self*` / `Self*mut`)かどうかを判定する
    pub fn is_self_ty(&self, name: &String) -> bool {
        self.hash.get(name).is_some_and(|v| match &v.size {
            node::TyNode::SelfTy(_) => true,
            node::TyNode::Pointer { ty_name, .. } => {
                matches!(**ty_name, node::TyNode::SelfTy(_))
            }
            _ => false,
        })
    }

    /// 指定された変数が引数か、ローカル変数かなどを返す
    pub fn get(&self, name: &String) -> &VarType {
        &self.hash
            .get(name)
            .expect(name)
            .attribute
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructTree {
    tree: HashMap<String, node::StructDefine>,
}

impl StructTree {
    pub fn new() -> Self {
        Self {
            tree: HashMap::new(),
        }
    }

    pub fn add(&mut self, info: &node::StructDefine) {
        self.tree.insert(info.name.to_string(), info.clone());
    }

    pub fn get(&self, name: &String) -> Option<&node::StructDefine> {
        self.tree.get(name)
    }

    /// 任意の構造体を指定し、その構造体にあるメンバー
    /// のバイトの位置を取得し返す関数
    pub fn get_pos(
        &self,
        // 構造体の名前
        name: &String,
        field_name: &String,
    ) -> usize {
        let mut byte_counter = 0;
        for member in self.tree.get(name).expect(name).fields.iter() {
            byte_counter += types::Size::new(&member.ty)
                .unwrap()
                .to_bytes();
            if member.name == field_name.as_str() {
                return byte_counter;
            }
        }

        panic!();
    }

    pub fn get_mem_size(
        &self,
        name: &String,
        field_name: &String,
    ) -> types::Size {
        let ty = self.tree
            .get(name)
            .unwrap()
            .fields
            .iter()
            .find(|v| &v.name == field_name)
            .unwrap()
            .ty
            .clone();
        types::Size::new(&ty).unwrap()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncDefInfo {
    pub module: Option<String>,
    pub args: Vec<node::ArgsNode>,
    pub body: Vec<inst::Inst>,
    pub ret_ty: Option<node::TyNode>,
    pub temp_ty: Vec<node::TyNode>,
    pub public: bool,
    pub stk_size: usize,
}

impl FuncDefInfo {
    /// 第一引数が`Self`型、または`Self`を指すポインタ型
    /// (`self: Self` / `self: Self*` / `self: Self*mut`)かどうかを判定する
    pub fn first_param_is_self(&self) -> bool {
        self.args.get(0).is_some_and(|f| match &f.ty {
            node::TyNode::SelfTy(..) => true,
            node::TyNode::Pointer { ty_name, .. } => {
                matches!(**ty_name, node::TyNode::SelfTy(..))
            }
            _ => false,
        })
    }

    pub fn get_ret_ty(&self) -> crate::gen::SelfPtrInfo {
        if self.ret_ty.is_none() {
            return types::Size::Void.wrap_dst_size();
        }
        match self.ret_ty.as_ref().unwrap() {
            node::TyNode::SelfTy(..) => None,
            node::TyNode::Ty(name)
                if !types::Size::is_builtin_ty_name(name) => None,
            ty => Some(types::Size::new(ty).unwrap()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncTree {
    pub func: HashMap<String, FuncDefInfo>,
}

impl FuncTree {
    pub fn new() -> Self {
        Self {
            func: HashMap::new(),
        }
    }

    /// `func`ハッシュマップに登録する際のキーを生成する
    ///
    /// 構造体のメゾットを展開した関数など、モジュール名を持つ
    /// 関数は`モジュール名::関数名`をキーにする(`StructName::method`の
    /// ように同名のメゾットが別の構造体にあっても衝突しないようにする)。
    /// モジュール名を持たない、通常の(トップレベルの)関数は
    /// そのまま関数名だけをキーにする
    fn make_key(
        name: &String,
        module_name: Option<&String>,
        temp_ty: &[node::TyNode],
    ) -> String {
        let base = match module_name {
            Some(module) => format!("{}::{}", module, name),
            None => name.clone(),
        };
        if temp_ty.is_empty() {
            base
        } else {
            format!("{}::<{:?}>", base, temp_ty)
        }
    }

    pub fn get(
        &self, 
        name: &String, 
        module_name: Option<&String>
    ) -> Option<FuncDefInfo> {
        let no_temp_ty: &[node::TyNode] = &[];
        self.get_with_temp(name, module_name, no_temp_ty)
    }

    pub fn get_with_temp(
        &self,
        name: &String,
        module_name: Option<&String>,
        temp_ty: &[node::TyNode],
    ) -> Option<FuncDefInfo> {
        self.func
            .get(&Self::make_key(name, module_name, temp_ty))
            .cloned()
    }

    pub fn add(
        &mut self,
        body: Vec<inst::Inst>,
        meta_data: &node::FuncDefine,
        ret_ty: &node::TyNode,
        stk_size: usize,
    ) {
        let key = Self::make_key(
            &meta_data.name, 
            meta_data.module.as_ref(),
            &meta_data.temp_ty,
        );
        self.func.insert(
            key,
            FuncDefInfo {
                // 構造体のメゾットとして展開された関数の場合、
                // 属している構造体の名前がここに入る
                module: meta_data.module.clone(),
                args: meta_data.params.clone(),
                body,
                ret_ty: Some(ret_ty.clone()),
                temp_ty: meta_data.temp_ty.clone(),
                public: meta_data.public,
                stk_size,
            },
        );
    }

    pub fn declare(&mut self, meta_data: &node::FuncDefine) {
        let key = Self::make_key(
            &meta_data.name,
            meta_data.module.as_ref(),
            &meta_data.temp_ty,
        );
        self.func.entry(key).or_insert_with(|| FuncDefInfo {
            module: meta_data.module.clone(),
            args: meta_data.params.clone(),
            body: Vec::new(),
            ret_ty: Some(meta_data.ret_ty.clone()),
            temp_ty: meta_data.temp_ty.clone(),
            public: meta_data.public,
            stk_size: 0,
        });
    }
}

#[derive(Clone, Debug)]
pub struct FuncDefMetaData {
    module: Option<String>,
    pub name: String,
    params: Vec<node::ArgsNode>,
    ret_ty: Option<node::TyNode>,
    // `#include`で他のファイルから読み込む際に、公開(pub)された
    // 関数かどうかを判定するために使う
    pub public: bool,
}

impl FuncDefMetaData {
    /// moduleは自分自身がどのモジュールに属しているか
    /// Noneの場合は、#includeで関数の名前ごと指定しているか
    /// 自分のファイルの中にあるかのどちらか
    pub fn new(
        info: &node::FuncDefine, 
        module: Option<&String>
    ) -> Self {
        Self {
            module: module.map(|v| v.clone()),
            name: info.name.clone(),
            params: info.params.clone(),
            ret_ty: Some(info.ret_ty.clone()),
            public: info.public,
        }
    }

    pub fn add_self_module_name(&mut self, self_name: &String) {
        self.module = Some(self_name.to_string());
    }

    /// この関数がどのモジュール名で登録されているかを返す
    /// (`#include`でモジュール名を指定せず取り込んだ関数は`None`)
    pub fn module(&self) -> Option<&String> {
        self.module.as_ref()
    }

    pub fn gen(&self, stk_size: usize) -> FuncDefInfo {
        FuncDefInfo {
            module: None,
            args: self.params.clone(),
            body: Vec::new(),
            ret_ty: self.ret_ty.clone(),
            temp_ty: Vec::new(),
            public: true,
            stk_size,
        }
    }
}
