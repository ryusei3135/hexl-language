
//! `extract_operand_text`からのみ呼び出すAPI

use super::*;


impl AsmEmitter {
    pub(super) fn param_ref(
        &mut self,
        param_name: &String,
    ) -> String {
        let var_info = self.var_hash_map.get(&param_name.to_string()).unwrap();
        let reg = self.asm_fmt.get_fmt_reg(&var_info.reg, &Size::DD);

        if let Some(ty) = var_info.size.is_pointer() {
            self.asm_fmt.fmt_ref_operand(
                &reg,
                &ty.to_bytes()
            )
        } else {
            reg
        }
    }

    pub(super) fn string_mem_ref(&mut self, parent_id: &usize) -> String {
        self.data_map
            .iter()
            .find(|v| &v.0 == parent_id)
            .unwrap()
            .1
            .clone()
    }

    /// 配列に値を代入するコードを生成
    pub(super) fn insert_arr_txt(
        &mut self, 
        name: &String,
        dst: &usize,
        index: &usize,
    ) -> String {
        let size = self.var_hash_map.get(&name.to_string()).unwrap().size.to_bytes();
        let pos = size * index + size;
        let mut src = self.extract_operand_text(&dst);
        src = src.replace(&format!("{}", size), &pos.to_string());
        src
    }

    /// 構造体を参照するコードを作成
    pub(super) fn ref_struct_txt(
        &mut self,
        src: &str,
        size: &usize
    ) -> String {
        self.asm_fmt.fmt_ref_operand(
            &self.asm_fmt.get_fmt_reg(
                &self.var_hash_map.get(&src.to_string()).expect(&src).reg,
                &Size::DQ
            ),
            &size,
        )
    }

    pub(super) fn gen_mov_code(
        &mut self, 
        name: &Option<String>, 
        src: &usize
    ) -> String {
        let Some(var_name) = name else {
            panic!();
        };

        let reg_num = if let Some(var) = self.var_hash_map.get(&*var_name) {
            var.reg
        } else {
            panic!("this var is not found -> {}", name.as_ref().unwrap());
        };
        
        if let Some(static_var) = self.data_map.iter().find(|v| &v.0 == src) {
            // static領域の変数を返す:
            //self.var_hash_map.entry(var_name.to_string()).or_insert(0);
            static_var.1.clone()
        } else {
            self.reg_idx = reg_num.clone();
            self.asm_fmt.get_fmt_reg(&reg_num, &Size::DD)
        }
    }

    /// メモリを参照するコードを生成する
    pub(super) fn ref_mem_value_txt(
        &mut self,
        kind: &inst::MemoryKind,
        size: &Size,
        parent_id: &usize
    ) -> String {
        if matches!(kind, inst::MemoryKind::Static) {
            // 静的領域の変数: データセクションに置いたラベルを参照する
            let name = self.data_map
                .iter()
                .find(|v| &v.0 == parent_id)
                .expect("static var label not found")
                .1
                .clone();
            self.asm_fmt.fmt_static_var_rip(&name)
        } else {
            // スタック領域の変数: %rbpからのオフセットを参照する
            self.asm_fmt.fmt_ref_operand(
                &"%rbp".to_string(),
                &size.to_bytes(),
            )
        }
    }
}


