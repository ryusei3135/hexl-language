
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
        // `index`は添字の数字そのものではなく、その数字を保持する
        // `Inst::Num`ノードのid(`curr_inst`内のインデックス)。
        // これを解決せずそのまま計算に使ってしまうと、添字ではなく
        // 「何番目に生成された命令か」という無関係な数字で
        // オフセットが計算されてしまい、間違った位置(あるいは
        // 全く無関係なメモリ)を指すオペランドが生成されてしまう
        let index_value = match &self.curr_inst[*index] {
            inst::Inst::Num { value, .. } => {
                value
                    .parse::<usize>()
                    .expect("配列の添字は数字である必要があります")
            }
            t => panic!("配列の添字には数字のノードが必要です: {:?}", t),
        };

        let size = self.var_hash_map.get(&name.to_string()).unwrap().size.to_bytes();
        let pos = size * index_value + size;

        // 配列本体(`dst`)が実際にメモリ上のどこにあるかを踏まえた
        // オペランドを取得した上で、文字列の中の数字を置換するのではなく、
        // 計算したオフセット(`pos`)で`%rbp`からのオペランドを
        // 直接組み立て直す。
        // (元のコードは`src.replace(&size.to_string(), &pos.to_string())`
        //  のように、既存のオペランド文字列に含まれる数字をそのまま
        //  置換していたため、オフセットが2桁以上になったときなどに
        //  無関係な数字まで置換してしまう可能性があった)
        let _ = self.extract_operand_text(&dst);
        self.asm_fmt.fmt_ref_operand(&"%rbp".to_string(), &pos)
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


