use super::*;

///  一つの変数のデータ
#[derive(Clone, PartialEq, Debug)]
pub struct VariableInfo {
    pub name: String,
    pub value: type_info::VarValue,
    pub var_type_name: Option<String>,
    pub method: Option<Vec<MethodInfo>>,
    pub multiple: Option<MultipleVar>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum MultipleVar {
    IsMut,
    IsImm,
}

#[derive(Clone, PartialEq, Debug)]
pub struct MethodInfo {
    pub name: String,
    pub node: node::CalculNode,
}

#[derive(Clone)]
pub struct VariableManager {
    // 変数のデータを保持する動的配列
    pub variables_info_vec: Vec<VariableInfo>,
    // variables_info_vecの変数のうちスタック領域に属している
    // 変数の場所を配列にしている
    pub region_stack_index: Vec<Vec<usize>>,
    // 関数の中にあるスタック領域をすべて管理
    pub local_scope: Vec<Vec<usize>>,
    // 静的領域にある変数の場所
    pub region_static_index: Vec<usize>,
}


impl VariableManager {
    pub fn new() -> Self {
        Self {
            variables_info_vec: Vec::<VariableInfo>::new(),
            region_stack_index: Vec::<Vec<usize>>::new(),
            local_scope: Vec::<Vec<usize>>::new(),
            region_static_index: Vec::<usize>::new(),
        }
    }
    /// 渡された変数の名前と合致する変数を各領域から探す
    pub fn search_var(&self, name: &String) -> Option<usize> {
        // 現在のスタック領域を探す
        for stack_pos in self.local_scope.last().unwrap() {
            for index in &self.region_stack_index[*stack_pos] {
                if self.variables_info_vec[*index].name == *name {
                    return Some(*index);
                }
            }
        }
        // 静的領域を探す
        for index in self.region_static_index.clone() {
            if self.variables_info_vec[index].name == *name {
                return Some(index);
            }
        }

        None
    }
    ///  変数のデータを返す
    pub fn get_var(&self, name: &String) -> Result<VariableInfo, err_kind::ErrorsKind> {
        if let Some(index) = self.search_var(name) {
            Ok(self.variables_info_vec[index].clone())
        } else {
            eprintln!("[err] this variable is not defined -> {}", name);
            Err(err_kind::ErrorsKind::UndefinedVariable)
        }
    }
    //  変数を追加
    pub fn add_var(
            &mut self,
            name: &String,
            value: type_info::VarValue,
            var_type_name: &Option<String>,
            region: VarRegion,
            multiple: Option<MultipleVar>,
    ) {
        if self.variables_info_vec.iter().find(|var| var.name == *name).is_some() {
            eprintln!("[name err]: variable `{}` is already defined", name);
            panic!("");
        } else {
            //  新しく作る変数の領域を設定
            match region {
                VarRegion::Heap => {},
                VarRegion::Static => {
                    self.region_static_index
                        .push(self.variables_info_vec.len() as usize);
                },
                VarRegion::Stack => {
                    self.region_stack_index
                        .last_mut()
                        .unwrap()
                        .push(self.variables_info_vec.len() as usize);
                },
            }

            // 型の情報を字列に変換
            let type_name: String = if let Some(name) = var_type_name {
                if !type_api::match_txt_to_value_type(name, &value) {
                    panic!("JJ");
                }

                name.to_string()
            } else {
                if let Ok(txt) = type_api::change_var_value_to_txt(&value) {
                    txt.to_string()
                } else {
                    panic!("ll");
                }
            };

            self.variables_info_vec.push(
                VariableInfo {
                    name: name.clone(),
                    value: value.clone(),
                    var_type_name: Some(type_name.clone()),
                    method: Some(Vec::<MethodInfo>::new()),
                    multiple: multiple,
                }
            );
        }
    }

    /// # 引数
    /// - name 変数の名前
    /// - value 変数に再代入する値
    ///
    /// 渡されたnameに対応した変数があるかつその変数
    /// が可変の場合のみOkを返す
    pub fn update_var(
            &mut self,
            name: &String,
            value: &type_info::VarValue
    ) -> Result<(), err_kind::ErrorsKind> {
        if let Some(index) = self.search_var(name) {
            // 変数が可変かを調べる
            match self.variables_info_vec[index].multiple {
                Some(MultipleVar::IsMut) => {
                    let var = &mut self.variables_info_vec[index];
                    var.value = value.clone();
                    Ok(())
                }
                // 変数が不変
                Some(MultipleVar::IsImm) => {
                    Err(err_kind::ErrorsKind::AssignmentToImmutableVariable)
                }
                None => {
                    Err(err_kind::ErrorsKind::AssignmentToImmutableVariable)
                }
            }
        } else {
            // 変数が存在しない
            Err(err_kind::ErrorsKind::UndefinedVariable)
        }
    }
    //  変数にメゾットを追加
    pub fn add_method(&mut self, node: node::CalculNode, name: String) {
        //  変数が存在しない場合エラー
        if self.variables_info_vec.last().is_none() {
            eprintln!("[name err]: variable `{}` is already defined", name);
            panic!("");
        } else {
            if let Some(last_var) = self.variables_info_vec.last_mut() {
                if let Some(method) = last_var.method.as_mut() {
                    if method.iter_mut().find(|var| var.name == name).is_none() {
                        method.push(
                            MethodInfo {
                                name: name,
                                node: node,
                            }
                        );
                    }
                }
            }
        }
    }
    //  メゾットのデータを取得
    pub fn get_method(
            &self,
            receiver_name: String,
            method_name: String
    ) -> MethodInfo {
        match self.get_var(&receiver_name) {
            Ok(info) => {
                info.method
                    .unwrap_or_else(|| {
                        panic!("this method is not found");
                    })
                    .iter()
                    .find(|m| m.name == method_name)
                    .map(|m| m.clone())
                    .unwrap_or_else(|| {
                        panic!("this method is not defined");
                    })
            }
            Err(e) => {
                match e {
                    err_kind::ErrorsKind::UndefinedVariable => {
                        panic!("this var is not found");
                    }
                    _ => panic!("err variable struct"),
                }
            },
        }
    }
}
