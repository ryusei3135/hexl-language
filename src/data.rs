use crate::parse::*;
use crate::sections;


#[derive(Clone, Debug, PartialEq)]
pub struct Data {
    pub name: String,
    pub pos: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodeData {
    datas: Vec<Data>,
    pub codes: Vec<u8>,
}

impl CodeData {
    pub fn new() -> Self {
        Self {
            datas: Vec::new(),
            codes: Vec::new(),
        }
    }

    pub fn new_block(&mut self, name: &String) {
        self.datas.push(
            Data {
                name: name.clone(),
                pos: self.codes.len(), 
            }
        );
    }

    pub fn find_name(&self, name: &String) -> Option<&Data> {
        self.datas.iter().find(|n| &n.name == name)
    }

    pub fn push_data(&mut self, code: Vec<u8>) {
        self.codes.extend(code);
    }

    pub fn get_code(self) -> Vec<u8> {
        self.codes
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct DataDefine {
    name: String,
    pos: usize,
    size: Size,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataInfo {
    datas: Vec<DataDefine>,
    pub codes: Vec<u8>,
}

impl DataInfo {
    pub fn new() -> Self {
        Self {
            datas: Vec::new(),
            codes: Vec::new(),
        }
    }

    pub fn new_data(&mut self, size: &Size, name: &String, value: &Vec<u8>) {
        self.datas.push(
            DataDefine {
                name: name.clone(),
                pos: self.codes.len(),
                size: size.clone(),
            }
        );
        self.codes.extend(value.clone());
    }

    pub fn find_name(&self, name: &String) -> bool {
        self.datas.iter().find(|n| &n.name == name).is_some()
    }

    pub fn get_code(self) -> Vec<u8> {
        self.codes
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct SymtabData {
    pub symtab: Vec<(String, sections::Elf64_Sym, usize)>,
}

impl SymtabData {
    pub fn new() -> Self {
        Self {
            symtab: vec![
                (
                    "\0".to_string(), sections::Elf64_Sym::default(), 0
                )
            ],
        }
    }

    pub fn add_symtab(
        &mut self,
        symtab: sections::Elf64_Sym,
        name: &String,
        pos: usize
    ) {
        self.symtab.push(
            (
                name.clone(),
                symtab.clone(),
                pos
            )
        );
    }

    pub fn find_name(&self, name: &String) -> Option<&(String, sections::Elf64_Sym, usize)> {
        self.symtab.iter().find(|v| &v.0 == name)
    }

    pub fn get_code(self) -> Vec<sections::Elf64_Sym> {
        self.symtab.iter().map(|v| v.1).collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlobalFunc {
    pub strtab_pos: usize,
    pub name: String,
}


#[derive(Clone, Debug, PartialEq)]
pub struct GlobalFuncData {
    func_data: Vec<GlobalFunc>,
}

impl GlobalFuncData {
    pub fn new() -> Self {
        Self {
            func_data: Vec::new(),
        }
    }

    pub fn add_global_func(&mut self, name: &String, strpos: &usize) {
        self.func_data.push(
            GlobalFunc {
                strtab_pos: strpos.clone(),
                name: name.clone()
            }
        ); 
    }

    pub fn find_name(&self, name: &String) -> Option<&GlobalFunc> {
        self.func_data.iter().find(|n| &n.name == name)
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct ExternFunc {
    pub name: String,
    pub pos: usize,
}


#[derive(Clone, Debug, PartialEq)]
pub struct ExternFuncData {
    datas: Vec<ExternFunc>,
}

impl ExternFuncData {
    pub fn new() -> Self {
        Self {
            datas: Vec::new(),
        }
    }

    pub fn add_extern_func(&mut self, name: &String, pos: &usize) {
        self.datas.push(
            ExternFunc {
                name: name.clone(),
                pos: pos.clone(),
            }
        );
    }

    pub fn find_name(&self, name: &String) -> Option<&ExternFunc> {
        self.datas.iter().find(|v| &v.name == name)
    }
} 


#[derive(Clone, Debug, PartialEq)]
pub struct UpdateLabel {
    pub name: String,
    pub pos: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateLabelData {
    pub datas: Vec<UpdateLabel>,
}

impl UpdateLabelData {
    pub fn new() -> Self {
        Self {
            datas: Vec::new(),
        }
    }

    pub fn add_update_label(&mut self, name: &String, pos: &usize) {
        self.datas.push(
            UpdateLabel {
                name: name.clone(),
                pos: pos.clone()
            }
        );
    }
}
