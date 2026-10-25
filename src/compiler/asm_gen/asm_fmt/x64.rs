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

/// スタックフレームのベースレジスタ
pub(in crate::compiler::asm_gen) const FRAME_BASE_REG: &str = "%rbp";

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
            "al", "cl", "dl", "bl", "sil", "dil", "r8b", "r9b", "r10b", "r11b", "r12b", "r13b",
            "r14b", "r15b",
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
                template: "{space}mov {src1}, {dst}\n{space}mul {src2}, {dst}\n",
            },
        ),
        (
            "imul",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}imul {src2}, {dst}\n",
            },
        ),
        (
            "div",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, %{0}\n{space}cltd\n{space}mov {src2}, %{1}\n{space}div %{1}\n{space}mov %{0}, {dst}\n",
            },
        ),
        (
            "idiv",
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
        // ===== setcc(`asm_emitter/set_cc.rs`) =====
        // 直前の`tcmp`のフラグで、`{dst}`(8bitのレジスタ)へ0/1を書く。
        // `set_b`/`set_a`/`set_b_e`/`set_a_e`は符号なし整数の比較(`<`はbelow, `>`はabove)
        (
            "set_l",
            StaticOperandInfo {
                len: 1,
                template: "{space}setl {dst}\n",
            },
        ),
        (
            "set_g",
            StaticOperandInfo {
                len: 1,
                template: "{space}setg {dst}\n",
            },
        ),
        (
            "set_l_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}setle {dst}\n",
            },
        ),
        (
            "set_g_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}setge {dst}\n",
            },
        ),
        (
            "set_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}sete {dst}\n",
            },
        ),
        (
            "set_ne",
            StaticOperandInfo {
                len: 1,
                template: "{space}setne {dst}\n",
            },
        ),
        (
            "set_b",
            StaticOperandInfo {
                len: 1,
                template: "{space}setb {dst}\n",
            },
        ),
        (
            "set_a",
            StaticOperandInfo {
                len: 1,
                template: "{space}seta {dst}\n",
            },
        ),
        (
            "set_b_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}setbe {dst}\n",
            },
        ),
        (
            "set_a_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}setae {dst}\n",
            },
        ),
        // 8bitの値(`setcc`の結果)を32bitへゼロ拡張する(上位32bitも0になる)
        (
            "movzbl",
            StaticOperandInfo {
                len: 2,
                template: "{space}movzbl {src1}, {dst}\n",
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
        // ===== 他のファイルから参照される共通のテンプレート =====
        // ラベル・ジャンプ
        (
            "label",
            StaticOperandInfo {
                len: 0,
                template: "{name}:\n",
            },
        ),
        (
            "jmp",
            StaticOperandInfo {
                len: 1,
                template: "{space}jmp {dst}\n",
            },
        ),
        // 浮動小数点レジスタをスタックへ退避・復元するための領域の確保/解放と、その参照
        (
            "sp_alloc_16",
            StaticOperandInfo {
                len: 0,
                template: "{space}sub $16, %rsp\n",
            },
        ),
        (
            "sp_free_16",
            StaticOperandInfo {
                len: 0,
                template: "{space}add $16, %rsp\n",
            },
        ),
        (
            "sp_ref",
            StaticOperandInfo {
                len: 0,
                template: "(%rsp)",
            },
        ),
        // 除算の準備(符号拡張/ゼロクリア)と除算
        (
            "cltd",
            StaticOperandInfo {
                len: 0,
                template: "{space}cltd\n",
            },
        ),
        (
            "cqto",
            StaticOperandInfo {
                len: 0,
                template: "{space}cqto\n",
            },
        ),
        (
            "cdq",
            StaticOperandInfo {
                len: 0,
                template: "{space}cdq\n",
            },
        ),
        (
            "cqo",
            StaticOperandInfo {
                len: 0,
                template: "{space}cqo\n",
            },
        ),
        (
            "zero_reg",
            StaticOperandInfo {
                len: 1,
                template: "{space}xor {dst}, {dst}\n",
            },
        ),
        (
            "idiv",
            StaticOperandInfo {
                len: 1,
                template: "{space}idiv {dst}\n",
            },
        ),
        (
            "udiv",
            StaticOperandInfo {
                len: 1,
                template: "{space}div {dst}\n",
            },
        ),
        // 符号反転・即値の加算
        (
            "neg",
            StaticOperandInfo {
                len: 1,
                template: "{space}neg {dst}\n",
            },
        ),
        (
            "add_imm",
            StaticOperandInfo {
                len: 1,
                template: "{space}add {src1}, {dst}\n",
            },
        ),
        // 整数の拡張(`get_sign_extend`/`get_zero_extend`/`get_int_extend`)
        (
            "movsbw",
            StaticOperandInfo {
                len: 2,
                template: "{space}movsbw {src1}, {dst}\n",
            },
        ),
        (
            "movsbl",
            StaticOperandInfo {
                len: 2,
                template: "{space}movsbl {src1}, {dst}\n",
            },
        ),
        (
            "movsbq",
            StaticOperandInfo {
                len: 2,
                template: "{space}movsbq {src1}, {dst}\n",
            },
        ),
        (
            "movswl",
            StaticOperandInfo {
                len: 2,
                template: "{space}movswl {src1}, {dst}\n",
            },
        ),
        (
            "movswq",
            StaticOperandInfo {
                len: 2,
                template: "{space}movswq {src1}, {dst}\n",
            },
        ),
        (
            "movslq",
            StaticOperandInfo {
                len: 2,
                template: "{space}movslq {src1}, {dst}\n",
            },
        ),
        (
            "movzbw",
            StaticOperandInfo {
                len: 2,
                template: "{space}movzbw {src1}, {dst}\n",
            },
        ),
        (
            "movzwl",
            StaticOperandInfo {
                len: 2,
                template: "{space}movzwl {src1}, {dst}\n",
            },
        ),
        // 静的領域のデータ定義(`.align` + ラベル + ディレクティブ)
        (
            "data_long",
            StaticOperandInfo {
                len: 0,
                template: ".align 4\n{name}: .long {value}\n",
            },
        ),
        (
            "data_float",
            StaticOperandInfo {
                len: 0,
                template: ".align 4\n{name}: .float {value}\n",
            },
        ),
        (
            "data_double",
            StaticOperandInfo {
                len: 0,
                template: ".align 8\n{name}: .double {value}\n",
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

/// フォーマットファイルに定義が無い命令のテンプレートとして使う、`GCC_X64`の定義を返す。
/// アセンブリ言語のテキストは、このファイルに書いたものから作る
pub(in crate::compiler::asm_gen) fn default_tmpl(key: &str) -> &'static str {
    GCC_X64
        .op
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.template)
        .unwrap_or_else(|| panic!("x64.rsに定義がありません: {}", key))
}

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
