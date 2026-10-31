use super::*;


impl MngAsmFmt {
    /// 外部に定義されている物のフォーマット
    pub(in crate::compiler::asm_gen) fn get_extern_func(&self, name: &str) -> String {
        self.fmt.func.extern_def.replace("{name}", name)
    }

    pub(in crate::compiler::asm_gen) fn func_frame_fmt(&self) -> String {
        self.fmt.fmt.frame.to_string()
    }

    pub(in crate::compiler::asm_gen) fn func_frame_end(&self) -> String {
        self.fmt.fmt.frame_end.to_string()
    }

    /// `ret`命令
    pub(in crate::compiler::asm_gen) fn get_ret(&self) -> String {
        self.tmpl_or_default("ret")
    }

    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn get_call_func_fmt(&self, func_name: &str) -> String {
        self.fmt.func.call.replace("{name}", func_name)
    }
}