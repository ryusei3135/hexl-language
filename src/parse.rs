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

#[derive(Clone, PartialEq, Debug)]
pub struct MemoryOperand {
    base: Option<Register>,
    idx: Option<Register>,
    scale: Scale,
    displacement: i32,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Operand {
    Reg(Register),
    Ref(String),
    Value(String),
    MemoryOperand(MemoryOperand),
}

impl Operand {
    pub fn gen_memory_operand(sib: &SIB, scale: Scale, disp: i32) -> Self {
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

    pub fn parser(&'a mut self, tkns: &'a Vec<lex::Tkn>) -> Result<&'a Vec<Node>, err::Err> {
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
                            operands.push(self.gen_operand_node()?);
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
                    let n = match self.advance() {
                        lex::Tkn::Name(_) => {
                            panic!();
                        },
                        lex::Tkn::Number(value) => {
                            Node::Define {
                                name: def_name,
                                value: value.parse::<i32>().unwrap().to_le_bytes().to_vec(),
                                size: size,
                            }
                        },
                        lex::Tkn::Str(value) => {
                            Node::Define {
                                name: def_name,
                                value: value.as_bytes().to_vec(),
                                size: Size::Str,
                            }
                        },
                        _ => panic!(),
                    };
                    self.nodes.push(n);
                },
                lex::Tkn::Str(_) => {},
                _ => {},
            }
        }

        self.tkns = None;
        Ok(&self.nodes)
    }

    fn get_number_node(&mut self) -> Option<String> {
        if let lex::Tkn::Number(value) = self.advance() {
            Some(value.clone())
        } else {
            None
        }
    }

    fn gen_operand_node(&mut self) -> Result<Operand, err::Err> {
        match self.advance() {
            lex::Tkn::AddrStart => {
                if let lex::Tkn::Name(_) = self.advance() {
                    Ok(self.gen_memory_operand()?)
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
    fn gen_memory_operand(&mut self) -> Result<Operand, err::Err> {
        match self.advance() {
            lex::Tkn::Name(value) => {
                self.sib.undefine_reg = Register::decide_reg(value);
                let result = Ok(self.index_parse()?);
                self.sib = SIB::new();
                result
            }
            _ => panic!(),
        }
    }

    fn scale_parse(&mut self) -> Result<Scale, err::Err> {
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

    fn displacement_parse(&mut self) -> Result<i32, err::Err> {
        match self.advance() {
            lex::Tkn::AddrEnd => return Ok(0i32),
            lex::Tkn::Add => {},
            _ => return Err(err::Err::UnexpectedToken("err tkn".to_string())),
        }

        match self.advance() {
            lex::Tkn::Number(value) => {
                Ok(value.parse::<i32>().unwrap())
            },
            _ => Err(err::Err::UnexpectedToken("kk".to_string())),
        }
    }

    fn index_parse(&mut self) -> Result<Operand, err::Err> {
        match self.advance() {
            lex::Tkn::Mul => {
                self.sib.index_reg = self.sib.undefine_reg.clone();
                let size = &self.sib.index_reg.as_ref().unwrap().get_reg_byte();
                if self.operand_size.is_some() && self.operand_size.as_ref().unwrap() != size {
                    return Err(err::Err::SyntaxErrTyNotMatch);
                }

                let scale = self.scale_parse()?;
                let disp = self.displacement_parse()?;
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

                if let lex::Tkn::Name(next_reg) = self.advance() {
                    self.sib.undefine_reg = Register::decide_reg(next_reg);
                    // レジスタのサイズを確認
                    let size = &self.sib.undefine_reg.as_ref().unwrap().get_reg_byte();
                    if self.operand_size.is_some() && self.operand_size.as_ref().unwrap() != size {
                        return Err(err::Err::SyntaxErrTyNotMatch);
                    }

                    Ok(self.index_parse()?)
                } else {
                    panic!()
                }
            }
            lex::Tkn::AddrEnd => {
                self.sib.base_reg = self.sib.undefine_reg.clone();
                Ok(Operand::gen_memory_operand(&self.sib, Scale::One, 0))
            }
            _ => Err(err::Err::UnexpectedToken("not register token".to_string())),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn gen_reg_node(reg: &str) -> lex::Tkn {
        lex::Tkn::Name(reg.to_string())
    }

    fn gen_number(value: &str) -> lex::Tkn {
        lex::Tkn::Number(value.to_string())
    }

    fn set_sib_tkns(parser: &mut Parser, base: &str, idx: &str) -> Vec<lex::Tkn> {
        let binding = vec![
            gen_reg_node(base),
            lex::Tkn::Add,
            gen_reg_node(idx),
            lex::Tkn::Mul,
            gen_number("4"),
            lex::Tkn::Add,
            gen_number("8"),
            lex::Tkn::AddrEnd,
        ];
        parser.idx = 0;
        binding
    }

    #[test]
    fn check_err_sib() {
        let mut parser = Parser::new();
        let binding = vec![
            gen_reg_node("rax"),
            lex::Tkn::Add,
            gen_reg_node("eax"),
            lex::Tkn::AddrEnd,
        ];
        parser.tkns = Some(&binding);
        assert_eq!(
            parser.gen_memory_operand(),
            Err(err::Err::SyntaxErrTyNotMatch),
        );
    }

    #[test]
    fn check_sib_node() {
        let mut parser = Parser::new();

        let binding = vec![
            gen_reg_node("eax"),
            lex::Tkn::Mul,
            gen_number("4"),
            lex::Tkn::AddrEnd,
        ];
        parser.tkns = Some(&binding);
        assert_eq!(
            parser.gen_memory_operand().unwrap(),
            Operand::MemoryOperand(MemoryOperand {
                base: None,
                idx: Some(Register::Eax),
                scale: Scale::Four,
                displacement: 0,
            })
        );
        let binding = vec![
            gen_reg_node("eax"),
            lex::Tkn::AddrEnd,
        ];
        parser.tkns = Some(&binding);
        parser.idx = 0;
        assert_eq!(
            parser.gen_memory_operand().unwrap(),
            Operand::MemoryOperand(MemoryOperand {
                base: Some(Register::Eax),
                idx: None,
                scale: Scale::One,
                displacement: 0,
            })
        );
        let binding = set_sib_tkns(&mut parser, "eax", "ecx");
        parser.tkns = Some(&binding);
        assert_eq!(
            parser.gen_memory_operand().unwrap(),
            Operand::MemoryOperand(MemoryOperand {
                base: Some(Register::Eax),
                idx: Some(Register::Ecx),
                scale: Scale::Four,
                displacement: 8,
            })
        );
        let binding = set_sib_tkns(&mut parser, "eax", "esp");
        parser.tkns = Some(&binding);
        assert_eq!(
            parser.gen_memory_operand().unwrap(),
            Operand::MemoryOperand(MemoryOperand {
                base: Some(Register::Esp),
                idx: Some(Register::Eax),
                scale: Scale::Four,
                displacement: 8,
            })
        );
    }
}
