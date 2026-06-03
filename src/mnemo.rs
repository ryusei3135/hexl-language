use crate::parse::Size;
use crate::reg::Register;


#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mnemonic {
    Add, Or, Adc, Sbb,
    And, Sub, Xor, Cmp,
    
    Rol, Ror, Rcl, Rcr,
    Shl, Shr, Sar,
    
    Inc, Dec, Call, Jmp, Push,

    Lea,

    // jcc
    Jo, Jno, Jc, Jnc,
    Jz, Jnz, Jna, Ja,
    Js, Jns, Jp, Jnp,
    Jl, Jge, Jle, Jg,

    Syscall,
    Ret,
    Mov,
}

impl Mnemonic {
    fn group_5_opcode(&self, size: &Size, dst_reg: &Register) -> Option<Vec<u8>> {
        let code = match &self {
            Self::Inc => 0xC0,
            Self::Dec => 0xC8,
           // Self::Call => 0xD0,
           // Self::Jmp => 0xE0,
            Self::Push => 0xF0,
            _ => return None,
        };

        Some(match size {
            Size::DB => vec![0xFE, code | dst_reg.get_reg_number()],
            Size::DW => vec![0x66, 0xFF, code | dst_reg.get_reg_number()],
            Size::DD => vec![0xFF, code],
            Size::DQ => vec![0x48, 0xFF, code | dst_reg.get_reg_number()],
            _ => panic!(),
        })
    }

    fn imm_group_2_opcode(&self, size: &Size, dst_reg: &Register, value: &String) -> Option<Vec<u8>> {
        let code = match &self {
            Self::Rol => 0xC0,
            Self::Ror => 0xC8,
            Self::Rcl => 0xD0,
            Self::Rcr => 0xD8,
            Self::Shl => 0xE0,
            Self::Shr => 0xE8,
            Self::Sar => 0xF8,
            _ => return None,
        };
        let opcode = if value.as_str() == "1" {
            0xD0
        } else {
            0xC0
        };
        Some(match size {
            Size::DB => vec![opcode, code | dst_reg.get_reg_number()],
            Size::DW => vec![0x66, opcode + 1, code | dst_reg.get_reg_number()],
            Size::DD => vec![opcode + 1, code],
            Size::DQ => vec![0x48, opcode + 1, code | dst_reg.get_reg_number()],
            _ => panic!(),
        })
    }

    pub fn imm_opcode(&self, size: &Size, dst_reg: &Register, value: Option<&String>) -> Vec<u8> {

        let code = match self {
            Self::Add => vec![0x00],
            Self::Or => vec![0x08],
            Self::Adc => vec![0x10],
            Self::Sbb => vec![0x18],
            Self::And => vec![0x20],
            Self::Sub => vec![0x28],
            Self::Xor => vec![0x30],
            Self::Cmp => vec![0x38],


            _=> Vec::new(),
        };

        if code.is_empty() {
            match self {
                Self::Mov => {
                    let mut opcode = match &size {
                        Size::DB => vec![0xB0],
                        Size::DW => vec![0x66, 0xB8],
                        Size::DD => vec![0xB8],
                        Size::DQ => vec![0x48, 0xB8],
                        _ => panic!(),
                    };
                    *opcode.last_mut().unwrap() += dst_reg.get_reg_number();
                    if let Some(src) = value {
                        let mut imm = src.parse::<usize>().unwrap().to_le_bytes().to_vec();
                        imm.resize(size.get_byte(), 0);
                        opcode.extend(imm);
                    }
                    opcode
                }
                // LLLLL
                Self::Lea => vec![0x8D, 0x05 | (dst_reg.get_reg_number() << 3)],
                _ => {
                    let mut opcode = if let Some(opcode) = self.imm_group_2_opcode(&size, &dst_reg, &value.unwrap()) {
                        opcode
                    } else if let Some(opcode) = self.group_5_opcode(&size, &dst_reg) {
                        opcode
                    } else {
                        panic!();
                    };

                    if let Some(src) = value {
                        let mut imm = src.parse::<usize>().unwrap().to_le_bytes().to_vec();
                        imm.resize(if size == &Size::DQ {4} else {size.get_byte()}, 0);
                        opcode.extend(imm);
                    }
                    opcode
                },
            }
        } else {
            let mut rex: Vec<u8> = if size == &Size::DB {
                vec![0x80]
            } else {
                match &size {
                    Size::DW => vec![0x66, 0x81],
                    Size::DD => vec![0x81],
                    Size::DQ => vec![0x48, 0x81],
                    _ => panic!(),
                }
            };
            rex.extend(vec![0b11000000 | (code[0] << 3) | dst_reg.get_reg_number()]);

            if let Some(src) = value {
                let mut imm = src.parse::<usize>().unwrap().to_le_bytes().to_vec();
                imm.resize(if size == &Size::DQ {4} else {size.get_byte()}, 0);
                rex.extend(imm);
            }
            rex
        }
    }

