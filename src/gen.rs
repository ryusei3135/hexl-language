//! バイトコードをトークンから生成する

use crate::{
    data,
    reg::Register,
    mnemo,
    sections::{self, *},
    parse::{self, *}
};



enum SectionFlag {
    Text,
    Data,
}

pub struct Generater {
    section_flag: Option<SectionFlag>,
    pub rela: Vec<sections::Elf64_Rela>,
    pub code_data: data::CodeData,
    pub data_info: data::DataInfo,
    pub symtab: data::SymtabData,
    global_func: data::GlobalFuncData,
    extern_func: data::ExternFuncData,
    update_label: data::UpdateLabelData,
    pub strtab: Vec<u8>,
}

impl Generater {
    pub fn new() -> Self {
        Self {
            section_flag: None,
            rela: Vec::new(),
            code_data: data::CodeData::new(),
            data_info: data::DataInfo::new(),
            symtab: data::SymtabData::new(),
            global_func: data::GlobalFuncData::new(),
            extern_func: data::ExternFuncData::new(),
            update_label: data::UpdateLabelData::new(),
            strtab: b"\0".to_vec(),
        }
    }

    pub fn gen_codes(&mut self, nodes: &Vec<Node>) {
        for node in nodes {
            match &node {
                Node::Section(section_name) => {
                    match section_name.as_str() {
                        "text" => self.section_flag = Some(SectionFlag::Text),
                        "data" => self.section_flag = Some(SectionFlag::Data),
                        _ => panic!(),
                    }
                }
                Node::PubFunc(name) => {
                    self.global_func.add_global_func(&name, &self.strtab.len());
                    self.strtab.extend(name.bytes());
                    self.strtab.push(b'\0');
                }
                Node::ExternFunc(name) => {
                    self.extern_func.add_extern_func(&name, &self.symtab.symtab.len());
                    self.symtab.add_symtab(
                        Elf64_Sym::make_undef_func(
                            self.symtab.symtab.len() as u32,
                            self.strtab.len() as u64
                        ),
                        &name,
                        self.strtab.len(),
                    );
                    self.strtab.extend(name.bytes());
                    self.strtab.push(b'\0');
                }
                Node::Block(name) => {
                    match &self.section_flag {
                        Some(SectionFlag::Text) => {
                            if name.as_str() == "_start" {
                                self.symtab.add_symtab(
                                    Elf64_Sym::make_global_func(
                                        self.strtab.len() as u32,
                                        self.code_data.codes.len() as u64
                                    ),
                                    &name,
                                    self.strtab.len(),
                                );
                                self.strtab.extend("_start\0".bytes());
                            }
                            // 関数が公開されているなら、登録する
                            if let Some(func_data) = self.global_func.find_name(&name) {
                                self.symtab.add_symtab(
                                    Elf64_Sym::make_global_func(
                                        func_data.strtab_pos.clone() as u32,
                                        self.code_data.codes.len() as u64
                                    ),
                                    &func_data.name,
                                    func_data.strtab_pos.clone(),
                                );
                            }
                            // バイトコードにブロックを登録
                            self.code_data.new_block(&name);
                        }
                        Some(SectionFlag::Data) => {
                        }
                        None => {},
                    }
                }
                Node::Instruct{ instruct, size } => {
                    match &instruct {
                        Instruction::ThreeOperand { op, dst, src, value } => {
                            self.three_opcodes(&size.as_ref().unwrap(), &op, &dst, &src, value.clone());
                        }
                        Instruction::TwoOperand { op, dst, src } => {
                            self.gen_opcodes(&size.as_ref().unwrap(), &op, &dst, &src);
                        }
                        Instruction::OneOperand { op, src } => {
                            self.one_opcodes(&size.as_ref().unwrap_or(&Size::DB), &op, &src);
                        }
                        Instruction::Opcode(opcode) => {
                            self.code_data.push_data(opcode.opcode(&Size::DB, None, None));
                        }
                    }
                }
                Node::Define { name, value, size } => {
                    if size == &Size::Str {
                        let symtab_len = self.symtab.symtab.len();
                        let mut byte_data = value.clone();

                        byte_data.push(b'\0');
                        self.symtab.add_symtab(
                            Elf64_Sym {
                                st_name: self.strtab.len() as u32,
                                st_info: sections::OBJ,
                                st_other: 0,
                                st_shndx: 2,
                                st_value: self.data_info.codes.len() as u64,
                                st_size: byte_data.len() as u64,
                            },
                            &name,
                            symtab_len,
                        );
                        self.strtab.extend(byte_data);
                    }
                    self.data_info.new_data(&size, &name, &value);
                }
                _ => {},
            }
        }
    }

    pub fn update_label(&mut self) {
        for data in &self.update_label.datas {
            if let Some(value) = self.code_data.find_name(&data.name) {
                let start: isize = data.pos as isize;
                let end: isize = value.pos as isize;
                self.code_data.codes[data.pos] = (end - start) as u8;
            }
        }
    }

    fn three_opcodes(
        &mut self, 
        size: &parse::Size,
        op: &mnemo::Mnemonic,
        dst: &Operand,
        src: &Operand,
        value: String
    ) {
        panic!();
    }

    fn one_opcodes(
        &mut self, 
        size: &parse::Size,
        op: &mnemo::Mnemonic,
        src: &Operand,
    ) {
        match src {
            Operand::Ref(name) => {
                let codes = op.opcode(&size, None, None);
                self.code_data.push_data(codes);

                if let Some(extern_func) = self.extern_func.find_name(&name) {
                    self.rela.push(sections::Elf64_Rela::new(
                        self.code_data.codes.len() as u64,
                        ((extern_func.pos << 32) | 2) as u64,
                        -4,
                    ));
                    self.code_data.push_data(vec![0, 0, 0, 0]);
                } else {
                    self.update_label.add_update_label(&name, &self.code_data.codes.len());
                    let mut imm: Vec<u8> = vec![0];
                    imm.resize(size.get_byte(), 0);
                    self.code_data.push_data(imm);
                }
            }
            _ => panic!(),
        }
    }

    fn gen_opcodes(
        &mut self,
        size: &parse::Size,
        op: &mnemo::Mnemonic,
        dst: &Operand,
        src: &Operand
    ) {
        match (dst, src) {
            (Operand::Reg(dst_reg), Operand::Reg(src_reg)) => {
                self.code_data.push_data(op.opcode(&size, Some(&dst_reg), Some(&src_reg)));
            }
            (Operand::Reg(dst_reg), Operand::Value(src)) => {
                self.code_data.push_data(op.imm_opcode(&size, &dst_reg, Some(&src)));
            }
            (Operand::Reg(dst_reg), Operand::Ref(name)) => {
                if self.data_info.find_name(&name) {
                    let codes = op.imm_opcode(&size, &dst_reg, None);
                    self.code_data.push_data(codes);

                    self.rela.push(sections::Elf64_Rela::new(
                        self.code_data.codes.len() as u64,
                        ((self.symtab.find_name(&name).unwrap().2 << 32) | 2) as u64,
                        -4,
                    ));
                    self.code_data.push_data(vec![0, 0, 0, 0]);
                } else if let Some(extern_func) = self.extern_func.find_name(&name) {
                    self.rela.push(sections::Elf64_Rela::new(
                        self.code_data.codes.len() as u64,
                        ((extern_func.pos << 32) | 2) as u64,
                        -4,
                    ));
                    self.code_data.push_data(vec![0, 0, 0, 0]);
                }
            }
            (_, _) => panic!("{:?}::{:?}", dst, src),
        }
    }
}
