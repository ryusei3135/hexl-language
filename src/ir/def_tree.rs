use super::*;


#[derive(Clone, Debug, PartialEq)]
pub enum VarType {
    Local(usize),// 変数の値や式のidx
    Param(usize),//これは、左から何番目の引数かを保存
}

#[derive(Clone, Debug, PartialEq)]
pub struct VarMetaData {
    pub attribute: VarType,
    pub size: node::TyNode,
}

impl VarMetaData {
    pub fn new(attribute: &VarType, size: &node::TyNode) -> Self {
        Self {
            attribute: attribute.clone(),
            size: size.clone(),
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
    ) {
        let var = match K {
            'l' => {
                VarType::Local(*var_index)
            }
            'p' => {
                VarType::Param(*var_index)
            }
            _ => panic!("system err VarTree::AddのKには、`l`か`p`以外入れられません"),
        };
        self.hash.insert(var_name.clone(), VarMetaData::new(&var, &var_ty));
    }

    pub fn get_ty_name(&self, name: &String) -> String {
        match &self.hash.get(name).unwrap().size {
            node::TyNode::Ty(name) => name.to_string(),
            t => panic!("{:?}", t),
        }
    }

    /// 指定された変数が引数か、ローカル変数かなどを返す
    pub fn get(&self, name: &String) -> &VarType {
        &self.hash.get(name).expect(name).attribute
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct StructTree {
    tree: HashMap<String, node::StructDefine>
}

impl StructTree {
    pub fn new() -> Self {
        Self {
            tree: HashMap::new()
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
        field_name: &String
    ) -> usize {
        let mut byte_counter = 0;
        for member in self.tree.get(name)
            .expect(name)
            .fields
            .iter() 
        {
            byte_counter += types::Size::new(&member.ty).to_bytes();
            if member.name == field_name.as_str() {
                return byte_counter;
            }
        }

        panic!();
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct FuncDefInfo {
    pub module: Option<String>,
    pub args: Vec<node::ArgsNode>,
    pub body: Vec<inst::Inst>,
    pub ret_ty: Option<node::TyNode>,
    pub public: bool,
    pub stk_size: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncTree{
    pub func: HashMap<String, FuncDefInfo>
}

impl FuncTree {
    pub fn new() -> Self {
        Self {
            func: HashMap::new(),
        }
    }

    pub fn get(&mut self, name: &String, module_name: Option<&String>) -> Option<FuncDefInfo> {
        if let Some(func) = self.func.get(name) {
            // 指定された関数がモジュールに入っていないかつ、自分も
            // モジュールを指定していない場合そのまま関数のデータを返す
            if module_name.is_none() && func.module.is_none() {
                return Some(func.clone());
            }

            if func.module == module_name.map(|v| v.to_string()) {
                Some(func.clone())
            } else {
                panic!("not found this module in {}", name);
            }
        } else {
            None
        }
    }
    
    pub fn add(
        &mut self,
        body: Vec<inst::Inst>,
        meta_data: &node::FuncDefine,
        ret_ty: &node::TyNode,
        stk_size: usize,
    ) {
        self.func.insert(
            meta_data.name.clone(),
            FuncDefInfo {
                module: None,
                args: meta_data.params.clone(),
                body,
                ret_ty: Some(ret_ty.clone()),
                public: meta_data.public,
                stk_size,
            }
        );
    }
}



#[derive(Clone, Debug)]
pub struct FuncDefMetaData {
    module: Option<String>,
    pub name: String,
    params: Vec<node::ArgsNode>,
    ret_ty: Option<node::TyNode>,
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
        }
    }

    pub fn add_self_module_name(&mut self, self_name: &String) {
        self.module = Some(self_name.to_string());
    }

    pub fn gen(&self, stk_size: usize) -> FuncDefInfo {
        FuncDefInfo {
            module: None,
            args: self.params.clone(),
            body: Vec::new(),
            ret_ty: self.ret_ty.clone(),
            public: true,
            stk_size 
        }
    }
}
