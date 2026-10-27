//! 出力するアセンブリ言語のフォーマットを管理する
//! アセンブリ言語のフォーマットを提供する:

use std::collections::HashMap;

use super::*;
use crate::compiler::err::{self, compiler_err::*};
use crate::compiler::ir::types;
use crate::compiler::models::AsmFmtData;
use crate::{asm_setting, compiler::asm_gen::RegisterMode};
use std::any::{Any, TypeId};

/// アセンブリ言語のフォーマットをするAPIを提供
pub struct MngAsmFmt {
    param_fmt: Vec<usize>,
    reg_fmt: asm_setting::Reg,
    opcode_fmt: HashMap<String, asm_setting::OperandInfo>,
    pub(super) asm_setting: Option<asm_setting::AsmSetting>,
    fmt: asm_setting::AsmFormat,
}

impl MngAsmFmt {
    pub(in crate::compiler::asm_gen) fn new(
        asm_setting: asm_setting::AsmSetting,
        asm_fmt: AsmFmtData,
        os: &str,
    ) -> Self {
        // === アセンブラのフォーマットの設定 ===
        if let Some(s) = asm_fmt {
            Self {
                param_fmt: s.args.fmt.get(os).unwrap().clone(),
                reg_fmt: s.reg.clone(),
                opcode_fmt: s.op.clone(),
                asm_setting: Some(asm_setting),
                fmt: s,
            }
        } else {
            let s = x64::GCC_X64.to_asm_format();
            Self {
                param_fmt: s.args.fmt.get(os).unwrap().clone(),
                reg_fmt: s.reg.clone(),
                opcode_fmt: s.op.clone(),
                asm_setting: Some(asm_setting),
                fmt: s,
            }
        }
    }

    /// 外部に定義されている物のフォーマット
    pub(in crate::compiler::asm_gen) fn get_extern_func(&self, name: &str) -> String {
        self.fmt.func.extern_def.replace("{name}", name)
    }

