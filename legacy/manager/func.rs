use crate::parse::node;


#[derive(Clone)]
pub struct FuncManager {
    pub func_datas: Vec<node::FuncNode>,
}

impl FuncManager {
    pub fn new() -> Self {
        Self {
            func_datas: Vec::<node::FuncNode>::new(),
        }
    }

    fn search_func(&self, name: &str) -> Option<i32> {
        if let Some(index) = self.func_datas.iter().position(|func| func.name == name) {
            return Some(index as i32);
        }
        None
    }

    pub fn get_func(&self, name: &str) -> node::FuncNode {
        if let Some(index) = self.search_func(name) {
            return self.func_datas[index as usize].clone();
        } else {
            panic!("this function is not found");
        }
    }

    pub fn add_func(&mut self, new_func_node: node::FuncNode) {
        if let Some(_) = self.search_func(new_func_node.name.as_str()) {
            panic!("function already defined");
        } else {
            self.func_datas.push(new_func_node);
        }
    }

    pub fn add_func_calcul_node(&mut self, mut node: node::CalculNode, brace_depth: i32) {
        if let Some(func) = self.func_datas.last_mut() {
            node.block = Some(brace_depth);
            func.nodes.push(node);
        } else {
            eprintln!("[syntax err]");
            eprintln!("You can't write code outside of a function");
            panic!("");
        }
    }
}