    pub fn opcode(
        &self,
        size: &Size,
        dst_reg: Option<&Register>,
        src_reg: Option<&Register>
    ) -> Vec<u8> {
        let bit = 
            if size == &Size::DB {
                0
            } else {
                1
            };
        let mut code = match self {
            Self::Add => vec![0x00],
            Self::Or => vec![0x08],
            Self::Adc => vec![0x10],
            Self::Sbb => vec![0x18],
            Self::And => vec![0x20],
            Self::Sub => vec![0x28],
            Self::Xor => vec![0x30],
            Self::Cmp => vec![0x38],

            Self::Push => vec![0x50],

            Self::Syscall => vec![0x0F, 0x05,],
            Self::Mov => vec![0x88],
            Self::Call => vec![0xE8],
            Self::Ret => vec![0xC3],

            Self::Jo => vec![0x70],
            Self::Jno => vec![0x71],
            Self::Jc => vec![0x72],
            Self::Jnc => vec![0x73],
            Self::Jz => vec![0x74],
            Self::Jnz => vec![0x75],
            Self::Jna => vec![0x76],
            Self::Ja => vec![0x77],
            Self::Js => vec![0x78],
            Self::Jns => vec![0x79],
            Self::Jp => vec![0x7a],
            Self::Jnp => vec![0x7B],
            Self::Jl => vec![0x7C],
            Self::Jge => vec![0x7D],
            Self::Jle => vec![0x7E],
            Self::Jg => vec![0x7F],
            _ => {
                if src_reg.is_none() || dst_reg.is_none() {
                    panic!();
                }
                if src_reg.unwrap() != &Register::Cl {
                    panic!("k");
                }
                let mut byte_code = vec![0xD2 + bit];

                byte_code.push(match &self {
                    Self::Rol => 0xC0,
                    Self::Ror => 0xC8,
                    Self::Rcl => 0xD0,
                    Self::Rcr => 0xD8,
                    Self::Shl => 0xE0,
                    Self::Shr => 0xE8,
                    Self::Sar => 0xF8,
                    _ => panic!(),
                } | dst_reg.unwrap().get_reg_number());
                let mut opcode = Vec::new();
                match &size {
                    Size::DW => {
                        opcode.push(0x66);
                    }
                    Size::DQ => {
                        opcode.push(0x48);
                    }
                    _ => {},
                }
                opcode.extend(byte_code);
                return opcode;
            }
        };
        if code.len() == 1 {
            *code.last_mut().unwrap() += bit;
            let mut opcode = Vec::new();
            match &size {
                Size::DW => {
                    opcode.push(0x66);
                    opcode.extend(code);
                    code = opcode;
                }
                Size::DQ => {
                    opcode.push(0x48);
                    opcode.extend(code);
                    code = opcode;
                }
                _ => {},
            }
        }
        if src_reg.is_some() || dst_reg.is_some() {
            code.push(self.gen_modrm(&dst_reg.unwrap(), &src_reg.unwrap()));
        }
        code
    }

    pub fn get_operand_len(&self) -> usize {
        match self {
            Self::Add => 2,
            Self::Or => 2,
            Self::Adc => 2,
            Self::Sbb => 2,
            Self::And => 2,
            Self::Sub => 2,
            Self::Xor => 2,
            Self::Cmp => 2,

            Self::Rol | Self::Ror | Self::Rcl | Self::Rcr |
            Self::Shl | Self::Shr | Self::Sar => 2,

            Self::Jo | Self::Jno | Self::Jc | Self::Jnc |
            Self::Jz | Self::Jnz | Self::Jna | Self::Ja |
            Self::Js | Self::Jns | Self::Jp | Self::Jnp |
            Self::Jl | Self::Jge | Self::Jle | Self::Jg => 1,
            

            Self::Inc | Self::Dec | Self::Call | Self::Jmp | Self::Push => 1,

            Self::Lea => 2,

            Self::Ret => 1,

            Self::Syscall => 0,
            Self::Mov => 2,
        }
    }

    pub fn gen_tkn_if_valid_str(name: &String) -> Option<Self> {
        let mnemonic = match name.as_str() {
            "add" => Mnemonic::Add,
            "or" => Mnemonic::Or,
            "adc" => Mnemonic::Adc,
            "sbb" => Mnemonic::Sbb,
            "and" => Mnemonic::And,
            "sub" => Mnemonic::Sub,
            "xor" => Mnemonic::Xor,
            "cmp" => Mnemonic::Cmp,
            
            "lea" => Mnemonic::Lea,
            
            "rol" => Self::Rol,
            "ror" => Self::Ror,
            "rcl" => Self::Rcl,
            "rcr" => Self::Rcr,
            "shl" => Self::Shl,
            "shr" => Self::Shr,
            "sar" => Self::Sar,

            "jo" => Self::Jo,
            "jno" => Self::Jno,
            "jc" => Self::Jc,
            "jnc" => Self::Jnc,
            "jz" => Self::Jz,
            "jnz" => Self::Jnz,
            "jna" => Self::Jna,
            "ja" => Self::Ja,
            "js" => Self::Js,
            "jns" => Self::Jns,
            "jp" => Self::Jp,
            "jnp" => Self::Jnp,
            "jl" => Self::Jl,
            "jge" => Self::Jge,
            "jle" => Self::Jle,
            "jg" => Self::Jg,

            "ret" => Self::Ret,

            "inc" => Self::Inc,
            "dec" => Self::Dec,
            "call" => Self::Call,
            "jmp" => Self::Jmp,
            "push" => Self::Push,

            "syscall" => Mnemonic::Syscall,
            "mov" => Mnemonic::Mov,
            _ => return None,
        };

        Some(mnemonic)
    }

    fn gen_modrm(&self, dst: &Register, src: &Register) -> u8 {
        0b11000000 | (dst.get_reg_number() << 3) | (src.get_reg_number())
    }
}
