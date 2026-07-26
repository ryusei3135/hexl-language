use regex::{Captures, Regex};
use super::*;



impl AsmEmitter {
    pub(super) fn format_inline_asm(&mut self, text: &String) -> String {
        let inline_var = Regex::new(r"\$\{([^}]+)\}").unwrap();

        let result = inline_var.replace_all(&text, |caps: &Captures| {
            // caps[1] で中身（変数名）を取得
            let inner = &caps[1]; 
            // デバッグ用に中身を表示
            // 静的変数の名前を取得して返す
            self.get_static_var_name(&inner.to_string())
        });
        result.to_string()
    }

    pub(super) fn deploy_inline_asm(
        &mut self,
        name: &String,
        nodes: &Vec<String>,
    ) {

        let mut pushed_regs: Vec<String> = Vec::new();

        if self.asm_fmt
            .inline_asm_list()
            .iter()
            .find(|v| v.as_str() == name.as_str())
            .is_some()
        {
            let mut inline_asm = String::new();
            for ref one_line in nodes.iter() { 
                // もしinlineアセンブラの中に現在使用中のレジスタがある場合は
                // スタックするアセンブリコードを生成する
                if let Some(reg) = self.search_reg(&self.asm_fmt, &one_line) {
                    pushed_regs.push(reg.clone());
                    self.asm_text.push_str(&self.asm_fmt.get_push(&reg));
                }

                let line = self.format_inline_asm(&one_line);
                inline_asm.push_str(
                    &format!("{}\n", line.as_str())
                );
            }
            self.asm_text.push_str(&inline_asm);

            while let Some(poped) = pushed_regs.pop() {
                self.asm_text.push_str(self.asm_fmt.get_pop(&poped).as_str());
            }
        } else {
            panic!();
        }
    }

    pub fn search_reg(&self, asm_fmt: &mng_fmt::MngAsmFmt, target: &String) -> Option<String> {
        self.var_hash_map
            .iter()
            .find(|&(_name, ref info)| {
                    let reg_txt = asm_fmt.get_fmt_reg(&info.reg, &Size::DD);
                    target.contains(&reg_txt)
                }
            )
            .map(|v| asm_fmt.get_fmt_reg(&v.1.reg, &Size::DQ))
    }
}
