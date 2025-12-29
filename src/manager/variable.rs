use crate::parse::node;

#[derive(Clone)]
pub struct VariableData {
    pub name: String,
    pub value: node::CalculNode,
    pub type_name: String,
}

pub struct VariableManager {
    pub variables: Vec<VariableData>,
}


impl VariableManager {
    pub fn new() -> Self {
        Self {
            variables: Vec::<VariableData>::new(),
        }
    }

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

    pub fn add_var(&mut self, name: String, value: node::CalculNode, type_name: String) {
        if let Some(var) = self.variables.iter_mut().find(|var| var.name == name) {
            eprintln!("[name err]: variable `{}` is already defined", name);
            panic!("");
        } else {
            self.variables.push(
                VariableData {
                    name: name,
                    value: value,
                    type_name: type_name
                }
            );
        }
    }

    pub fn update_var(&mut self, name: String, value: node::CalculNode) {
        if let Some(var) = self.variables.iter_mut().find(|var| var.name == name) {
            var.value = value;
        } else {
            eprintln!("[name err]: undefined variable `{}`", name);
            panic!("");
        }
    }
}