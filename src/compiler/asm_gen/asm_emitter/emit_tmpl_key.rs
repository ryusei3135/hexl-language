//!
//! IRの式のノードからテンプレートのキーを
//! 出力するAPIを提供

use super::*;


impl AsmEmitter {
    /// 式の種類から、アセンブリのテンプレートを引くキーを返す
    pub(super) fn expr_tmpl_key(expr: &inst::ExprInst, insts: &[Inst]) -> &'static str {
        if expr.is_float(insts) {
            match expr.kind {
                inst::ExprKind::Add => "float_add",
                inst::ExprKind::Sub => "float_sub",
                // 乗算は`add`のテンプレートの命令を`imul`に置き換えて使う
                // (2オペランドの`imul`は`%rax`/`%rdx`を書き換えない)
                inst::ExprKind::Mul => "float_mul",
                inst::ExprKind::Div | inst::ExprKind::Surplus => "float_div",
                inst::ExprKind::LessThen => "cmp_l",
                inst::ExprKind::GreaterThen => "cmp_g",
                inst::ExprKind::LessThenEq => "cmp_l_e",
                inst::ExprKind::GreaterThenEq => "cmp_g_e",
                inst::ExprKind::Equal => "cmp_e",
                inst::ExprKind::NotEq => "cmp_ne",
                _ => panic!(),
            }
        } else {
            match expr.kind {
                inst::ExprKind::Inc => "inc",
                inst::ExprKind::Dec => "dec",
                inst::ExprKind::AndCalcul => "and",
                inst::ExprKind::XorCalcul => "xor",
                inst::ExprKind::OrCalcul => "or",
                inst::ExprKind::LShift => "lshift",
                inst::ExprKind::RShift => "rshist",
                inst::ExprKind::Add => "add",
                inst::ExprKind::Sub => "sub",
                // 乗算は`add`のテンプレートの命令を`imul`に置き換えて使う
                // (2オペランドの`imul`は`%rax`/`%rdx`を書き換えない)
                inst::ExprKind::Mul => {
                    if expr.is_unsigned(insts) {
                        "mul"
                    } else {
                        "imul"
                    }
                }
                inst::ExprKind::Div | inst::ExprKind::Surplus => {
                    if expr.is_unsigned(insts) {
                        "div"
                    } else {
                        "idiv"
                    }
                }
                inst::ExprKind::LessThen => "cmp_l",
                inst::ExprKind::GreaterThen => "cmp_g",
                inst::ExprKind::LessThenEq => "cmp_l_e",
                inst::ExprKind::GreaterThenEq => "cmp_g_e",
                inst::ExprKind::Equal => "cmp_e",
                inst::ExprKind::NotEq => "cmp_ne",
            }
        }
    }

    /// 式の種類から、ニーモニックのサイズ調整に使う、実際のニーモニックの文字列を返す
    /// (`cmp_l`/`cmp_g`はテンプレートを引くためのキーであって、
    ///  実際にテンプレート中で使われるニーモニックは"cmp"なので、
    ///  `fmt_mnemonic_resize`にはそちらを渡す必要がある)
    pub(super) fn expr_mnemonic(kind: &inst::ExprKind) -> &'static str {
        match kind {
            inst::ExprKind::Inc => "inc",
            inst::ExprKind::Dec => "dec",
            inst::ExprKind::AndCalcul => "and",
            inst::ExprKind::XorCalcul => "xor",
            inst::ExprKind::OrCalcul => "or",
            inst::ExprKind::LShift => "lshift",
            inst::ExprKind::RShift => "rshist",
            inst::ExprKind::Add => "add",
            inst::ExprKind::Sub => "sub",
            inst::ExprKind::Mul => "imul",
            inst::ExprKind::Div | inst::ExprKind::Surplus => unreachable!(),
            inst::ExprKind::LessThen
            | inst::ExprKind::GreaterThen
            | inst::ExprKind::LessThenEq
            | inst::ExprKind::GreaterThenEq
            | inst::ExprKind::NotEq
            | inst::ExprKind::Equal => "cmp",
        }
    }
}