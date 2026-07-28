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
    pub(super) fn deploy_inline_asm(
        &mut self,
        name: &String,
        lines: &Vec<(String, Vec<usize>)>,
    ) {
        if self.asm_fmt
            .inline_asm_list()
            .iter()
            .find(|v| v.as_str() == name.as_str())
            .is_some()
        {
            for (template, operand_ids) in lines.iter() {
                let mut asm_line = template.clone();

                for (index, operand_id) in operand_ids.iter().enumerate() {
                    let operand_text = self.extract_operand_text(operand_id);
                    asm_line = asm_line.replace(
                        &format!("{{{}}}", index),
                        &operand_text,
                    );
                }

                asm_line.push('\n');
                self.asm_text.push_str(&asm_line);
            }
        } else {
            panic!();
        }
    }
}
