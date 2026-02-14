use super::*;


#[derive(Clone)]
pub struct FuncManager {
    pub func_datas: Vec<FuncNode>,
}

#[derive(Clone,PartialEq)]
pub struct FuncArgsNode {
    pub name: String,
    pub type_name: Option<String>,
    pub next: Option<Box<FuncArgsNode>>,
    pub multiple: Option<variable::MultipleVar>,
}

#[derive(Clone, PartialEq)]
pub struct FuncNode {
    //  関数の名前
    pub name: String,
    pub args: FuncArgsNode,
    pub ret_type: type_info::VarType,
    //  関数の処理
    pub nodes: Vec<node::CalculNode>,
}

impl FuncManager {
    pub fn new() -> Self {
        Self {
            func_datas: Vec::<FuncNode>::new(),
        }
    }

    fn search_func(&self, name: &str) -> Option<usize> {
        if let Some(index) = self.func_datas.iter().position(|func| func.name == name) {
            return Some(index);
        }
        None
    }

    pub fn get_func(&self, name: &str) -> Result<FuncNode, err_kind::ErrorsKind> {
        if let Some(index) = self.search_func(name) {
            Ok(self.func_datas[index].clone())
        } else {
            Err(err_kind::ErrorsKind::UndefinedFunction)
        }
    }

    pub fn add_func(&mut self, new_func_node: FuncNode) -> Result<(), err_kind::ErrorsKind> {
        if let Some(_) = self.search_func(new_func_node.name.as_str()) {
            Err(err_kind::ErrorsKind::FunctionAlreadyDefined)
        } else {
            self.func_datas.push(new_func_node);
            Ok(())
        }
    }

    pub fn add_func_calcul_node(&mut self, mut node: node::CalculNode, brace_depth: i32) {
        if let Some(func) = self.func_datas.last_mut() {
            node.block = Some(brace_depth as usize);
            func.nodes.push(node.clone());
        } else {
            eprintln!("[syntax err]");
            eprintln!("You can't write code outside of a function");
            panic!("");
        }
    }
}
