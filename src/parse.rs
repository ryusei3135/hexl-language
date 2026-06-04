use crate::{
    err,
    lex,
    reg::Register,
    mnemo::{self, Mnemonic},
};

//#[derive(Clone, PartialEq, Debug)]
//pub enum Modrm {
  //  RegToReg,
    //ValueToReg,
    //ImmToReg,
//}


pub type DispTy = Option<Vec<u8>>;


#[derive(Clone, PartialEq, Debug)]
pub enum Size {
    DB,
    DD,
    DW,
    DQ,
    Str,
}

impl Size {
    pub fn get_byte(&self) -> usize {
        match self {
            Self::DB | Self::Str => 1,
            Self::DW => 2,
            Self::DD => 4,
            Self::DQ => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    One = 1,
    Two = 2,
    Four = 4,
    Eight = 8,
}

impl Scale {
    pub fn get_num(self) -> u8 {
        match self {
            Self::One => 0b0000_0000,
            Self::Two => 0b0100_0000,
            Self::Four => 0b1000_0000,
            Self::Eight => 0b1100_0000,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct MemoryOperand {
    pub base: Option<Register>,
    pub idx: Option<Register>,
    pub scale: Scale,
    pub displacement: DispTy,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Operand {
    Reg(Register),
    Ref(String),
    Value(String),
    MemoryOperand(MemoryOperand),
}

impl Operand {
    pub fn gen_memory_operand(sib: &SIB, scale: Scale, disp: DispTy) -> Self {
        Self::MemoryOperand(MemoryOperand {
            base: {
                if sib.index_reg.is_some() && sib.index_reg.as_ref().unwrap().is_sp() {
                    sib.index_reg.clone()
                } else {
                    if sib.base_reg.is_some() {
                        sib.base_reg.clone()
                    } else {
                        None
                    }
                }
            },
            idx: {
                if sib.index_reg.is_some() && sib.index_reg.as_ref().unwrap().is_sp() {
                    sib.base_reg.clone()
                } else {
                    if sib.index_reg.is_some() {
                        sib.index_reg.clone()
                    } else {
                        None
                    }
                }
            },
            scale,
            displacement: disp,
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Instruction {
    ThreeOperand {
        op: Mnemonic,
        dst: Operand,
        src: Operand,
        value: String,
    },
    TwoOperand {
        op: Mnemonic,
        dst: Operand,
        src: Operand,
    },
    OneOperand {
        op: Mnemonic,
        src: Operand,
    },
    Opcode(mnemo::Mnemonic),
}

#[derive(Clone, PartialEq, Debug)]
pub enum Node {
    Instruct {
        instruct: Instruction,
        size: Option<Size>,
    },
    Block(String),
    Section(String),
    Define {
        name: String,
        value: Vec<u8>,
        size: Size,
    },
    CnstValue {
        name: String,
        value: Vec<u8>,
        size: Size,
    },
    PubFunc(String),
    ExternFunc(String),
}

#[derive(Clone, PartialEq, Debug)]
struct SIB {
    pub index_reg: Option<Register>,
    pub base_reg: Option<Register>,
    pub undefine_reg: Option<Register>,
}

impl SIB {
    pub fn new() -> Self {
        Self {
            index_reg: None,
            base_reg: None,
            undefine_reg: None,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Parser<'a> {
    nodes: Vec<Node>,
    pub idx: usize,
    pub tkns: Option<&'a Vec<lex::Tkn>>,
    sib: SIB,
    operand_size: Option<Size>,
}

impl<'a> Parser<'a> {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            idx: 0,
            tkns: None,
            sib: SIB::new(),
            operand_size: None,
        }
    }

    pub fn parser(&'a mut self, tkns: &'a Vec<lex::Tkn>) -> Result<&'a Vec<Node>, err::Err<'static>> {
        self.tkns = Some(tkns);

        while self.tkns.unwrap().len() > self.idx {
            match self.advance() {
                lex::Tkn::Extern => {
                    if let lex::Tkn::Name(value) = self.advance().clone() {
                        self.nodes.push(Node::ExternFunc(value));
                    } else {
                        return Err(err::Err::UnexpectedToken("extern func not found name token".to_string()));
                    }
                }
                lex::Tkn::Pub => {
                    if let lex::Tkn::Name(value) = self.advance().clone() {
                        self.nodes.push(Node::PubFunc(value));
                    } else {
                        return Err(err::Err::UnexpectedToken("pub func not found name token".to_string()));
                    }
                }
                lex::Tkn::Name(value) => {
                    if let Some(mnemonic) = Mnemonic::gen_tkn_if_valid_str(value) {
                        // 処理するデータサイズを初期化
                        self.operand_size = None;

                        let mut operands: Vec<Operand> = Vec::new();

                        let op_len = mnemonic.get_operand_len();

                        if op_len != 0 {
                            operands.push(self.gen_operand_node().unwrap());
                            if op_len >= 2 && self.advance() == &lex::Tkn::Comma {
                                operands.push(self.gen_operand_node()?);
                                if self.advance() == &lex::Tkn::Comma {
                                    let value = self.get_number_node().unwrap();
                                    self.nodes.push(
                                        Node::Instruct {
                                            instruct: Instruction::ThreeOperand {
                                                op: mnemonic.clone(),
                                                dst: operands[0].clone(),
                                                src: operands[1].clone(),
                                                value: value,
                                            },
                                            size: self.operand_size.clone(),
                                        }
                                    )
                                } else {
                                    self.nodes.push(
                                        Node::Instruct {
                                            instruct: Instruction::TwoOperand {
                                                op: mnemonic.clone(),
                                                dst: operands[0].clone(),
                                                src: operands[1].clone(),
                                            },
                                            size: self.operand_size.clone(),
                                        }
                                    )
                                }
                            } else {
                                self.nodes.push(
                                    Node::Instruct {
                                        instruct: Instruction::OneOperand {
                                            op: mnemonic.clone(),
                                            src: operands[0].clone(),
                                        },
                                        size: self.operand_size.clone(),
                                    }
                                );
                            }
                        } else {
                            self.nodes.push(
                                Node::Instruct {
                                    instruct: Instruction::Opcode(mnemonic),
                                    size: None,
                                }
                            );
                        }
                    }
                },
                lex::Tkn::Number(_) => {},
                lex::Tkn::Section => {
                    let n = if let lex::Tkn::Name(name) = self.advance() {
                        Node::Section(name.clone())
                    } else {
                        panic!();
                    };
                    self.nodes.push(n);
                },
                lex::Tkn::Block(name) => {
                    let def_name = name.to_string();

                    let size = match &self.tkns.unwrap()[self.idx] {
                        lex::Tkn::Name(v) => match v.as_str() {
                            "db" => Size::DB,
                            "dw" => Size::DW,
                            "dd" => Size::DD,
                            "dq" => Size::DQ,
                            _ => {
                                let n = Node::Block(def_name);
                                self.nodes.push(n);
                                continue;
                            }
                        }
                        _ => {
                            let n = Node::Block(def_name);
                            self.nodes.push(n);
                            continue;
                        }
                    };
                    self.idx += 1;
                    let curr_tkn = self.advance().clone();
                    self.nodes.push(self.gen_define_node(curr_tkn, &def_name, &size));
                },
                lex::Tkn::Str(_) => {},
                _ => {},
            }
        }

        self.tkns = None;
        Ok(&self.nodes)
    }
   
    #[inline(always)]
    fn gen_define_node(&self, tkn: lex::Tkn, def_name: &String, size: &Size) -> Node {
        match &tkn {
            lex::Tkn::Name(_) => {
                panic!();
            },
            lex::Tkn::Number(value) => {
                Node::Define {
                    name: def_name.clone(),
                    value: value.parse::<i32>().unwrap().to_le_bytes().to_vec(),
                    size: size.clone(),
                }
            },
            lex::Tkn::Str(value) => {
                Node::Define {
                    name: def_name.clone(),
                    value: value.as_bytes().to_vec(),
                    size: Size::Str,
                }
            },
            _ => panic!(),
        }
    }

    fn get_number_node(&mut self) -> Option<String> {
        if let lex::Tkn::Number(value) = self.advance() {
            Some(value.clone())
        } else {
            None
        }
    }

    fn gen_operand_node(&mut self) -> Result<Operand, err::Err<'static>> {
        match self.advance() {
            lex::Tkn::AddrStart => {
                if let lex::Tkn::Name(value) = self.advance().clone() {
                    Ok(self.gen_memory_operand(&value)?)
                } else {
                    panic!()
                }
            }
            lex::Tkn::Name(value) => {
                if let Some(reg) = Register::decide_reg(&value) {
                    self.operand_size = Some(reg.get_reg_byte());
                    Ok(Operand::Reg(reg))
                } else {
                    Ok(Operand::Ref(value.clone()))
                }
            }
            lex::Tkn::Number(value) => {
                Ok(Operand::Value(value.clone()))
            }
            lex::Tkn::Comma => self.gen_operand_node(),
            _ => {
                Err(err::Err::UnexpectedToken("operand".to_string()))
            }
        }
    }

    pub fn advance(&mut self) -> &lex::Tkn {
        self.idx += 1;
        &self.tkns.unwrap()[self.idx - 1]
    }
}

/// SIBの処理全般
impl<'a> Parser<'a> {
    pub fn gen_memory_operand(&mut self, value: &String) -> Result<Operand, err::Err<'static>> {
        self.sib.undefine_reg = Register::decide_reg(value);
        let result = Ok(self.index_parse()?);
        self.sib = SIB::new();
        result
    }

    fn scale_parse(&mut self) -> Result<Scale, err::Err<'static>> {
        match self.advance() {
            lex::Tkn::Number(value) => {
                match value.as_str() {
                    "2" => Ok(Scale::Two),
                    "4" => Ok(Scale::Four),
                    "8" => Ok(Scale::Eight),
                    _ => Err(err::Err::UnexpectedToken("err token".to_string())),
                }
            }
            _ => Err(err::Err::UnexpectedToken("unexpected token".to_string())),
        }
    }

    fn displacement_parse(&mut self, num_value: &Option<String>) -> Result<DispTy, err::Err<'static>> {
        let gen_byte_data = |value: &String| -> Result<DispTy, err::Err<'static>> {
            if let Ok(byte_value) = value.parse::<u8>() {
                Ok(Some(vec![byte_value]))
            } else if let Ok(dbyte_value) = value.parse::<u32>() {
                Ok(Some(dbyte_value.to_le_bytes().to_vec()))
            } else {
                Err(
                    err::Err::SyntaxErr(
                        err::SyntaxErr::UnmatchNumberSize{
                            expect: "1byte or 4byte",
                            found: "other",
                            msg: None
                        }
                    )
                )
            }
        };
        if let Some(value) = num_value {
            gen_byte_data(&value)
        } else {
            match self.advance() {
                lex::Tkn::AddrEnd => return Ok(None),
                lex::Tkn::Add => {},
                _ => return Err(err::Err::UnexpectedToken("err tkn".to_string())),
            }

            match self.advance() {
                lex::Tkn::Number(value) => {
                    gen_byte_data(&value)
                },
                _ => Err(err::Err::UnexpectedToken("kk".to_string())),
            }
        }
    }

    fn index_parse(&mut self) -> Result<Operand, err::Err<'static>> {
        match self.advance() {
            lex::Tkn::Mul => {
                self.sib.index_reg = self.sib.undefine_reg.clone();
                let size = &self.sib.index_reg.as_ref().unwrap().get_reg_byte();
                if self.operand_size.is_some() && self.operand_size.as_ref().unwrap() != size {
                    return Err(err::Err::SyntaxErrTyNotMatch);
                }

                let scale = self.scale_parse()?;
                let disp: DispTy = self.displacement_parse(&None)?;
                Ok(
                    Operand::gen_memory_operand(
                        &self.sib,
                        scale,
                        disp,
                    )
                )
            }
            lex::Tkn::Add => {
                self.sib.base_reg = self.sib.undefine_reg.clone();
                self.operand_size = Some(self.sib.base_reg.as_ref().unwrap().get_reg_byte());

                match self.advance().clone() {
                    lex::Tkn::Name(ref next_reg) => {
                        self.sib.undefine_reg = Register::decide_reg(next_reg);
                        // レジスタのサイズを確認
                        let size = &self.sib.undefine_reg.as_ref().unwrap().get_reg_byte();
                        if self.operand_size.is_some() && self.operand_size.as_ref().unwrap() != size {
                            return Err(err::Err::SyntaxErrTyNotMatch);
                        }

                        Ok(self.index_parse()?)
                    }
                    lex::Tkn::Number(value) => {
                        let disp = self.displacement_parse(&Some(value))?;
                        Ok(Operand::gen_memory_operand(&self.sib, Scale::One, None))
                    }
                    t => panic!("{:?}", self.tkns),
                }
            }
            lex::Tkn::AddrEnd => {
                self.sib.base_reg = self.sib.undefine_reg.clone();
                Ok(Operand::gen_memory_operand(&self.sib, Scale::One, None))
            }
            _ => Err(err::Err::UnexpectedToken("not register token".to_string())),
        }
    }
}


