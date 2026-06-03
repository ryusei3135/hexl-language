use crate::{
    parse::Size,
};


#[derive(Clone, PartialEq, Debug)]
pub enum Register {
    // 64bit
    Rax, Rcx, Rdx, Rbx,
    Rsp, Rbp, Rsi, Rdi,
    R8, R9, R10, R11,
    R12, R13, R14, R15,

    Rip,
    // 32bit
    Eax, Ecx, Edx, Ebx,
    Esp, Ebp, Esi, Edi,

    R8D, R9D, R10D, R11D,
    R12D, R13D, R14D, R15D,

    // 16bit
    Ax, Cx, Dx, Bx,
    Sp, Bp, Si, Di,
    R8W, R9W, R10W, R11W,
    R12W, R13W, R14W, R15W,
    // 8bit
    Al, Cl, Dl, Bl,
    Ah, Ch, Dh, Bh,
    R8B, R9B, R10B, R11B,
    R12B, R13B, R14B, R15B,
}

impl Register {
    pub fn is_sp(&self) -> bool {
        if self == &Self::Esp || self == &Self::Rsp {
            true
        } else {
            false
        }
    }

    pub fn decide_reg(value: &String) -> Option<Register> {
        Some(match value.as_str() {
            "rax" => Register::Rax,
            "rcx" => Register::Rcx,
            "rdx" => Register::Rdx,
            "rbx" => Register::Rbx,
            "rsp" => Register::Rsp,
            "rbp" => Register::Rbp,
            "rsi" => Register::Rsi,
            "rdi" => Register::Rdi,
            "rip" => Register::Rip,
            "r8" => Register::R8,
            "r9" => Register::R9,
            "r10" => Register::R10,
            "r11" => Register::R11,
            "r12" => Register::R12,
            "r13" => Register::R13,
            "r14" => Register::R14,
            "r15" => Register::R15,

            "eax" => Register::Eax,
            "ecx" => Register::Ecx,
            "edx" => Register::Edx,
            "ebx" => Register::Ebx,
            "esp" => Register::Esp,
            "ebp" => Register::Ebp,
            "esi" => Register::Esi,
            "edi" => Register::Edi,
            "r8d" => Register::R8D,
            "r9d" => Register::R9D,
            "r10d" => Register::R10D,
            "r11d" => Register::R11D,
            "r12d" => Register::R12D,
            "r13d" => Register::R13D,
            "r14d" => Register::R14D,
            "r15d" => Register::R15D,

            "ax" => Register::Ax,
            "cx" => Register::Cx,
            "dx" => Register::Dx,
            "bx" => Register::Bx,
            "sp" => Register::Sp,
            "bp" => Register::Bp,
            "si" => Register::Si,
            "di" => Register::Di,
            "r8w" => Register::R8W,
            "r9w" => Register::R9W,
            "r10w" => Register::R10W,
            "r11w" => Register::R11W,
            "r12w" => Register::R12W,
            "r13w" => Register::R13W,
            "r14w" => Register::R14W,
            "r15w" => Register::R15W,

            "al" => Register::Al,
            "cl" => Register::Cl,
            "dl" => Register::Dl,
            "bl" => Register::Bl,
            "ah" => Register::Ah,
            "ch" => Register::Ch,
            "dh" => Register::Dh,
            "bh" => Register::Bh,
            "r8b" => Register::R8B,
            "r9b" => Register::R9B,
            "r10b" => Register::R10B,
            "r11b" => Register::R11B,
            "r12b" => Register::R12B,
            "r13b" => Register::R13B,
            "r14b" => Register::R14B,
            "r15b" => Register::R15B,

            _ => return None,
        })
    }

    pub fn get_reg_byte(&self) -> Size {
        match self {
            Self::Ax | Self::Cx | Self::Dx | Self::Bx
            | Self::Sp | Self::Bp | Self::Si | Self::Di => Size::DW,
            Self::Eax | Self::Ecx | Self::Edx | Self::Ebx
            | Self::Esp | Self::Ebp | Self::Esi | Self::Edi => Size::DD,
            Self::Rax | Self::Rcx | Self::Rdx | Self::Rbx
            | Self::Rsp | Self::Rbp | Self::Rsi | Self::Rdi | Self::Rip => Size::DQ,
            Self::Al | Self::Cl | Self::Dl | Self::Bl
            | Self::Ah | Self::Ch | Self::Dh | Self::Bh => Size::DB,
            Self::R8 | Self::R9 | Self::R10 | Self::R11 |
            Self::R12 | Self::R13 | Self::R14 | Self::R15 => Size::DQ,
            Self::R8D | Self::R9D | Self::R10D | Self::R11D |
            Self::R12D | Self::R13D | Self::R14D | Self::R15D => Size::DD,
            Self::R8W | Self::R9W | Self::R10W | Self::R11W |
            Self::R12W | Self::R13W | Self::R14W | Self::R15W => Size::DW,
            Self::R8B | Self::R9B | Self::R10B | Self::R11B |
            Self::R12B | Self::R13B | Self::R14B | Self::R15B => Size::DB,
        }
    }

    pub fn get_reg_number(&self) -> u8 {
        match self {
            Self::Al | Self::Eax | Self::Ax | Self::Rax => 0,
            Self::Cl | Self::Ecx | Self::Cx | Self::Rcx => 1,
            Self::Dl | Self::Edx | Self::Dx | Self::Rdx => 2,
            Self::Bl | Self::Ebx | Self::Bx | Self::Rbx => 3,
            Self::Ah | Self::Esp | Self::Sp | Self::Rsp => 4,
            Self::Ch | Self::Ebp | Self::Bp | Self::Rbp => 5,
            Self::Dh | Self::Esi | Self::Si | Self::Rsi => 6,
            Self::Bh | Self::Edi | Self::Di | Self::Rdi => 7,
            Self::R8 | Self::R8D | Self::R8W | Self::R8B => 0,
            Self::R9 | Self::R9D | Self::R9W | Self::R9B => 1,
            Self::R10 | Self::R10D | Self::R10W | Self::R10B => 2,
            Self::R11 | Self::R11D | Self::R11W | Self::R11B => 3,
            Self::R12 | Self::R12D | Self::R12W | Self::R12B => 4,
            Self::R13 | Self::R13D | Self::R13W | Self::R13B => 5,
            Self::R14 | Self::R14D | Self::R14W | Self::R14B => 6,
            Self::R15 | Self::R15D | Self::R15W | Self::R15B => 7,
            Self::Rip => 5,
        }
    }
}
