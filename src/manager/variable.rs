use crate::parse::node;


pub struct VariableData {
    pub name: String,
    pub value: node::CalculNode,
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

    pub fn get_var(&self, name: String) -> node::CalculNode {
        self.variables
            .iter()
            .find(|var| var.name == name)
            .map(|var| var.value.clone())
            .unwrap_or_else(|| {
                eprintln!("[err] {}", name);
                panic!("this variable is not defined");
            })
    }

    pub fn add_var(&mut self, name: String, value: node::CalculNode) {
        if let Some(var) = self.variables.iter_mut().find(|var| var.name == name) {
            var.value = value;
        } else {
            self.variables.push(
                VariableData {
                    name: name,
                    value: value
                }
            );
        }
    }
}