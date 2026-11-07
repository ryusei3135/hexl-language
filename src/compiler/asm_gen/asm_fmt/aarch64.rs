//! `assembler/aarch64` が受け付ける GNU as 風の構文を使った静的フォーマット。
//!
//! このアセンブラでは汎用レジスターを `%xN` / `%wN`、即値を `$N` と書く。
//! 浮動小数点命令や x86 固有の部分レジスター命令はフォーマットに含めない。

use super::static_fmt::*;

/// スタックフレームのベースレジスタ
pub(in crate::compiler::asm_gen) const FRAME_BASE_REG: &str = "%x29";

const W_REGS: &[&str] = &[
    "w0", "w1", "w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10", "w11", "w12", "w13", "w14",
    "w15", "w16", "w17", "w18", "w19", "w20", "w21", "w22", "w23", "w24", "w25", "w26", "w27",
    "w28", "w29", "w30",
];

const X_REGS: &[&str] = &[
    "x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9", "x10", "x11", "x12", "x13", "x14",
    "x15", "x16", "x17", "x18", "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27",
    "x28", "x29", "x30",
];

pub(in crate::compiler::asm_gen::asm_fmt) static AARCH64: StaticAsmFormat = StaticAsmFormat {
    reg: StaticReg {
        // AArch64 has no independently addressable byte/halfword GPRs. The assembler
        // uses W registers for these operations and determines access width by mnemonic.
        db: W_REGS,
        dw: W_REGS,
        dd: W_REGS,
        dq: X_REGS,
        xmm: &[],
        ymm: &[],
        zmm: &[],
    },

    args: StaticFuncArgsReg {
        fmt: &[
            ("linux", &[0, 1, 2, 3, 4, 5, 6, 7]),
            ("win", &[0, 1, 2, 3, 4, 5, 6, 7]),
        ],
    },

    section: ".{name}\n",

    fmt: StaticValueFmt {
        reg: "%{}",
        num: "${}",
        static_var: "{name}",
        string: "{name}: .asciz \"{}\"\n",
        global: ".global {name}\n",
        data: StaticDataFmt {
            head: "{space}stp %x29, %x30, [%sp, $-16]!\n{space}mov %x29, %sp\n{space}sub %sp, %sp, ${size}\n",
            fmt: "{space}str {dst}, [%x29, $-{size}]\n",
        },
        op_size: StaticOpSizeFmt {
            db: "b",
            dw: "h",
            dd: "w",
            dq: "x",
        },
        mnemonic_size: false,
        ref_stack: "[{src}, $-{size}]",
        frame: "{space}stp %x29, %x30, [%sp, $-16]!\n{space}mov %x29, %sp\n",
        frame_end: "{space}mov %sp, %x29\n{space}ldp %x29, %x30, [%sp], $16\n",
    },

    op: &[
        (
            "inc",
            StaticOperandInfo {
                len: 1,
                template: "{space}add {dst}, {dst}, $1\n",
            },
        ),
        (
            "dec",
            StaticOperandInfo {
                len: 1,
                template: "{space}sub {dst}, {dst}, $1\n",
            },
        ),
        (
            "push",
            StaticOperandInfo {
                len: 1,
                template: "{space}str {dst}, [%sp, $-8]!\n",
            },
        ),
        (
            "pop",
            StaticOperandInfo {
                len: 1,
                template: "{space}ldr {dst}, [%sp], $8\n",
            },
        ),
        (
            "and",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}and {dst}, {dst}, {src2}\n",
            },
        ),
        (
            "xor",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}eor {dst}, {dst}, {src2}\n",
            },
        ),
        (
            "or",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}orr {dst}, {dst}, {src2}\n",
            },
        ),
        (
            "add",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}add {dst}, {dst}, {src2}\n",
            },
        ),
        (
            "sub",
            StaticOperandInfo {
                len: 2,
                template: "{space}mov {src1}, {dst}\n{space}sub {dst}, {dst}, {src2}\n",
            },
        ),
        (
            "mul",
            StaticOperandInfo {
                len: 2,
                template: "{space}mul {dst}, {src1}, {src2}\n",
            },
        ),
        (
            "imul",
            StaticOperandInfo {
                len: 2,
                template: "{space}mul {dst}, {src1}, {src2}\n",
            },
        ),
        (
            "div",
            StaticOperandInfo {
                len: 2,
                template: "{space}udiv {dst}, {src1}, {src2}\n",
            },
        ),
        (
            "idiv",
            StaticOperandInfo {
                len: 1,
                template: "{space}sdiv {dst}, {dst}, {src1}\n",
            },
        ),
        (
            "udiv",
            StaticOperandInfo {
                len: 1,
                template: "{space}udiv {dst}, {dst}, {src1}\n",
            },
        ),
        (
            "sur",
            StaticOperandInfo {
                len: 2,
                template: "{space}sdiv {dst}, {src1}, {src2}\n{space}msub {dst}, {dst}, {src2}, {src1}\n",
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
                template: "{space}cmp {src1}, {src2}\n{space}b.lt {label}\n",
            },
        ),
        (
            "cmp_g",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n{space}b.gt {label}\n",
            },
        ),
        (
            "cmp_l_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n{space}b.le {label}\n",
            },
        ),
        (
            "cmp_g_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n{space}b.ge {label}\n",
            },
        ),
        (
            "cmp_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n{space}b.eq {label}\n",
            },
        ),
        (
            "cmp_ne",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n{space}b.ne {label}\n",
            },
        ),
        (
            "tcmp",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n",
            },
        ),
        (
            "cmov_l",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, lt\n",
            },
        ),
        (
            "cmov_g",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, gt\n",
            },
        ),
        (
            "cmov_l_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, le\n",
            },
        ),
        (
            "cmov_g_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, ge\n",
            },
        ),
        (
            "cmov_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, eq\n",
            },
        ),
        (
            "cmov_ne",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, ne\n",
            },
        ),
        (
            "cmov_b",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, lo\n",
            },
        ),
        (
            "cmov_a",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, hi\n",
            },
        ),
        (
            "cmov_b_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, ls\n",
            },
        ),
        (
            "cmov_a_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}csel {dst}, {src1}, {dst}, hs\n",
            },
        ),
        (
            "set_l",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, lt\n",
            },
        ),
        (
            "set_g",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, gt\n",
            },
        ),
        (
            "set_l_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, le\n",
            },
        ),
        (
            "set_g_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, ge\n",
            },
        ),
        (
            "set_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, eq\n",
            },
        ),
        (
            "set_ne",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, ne\n",
            },
        ),
        (
            "set_b",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, lo\n",
            },
        ),
        (
            "set_a",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, hi\n",
            },
        ),
        (
            "set_b_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, ls\n",
            },
        ),
        (
            "set_a_e",
            StaticOperandInfo {
                len: 1,
                template: "{space}cset {dst}, hs\n",
            },
        ),
        (
            "movzbl",
            StaticOperandInfo {
                len: 2,
                template: "{space}uxtb {dst}, {src1}\n",
            },
        ),
        (
            "cmp_b",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n{space}b.lo {label}\n",
            },
        ),
        (
            "cmp_a",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n{space}b.hi {label}\n",
            },
        ),
        (
            "cmp_b_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n{space}b.ls {label}\n",
            },
        ),
        (
            "cmp_a_e",
            StaticOperandInfo {
                len: 2,
                template: "{space}cmp {src1}, {src2}\n{space}b.hs {label}\n",
            },
        ),
        (
            "movzbq",
            StaticOperandInfo {
                len: 2,
                template: "{space}uxtb {dst}, {src1}\n",
            },
        ),
        (
            "movzwq",
            StaticOperandInfo {
                len: 2,
                template: "{space}uxth {dst}, {src1}\n",
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
                template: "{space}b {dst}\n",
            },
        ),
        (
            "sp_alloc_16",
            StaticOperandInfo {
                len: 0,
                template: "{space}sub %sp, %sp, $16\n",
            },
        ),
        (
            "sp_free_16",
            StaticOperandInfo {
                len: 0,
                template: "{space}add %sp, %sp, $16\n",
            },
        ),
        (
            "sp_alloc_8",
            StaticOperandInfo {
                len: 0,
                template: "{space}sub %sp, %sp, $8\n",
            },
        ),
        (
            "sp_free_8",
            StaticOperandInfo {
                len: 0,
                template: "{space}add %sp, %sp, $8\n",
            },
        ),
        (
            "sp_ref",
            StaticOperandInfo {
                len: 0,
                template: "[%sp]",
            },
        ),
        // AArch64 division instructions do not require a sign-extension preparation.
        (
            "cltd",
            StaticOperandInfo {
                len: 0,
                template: "",
            },
        ),
        (
            "cqto",
            StaticOperandInfo {
                len: 0,
                template: "",
            },
        ),
        (
            "cdq",
            StaticOperandInfo {
                len: 0,
                template: "",
            },
        ),
        (
            "cqo",
            StaticOperandInfo {
                len: 0,
                template: "",
            },
        ),
        (
            "zero_reg",
            StaticOperandInfo {
                len: 1,
                template: "{space}mov {dst}, $0\n",
            },
        ),
        (
            "neg",
            StaticOperandInfo {
                len: 1,
                template: "{space}neg {dst}, {dst}\n",
            },
        ),
        (
            "add_imm",
            StaticOperandInfo {
                len: 1,
                template: "{space}add {dst}, {dst}, {src1}\n",
            },
        ),
        (
            "movsbw",
            StaticOperandInfo {
                len: 2,
                template: "{space}sxtb {dst}, {src1}\n",
            },
        ),
        (
            "movsbl",
            StaticOperandInfo {
                len: 2,
                template: "{space}sxtb {dst}, {src1}\n",
            },
        ),
        (
            "movsbq",
            StaticOperandInfo {
                len: 2,
                template: "{space}sxtb {dst}, {src1}\n",
            },
        ),
        (
            "movswl",
            StaticOperandInfo {
                len: 2,
                template: "{space}sxth {dst}, {src1}\n",
            },
        ),
        (
            "movswq",
            StaticOperandInfo {
                len: 2,
                template: "{space}sxth {dst}, {src1}\n",
            },
        ),
        (
            "movslq",
            StaticOperandInfo {
                len: 2,
                template: "{space}sxtw {dst}, {src1}\n",
            },
        ),
        (
            "movzbw",
            StaticOperandInfo {
                len: 2,
                template: "{space}uxtb {dst}, {src1}\n",
            },
        ),
        (
            "movzwl",
            StaticOperandInfo {
                len: 2,
                template: "{space}uxth {dst}, {src1}\n",
            },
        ),
        (
            "data_long",
            StaticOperandInfo {
                len: 0,
                template: ".balign 4\n{name}: .word {value}\n",
            },
        ),
        (
            "data_quad",
            StaticOperandInfo {
                len: 0,
                template: ".balign 8\n{name}: .quad {value}\n",
            },
        ),
        (
            "data_float",
            StaticOperandInfo {
                len: 0,
                template: ".balign 4\n{name}: .float {value}\n",
            },
        ),
        (
            "data_double",
            StaticOperandInfo {
                len: 0,
                template: ".balign 8\n{name}: .double {value}\n",
            },
        ),
        (
            "address",
            StaticOperandInfo {
                len: 1,
                template: "{space}adr {dst}, {src1}\n",
            },
        ),
        (
            "mul_lea_scale",
            StaticOperandInfo {
                len: 1,
                template: "{space}add {dst}, {src1}, {src1}, lsl ${scale}\n",
            },
        ),
        (
            "div_shr",
            StaticOperandInfo {
                len: 1,
                template: "{space}lsr {dst}, {dst}, {src1}\n",
            },
        ),
        (
            "div_sar",
            StaticOperandInfo {
                len: 1,
                template: "{space}asr {dst}, {dst}, {src1}\n",
            },
        ),
        (
            "div_add",
            StaticOperandInfo {
                len: 1,
                template: "{space}add {dst}, {dst}, {src1}\n",
            },
        ),
        (
            "mul_lea_x8",
            StaticOperandInfo {
                len: 1,
                template: "{space}add {dst}, {src1}, {src1}, lsl $3\n",
            },
        ),
    ],

    func: StaticFunc {
        extern_def: ".extern {name}\n",
        ret: 0,
        call: "{space}bl {name}\n",
    },
};

/// フォーマットに定義が無い命令は、AArch64用の定義から検索する。
pub(in crate::compiler::asm_gen) fn default_tmpl(key: &str) -> &'static str {
    AARCH64
        .op
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.template)
        .unwrap_or_else(|| panic!("aarch64.rsに定義がありません: {}", key))
}
