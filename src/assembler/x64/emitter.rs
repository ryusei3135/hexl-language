use std::collections::HashMap;

mod inst_table;

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Instruction(Instruction),
    Label(String),
    Directive(Directive),
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub mnemonic: String,
    pub operands: Vec<Operand>,
}

#[derive(Debug, Clone)]
pub enum Operand {
    Register(Reg),
    Immediate(i64),
    Symbol(String),
    Memory(MemoryOperand),
}

#[derive(Debug, Clone)]
pub struct MemoryOperand {
    pub displacement: i64,
    pub base: Option<Reg>,
    pub index: Option<Reg>,
    pub scale: u8,
    pub symbol: Option<String>,
}


// `Reg` の定義は `reg.rs` (レジスタを追加する場合はそちらを編集)
pub use super::reg::Reg;

// ============================================================
// Directives
// ============================================================

#[derive(Debug, Clone)]
pub enum Directive {
    Text,
    Data,

    Extern(String),
    Global(String),

    Db(Vec<DataValue>),
    Dw(Vec<DataValue>),
    Dd(Vec<DataValue>),
    Dq(Vec<DataValue>),

    // `.align N` : 現在のセクションを N バイト境界まで詰める
    Align(usize),

    // `.zero N` / `.skip N` / `.space N` : N バイトの 0 を出力する
    Zero(usize),
}

#[derive(Debug, Clone)]
pub enum DataValue {
    Integer(i64),
    String(Vec<u8>),
    Symbol(String),
}

// ============================================================
// Section
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Text,
    Data,
}

// ============================================================
// Relocation / Fixup
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixupKind {
    Rel32,
    RipRelative32,
    Abs32,
    Abs64,
}

#[derive(Debug, Clone)]
pub struct Fixup {
    pub section: Section,
    pub offset: usize,
    pub symbol: String,
    pub kind: FixupKind,
}

// ============================================================
// Emitter
// ============================================================

pub struct Emitter {
    pub text: Vec<u8>,
    pub data: Vec<u8>,

    pub current_section: Section,

    pub text_labels: HashMap<String, usize>,
    pub data_labels: HashMap<String, usize>,

    pub globals: Vec<String>,
    pub externs: Vec<String>,

    pub fixups: Vec<Fixup>,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            text: Vec::new(),
            data: Vec::new(),

            current_section: Section::Text,

            text_labels: HashMap::new(),
            data_labels: HashMap::new(),

            globals: Vec::new(),
            externs: Vec::new(),

