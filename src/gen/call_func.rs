use super::*;


impl AsmEmitter {
    pub(super) fn emit_call_func(
        &mut self,
        meta_data: &inst::CallFuncMetaData
    ) -> String {
        // 生成するアセンブリコード
        let mut call_func = String::new();
        println!("{:?}", meta_data);

        for (index, param) in meta_data.params.iter().enumerate() {
            println!(">> {:?}", param);
            // 引数のレジスタを取得
            let param_reg = self.asm_fmt.get_fmt_param::<usize>(&index);
            // 引数のレジスタと値のidを挿入
            let param_asm = self.format_line("mov", Some(&param_reg), &param, None);
            call_func.push_str(&param_asm);
        }
        call_func.push_str(&self.asm_fmt.get_call_func_fmt(&meta_data.name));
        call_func
    }
}
