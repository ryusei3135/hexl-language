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
}
