//! `asm_fmts/gcc_x64.yaml` をハードコードした静的データ
//!
//! `String` / `Vec` / `HashMap` は `static` で直接初期化できないため、
//! `&'static str` とスライスで同じ構造を持つ型を用意している。
//! 既存の `asm_setting::AsmFormat` が必要な場合は
//! `GCC_X64.to_asm_format()` で変換できる。

use std::collections::HashMap;

use crate::asm_setting::{
    AsmFormat, DataFmt, Func, FuncArgsReg, OpSizeFmt, OperandInfo, Reg, ValueFmt,
};

pub struct StaticReg {
    pub db: &'static [&'static str],
    pub dw: &'static [&'static str],
    pub dd: &'static [&'static str],
    pub dq: &'static [&'static str],
    pub xmm: &'static [&'static str],
    pub ymm: &'static [&'static str],
    pub zmm: &'static [&'static str],
}

pub struct StaticOperandInfo {
    pub len: usize,
    pub template: &'static str,
}

pub struct StaticFunc {
    pub extern_def: &'static str,
    pub ret: usize,
    pub call: &'static str,
}

pub struct StaticFuncArgsReg {
    /// (OS名, レジスタ番号の並び)
    pub fmt: &'static [(&'static str, &'static [usize])],
}

pub struct StaticDataFmt {
    pub head: &'static str,
    pub fmt: &'static str,
}

pub struct StaticOpSizeFmt {
    pub db: &'static str,
    pub dw: &'static str,
    pub dd: &'static str,
    pub dq: &'static str,
}

pub struct StaticValueFmt {
    pub reg: &'static str,
    pub num: &'static str,
    pub static_var: &'static str,
    pub string: &'static str,
    pub global: &'static str,
    pub data: StaticDataFmt,
    pub op_size: StaticOpSizeFmt,
    pub mnemonic_size: bool,
    pub ref_stack: &'static str,
    pub frame: &'static str,
    pub frame_end: &'static str,
}

pub struct StaticAsmFormat {
    pub reg: StaticReg,
    pub args: StaticFuncArgsReg,
    pub section: &'static str,
    pub fmt: StaticValueFmt,
    /// (命令名, 情報)
    pub op: &'static [(&'static str, StaticOperandInfo)],
    pub func: StaticFunc,
}