            fixups: Vec::new(),
        }
    }


    pub fn emit_program(&mut self, program: &Program) -> Result<(), String> {
        for statement in &program.statements {
            self.emit_statement(statement)?;
        }

        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), String> {
        self.resolve_fixups()
    }

    fn emit_statement(&mut self, statement: &Statement) -> Result<(), String> {
        match statement {
            Statement::Instruction(inst) => self.emit_instruction(inst),

            Statement::Label(name) => self.emit_label(name),

            Statement::Directive(dir) => self.emit_directive(dir),
        }
    }


    fn emit_label(&mut self, name: &str) -> Result<(), String> {
        let offset = self.current_offset();

        let table = match self.current_section {
            Section::Text => &mut self.text_labels,
            Section::Data => &mut self.data_labels,
        };

        if table.contains_key(name) {
            return Err(format!("duplicate label: {}", name));
        }

        table.insert(name.to_string(), offset);

        Ok(())
    }


    fn emit_directive(&mut self, directive: &Directive) -> Result<(), String> {
        match directive {
            Directive::Text => {
                self.current_section = Section::Text;
            }
            Directive::Data => {
                self.current_section = Section::Data;
            }
            Directive::Extern(name) => {
                if !self.externs.contains(name) {
                    self.externs.push(name.clone());
                }
            }
            Directive::Global(name) => {
                if !self.globals.contains(name) {
                    self.globals.push(name.clone());
                }
            }

            Directive::Db(values) => {
                self.emit_data(values, 1)?;
            }

            Directive::Dw(values) => {
                self.emit_data(values, 2)?;
            }

            Directive::Dd(values) => {
                self.emit_data(values, 4)?;
            }

            Directive::Dq(values) => {
                self.emit_data(values, 8)?;
            }

            Directive::Align(align) => {
                self.emit_align(*align)?;
            }

            Directive::Zero(count) => {
                self.emit_zero(*count);
            }
        }

        Ok(())
    }

    fn current_buffer(&mut self) -> &mut Vec<u8> {
        match self.current_section {
            Section::Text => &mut self.text,
            Section::Data => &mut self.data,
        }
    }

    fn emit_align(&mut self, align: usize) -> Result<(), String> {
        if align == 0 {
            return Err("`.align` requires a positive value".into());
        }

        // .text は NOP (0x90)、それ以外は 0 で埋める
        let pad_byte = match self.current_section {
            Section::Text => 0x90,
            Section::Data => 0x00,
        };

        let buffer = self.current_buffer();
        let remainder = buffer.len() % align;

        if remainder != 0 {
            let padding = align - remainder;
            buffer.extend(std::iter::repeat(pad_byte).take(padding));
        }

        Ok(())
    }

    fn emit_zero(&mut self, count: usize) {
        let buffer = self.current_buffer();
        buffer.extend(std::iter::repeat(0u8).take(count));
    }

    fn emit_data(&mut self, values: &[DataValue], size: usize) -> Result<(), String> {
        if self.current_section != Section::Data {
            return Err("data directive outside .data".into());
        }

        for value in values {
            match value {
                DataValue::Integer(value) => {
                    let bytes = value.to_le_bytes();

                    self.data.extend_from_slice(&bytes[..size]);
                }

                DataValue::String(bytes) => {
                    if size != 1 {
                        return Err("string is only valid for db".into());
                    }

                    self.data.extend_from_slice(bytes);
                }

                DataValue::Symbol(symbol) => {
                    let offset = self.data.len();

                    for _ in 0..size {
                        self.data.push(0);
                    }

                    self.fixups.push(Fixup {
                        section: Section::Data,
                        offset,
                        symbol: symbol.clone(),
                        kind: match size {
                            4 => FixupKind::Abs32,
                            8 => FixupKind::Abs64,
                            _ => {
                                return Err("unsupported data relocation size".into());
                            }
                        },
                    });
                }
            }
        }

        Ok(())
    }

    // ========================================================
    // Instruction dispatcher
    // ========================================================

    fn emit_instruction(&mut self, inst: &Instruction) -> Result<(), String> {
        // ニーモニック -> 処理 の対応表は `emitter/inst_table.rs`
        inst_table::dispatch(self, inst)
    }

    // ========================================================
    // push
    // ========================================================

    fn emit_push(&mut self, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 1 {
            return Err("push requires one operand".into());
        }

        match &operands[0] {
            Operand::Register(reg) => {
                if reg.bits() != 64 {
                    return Err("push requires 64-bit register".into());
                }
                let id = reg.id();
                if id >= 8 {
                    self.emit_u8(0x41);
                }
                self.emit_u8(0x50 + (id & 7));
                Ok(())
            }

            Operand::Immediate(value) => {
                if *value >= -128 && *value <= 127 {
                    self.emit_u8(0x6A);
                    self.emit_u8(*value as i8 as u8);
                } else {
                    self.emit_u8(0x68);
                    self.emit_i32(*value as i32);
                }

                Ok(())
            }

            _ => Err("unsupported push operand".into()),
        }
    }

    // ========================================================
    // pop
    // ========================================================

    fn emit_pop(&mut self, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 1 {
            return Err("pop requires one operand".into());
        }

        match &operands[0] {
            Operand::Register(reg) => {
                if reg.bits() != 64 {
                    return Err("pop requires 64-bit register".into());
                }

                let id = reg.id();

                if id >= 8 {
                    self.emit_u8(0x41);
                }

                self.emit_u8(0x58 + (id & 7));

                Ok(())
            }

            _ => Err("unsupported pop operand".into()),
        }
    }

    // ========================================================
    // mov
    // ========================================================

    fn emit_mov(&mut self, operands: &[Operand], size_bits: u32) -> Result<(), String> {
        if operands.len() != 2 {
            return Err("mov requires two operands".into());
        }

        let src = &operands[0];
        let dst = &operands[1];

        match (src, dst) {
            (Operand::Immediate(value), Operand::Register(dst)) => {
                self.emit_mov_immediate(*value, *dst)
            }
            (Operand::Register(src), Operand::Register(dst)) => self.emit_mov_reg_reg(*src, *dst),
            (Operand::Register(src), Operand::Memory(mem)) => self.emit_mov_reg_mem(*src, mem),
            (Operand::Memory(mem), Operand::Register(dst)) => self.emit_mov_mem_reg(mem, *dst),
            (Operand::Symbol(symbol), Operand::Register(dst)) => {
                self.emit_mov_symbol_reg(symbol, *dst)
            }
            (Operand::Immediate(value), Operand::Memory(mem)) => {
                self.emit_mov_immediate_mem(*value, mem, size_bits)
            }
            _ => Err("unsupported mov operands".into()),
        }
    }

    // ========================================================
    // mov immediate -> register
    // ========================================================

    fn emit_mov_immediate(&mut self, value: i64, dst: Reg) -> Result<(), String> {
        match dst.bits() {
            64 => {
                let id = dst.id();
                self.emit_rex(true, false, false, id >= 8);
                self.emit_u8(0xB8 + (id & 7));
                self.emit_u64(value as u64);
            }
            32 => {
                let id = dst.id();
                self.emit_rex(false, false, false, id >= 8);
                self.emit_u8(0xB8 + (id & 7));
                self.emit_u32(value as u32);
            }
            16 => {
                self.emit_u8(0x66);
                let id = dst.id();
                self.emit_rex(false, false, false, id >= 8);
                self.emit_u8(0xB8 + (id & 7));
                self.emit_u16(value as u16);
            }
            _ => {
                return Err("unsupported mov immediate size".into());
            }
        }

        Ok(())
    }

    // ========================================================
    // mov register -> register
    // ========================================================

    fn emit_mov_reg_reg(&mut self, src: Reg, dst: Reg) -> Result<(), String> {
        if src.bits() != dst.bits() {
            return Err("register size mismatch".into());
        }

        let bits = src.bits();

        if bits == 16 {
            self.emit_u8(0x66);
        }

        let w = bits == 64;

        self.emit_rex(w, src.id() >= 8, false, dst.id() >= 8);

        match bits {
            8 => self.emit_u8(0x88),
            16 | 32 | 64 => self.emit_u8(0x89),

            _ => {
                return Err("invalid register size".into());
            }
        }

        let modrm = 0b11_000_000 | ((src.id() & 7) << 3) | (dst.id() & 7);

        self.emit_u8(modrm);

        Ok(())
    }

    // ========================================================
    // mov register -> memory
    // ========================================================

    fn emit_mov_reg_mem(&mut self, src: Reg, mem: &MemoryOperand) -> Result<(), String> {
        let bits = src.bits();

        if bits == 16 {
            self.emit_u8(0x66);
        }

        self.emit_rex(
            bits == 64,
            src.id() >= 8,
            self.memory_index_extended(mem),
            self.memory_base_extended(mem),
        );

        match bits {
            8 => self.emit_u8(0x88),
            16 | 32 | 64 => self.emit_u8(0x89),

            _ => {
                return Err("invalid source register size".into());
            }
        }

        self.emit_memory_modrm(src.id(), mem)
    }

    // ========================================================
    // mov immediate -> memory
    //
    // movl $10, -4(%rbp)  (size_bits comes from the mnemonic
    // suffix, since an immediate operand carries no size and a
    // bare memory operand has no register to infer it from)
    // ========================================================

    fn emit_mov_immediate_mem(
        &mut self,
        value: i64,
        mem: &MemoryOperand,
        size_bits: u32,
    ) -> Result<(), String> {
        if size_bits == 16 {
            self.emit_u8(0x66);
        }

        self.emit_rex(
            size_bits == 64,
            false,
            self.memory_index_extended(mem),
            self.memory_base_extended(mem),
        );

        match size_bits {
            8 => self.emit_u8(0xC6),
            16 | 32 | 64 => self.emit_u8(0xC7),
            _ => return Err("unsupported mov immediate size".into()),
        }

        self.emit_memory_modrm(0, mem)?;

        match size_bits {
            8 => self.emit_u8(value as i8 as u8),
            16 => self.emit_u16(value as u16),
            32 | 64 => self.emit_i32(value as i32),
            _ => return Err("unsupported mov immediate size".into()),
        }

        Ok(())
    }

    // ========================================================
    // mov memory -> register
    // ========================================================

    fn emit_mov_mem_reg(&mut self, mem: &MemoryOperand, dst: Reg) -> Result<(), String> {
        let bits = dst.bits();

        if bits == 16 {
            self.emit_u8(0x66);
        }

        self.emit_rex(
            bits == 64,
            dst.id() >= 8,
            self.memory_index_extended(mem),
            self.memory_base_extended(mem),
        );

        match bits {
            8 => self.emit_u8(0x8A),
            16 | 32 | 64 => self.emit_u8(0x8B),

            _ => {
                return Err("invalid destination register size".into());
            }
        }

        self.emit_memory_modrm(dst.id(), mem)
    }

    // ========================================================
    // mov symbol -> register
    //
    // mov foo, %rax
    // ========================================================

    fn emit_mov_symbol_reg(&mut self, symbol: &str, dst: Reg) -> Result<(), String> {
        if dst.bits() != 64 {
            return Err("symbol mov currently requires 64-bit register".into());
        }

        // mov r64, moffs64
        //
        // For a relocatable object this is represented
        // with an absolute 64-bit relocation.
        self.emit_rex(true, false, false, false);

        self.emit_u8(0xA1);

        let offset = self.current_offset();

        self.emit_u64(0);

        self.fixups.push(Fixup {
            section: self.current_section,
            offset,
            symbol: symbol.to_string(),
            kind: FixupKind::Abs64,
        });

        Ok(())
    }

    // ========================================================
    // add
    // ========================================================

    fn emit_add(&mut self, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 2 {
            return Err("add requires two operands".into());
        }

        match (&operands[0], &operands[1]) {
            (Operand::Immediate(value), Operand::Register(dst)) => {
                self.emit_group_imm(*value, *dst, 0)
            }

            (Operand::Register(src), Operand::Register(dst)) => {
                self.emit_binary_reg_reg(*src, *dst, 0x01)
            }

            (Operand::Register(src), Operand::Memory(mem)) => {
                self.emit_binary_reg_mem(*src, mem, 0x01)
            }

            (Operand::Memory(mem), Operand::Register(dst)) => {
                self.emit_binary_mem_reg(mem, *dst, 0x03)
            }

            _ => Err("unsupported add operands".into()),
        }
    }

    // ========================================================
    // sub
    // ========================================================

    fn emit_sub(&mut self, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 2 {
            return Err("sub requires two operands".into());
        }

        match (&operands[0], &operands[1]) {
            (Operand::Immediate(value), Operand::Register(dst)) => {
                self.emit_group_imm(*value, *dst, 5)
            }

            (Operand::Register(src), Operand::Register(dst)) => {
                self.emit_binary_reg_reg(*src, *dst, 0x29)
            }

            (Operand::Register(src), Operand::Memory(mem)) => {
                self.emit_binary_reg_mem(*src, mem, 0x29)
            }

            (Operand::Memory(mem), Operand::Register(dst)) => {
                self.emit_binary_mem_reg(mem, *dst, 0x2B)
            }

            _ => Err("unsupported sub operands".into()),
        }
    }

    // ========================================================
    // add/sub immediate
    // ========================================================

    fn emit_group_imm(&mut self, value: i64, dst: Reg, group: u8) -> Result<(), String> {
        if dst.bits() == 16 {
            self.emit_u8(0x66);
        }

        self.emit_rex(dst.bits() == 64, false, false, dst.id() >= 8);

        if value >= -128 && value <= 127 {
            self.emit_u8(0x83);

            self.emit_u8(0b11_000_000 | ((group & 7) << 3) | (dst.id() & 7));

            self.emit_u8(value as i8 as u8);
        } else {
            self.emit_u8(0x81);

            self.emit_u8(0b11_000_000 | ((group & 7) << 3) | (dst.id() & 7));

            self.emit_i32(value as i32);
        }

        Ok(())
    }

    // ========================================================
    // binary register/register
    // ========================================================

    fn emit_binary_reg_reg(&mut self, src: Reg, dst: Reg, opcode: u8) -> Result<(), String> {
        if src.bits() != dst.bits() {
            return Err("register size mismatch".into());
        }

        if src.bits() == 16 {
            self.emit_u8(0x66);
        }

        self.emit_rex(src.bits() == 64, src.id() >= 8, false, dst.id() >= 8);

        self.emit_u8(opcode);

        self.emit_u8(0b11_000_000 | ((src.id() & 7) << 3) | (dst.id() & 7));

        Ok(())
    }

    // ========================================================
    // binary register -> memory
    // ========================================================

    fn emit_binary_reg_mem(
        &mut self,
        src: Reg,
        mem: &MemoryOperand,
        opcode: u8,
    ) -> Result<(), String> {
        if src.bits() == 16 {
            self.emit_u8(0x66);
        }

        self.emit_rex(
            src.bits() == 64,
            src.id() >= 8,
            self.memory_index_extended(mem),
            self.memory_base_extended(mem),
        );

        self.emit_u8(opcode);

        self.emit_memory_modrm(src.id(), mem)
    }

    // ========================================================
    // binary memory -> register
    // ========================================================

    fn emit_binary_mem_reg(
        &mut self,
        mem: &MemoryOperand,
        dst: Reg,
        opcode: u8,
    ) -> Result<(), String> {
        if dst.bits() == 16 {
            self.emit_u8(0x66);
        }

        self.emit_rex(
            dst.bits() == 64,
            dst.id() >= 8,
            self.memory_index_extended(mem),
            self.memory_base_extended(mem),
        );

        self.emit_u8(opcode);

        self.emit_memory_modrm(dst.id(), mem)
    }

    // ========================================================
    // imul
    // ========================================================

    fn emit_imul(&mut self, operands: &[Operand]) -> Result<(), String> {
        match operands.len() {
            1 => self.emit_unary_group(operands, 5),

            2 => {
                let src = match &operands[0] {
                    Operand::Register(r) => *r,
                    _ => {
                        return Err("imul source must be register".into());
                    }
                };

                let dst = match &operands[1] {
                    Operand::Register(r) => *r,
                    _ => {
                        return Err("imul destination must be register".into());
                    }
                };

                if src.bits() == 16 {
                    self.emit_u8(0x66);
                }

                self.emit_rex(src.bits() == 64, dst.id() >= 8, false, src.id() >= 8);

                self.emit_u8(0x0F);
                self.emit_u8(0xAF);

                self.emit_u8(0b11_000_000 | ((dst.id() & 7) << 3) | (src.id() & 7));

                Ok(())
            }

            3 => {
                let src = match &operands[0] {
                    Operand::Immediate(v) => *v,
                    _ => {
                        return Err("imul immediate form requires immediate".into());
                    }
                };

                let src_reg = match &operands[1] {
                    Operand::Register(r) => *r,
                    _ => {
                        return Err("imul source must be register".into());
                    }
                };

                let dst = match &operands[2] {
                    Operand::Register(r) => *r,
                    _ => {
                        return Err("imul destination must be register".into());
                    }
                };

                if src_reg.bits() != dst.bits() {
                    return Err("imul register size mismatch".into());
                }

                if dst.bits() == 16 {
                    self.emit_u8(0x66);
                }

                self.emit_rex(dst.bits() == 64, dst.id() >= 8, false, src_reg.id() >= 8);

                if src >= -128 && src <= 127 {
                    self.emit_u8(0x6B);

                    self.emit_u8(0b11_000_000 | ((dst.id() & 7) << 3) | (src_reg.id() & 7));

                    self.emit_u8(src as i8 as u8);
                } else {
                    self.emit_u8(0x69);

                    self.emit_u8(0b11_000_000 | ((dst.id() & 7) << 3) | (src_reg.id() & 7));

                    self.emit_i32(src as i32);
                }

                Ok(())
            }

            _ => Err("invalid imul operand count".into()),
        }
    }

    // ========================================================
    // mul
    // ========================================================

    fn emit_mul(&mut self, operands: &[Operand]) -> Result<(), String> {
        self.emit_unary_group(operands, 4)
    }

    // ========================================================
    // div
    // ========================================================

    fn emit_div(&mut self, operands: &[Operand]) -> Result<(), String> {
        self.emit_unary_group(operands, 6)
    }

    // ========================================================
    // idiv
    // ========================================================

    fn emit_idiv(&mut self, operands: &[Operand]) -> Result<(), String> {
        self.emit_unary_group(operands, 7)
    }

    // ========================================================
    // unary F7 group
    // ========================================================

    fn emit_unary_group(&mut self, operands: &[Operand], group: u8) -> Result<(), String> {
        if operands.len() != 1 {
            return Err("instruction requires one operand".into());
        }

        match &operands[0] {
            Operand::Register(reg) => {
                if reg.bits() == 16 {
                    self.emit_u8(0x66);
                }

                self.emit_rex(reg.bits() == 64, false, false, reg.id() >= 8);

                self.emit_u8(0xF7);

                self.emit_u8(0b11_000_000 | ((group & 7) << 3) | (reg.id() & 7));

                Ok(())
            }

            Operand::Memory(mem) => {
                self.emit_rex(
                    true,
                    false,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                );

                self.emit_u8(0xF7);

                self.emit_memory_modrm(group, mem)
            }

            _ => Err("invalid unary operand".into()),
        }
    }

    // ========================================================
    // cltd / cdq
    // ========================================================

    fn emit_cltd(&mut self) -> Result<(), String> {
        self.emit_u8(0x99);
        Ok(())
    }

    // ========================================================
    // cmp
    // ========================================================

    fn emit_cmp(&mut self, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 2 {
            return Err("cmp requires two operands".into());
        }

        match (&operands[0], &operands[1]) {
            (Operand::Immediate(value), Operand::Register(dst)) => {
                self.emit_group_imm(*value, *dst, 7)
            }

            (Operand::Register(src), Operand::Register(dst)) => {
                self.emit_binary_reg_reg(*src, *dst, 0x39)
            }

            (Operand::Register(src), Operand::Memory(mem)) => {
                self.emit_binary_reg_mem(*src, mem, 0x39)
            }

            (Operand::Memory(mem), Operand::Register(dst)) => {
                self.emit_binary_mem_reg(mem, *dst, 0x3B)
            }

            _ => Err("unsupported cmp operands".into()),
        }
    }

    // ========================================================
    // lea
    // ========================================================

    fn emit_lea(&mut self, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 2 {
            return Err("lea requires two operands".into());
        }

        let mem = match &operands[0] {
            Operand::Memory(m) => m,
            _ => {
                return Err("lea source must be memory".into());
            }
        };

        let dst = match &operands[1] {
            Operand::Register(r) => *r,
            _ => {
                return Err("lea destination must be register".into());
            }
        };

        if dst.bits() == 16 {
            self.emit_u8(0x66);
        }

        self.emit_rex(
            dst.bits() == 64,
            dst.id() >= 8,
            self.memory_index_extended(mem),
            self.memory_base_extended(mem),
        );

        self.emit_u8(0x8D);

        self.emit_memory_modrm(dst.id(), mem)
    }

    // ========================================================
    // jmp
    // ========================================================

    fn emit_jmp(&mut self, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 1 {
            return Err("jmp requires one operand".into());
        }

        let symbol = match &operands[0] {
            Operand::Symbol(s) => s.clone(),

            _ => {
                return Err("jmp target must be symbol".into());
            }
        };

        self.emit_u8(0xE9);

        let offset = self.current_offset();

        self.emit_i32(0);

        self.fixups.push(Fixup {
            section: self.current_section,
            offset,
            symbol,
            kind: FixupKind::Rel32,
        });

        Ok(())
    }

    // ========================================================
    // conditional jump
    // ========================================================

    fn emit_jcc(&mut self, condition: Condition, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 1 {
            return Err("conditional jump requires one operand".into());
        }

        let symbol = match &operands[0] {
            Operand::Symbol(s) => s.clone(),

            _ => {
                return Err("jump target must be symbol".into());
            }
        };

        self.emit_u8(0x0F);

        self.emit_u8(condition_opcode(condition));

        let offset = self.current_offset();

        self.emit_i32(0);

        self.fixups.push(Fixup {
            section: self.current_section,
            offset,
            symbol,
            kind: FixupKind::Rel32,
        });

        Ok(())
    }

    // ========================================================
    // call
    // ========================================================

    fn emit_call(&mut self, operands: &[Operand]) -> Result<(), String> {
        if operands.len() != 1 {
            return Err("call requires one operand".into());
        }

        match &operands[0] {
            Operand::Symbol(symbol) => {
                self.emit_u8(0xE8);

                let offset = self.current_offset();

                self.emit_i32(0);

                self.fixups.push(Fixup {
                    section: self.current_section,
                    offset,
                    symbol: symbol.clone(),
                    kind: FixupKind::Rel32,
                });

                Ok(())
            }

            Operand::Register(reg) => {
                self.emit_rex(true, false, false, reg.id() >= 8);

                self.emit_u8(0xFF);

                self.emit_u8(0b11_010_000 | (reg.id() & 7));

                Ok(())
            }

            Operand::Memory(mem) => {
                self.emit_rex(
                    true,
                    false,
                    self.memory_index_extended(mem),
                    self.memory_base_extended(mem),
                );

                self.emit_u8(0xFF);

                self.emit_memory_modrm(2, mem)
            }

            _ => Err("unsupported call operand".into()),
        }
    }

    // ========================================================
    // ret
    // ========================================================

    fn emit_ret(&mut self, operands: &[Operand]) -> Result<(), String> {
        if !operands.is_empty() {
            return Err("ret takes no operands".into());
        }

        self.emit_u8(0xC3);

        Ok(())
    }

    // ========================================================
    // leave
    // ========================================================

    fn emit_leave(&mut self, operands: &[Operand]) -> Result<(), String> {
        if !operands.is_empty() {
            return Err("leave takes no operands".into());
        }

        self.emit_u8(0xC9);

        Ok(())
    }

    // ========================================================
    // syscall
    // ========================================================

    fn emit_syscall(&mut self, operands: &[Operand]) -> Result<(), String> {
        if !operands.is_empty() {
            return Err("syscall takes no operands".into());
        }

        self.emit_u8(0x0F);
        self.emit_u8(0x05);

        Ok(())
    }


    fn emit_memory_modrm(&mut self, reg_field: u8, mem: &MemoryOperand) -> Result<(), String> {
        // RIP-relative
        if mem.base == Some(Reg::Rip) {
            if mem.index.is_some() {
                return Err("RIP-relative addressing cannot have index".into());
            }

            let modrm = ((reg_field & 7) << 3) | 0b101;

            self.emit_u8(modrm);

            let offset = self.current_offset();

            self.emit_i32(0);

            if let Some(symbol) = &mem.symbol {
                self.fixups.push(Fixup {
                    section: self.current_section,
                    offset,
                    symbol: symbol.clone(),
                    kind: FixupKind::RipRelative32,
                });
            } else {
                self.patch_i32(self.current_section, offset, mem.displacement as i32)?;
            }

            return Ok(());
        }

        let base = match mem.base {
            Some(base) => base,
            None => {
                return Err("memory operand requires base register".into());
            }
        };

        let base_id = base.id();

        let has_index = mem.index.is_some();

        let needs_sib = has_index || (base_id & 7) == 4;

        let displacement = mem.displacement;

        let (mod_bits, disp_size) = if displacement == 0 && (base_id & 7) != 5 {
            (0b00, 0)
        } else if displacement >= -128 && displacement <= 127 {
            (0b01, 1)
        } else {
            (0b10, 4)
        };

        let rm = if needs_sib { 0b100 } else { base_id & 7 };

        let modrm = (mod_bits << 6) | ((reg_field & 7) << 3) | rm;

        self.emit_u8(modrm);

        if needs_sib {
            let scale_bits = match mem.scale {
                1 => 0b00,
                2 => 0b01,
                4 => 0b10,
                8 => 0b11,

                _ => {
                    return Err("invalid SIB scale".into());
                }
            };

            let index_bits = match mem.index {
                Some(index) => index.id() & 7,
                None => 0b100,
            };

            let sib = (scale_bits << 6) | (index_bits << 3) | (base_id & 7);

            self.emit_u8(sib);
        }

        match disp_size {
            0 => {}

            1 => {
                self.emit_u8(displacement as i8 as u8);
            }

            4 => {
                self.emit_i32(displacement as i32);
            }

            _ => unreachable!(),
        }

        Ok(())
    }

    // ========================================================
    // REX
    //
    // 0100WRXB
    // ========================================================

    fn emit_rex(&mut self, w: bool, r: bool, x: bool, b: bool) {
        let rex = 0x40 | ((w as u8) << 3) | ((r as u8) << 2) | ((x as u8) << 1) | (b as u8);
        if rex != 0x40 {
            self.emit_u8(rex);
        }
    }


    fn memory_base_extended(&self, mem: &MemoryOperand) -> bool {
        match mem.base {
            Some(Reg::Rip) | None => false,
            Some(reg) => reg.id() >= 8,
        }
    }

    fn memory_index_extended(&self, mem: &MemoryOperand) -> bool {
        match mem.index {
            Some(reg) => reg.id() >= 8,
            None => false,
        }
    }


    fn resolve_fixups(&mut self) -> Result<(), String> {
        let fixups = self.fixups.clone();

        for fixup in fixups {
            let target = self.find_symbol(&fixup.symbol);

            match target {
                Some((target_section, target_offset)) => match fixup.kind {
                    FixupKind::Rel32 => {
                        self.resolve_rel32(&fixup, target_section, target_offset)?;
                    }

                    FixupKind::RipRelative32 => {
                        self.resolve_rip32(&fixup, target_section, target_offset)?;
                    }

                    FixupKind::Abs32 => {
                        self.resolve_abs32(&fixup, target_section, target_offset)?;
                    }

                    FixupKind::Abs64 => {
                        self.resolve_abs64(&fixup, target_section, target_offset)?;
                    }
                },

                None => {
                    if self.externs.contains(&fixup.symbol) {
                        continue;
                    }

                    return Err(format!("undefined symbol: {}", fixup.symbol));
                }
            }
        }

        Ok(())
    }


    fn resolve_rel32(
        &mut self,
        fixup: &Fixup,
        target_section: Section,
        target_offset: usize,
    ) -> Result<(), String> {
        if fixup.section != target_section {
            return Err("rel32 target is in another section".into());
        }

        let next = fixup.offset + 4;

        let relative = target_offset as i64 - next as i64;

        if relative < i32::MIN as i64 || relative > i32::MAX as i64 {
            return Err("rel32 out of range".into());
        }

        self.patch_i32(fixup.section, fixup.offset, relative as i32)
    }


    fn resolve_rip32(
        &mut self,
        fixup: &Fixup,
        target_section: Section,
        target_offset: usize,
    ) -> Result<(), String> {
        if fixup.section != target_section {
            return Ok(());
        }

        let next = fixup.offset + 4;

        let relative = target_offset as i64 - next as i64;

        if relative < i32::MIN as i64 || relative > i32::MAX as i64 {
            return Err("RIP-relative displacement out of range".into());
        }

        self.patch_i32(fixup.section, fixup.offset, relative as i32)
    }

    fn resolve_abs32(
        &mut self,
        fixup: &Fixup,
        _target_section: Section,
        target_offset: usize,
    ) -> Result<(), String> {
        if target_offset > u32::MAX as usize {
            return Err("absolute 32-bit relocation overflow".into());
        }

        self.patch_u32(fixup.section, fixup.offset, target_offset as u32)
    }


    fn resolve_abs64(
        &mut self,
        fixup: &Fixup,
        _target_section: Section,
        target_offset: usize,
    ) -> Result<(), String> {
        self.patch_u64(fixup.section, fixup.offset, target_offset as u64)
    }


    fn find_symbol(&self, symbol: &str) -> Option<(Section, usize)> {
        if let Some(offset) = self.text_labels.get(symbol) {
            return Some((Section::Text, *offset));
        }

        if let Some(offset) = self.data_labels.get(symbol) {
            return Some((Section::Data, *offset));
        }

        None
    }

    fn patch_i32(&mut self, section: Section, offset: usize, value: i32) -> Result<(), String> {
        self.patch_bytes(section, offset, &value.to_le_bytes())
    }

    fn patch_u32(&mut self, section: Section, offset: usize, value: u32) -> Result<(), String> {
        self.patch_bytes(section, offset, &value.to_le_bytes())
    }

    fn patch_u64(&mut self, section: Section, offset: usize, value: u64) -> Result<(), String> {
        self.patch_bytes(section, offset, &value.to_le_bytes())
    }

    fn patch_bytes(&mut self, section: Section, offset: usize, bytes: &[u8]) -> Result<(), String> {
        let buffer = match section {
            Section::Text => &mut self.text,
            Section::Data => &mut self.data,
        };

        let end = offset + bytes.len();

        if end > buffer.len() {
            return Err("fixup offset out of range".into());
        }

        buffer[offset..end].copy_from_slice(bytes);

        Ok(())
    }

    // ========================================================
    // Output helpers
    // ========================================================

    fn current_offset(&self) -> usize {
        match self.current_section {
            Section::Text => self.text.len(),
            Section::Data => self.data.len(),
        }
    }

    fn current_bytes_mut(&mut self) -> &mut Vec<u8> {
        match self.current_section {
            Section::Text => &mut self.text,
            Section::Data => &mut self.data,
        }
    }

    fn emit_u8(&mut self, value: u8) {
        self.current_bytes_mut().push(value);
    }

    fn emit_u16(&mut self, value: u16) {
        self.current_bytes_mut()
            .extend_from_slice(&value.to_le_bytes());
    }

    fn emit_u32(&mut self, value: u32) {
        self.current_bytes_mut()
            .extend_from_slice(&value.to_le_bytes());
    }

    fn emit_i32(&mut self, value: i32) {
        self.emit_u32(value as u32);
    }

    fn emit_u64(&mut self, value: u64) {
        self.current_bytes_mut()
            .extend_from_slice(&value.to_le_bytes());
    }
}

// ============================================================
// Condition
// ============================================================

#[derive(Debug, Clone, Copy)]
pub enum Condition {
    E,
    NE,
    L,
    LE,
    G,
    GE,
}

fn condition_opcode(condition: Condition) -> u8 {
    match condition {
        Condition::E => 0x84,
        Condition::NE => 0x85,
        Condition::L => 0x8C,
        Condition::GE => 0x8D,
        Condition::LE => 0x8E,
        Condition::G => 0x8F,
    }
}
