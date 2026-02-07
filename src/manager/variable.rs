use super::*;

///  一つの変数のデータ
#[derive(Clone)]
pub struct VariableInfo {
    pub name: String,
    pub value: type_info::VarValue,
    pub method: Option<Vec<MethodInfo>>,
}

#[derive(Clone)]
pub struct MethodInfo {
    pub name: String,
    pub node: node::CalculNode,
}

pub struct VariableManager {
    // 変数のデータを保持する動的配列
    pub variables_info_vec: Vec<VariableInfo>,
    // variables_info_vecの変数のうちスタック領域に属している
    // 変数の場所を配列にしている
    pub region_stack_index: Vec<Vec<usize>>,
    // 静的領域にある変数の場所
    pub region_static_index: Vec<usize>,
}


impl VariableManager {
    pub fn new() -> Self {
        Self {
            variables_info_vec: Vec::<VariableInfo>::new(),
            region_stack_index: Vec::<Vec<usize>>::new(),
            region_static_index: Vec::<usize>::new(),
        }
    }
    /// 渡された変数の名前と合致する変数を各領域から探す
    pub fn search_var(&self, name: &String) -> Option<usize> {
        // 現在のスタック領域を探す
        for index in self.region_stack_index.last().unwrap() {
            if self.variables_info_vec[*index].name == *name {
                return Some(*index);
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
    /// 新しくスタック領域を作成
    pub fn make_new_stack(&mut self) {
        self.region_stack_index.push(Vec::<usize>::new());
    }
    /// スタック領域を削除
    pub fn remove_stack(&mut self) {
        if let Some(index_vec) = self.region_stack_index.pop() {
            for index in index_vec.iter().rev() {
                self.variables_info_vec.remove(*index);
            }
        }
    }
    ///  変数のデータを返す
    pub fn get_var(&self, name: String) -> Result<VariableInfo, define_msg::VarErrorOrLog> {
        if let Some(index) = self.search_var(&name) {
            Ok(self.variables_info_vec[index].clone())
        } else {
            eprintln!("[err] this variable is not defined -> {}", name);
            Err(define_msg::VarErrorOrLog::VarIsNotDefined)
        }
    }
    //  変数を追加
    pub fn add_var(
            &mut self,
            name: String,
            value: type_info::VarValue,
            region: VarRegion,
    ) {
        if self.variables_info_vec.iter().find(|var| var.name == name).is_some() {
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

            self.variables_info_vec.push(
                VariableInfo {
                    name: name,
                    value: value,
                    method: Some(Vec::<MethodInfo>::new()),
                }
            );
        }
    }
    //  変数の値を上書き
    pub fn update_var(&mut self, name: String, value: type_info::VarValue) -> bool {
        if let Some(index) = self.search_var(&name) {
            let var = &mut self.variables_info_vec[index];
            var.value = value;
            return true;
        } else {
            eprintln!("[name err]: undefined variable `{}`", name);
        }

        false
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
    pub fn get_method(&self, receiver_name: String, method_name: String) -> MethodInfo {
        match self.get_var(receiver_name) {
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
                    define_msg::VarErrorOrLog::VarIsNotDefined => {
                        panic!("this var is not found");
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RANGE: [&str; 5] = [
        "a",
        "b",
        "c",
        "d",
        "e",
    ];

    /// スタック領域だけ確認
    #[test]
    fn check_remove_stack() {
        let mut variables = VariableManager::new();
        for i in 0..3 {
            variables.make_new_stack();
            for v in RANGE {
                variables.add_var(
                    (v.to_owned() + i.to_string().as_str()).to_string(),
                    type_info::VarValue::Int32(10),
                    VarRegion::Stack
                );
            }
        }
        assert_eq!(variables.variables_info_vec.len(), 15);
        assert_eq!(variables.region_stack_index.len(), 3);
        variables.remove_stack();
        assert_eq!(variables.variables_info_vec.len(), 10);
    }

    #[test]
    fn check_stack_and_static() {
        let mut variables = VariableManager::new();
        for i in 0..3 {
            variables.make_new_stack();
            for v in RANGE {
                variables.add_var(
                    (v.to_owned() + i.to_string().as_str()).to_string(),
                    type_info::VarValue::Int32(10),
                    VarRegion::Stack
                );
            }
            variables.add_var(
                ("k".to_owned() + i.to_string().as_str()).to_string(),
                type_info::VarValue::Int32(10),
                VarRegion::Static
            );
        }
        assert_eq!(variables.variables_info_vec.len(), 18);
        assert_eq!(variables.region_stack_index.len(), 3);
        variables.remove_stack();
        assert_eq!(variables.variables_info_vec.len(), 13);
    }
}
