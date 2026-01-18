use super::*;

///  一つの変数のデータ
#[derive(Clone)]
pub struct VariableInfo {
    pub name: String,
    pub value: type_info::VarValue,
    pub type_name: String,
    pub method: Option<Vec<MethodInfo>>,
}

#[derive(Clone)]
pub struct MethodInfo {
    pub name: String,
    pub node: node::CalculNode,
}

pub struct VariableManager {
    pub variables_info_vec: Vec<VariableInfo>,
}


impl VariableManager {
    pub fn new() -> Self {
        Self {
            variables_info_vec: Vec::<VariableInfo>::new(),
        }
    }
    //  変数のデータを返す
    pub fn get_var(&self, name: String) -> VariableInfo {
        self.variables_info_vec
            .iter()
            .find(|var| var.name == name)
            .map(|var| var.clone())
            .unwrap_or_else(|| {
                eprintln!("[err] {}", name);
                panic!("this variable is not defined");
            })
    }
    //  変数を追加
    pub fn add_var(&mut self, name: String, value: type_info::VarValue, type_name: String) {
        if self.variables_info_vec.iter_mut().find(|var| var.name == name).is_some() {
            eprintln!("[name err]: variable `{}` is already defined", name);
            panic!("");
        } else {
            self.variables_info_vec.push(
                VariableInfo {
                    name: name,
                    value: value,
                    type_name: type_name,
                    method: Some(Vec::<MethodInfo>::new()),
                }
            );
        }
    }
    //  変数の値を上書き
    pub fn update_var(&mut self, name: String, value: type_info::VarValue) -> bool {
        if let Some(var) = self.variables_info_vec.iter_mut().find(|var| var.name == name) {
            var.value = value;
            true
        } else {
            eprintln!("[name err]: undefined variable `{}`", name);
            false
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
    pub fn get_method(&self, receiver_name: String, method_name: String) -> MethodInfo {
        let receiver = self.get_var(receiver_name).method;
        receiver
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
}
