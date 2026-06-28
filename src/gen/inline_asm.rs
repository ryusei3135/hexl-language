use regex::{Captures, Regex};
use super::*;



impl AsmEmitter {
    pub(super) fn format_inline_asm(&mut self, target: &String) -> String {
        let inline_var = Regex::new(r"\$\{([^}]+)\}").unwrap();

        inline_var.replace_all(target, |caps: &Captures| {
            // caps[1] で ${..} の中身（.. の部分）を取得
            let inner = &caps[1];

            let var_name = self.get_static_var_name(&inner.to_string());
            // 【ここに加工処理を書く】
            // 例: 大文字にして、末尾に ' を付ける
            let modified = format!("{}", var_name);
            
            modified
        }).to_string()
    }

    pub(super) fn deploy_inline_asm(&mut self, name: &String, nodes: &Vec<String>) {
        if self.asm_fmt
            .inline_asm_list()
            .iter()
            .find(|v| v.as_str() == name.as_str())
            .is_some()
        {
            let mut inline_asm = String::new();
            for node in nodes.iter() { 
                inline_asm.push_str(
                    &format!("{}\n", self.format_inline_asm(&node).as_str())
                );
            }
            self.asm_text.push_str(&inline_asm)
        } else {
            panic!();
        }
    }
}