pub(in crate::compiler::asm_gen::asm_fmt) static GCC_X64: StaticAsmFormat = StaticAsmFormat {
    reg: StaticReg {
        db: &[
            "al", "cl", "dl", "bl", "spl", "bpl", "sil", "dil", "r8b", "r9b", "r10b", "r11b",
            "r12b", "r13b", "r14b", "r15b",
        ],
        dw: &[
            "ax", "cx", "dx", "bx", "si", "di", "r8w", "r9w", "r10w", "r11w", "r12w", "r13w",
            "r14w", "r15w",
        ],
        dd: &[
            "eax", "ecx", "edx", "ebx", "esi", "edi", "r8d", "r9d", "r10d", "r11d", "r12d", "r13d",
            "r14d", "r15d",
        ],
        dq: &[
            "rax", "rcx", "rdx", "rbx", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13",
            "r14", "r15",
        ],
        xmm: &[
            "xmm0", "xmm1", "xmm2", "xmm3", "xmm4", "xmm5", "xmm6", "xmm7", "xmm8", "xmm9",
            "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15",
        ],
        ymm: &[
            "ymm0", "ymm1", "ymm2", "ymm3", "ymm4", "ymm5", "ymm6", "ymm7", "ymm8", "ymm9",
            "ymm10", "ymm11", "ymm12", "ymm13", "ymm14", "ymm15",
        ],
        zmm: &[
            "zmm0", "zmm1", "zmm2", "zmm3", "zmm4", "zmm5", "zmm6", "zmm7", "zmm8", "zmm9",
            "zmm10", "zmm11", "zmm12", "zmm13", "zmm14", "zmm15",
        ],
    },

    args: StaticFuncArgsReg {
        fmt: &[("linux", &[5, 4, 2, 1, 7, 8]), ("win", &[1, 2, 6, 7])],
    },

    section: ".{name}\n",

    fmt: StaticValueFmt {
        reg: "%{}",
        num: "${}",
        static_var: "{name}(%rip)",
        string: "{name}: .ascii \"{}\"\n",
        global: ".global {name}\n",
        data: StaticDataFmt {
            head: "{space}push %rbp\n{space}mov %rsp, %rbp\n{space}sub ${size}, %rsp\n",
            fmt: "{space}mov {dst}, -{size}(%rbp)\n",
        },
        op_size: StaticOpSizeFmt {
            db: "b",
            dw: "w",
            dd: "l",
            dq: "q",
        },
        mnemonic_size: true,
        ref_stack: "-{size}({src})",
        frame: "{space}push %rbp\n{space}mov %rsp, %rbp\n",
        frame_end: "{space}leave\n",
    },

    op: &[
        (
            "inc",
            StaticOperandInfo {
                len: 1,
                template: "{space}inc {dst}\n",
            },
        ),
        (
            "dec",
            StaticOperandInfo {
                len: 1,
                template: "{space}dec {dst}\n",
            },
        ),
        (
            "push",
            StaticOperandInfo {
                len: 1,
                template: "{space}push {dst}\n",
            },
        ),
        (
            "pop",
            StaticOperandInfo {
                len: 1,
                template: "{space}pop {dst}\n",
            },
        ),
        // 論理演算
        (
            "and",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}and {src2}, {dst}\n",
            },
        ),
        (
            "xor",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}xor {src2}, {dst}\n",
            },
        ),
        (
            "or",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}or {src2}, {dst}\n",
            },
        ),
        (
            "lshift",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}shl {src2}, {dst}\n",
            },
        ),
        (
            "rshift",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}shr {src2}, {dst}\n",
            },
        ),
        (
            "add",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}add {src2}, {dst}\n",
            },
        ),
        (
            "sub",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}sub {src2}, {dst}\n",
            },
        ),
        (
            "mul",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}imul {src2}, {dst}\n",
            },
        ),
        (
            "div",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, %{0}\n{space}cltd\n{space}mov {src2}, %{1}\n{space}idiv %{1}\n{space}mov %{0}, {dst}\n",
            },
        ),
        (
            "sur",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, %{2}\n{space}cltd\n{space}mov {src2}, %{1}\n{space}idiv %{1}\n{space}mov %{2}, {dst}\n",
            },
        ),
        (
            "mov",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n",
            },
        ),
        (
            "cmp_l",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}jl {label}\n",
            },
        ),
        (
            "cmp_g",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}jg {label}\n",
            },
        ),
        (
            "cmp_l_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}jle {label}\n",
            },
        ),
        (
            "cmp_g_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}jge {label}\n",
            },
        ),
        (
            "cmp_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}jz {label}\n",
            },
        ),
        (
            "cmp_ne",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}jnz {label}\n",
            },
        ),
        // ===== 三項演算子(`asm_emitter/ternary.rs`) =====
        // 比較だけ(フラグを設定する。ジャンプはしない)
        (
            "tcmp",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n",
            },
        ),
        // 条件が真のとき`{src1}`(レジスタ)を`{dst}`へ移す。
        // `{dst}`へ偽のときの値を`mov`してから、直前の`cmp`のフラグで使う
        (
            "cmov_l",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmovl {src1}, {dst}\n",
            },
        ),
        (
            "cmov_g",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmovg {src1}, {dst}\n",
            },
        ),
        (
            "cmov_l_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmovle {src1}, {dst}\n",
            },
        ),
        (
            "cmov_g_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmovge {src1}, {dst}\n",
            },
        ),
        (
            "cmov_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmove {src1}, {dst}\n",
            },
        ),
        (
            "cmov_ne",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmovne {src1}, {dst}\n",
            },
        ),
        (
            "cmov_b",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmovb {src1}, {dst}\n",
            },
        ),
        (
            "cmov_a",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmova {src1}, {dst}\n",
            },
        ),
        (
            "cmov_b_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmovbe {src1}, {dst}\n",
            },
        ),
        (
            "cmov_a_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmovae {src1}, {dst}\n",
            },
        ),
        // 符号なし整数の比較(`<`はbelow, `>`はabove)
        (
            "cmp_b",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}jb {label}\n",
            },
        ),
        (
            "cmp_a",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}ja {label}\n",
            },
        ),
        (
            "cmp_b_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}jbe {label}\n",
            },
        ),
        (
            "cmp_a_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src2}, {src1}\n{space}jae {label}\n",
            },
        ),
        // 符号なし整数を64bitへゼロ拡張する(`movl`は上位32bitが0になるので不要)
        (
            "movzbq",
            StaticOperandInfo {
                len: 2,
                template: "{space}movzbq {src1}, {dst}\n",
            },
        ),
        (
            "movzwq",
            StaticOperandInfo {
                len: 2,
                template: "{space}movzwq {src1}, {dst}\n",
            },
        ),
        (
            "ret",
            StaticOperandInfo {
                len: 1,
                template: "{space}ret\n",
            },
        ),
        (
            "address",
            StaticOperandInfo {
                len: 1,
                template: "{space}lea {src1}, {dst}\n",
            },
        ),
        // ===== 型変換(`asm_emitter/cast.rs`) =====
        // 整数 → 浮動小数点数(`l`は32bit、`q`は64bitの整数)
        (
            "cvtsi2ssl",
            StaticOperandInfo {
                len: 2,
                template: "{space}cvtsi2ssl {src1}, {dst}\n",
            },
        ),
        (
            "cvtsi2ssq",
            StaticOperandInfo {
                len: 2,
                template: "{space}cvtsi2ssq {src1}, {dst}\n",
            },
        ),
        (
            "cvtsi2sdl",
            StaticOperandInfo {
                len: 2,
                template: "{space}cvtsi2sdl {src1}, {dst}\n",
            },
        ),
        (
            "cvtsi2sdq",
            StaticOperandInfo {
                len: 2,
                template: "{space}cvtsi2sdq {src1}, {dst}\n",
            },
        ),
        // 浮動小数点数 → 整数(小数点以下を切り捨てる)
        (
            "cvttss2si",
            StaticOperandInfo {
                len: 2,
                template: "{space}cvttss2si {src1}, {dst}\n",
            },
        ),
        (
            "cvttsd2si",
            StaticOperandInfo {
                len: 2,
                template: "{space}cvttsd2si {src1}, {dst}\n",
            },
        ),
        // `float` ⇔ `f64`
        (
            "cvtss2sd",
            StaticOperandInfo {
                len: 2,
                template: "{space}cvtss2sd {src1}, {dst}\n",
            },
        ),
        (
            "cvtsd2ss",
            StaticOperandInfo {
                len: 2,
                template: "{space}cvtsd2ss {src1}, {dst}\n",
            },
        ),
        // ===== 単精度浮動小数点 =====
        (
            "float_mov",
            StaticOperandInfo {
                len: 1,
                template: "{space}movss {src1}, {dst}\n",
            },
        ),
        (
            "float_add",
            StaticOperandInfo {
                len: 1,
                template: "{space}movss {src2}, {dst}\n{space}addss {src1}, {dst}\n",
            },
        ),
        (
            "float_mul",
            StaticOperandInfo {
                len: 1,
                template: "{space}movss {src2}, {dst}\n{space}mulss {src1}, {dst}\n",
            },
        ),
        (
            "float_div",
            StaticOperandInfo {
                len: 1,
                template: "{space}movss {src2}, {dst}\n{space}divss {src1}, {dst}\n",
            },
        ),
        (
            "float_sub",
            StaticOperandInfo {
                len: 1,
                template: "{space}movss {src2}, {dst}\n{space}subss {src1}, {dst}\n",
            },
        ),
        (
            "float_cmp",
            StaticOperandInfo {
                len: 1,
                template: "{space}comiss {src1}, {src2}\n",
            },
        ),
        (
            "float_ucmp",
            StaticOperandInfo {
                len: 1,
                template: "{space}ucomiss {src1}, {src2}\n",
            },
        ),
        // ===== 倍制度浮動小数点 =====
        (
            "f64_mov",
            StaticOperandInfo {
                len: 1,
                template: "{space}movsd {src1}, {dst}\n",
            },
        ),
        (
            "f64_add",
            StaticOperandInfo {
                len: 1,
                template: "{space}movsd {src1}, {dst}\n{space}addsd {src2}, {dst}\n",
            },
        ),
        (
            "f64_mul",
            StaticOperandInfo {
                len: 1,
                template: "{space}movsd {src1}, {dst}\n{space}mulsd {src2}, {dst}\n",
            },
        ),
        (
            "f64_div",
            StaticOperandInfo {
                len: 1,
                template: "{space}movsd {src1}, {dst}\n{space}divsd {src2}, {dst}\n",
            },
        ),
        (
            "f64_sub",
            StaticOperandInfo {
                len: 1,
                template: "{space}movsd {src1}, {dst}\n{space}subsd {src2}, {dst}\n",
            },
        ),
        (
            "f64_cmp",
            StaticOperandInfo {
                len: 1,
                template: "{space}comisd {src1}, {src2}\n",
            },
        ),
        (
            "f64_ucmp",
            StaticOperandInfo {
                len: 1,
                template: "{space}ucomisd {src1}, {src2}\n",
            },
        ),
        // ===== 倍制度パック浮動小数点 =====
        (
            "float_p_mov",
            StaticOperandInfo {
                len: 1,
                template: "{space}movaps {src1}, {dst}\n",
            },
        ),
        (
            "float_p_add",
            StaticOperandInfo {
                len: 1,
                template: "{space}movaps {src1}, {dst}\n{space}addps {src2}, {dst}\n",
            },
        ),
        (
            "float_p_mul",
            StaticOperandInfo {
                len: 1,
                template: "{space}movaps {src1}, {dst}\n{space}mulps {src2}, {dst}\n",
            },
        ),
        (
            "float_p_div",
            StaticOperandInfo {
                len: 1,
                template: "{space}movaps {src1}, {dst}\n{space}divps {src2}, {dst}\n",
            },
        ),
        (
            "float_p_sub",
            StaticOperandInfo {
                len: 1,
                template: "{space}movaps {src1}, {dst}\n{space}subps {src2}, {dst}\n",
            },
        ),
        (
            "float_p_cmp",
            StaticOperandInfo {
                len: 1,
                template: "{space}cmpps {src1}, {src2}\n",
            },
        ),
    ],

    func: StaticFunc {
        extern_def: ".extern {name}\n",
        ret: 0,
        call: "{space}call {name}\n",
    },
};