    /// 設定された、関数の引数のレジスタを返す
    /// ## Rについて
    /// - Stringの場合はレジスタが返される
    /// - usizeの場合はレジスタの番号が返される
    /// ## 引数
    /// - param_idx = 引数の場所
    pub(in crate::compiler::asm_gen) fn get_fmt_param<R: 'static>(
        &self,
        param_idx: usize,
        size: &Size,
    ) -> Result<R, err::ErrKind> {
        if TypeId::of::<R>() != TypeId::of::<usize>() && TypeId::of::<R>() != TypeId::of::<String>()
        {
            panic!("この型は無効です,")
        }

        Ok(if TypeId::of::<R>() == TypeId::of::<String>() {
            let result: Box<dyn Any> = Box::new(
                if size.is_float() {
                    self.get_fmt_float_reg(self.param_fmt[param_idx], &size)?
                } else {
                    self.get_fmt_reg(self.param_fmt[param_idx], &size)?
                }
                .to_string(),
            );
            result.downcast::<R>().ok().map(|b| *b).unwrap()
        } else {
            let result: Box<dyn Any> = Box::new(self.param_fmt[param_idx]);
            result.downcast::<R>().ok().map(|b| *b).unwrap()
        })
    }

    pub(in crate::compiler::asm_gen) fn fmt_ref_operand(&self, reg: &str, size: usize) -> String {
        self.fmt
            .fmt
            .ref_stack
            .replace("{src}", reg)
            .replace("{size}", size.to_string().as_str())
    }

    /// 添字レジスタ付きのメモリ参照を返す
    /// (AT&Tなら`-offset(base, index, scale)`)
    ///
    /// アドレスは `base + index * scale - offset`。
    /// `ref_stack`のテンプレートに`(`が含まれるAT&T形式の場合は
    /// `{src}`を`base, index, scale`に置き換え、それ以外
    /// (`[{src}-{size}]`など)は`{src}`を`base+index*scale`に置き換える。
    /// `offset`が`0`のときはオフセット部分を付けない
    pub(in crate::compiler::asm_gen) fn fmt_ref_operand_indexed(
        &self,
        base: &str,
        index: &str,
        scale: usize,
        offset: usize,
    ) -> String {
        assert!(
            matches!(scale, 1 | 2 | 4 | 8),
            "添字付きアドレッシングのスケールは1/2/4/8のみ対応しています: {}",
            scale
        );
        let tmpl = &self.fmt.fmt.ref_stack;
        let addr = if tmpl.contains('(') {
            format!("{}, {}, {}", base, index, scale)
        } else {
            format!("{}+{}*{}", base, index, scale)
        };
        let mut s = tmpl.replace("{src}", &addr);
        if offset == 0 {
            s = s
                .replace("-{size}", "")
                .replace("+{size}", "")
                .replace("{size}", "");
        } else {
            s = s.replace("{size}", &offset.to_string());
        }
        s
    }

    /// 符号反転(`neg`)の命令を返す。
    /// フォーマットに`neg`が定義されていなければ`neg {dst}`を使う
    pub(in crate::compiler::asm_gen) fn get_neg(&self, reg: &str) -> String {
        self.opcode_fmt
            .get("neg")
            .map(|o| o.template.clone())
            .unwrap_or_else(|| x64::default_tmpl("neg").to_string())
            .replace("{dst}", reg)
    }

    /// レジスタに即値を足す命令を返す(AT&Tなら`add $4, %rcx`)。
    /// フォーマットに`add_imm`が定義されていればそのテンプレートを使う
    pub(in crate::compiler::asm_gen) fn get_add_imm(&self, imm: &str, reg: &str) -> String {
        self.opcode_fmt
            .get("add_imm")
            .map(|o| o.template.clone())
            .unwrap_or_else(|| x64::default_tmpl("add_imm").to_string())
            .replace("{src1}", imm)
            .replace("{dst}", reg)
    }

    /// 符号拡張して64bitレジスタへ読み込む命令を返す
    /// (AT&Tなら`movslq %ecx, %rcx`/`movsbq`/`movswq`)
    ///
    /// ## 引数
    /// - kind = 元のサイズの接尾辞(`b`/`w`/`l`)
    ///
    /// フォーマットに`movsbq`/`movswq`/`movslq`が定義されていればその
    /// テンプレートを、なければAT&T形式のテンプレートを使う
    pub(in crate::compiler::asm_gen) fn get_sign_extend(
        &self,
        kind: char,
        src: &str,
        dst: &str,
    ) -> String {
        let key = format!("movs{}q", kind);
        self.opcode_fmt
            .get(&key)
            .map(|o| o.template.clone())
            .unwrap_or_else(|| x64::default_tmpl(&key).to_string())
            .replace("{src1}", src)
            .replace("{dst}", dst)
    }

    /// ゼロ拡張して64bitレジスタへ読み込む命令を返す
    /// (AT&Tなら`movzbq`/`movzwq`)
    ///
    /// 32bitからのゼロ拡張には専用の命令が要らない
    /// (32bitレジスタへの`mov`で上位32bitが0になる)ので、
    /// `kind`は`b`/`w`のみ。
    ///
    /// フォーマットに`movzbq`/`movzwq`が定義されていればそのテンプレートを、
    /// なければAT&T形式のテンプレートを使う
    pub(in crate::compiler::asm_gen) fn get_zero_extend(
        &self,
        kind: char,
        src: &str,
        dst: &str,
    ) -> String {
        let key = format!("movz{}q", kind);
        self.opcode_fmt
            .get(&key)
            .map(|o| o.template.clone())
            .unwrap_or_else(|| x64::default_tmpl(&key).to_string())
            .replace("{src1}", src)
            .replace("{dst}", dst)
    }

    /// 整数を大きい幅へ拡張する命令を返す(AT&Tなら`movsbl`/`movzwq`/`movslq`など)
    ///
    /// - signed: `true`なら符号拡張(`movs..`)、`false`ならゼロ拡張(`movz..`)
    /// - from/to: 拡張前/拡張後の幅(`b`/`w`/`l`/`q`)
    ///
    /// フォーマットに`movs{from}{to}`/`movz{from}{to}`が定義されていればそのテンプレートを、
    /// なければAT&Tの命令を返す(`get_sign_extend`/`get_zero_extend`と同じキー)。
    /// 符号なしの32bit → 64bitに専用の命令は無い(`movl`で上位32bitが0になる)ので、
    /// 呼び出し側で`mov`にすること
    pub(in crate::compiler::asm_gen) fn get_int_extend(
        &self,
        signed: bool,
        from: char,
        to: char,
        src: &str,
        dst: &str,
    ) -> String {
        let mnemonic = format!("mov{}{}{}", if signed { 's' } else { 'z' }, from, to);
        self.opcode_fmt
            .get(&mnemonic)
            .map(|o| o.template.clone())
            .unwrap_or_else(|| x64::default_tmpl(&mnemonic).to_string())
            .replace("{src1}", src)
            .replace("{dst}", dst)
    }

    /// 符号なし整数の比較(`<`/`>`)のテンプレートを返す。
    /// `key`は符号付きの比較のキー(`cmp_l`/`cmp_g`)
    ///
    /// フォーマットに`cmp_b`(below)/`cmp_a`(above)が定義されていれば
    /// それを使う。なければ符号付きのテンプレートの`jl`/`jg`を
    /// `jb`/`ja`に置き換えて使う。
    /// `==`/`!=`は符号に関係ないので、他のキーはそのまま返す
    pub(in crate::compiler::asm_gen) fn get_unsigned_cmp_tmpl(&self, key: &str) -> String {
        let (unsigned_key, from, to) = match key {
            "cmp_l" => ("cmp_b", "jl", "jb"),
            "cmp_g" => ("cmp_a", "jg", "ja"),
            "cmp_l_e" => ("cmp_b_e", "jle", "jbe"),
            "cmp_g_e" => ("cmp_a_e", "jge", "jae"),
            _ => return self.get_opcode_tmpl(key),
        };
        match self.opcode_fmt.get(unsigned_key) {
            Some(o) => o.template.clone(),
            None => self.get_opcode_tmpl(key).replace(from, to),
        }
    }

    /// 符号なし整数の比較に対応する、`cmov`のテンプレートを返す
    /// (`cmov_l`/`cmov_g`/`cmov_l_e`/`cmov_g_e`のみ。それ以外は同じキーのテンプレート)
    ///
    /// フォーマットに符号なし用のキー(`cmov_b`など)が無い場合は、
    /// 符号付き用のテンプレートの`cmovl`などを`cmovb`などに置き換える
    pub(in crate::compiler::asm_gen) fn get_unsigned_cmov_tmpl(&self, key: &str) -> String {
        let (unsigned_key, from, to) = match key {
            "cmov_l" => ("cmov_b", "cmovl", "cmovb"),
            "cmov_g" => ("cmov_a", "cmovg", "cmova"),
            "cmov_l_e" => ("cmov_b_e", "cmovle", "cmovbe"),
            "cmov_g_e" => ("cmov_a_e", "cmovge", "cmovae"),
            _ => return self.get_opcode_tmpl(key),
        };
        match self.opcode_fmt.get(unsigned_key) {
            Some(o) => o.template.clone(),
            None => self.get_opcode_tmpl(key).replace(from, to),
        }
    }

    /// 符号なし整数の比較に対応する、`setcc`のテンプレートを返す
    /// (`set_l`/`set_g`/`set_l_e`/`set_g_e`のみ。それ以外は同じキーのテンプレート)
    ///
    /// フォーマットに符号なし用のキー(`set_b`など)が無い場合は、
    /// 符号付き用のテンプレートの`setl`などを`setb`などに置き換える
    pub(in crate::compiler::asm_gen) fn get_unsigned_set_tmpl(&self, key: &str) -> String {
        let (unsigned_key, from, to) = match key {
            "set_l" => ("set_b", "setl", "setb"),
            "set_g" => ("set_a", "setg", "seta"),
            "set_l_e" => ("set_b_e", "setle", "setbe"),
            "set_g_e" => ("set_a_e", "setge", "setae"),
            _ => return self.get_opcode_tmpl(key),
        };
        match self.opcode_fmt.get(unsigned_key) {
            Some(o) => o.template.clone(),
            None => self.get_opcode_tmpl(key).replace(from, to),
        }
    }

    /// オフセット無しのメモリ参照(AT&T構文なら`(%rbx)`)を作る。
    ///
    /// `ref_stack`のテンプレート(`-{size}({src})`など)から、オフセット部分
    /// (`-{size}`/`+{size}`/`{size}`)だけを取り除いて使うため、
    /// フォーマットごとの書き方(`[{src}-{size}]`など)にもそのまま対応できる
    pub(in crate::compiler::asm_gen) fn fmt_ref_operand_no_offset(&self, reg: &str) -> String {
        self.fmt
            .fmt
            .ref_stack
            .replace("{src}", reg)
            .replace("-{size}", "")
            .replace("+{size}", "")
            .replace("{size}", "")
    }

    /// ニーモニックにサイズ接尾辞を付けるフォーマット(AT&T構文)かどうか
    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn mnemonic_size(&self) -> bool {
        self.fmt.fmt.mnemonic_size
    }

    pub(in crate::compiler::asm_gen) fn func_frame_fmt(&self) -> String {
        self.fmt.fmt.frame.to_string()
    }

    pub(in crate::compiler::asm_gen) fn func_frame_end(&self) -> String {
        self.fmt.fmt.frame_end.to_string()
    }

    /// 予約されていた、スタックのサイズ分
    pub(in crate::compiler::asm_gen) fn gen_stack_frame(&self, size: usize) -> String {
        // 16バイト境界に切り上げてアライメントする
        // (例: size=1..16 -> 16, size=17..32 -> 32)
        // `push %rbp`の後の`%rsp`は16バイト境界にあるので、確保するサイズも
        // 16の倍数にしないと、`call`の時点でSystem V ABIのスタックの
        // アライメントが崩れる
        const STACK_ALIGN: usize = 16;
        let alignment_size = size.div_ceil(STACK_ALIGN) * STACK_ALIGN;
        self.fmt
            .fmt
            .data
            .head
            // 確保するスタックのサイズ
            .replace("{size}", &alignment_size.to_string())
    }

    /// 構造体のメンバーをフォーマット
    /// ## 引数
    /// - value = 代入する値
    /// - size = このメンバーの型のサイズ(movのニーモニックの決定にのみ使う)
    /// - offset = `%rbp`からのこのメンバーの、確定済みのオフセット
    ///   (呼び出し側で、これまでの`stk_use_counter`とこのメンバー分の
    ///   サイズを合計した値を渡す。ここで更にサイズを足してはいけない)
    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn get_fmt_struct_member(
        &self,
        value: String,
        size: &types::Size,
        offset: usize,
    ) -> String {
        let fmted = self
            .fmt
            .fmt
            .data
            .fmt
            .replace("{dst}", value.as_str())
            .replace("{size}", offset.to_string().as_str());
        let s_fmt = match &size {
            types::Size::DB | types::Size::UDB => crate::mov_size_fmt!(self, db),
            types::Size::DW | types::Size::UDW => crate::mov_size_fmt!(self, dw),
            types::Size::DD | types::Size::UDD => crate::mov_size_fmt!(self, dd),
            types::Size::DQ | types::Size::UDQ => crate::mov_size_fmt!(self, dq),
            types::Size::Array { size, .. } => {
                return self.get_fmt_struct_member(value, &size, offset);
            }
            t => panic!("{:?}", t),
        };
        fmted.replace("mov", &format!("mov{}", s_fmt))
    }

    /// ニーモニックのサイズを調整
    pub(in crate::compiler::asm_gen) fn fmt_mnemonic_resize(
        &self,
        mnemonic: &str,
        value: &str,
        size: &types::Size,
    ) -> String {
        self.fmt_mnemonic_resize_inner(mnemonic, value, size, false)
    }

    /// メモリを読み書きする命令は、フォーマット設定に関係なく
    /// オペランドのサイズをニーモニックへ付ける。
    pub(in crate::compiler::asm_gen) fn fmt_memory_mnemonic_resize(
        &self,
        mnemonic: &str,
        value: &str,
        size: &types::Size,
    ) -> String {
        self.fmt_mnemonic_resize_inner(mnemonic, value, size, true)
    }

    fn fmt_mnemonic_resize_inner(
        &self,
        mnemonic: &str,
        value: &str,
        size: &types::Size,
        is_memory_access: bool,
    ) -> String {
        if matches!(size, types::Size::Pointer { .. }) {
            return self.fmt_mnemonic_resize_inner(
                mnemonic,
                value,
                &types::Size::DQ,
                is_memory_access,
            );
        }

        if size.is_float() {
            return value.to_string();
        }

        let s_fmt = match &size {
            types::Size::DB | types::Size::UDB => crate::mov_size_fmt!(self, db),
            types::Size::DW | types::Size::UDW => crate::mov_size_fmt!(self, dw),
            types::Size::DD | types::Size::UDD => crate::mov_size_fmt!(self, dd),
            types::Size::DQ | types::Size::UDQ => crate::mov_size_fmt!(self, dq),
            types::Size::Pointer { .. } => unreachable!(),
            types::Size::Array { size, .. } => {
                return self.fmt_mnemonic_resize_inner(mnemonic, value, size, is_memory_access);
            }
            t => panic!("{:?}", t),
        };
        value.replace(mnemonic, &format!("{}{}", mnemonic, s_fmt))
    }

    /// 出力するアセンブリ全体の、サイズ接尾辞(gas/AT&T構文の`b`/`w`/`l`/`q`)が
    /// 無い整数命令に、必ずサイズ接尾辞を付ける。
    ///
    /// 生成した命令はすべて、最後にこの関数を通す(`to_asm_text`)。
    /// これにより、テンプレートを直接埋めただけの行(`sub $16, %rsp`や
    /// `push`/`pop`など)にも漏れなく付く。
    ///
    /// - 対象: `mov` `add` `sub` `imul` `cmp` `test` `and` `or` `xor` `lea`
    ///   `neg` `not` `inc` `dec` `shl` `shr` `sar` `idiv` `div` `push` `pop` など
    /// - サイズは、最後のレジスタオペランドの幅で決める。
    ///   `lea` `push` `pop`は常に64bit(`q`)
    /// - すでに接尾辞のある命令(`movl`/`addq`など)・SSE命令・`call`/`jmp`/`ret`・
    ///   ラベル・ディレクティブは変えない
    /// - `RAW_ASM_MARK`が付いた行(`#asm`ブロックのユーザーが書いた行)は、
    ///   そのまま(マークだけ取り除いて)出力する
    pub(in crate::compiler::asm_gen) fn ensure_mnemonic_size(&self, asm: &str) -> String {
        let mut out = String::with_capacity(asm.len() + asm.len() / 8);
        for line in asm.split_inclusive('\n') {
            if let Some(raw) = line.strip_suffix(&format!("{}\n", RAW_ASM_MARK)) {
                out.push_str(raw);
                out.push('\n');
                continue;
            }
            out.push_str(&Self::sized_line(line));
        }
        out
    }

    /// `ensure_mnemonic_size`の1行分
    fn sized_line(line: &str) -> String {
        let body = line.trim_start();
        let indent = &line[..line.len() - body.len()];
        let Some((mnemonic, operands)) = body.split_once(char::is_whitespace) else {
            return line.to_string();
        };
        const SIZED: &[&str] = &[
            "mov", "add", "sub", "imul", "mul", "idiv", "div", "cmp", "test", "and", "or", "xor",
            "neg", "not", "inc", "dec", "shl", "shr", "sar", "sal", "rol", "ror", "adc", "sbb",
            "lea", "push", "pop",
        ];
        if !SIZED.contains(&mnemonic) {
            return line.to_string();
        }
        let suffix = if matches!(mnemonic, "lea" | "push" | "pop") {
            Some('q')
        } else {
            Self::last_reg_suffix(operands)
        };
        match suffix {
            Some(c) => format!("{}{}{} {}", indent, mnemonic, c, operands.trim_start()),
            None => line.to_string(),
        }
    }

    /// オペランドの並びから、最後のレジスタオペランドの幅に対応する接尾辞を返す
    /// (`(%rbp)`のようなメモリ参照の中のレジスタは数えない)
    fn last_reg_suffix(operands: &str) -> Option<char> {
        let mut depth = 0i32;
        let mut parts = Vec::<&str>::new();
        let mut start = 0;
        for (i, c) in operands.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(&operands[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        parts.push(&operands[start..]);

        parts.iter().rev().find_map(|p| {
            let name = p.trim().strip_prefix('%')?;
            if name.contains('(') || name.starts_with("xmm") || name.starts_with("ymm") {
                return None;
            }
            Self::reg_width_suffix(name)
        })
    }

    fn reg_width_suffix(name: &str) -> Option<char> {
        const Q: &[&str] = &["rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp"];
        const L: &[&str] = &["eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp"];
        const W: &[&str] = &["ax", "bx", "cx", "dx", "si", "di", "bp", "sp"];
        const B: &[&str] = &[
            "al", "bl", "cl", "dl", "sil", "dil", "bpl", "spl", "ah", "bh", "ch", "dh",
        ];
        if Q.contains(&name) {
            return Some('q');
        }
        if L.contains(&name) {
            return Some('l');
        }
        if W.contains(&name) {
            return Some('w');
        }
        if B.contains(&name) {
            return Some('b');
        }
        // r8 - r15 (+ d/w/b)
        let rest = name.strip_prefix('r')?;
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: u32 = digits.parse().ok()?;
        if !(8..=15).contains(&n) {
            return None;
        }
        match &rest[digits.len()..] {
            "" => Some('q'),
            "d" => Some('l'),
            "w" => Some('w'),
            "b" => Some('b'),
            _ => None,
        }
    }

    pub(in crate::compiler::asm_gen) fn get_push(&self, reg: &str) -> String {
        self.get_opcode_tmpl(&"push".to_string())
            .replace("{dst}", reg)
    }

    pub(in crate::compiler::asm_gen) fn get_pop(&self, reg: &str) -> String {
        self.get_opcode_tmpl(&"pop".to_string())
            .replace("{dst}", reg)
    }

    pub(in crate::compiler::asm_gen) fn get_str_fmt(&self, value: &str, label: &str) -> String {
        self.fmt
            .fmt
            .string
            .replace("{}", value)
            .replace("{name}", &label)
    }

    pub(in crate::compiler::asm_gen) fn get_static_num_fmt(
        &self,
        value: &str,
        label: &str,
        _size: &types::Size,
    ) -> String {
        self.tmpl_or_default("data_long")
            .replace("{name}", label)
            .replace("{value}", &value.replace("$", ""))
    }

    pub(in crate::compiler::asm_gen) fn get_float_fmt(
        &self,
        value: &str,
        label: &str,
        size: &types::Size,
    ) -> String {
        let key = match size {
            types::Size::F32 => "data_float",
            types::Size::F64 => "data_double",
            _ => panic!("float literal has non-floating-point type: {:?}", size),
        };
        self.tmpl_or_default(key)
            .replace("{name}", label)
            .replace("{value}", value)
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn get_global_fmt(&self, name: &str) -> String {
        self.fmt.fmt.global.replace("{name}", name)
    }

    /// 静的領域の変数に%ripをつけて返す
    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn fmt_static_var_rip(&self, name: &str) -> String {
        self.fmt.fmt.static_var.replace("{name}", name)
    }

    /// エントリーポイントを作成
    pub(in crate::compiler::asm_gen) fn get_entry_point(&self) -> String {
        self.fmt
            .fmt
            .global
            .replace("{name}", &self.asm_setting.as_ref().unwrap().entry)
    }

    /// アセンブリ言語のセクションを定義するフォーマット
    pub(in crate::compiler::asm_gen) fn get_section_fmt(&self, section: &str) -> String {
        self.fmt.section.replace("{name}", section)
    }

    /// オペコードのフォーマット。フォーマットファイルに定義が無ければ`x64.rs`の定義を返す
    pub(in crate::compiler::asm_gen) fn tmpl_or_default(&self, key: &str) -> String {
        self.opcode_fmt
            .get(key)
            .map(|o| o.template.clone())
            .unwrap_or_else(|| x64::default_tmpl(key).to_string())
    }

    /// ラベルの定義(`name:`)
    pub(in crate::compiler::asm_gen) fn get_label_fmt(&self, name: &str) -> String {
        self.tmpl_or_default("label").replace("{name}", name)
    }

    /// 無条件ジャンプ(`jmp name`)
    pub(in crate::compiler::asm_gen) fn get_jmp(&self, name: &str) -> String {
        self.tmpl_or_default("jmp").replace("{dst}", name)
    }

    /// 生成済みのアセンブリの1行が、無条件ジャンプかどうか
    pub(in crate::compiler::asm_gen) fn is_jmp_line(&self, line: &str) -> bool {
        let tmpl = self.tmpl_or_default("jmp");
        let head = tmpl.split("{dst}").next().unwrap_or("");
        let head = head.replace("{space}", "");
        let line = line.trim().trim_start_matches("{space}").trim_start();
        line.starts_with(head.trim_start())
    }

    /// `ret`命令
    pub(in crate::compiler::asm_gen) fn get_ret(&self) -> String {
        self.tmpl_or_default("ret")
    }

    /// 浮動小数点レジスタの退避用に、スタックを16byte確保する
    pub(in crate::compiler::asm_gen) fn get_stack_alloc_16(&self) -> String {
        self.tmpl_or_default("sp_alloc_16")
    }

    /// `get_stack_alloc_16`で確保したスタックを解放する
    pub(in crate::compiler::asm_gen) fn get_stack_free_16(&self) -> String {
        self.tmpl_or_default("sp_free_16")
    }

    /// スタックの先頭(`%rsp`)のメモリ参照
    pub(in crate::compiler::asm_gen) fn get_stack_top_ref(&self) -> String {
        self.tmpl_or_default("sp_ref")
    }

    /// レジスタを0にする命令(`xor r, r`)
    pub(in crate::compiler::asm_gen) fn get_zero_reg(&self, reg: &str) -> String {
        self.tmpl_or_default("zero_reg").replace("{dst}", reg)
    }

    /// 除算(`idiv`/`div`)。`unsigned`なら`div`
    pub(in crate::compiler::asm_gen) fn get_div_inst(
        &self,
        unsigned: bool,
        operand: &str,
    ) -> String {
        let key = if unsigned { "udiv" } else { "idiv" };
        self.tmpl_or_default(key).replace("{dst}", operand)
    }

    /// 除算の前に、`rax`の符号を`rdx`へ拡張する命令(`cltd`/`cqto`、Intel形式では`cdq`/`cqo`)
    pub(in crate::compiler::asm_gen) fn get_sign_extend_acc(&self, is_64bit: bool) -> String {
        let key = match (is_64bit, self.mnemonic_size()) {
            (true, true) => "cqto",
            (true, false) => "cqo",
            (false, true) => "cltd",
            (false, false) => "cdq",
        };
        self.tmpl_or_default(key)
    }

    /// オペコードのフォーマット
    pub(in crate::compiler::asm_gen) fn get_opcode_tmpl(&self, key: &str) -> String {
        self.opcode_fmt
            .get(key)
            // 渡されたキーがない
            .expect(&format!("not found key {}", key))
            .template
            .clone()
    }

    /// 数字のフォーマット
    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn get_fmt_num(&self, value: &str) -> String {
        self.fmt.fmt.num.replace("{}", value).to_string()
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn get_call_func_fmt(&self, func_name: &str) -> String {
        self.fmt.func.call.replace("{name}", func_name)
    }

    pub(in crate::compiler::asm_gen) fn get_fmt_reg(
        &self,
        reg_num: usize,
        size: &Size,
    ) -> Result<String, err::ErrKind> {
        let reg = match &size {
            Size::DB | Size::UDB => &self.reg_fmt.db,
            Size::DW | Size::UDW => &self.reg_fmt.dw,
            Size::DD | Size::UDD => &self.reg_fmt.dd,
            Size::DQ | Size::UDQ => &self.reg_fmt.dq,
            Size::Pointer { .. } => &self.reg_fmt.dq,
            t => {
                crate::GenCompilerErr!(CompilerErrKind::ThisTyIsCantConvertRegsiter(size.clone()))?
            }
        };
        Ok(self
            .fmt
            .fmt
            .reg
            .replace("{}", reg[reg_num].as_str())
            .to_string())
    }

    pub(in crate::compiler::asm_gen) fn get_fmt_float_reg(
        &self,
        reg_num: usize,
        size: &Size,
    ) -> Result<String, err::ErrKind> {
        let reg = match &size {
            Size::F32 => &self.reg_fmt.xmm,
            Size::F64 => &self.reg_fmt.xmm,
            Size::Pointer { .. } => &self.reg_fmt.dq,
            _ => {
                crate::GenCompilerErr!(CompilerErrKind::ThisTyIsCantConvertRegsiter(size.clone()))?
            }
        };
        Ok(self
            .fmt
            .fmt
            .reg
            .replace("{}", reg[reg_num].as_str())
            .to_string())
    }

    // /// レジスタ文字列からレジスタのサイズを取得する
    // pub(in crate::compiler::asm_gen)
    // fn get_reg_size(
    //     &self,
    //     reg_name: &str
    // ) -> Option<Size> {
    //     for (size, registers) in [
    //         (Size::DB, &self.reg_fmt.db),
    //         (Size::DW, &self.reg_fmt.dw),
    //         (Size::DD, &self.reg_fmt.dd),
    //         (Size::DQ, &self.reg_fmt.dq),
    //     ] {
    //         for register in registers {
    //             let formatted = self.fmt.fmt.reg.replace("{}", register);
    //             if formatted == reg_name {
    //                 return Some(size);
    //             }
    //         }
    //     }
    //     None
    // }

    /// 渡された文字列がレジスタのオペランド(`%eax`など)である場合、
    /// 「同じレジスタ」の`size`版のオペランド文字列(`%rax`など)に
    /// 変換して返す。
    ///
    /// `operand`がレジスタでない場合(即値やメモリ参照`-8(%rbp)`など)
    /// は、`get_reg_size`が`None`を返すのでそのまま変更せず返す。
    pub(in crate::compiler::asm_gen) fn resize_reg_operand(
        &self,
        operand: &str,
        size: &Size,
    ) -> Result<String, err::ErrKind> {
        for (_, registers) in [
            (Size::DB, &self.reg_fmt.db),
            (Size::DW, &self.reg_fmt.dw),
            (Size::DD, &self.reg_fmt.dd),
            (Size::DQ, &self.reg_fmt.dq),
        ] {
            for (reg_num, register) in registers.iter().enumerate() {
                let formatted = self.fmt.fmt.reg.replace("{}", register);
                if formatted == operand {
                    return self.get_fmt_reg(reg_num, size);
                }
            }
        }
        Ok(operand.to_string())
    }

    /// 確保されているレジスタの本数
    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn reg_count(&self) -> usize {
        self.reg_fmt.dq.len()
    }

    pub(in crate::compiler::asm_gen) fn float_reg_count(&self) -> usize {
        self.reg_fmt.xmm.len()
    }

    /// `(レジスタ番号, レジスタ名)`のペアを、確保されている
    /// 全レジスタ・全サイズ分列挙する。
    pub(in crate::compiler::asm_gen) fn all_reg_names(
        &self,
    ) -> Result<Vec<(RegisterMode, String)>, err::ErrKind> {
        let mut result = Vec::new();
        for reg_idx in 0..self.reg_count() {
            for size in [Size::DQ, Size::DD, Size::DW, Size::DB] {
                result.push((
                    RegisterMode::Normal(reg_idx),
                    self.get_fmt_reg(reg_idx, &size)?,
                ));
            }
        }
        for reg_idx in 0..self.float_reg_count() {
            for size in [Size::F32, Size::F64] {
                result.push((
                    RegisterMode::Float(reg_idx),
                    self.get_fmt_float_reg(reg_idx, &size)?,
                ));
            }
        }
        Ok(result)
    }

    /// 先頭から`count`個の引数が渡されるレジスタの番号
    pub(in crate::compiler::asm_gen) fn param_regs(&self, count: usize) -> Vec<usize> {
        self.param_fmt.iter().take(count).copied().collect()
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn inline_asm_list(&self) -> Vec<String> {
        self.asm_setting.as_ref().unwrap().get_inline_asm_list()
    }
}

/// `#asm`ブロックのユーザーが書いた行の末尾に付けるマーク。
/// `AsmFmt::ensure_mnemonic_size`は、このマークの付いた行を変更しない
/// (マークは出力時に取り除く)
pub(in crate::compiler::asm_gen) const RAW_ASM_MARK: &str = "\u{1}hexl_raw";

#[cfg(test)]
mod ensure_size_tests {
    use super::*;

    fn sz(line: &str) -> String {
        MngAsmFmt::sized_line(line)
    }

    #[test]
    fn adds_suffix_from_last_register() {
        assert_eq!(sz("sub $16, %rsp\n"), "subq $16, %rsp\n");
        assert_eq!(sz("mov %eax, %ecx\n"), "movl %eax, %ecx\n");
        assert_eq!(sz("add %r8b, %r9b\n"), "addb %r8b, %r9b\n");
        assert_eq!(sz("mov -4(%rbp), %ax\n"), "movw -4(%rbp), %ax\n");
        assert_eq!(sz("  push %rax\n"), "  pushq %rax\n");
        assert_eq!(sz("lea -8(%rbp), %rdi\n"), "leaq -8(%rbp), %rdi\n");
        assert_eq!(sz("shl %cl, %edx\n"), "shll %cl, %edx\n");
    }

    #[test]
    fn keeps_already_sized_and_others() {
        for l in [
            "movl %eax, -4(%rbp)\n",
            "addq $1, -8(%rbp)\n",
            "movss %xmm0, -4(%rbp)\n",
            "call foo\n",
            "ret\n",
            "jmp .L1\n",
            "main:\n",
            ".global main\n",
            "cltd\n",
            "movzbl %al, %eax\n",
        ] {
            assert_eq!(sz(l), l);
        }
    }
}
