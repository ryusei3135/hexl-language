
use crate::parse::node;

#[derive(Clone)]
pub struct VariableData {
    pub name: String,
    pub value: node::CalculNode,
    pub type_name: String,
    pub method: Option<Vec<methods>>,
}

pub struct VariableManager {
    pub variables: Vec<VariableData>,
}

#[derive(Clone)]
pub struct methods {
    pub name: String,
    pub node: node::CalculNode,
}


impl VariableManager {
    pub fn new() -> Self {
        Self {
            variables: Vec::<VariableData>::new(),
        }
    }
    //  変数のデータを返す
    pub fn get_var(&self, name: String) -> VariableData {
        self.variables
            .iter()
            .find(|var| var.name == name)
            .map(|var| var.clone())
            .unwrap_or_else(|| {
                eprintln!("[err] {}", name);
                panic!("this variable is not defined");
            })
    }
    //  メゾットのデータを取得
    pub fn get_method(&self, receiver_name: String, method_name: String) -> methods {
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
    //  変数を追加
    pub fn add_var(&mut self, name: String, value: node::CalculNode, type_name: String) {
        if self.variables.iter_mut().find(|var| var.name == name).is_some() {
            eprintln!("[name err]: variable `{}` is already defined", name);
            panic!("");
        } else {
            self.variables.push(
                VariableData {
                    name: name,
                    value: value,
                    type_name: type_name,
                    method: Some(Vec::<methods>::new()),
                }
            );
        }
    }
    //  変数にメゾットを追加
    pub fn add_method(&mut self, node: node::CalculNode, name: String) {
        if self.variables.last().is_none() {
            eprintln!("[name err]: variable `{}` is already defined", name);
            panic!("");
        } else {
            if let Some(last_var) = self.variables.last_mut() {
                if let Some(method) = last_var.method.as_mut() {
                    if method.iter_mut().find(|var| var.name == name).is_none() {
                        method.push(
                            methods {
                                name: name,
                                node: node,
                            }
                        );
                    }
                }
            }
        }
    }
    //  変数の値を上書き
    pub fn update_var(&mut self, name: String, value: node::CalculNode) {
        if let Some(var) = self.variables.iter_mut().find(|var| var.name == name) {
            var.value = value;
        } else {
            eprintln!("[name err]: undefined variable `{}`", name);
            panic!("");
        }
    }
}
