use super::*;


impl MngAsmFmt {
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
}