fn to_strings(src: &[&str]) -> Vec<String> {
    src.iter().map(|s| s.to_string()).collect()
}

impl StaticAsmFormat {
    /// 既存の `AsmFormat`(`AsmEmitter` が受け取る型)へ変換する
    pub fn to_asm_format(&self) -> AsmFormat {
        AsmFormat {
            reg: Reg {
                db: to_strings(self.reg.db),
                dw: to_strings(self.reg.dw),
                dd: to_strings(self.reg.dd),
                dq: to_strings(self.reg.dq),
                xmm: to_strings(self.reg.xmm),
                ymm: to_strings(self.reg.ymm),
                zmm: to_strings(self.reg.zmm),
            },
            args: FuncArgsReg {
                fmt: self
                    .args
                    .fmt
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_vec()))
                    .collect::<HashMap<_, _>>(),
            },
            section: self.section.to_string(),
            fmt: ValueFmt {
                reg: self.fmt.reg.to_string(),
                num: self.fmt.num.to_string(),
                static_var: self.fmt.static_var.to_string(),
                string: self.fmt.string.to_string(),
                global: self.fmt.global.to_string(),
                data: DataFmt {
                    head: self.fmt.data.head.to_string(),
                    fmt: self.fmt.data.fmt.to_string(),
                },
                op_size: OpSizeFmt {
                    db: self.fmt.op_size.db.to_string(),
                    dw: self.fmt.op_size.dw.to_string(),
                    dd: self.fmt.op_size.dd.to_string(),
                    dq: self.fmt.op_size.dq.to_string(),
                },
                mnemonic_size: self.fmt.mnemonic_size,
                ref_stack: self.fmt.ref_stack.to_string(),
                frame: self.fmt.frame.to_string(),
                frame_end: self.fmt.frame_end.to_string(),
            },
            op: self
                .op
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        OperandInfo {
                            len: v.len,
                            template: v.template.to_string(),
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
            func: Func {
                extern_def: self.func.extern_def.to_string(),
                ret: self.func.ret,
                call: self.func.call.to_string(),
            },
        }
    }
}
