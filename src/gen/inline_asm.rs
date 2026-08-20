use super::*;

impl AsmEmitter {
    /// inlineアセンブラのブロックを、実際のアセンブリテキストとして
    /// `self.asm_text`に書き出す。
    ///
    /// `lines`の各要素は`(プレースホルダー入りの行, オペランドのidx)`。
    /// オペランドは変数・構造体のメンバー・ポインタの参照/アドレス取得
    /// などを表すIRノードのidxで、通常の式と同じ`extract_operand_text`
    /// を使ってオペランドの文字列(レジスタ名やメモリ参照など)へ変換し、
    /// `{0}`, `{1}`, ... のプ出現順に埋め込む。
    pub(super) fn deploy_inline_asm(&mut self, name: &String, lines: &Vec<(String, Vec<usize>)>) {
        if self
            .asm_fmt
            .inline_asm_list()
            .iter()
            .find(|v| v.as_str() == name.as_str())
            .is_some()
        {
            // インラインアセンブラの内容によって、現在使用中の
            // レジスタの値が破壊されてしまう可能性があるため、
            // 展開する前に使用中のレジスタを全てスタックに退避(push)する
            let used_regs = self.used_reg.used_regs();
            for reg in used_regs.iter() {
                let reg_name = self.asm_fmt.get_fmt_reg(reg, &Size::DQ);
                let push_asm = self.asm_fmt.get_push(&reg_name);
                self.asm_text.push_str(&push_asm);
            }

            for (template, operand_ids) in lines.iter() {
                let mut asm_line = template.clone();

                for (index, operand_id) in operand_ids.iter().enumerate() {
                    let operand_text = self.extract_operand_text(operand_id, false);
                    asm_line = asm_line.replace(&format!("{{{}}}", index), &operand_text);
                }

                asm_line.push('\n');
                self.asm_text.push_str(&asm_line);
            }

            // 退避しておいたレジスタの値を、pushした時とは逆順に
            // pop して元の状態に戻す
            for reg in used_regs.iter().rev() {
                let reg_name = self.asm_fmt.get_fmt_reg(reg, &Size::DQ);
                let pop_asm = self.asm_fmt.get_pop(&reg_name);
                self.asm_text.push_str(&pop_asm);
            }
        } else {
            panic!();
        }
    }
}
