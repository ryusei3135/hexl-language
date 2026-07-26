///! 関数の中身を生成する関数は`src/gen/call_func.rs`にある


use super::*;
use crate::asm_setting;


// struct Reg

//   dd: Vec<String>,
//   dq: Vec<String>,

// struct OperandInfo
//    len: usize,
//    template: String,

// struct AsmFormat {
//    reg: Reg,
//    op: HashMap<String, OperandInfo>,

#[derive(Debug, Clone)]
pub struct VarIndexInfo {
    pub reg: usize,
    pub index: usize,
}

pub struct AsmEmitter { 
    pub(super) asm_text: String,
    pub(super) data_sec_text: String,
    pub(super) data_idx: usize,
    pub(super) reg_idx: usize,
    pub(super) expr_vars: Vec<String>,
    pub(super) reserved_label_name: Option<String>,

    pub(super) asm_fmt: mng_fmt::MngAsmFmt, 

    pub(super) curr_inst: Vec<inst::Inst>,
    // (親のid, 変数の名前)
    pub(super) data_map: Vec<(usize, String)>,
    pub(super) last_inst_idx: Vec<(usize, usize)>,
    pub(super) var_hash_map: HashMap<String, VarIndexInfo>,
}

impl AsmEmitter {
    pub fn new(
        asm_setting: asm_setting::AsmSetting,
        asm_fmt: asm_setting::AsmFormat
    ) -> Self {
        let mut me = Self {
            asm_text: String::new(),
            data_sec_text: String::new(),
            data_idx: 0,
            reg_idx: 0,
            expr_vars: Vec::new(),
            reserved_label_name: None,

            asm_fmt: mng_fmt::MngAsmFmt::new(asm_setting, asm_fmt), 
            curr_inst: Vec::new(),
            data_map: Vec::new(),
            last_inst_idx: Vec::new(),
            var_hash_map: HashMap::new(),
        };
        me.data_sec_text.push_str(&me.asm_fmt.get_section_fmt("data"));
        me
    }

    pub fn to_asm_text(
        &mut self,
        func_tree: &mut def_tree::FuncTree,
        asm_fmt_name: &Option<String>,
        extern_funcs: &Vec<inst::Inst>,
        global_funcs: &Vec<String>,
    ) -> String {
        self.asm_text = String::from(&self.asm_fmt.get_entry_point());

        self.gen_global_func_asm(&global_funcs);
        self.gen_extern_func_asm(&extern_funcs);

        // エントリーポイントを先頭に配置
        if let Some(ref mut meta_data)
            = func_tree
                .func
                .remove_entry("_start")
        {
            self.build_func_process(
                meta_data,
                &asm_fmt_name
            );
        }

        for mut func_meta_data in func_tree.func.drain() {
            // == アセンブリ言語の生成 ==
            self.build_func_process(
                &mut func_meta_data,
                &asm_fmt_name
            );
            // == データの初期化 ==
            self.var_hash_map = HashMap::new();
            self.reg_idx = 0;
        }
        format!(
            "{}\n{}\n{}",
            self.data_sec_text,
            self.asm_fmt.get_section_fmt("text"),
            self.asm_text.replace("{space}", "  ")
        )
    }

    #[inline(always)]
    fn gen_global_func_asm(&mut self, global_funcs: &Vec<String>) {
        // 自身が公開する関数を生成
        for func_name in global_funcs.iter() {
            self.asm_text.push_str(
                self.asm_fmt
                .get_global_fmt(func_name)
                .as_str()
            );
        }
    }

    #[inline(always)]
    fn gen_extern_func_asm(&mut self, extern_funcs: &Vec<inst::Inst>) {
        for func in extern_funcs.iter() {
            if let inst::Inst::ExternFunc(name) = func {
                self.asm_text
                    .push_str(
                        self.asm_fmt
                        .get_extern_func(&name)
                        .as_str()
                    );
            } else {
                panic!();
            }
        }
    }


    #[inline(always)]
    pub(super) fn get_static_var_name(&mut self, name: &String) -> String {
        let index = self.var_hash_map.get(name).expect(&format!("this -> {}", name)).index;
        self.extract_operand_text(&index).to_string()
    }

    #[inline(always)]
    pub(super) fn insert_var_info(&mut self, name: &String, index: &usize, reg_idx: usize) {
        self.expr_vars.push(name.clone());
        self.var_hash_map.insert(
            name.clone(),
            VarIndexInfo {
                reg: reg_idx,
                index: *index,
            }
        );
    }

    #[inline(always)]
    pub(super) fn update_value_info(&mut self, name: &String, index: &usize) {
        self.var_hash_map.get_mut(name).unwrap().index = *index;
    }

    #[inline(always)]
    pub(super) fn update_value_reg(&mut self, name: &String, reg: &usize) {
        self.var_hash_map.get_mut(name).unwrap().reg = *reg;
    }

    /// 渡された情報を、設定したアセンブリ言語のフォーマット
    /// 通りに加工する。
    pub(super) fn format_line(
        &mut self,
        opcode: &str,
        dst: Option<&usize>,
        src1: &usize,
        src2: Option<&usize>
    ) -> String {
        let mut formated = if let inst::Inst::Struct(..) = &self.curr_inst[*src1] {
            // 構造体の生成
            let mut txt = self.extract_operand_text(src1);
            txt.push_str(
                self.asm_fmt
                .get_opcode_tmpl(opcode)
                .replace("{dst}", &self.get_reg(dst, &Size::DQ))
                .replace("{src1}", "%rbp")
                .as_str()
            );
            txt
        } else {
            self.asm_fmt
            .get_opcode_tmpl(opcode)
            .replace("{dst}", &self.get_reg(dst, &Size::DD))
            .replace("{src1}", &self.extract_operand_text(src1))
        };

        if self.check_node_is_struct(&src1) {
            formated = self.asm_fmt.mem_ref_fmt(&formated, &Size::DD);
        } else if let Some(size) = self.check_node_is_memory_value(&src1) {
            formated = self.asm_fmt.mem_ref_fmt(&formated, &size);
        }


        if let Some(src2_id) = src2 {
            formated.replace("{src2}", &self.extract_operand_text(src2_id))
        } else {
            formated
        }
    }

    fn check_node_is_struct(&self, node_idx: &usize) -> bool {
        match &self.curr_inst[*node_idx] {
            inst::Inst::RefStruct { .. } => {
                true
            }
            _ => false,
        }
    }

    /// 渡されたノードがスタック/静的領域の変数(`MemoryValue`)を
    /// 参照している場合、そのサイズを返す。
    /// これは`mov`命令に付けるサイズ接尾辞(`movl`など)を
    /// 決定するために使う。
    fn check_node_is_memory_value(&self, node_idx: &usize) -> Option<Size> {
        match &self.curr_inst[*node_idx] {
            inst::Inst::MemoryValue(inst::MemoryInst::Memory { size, .. }) => {
                Some(size.clone())
            }
            _ => None,
        }
    }

    /// ## 引数
    /// - reg_idx これは必ずusizeで無ければいけない、
    fn get_reg(&self, reg_idx: Option<&usize>, size: &Size) -> String {
        let num = 
            if reg_idx.is_none() {
                self.reg_idx
            } else {
                *reg_idx.unwrap()
            };
        self.asm_fmt.get_fmt_reg(&num, &size)
    }

    pub(super) fn extract_operand_text(
        &mut self,
        parent_id: &usize,
    ) -> String {
        match &self.curr_inst[*parent_id] {
            inst::Inst::Num {  value, .. } => {
                self.asm_fmt.get_fmt_num(&value)
            }
            inst::Inst::Param(param) => {
                let num = self.var_hash_map.get(&param.name).unwrap().reg;

                self.asm_fmt.get_fmt_reg(&num, &Size::DD)
            }
            inst::Inst::AssignVar { ref name, .. } => {
                let num = self.var_hash_map.get(&name.to_string()).unwrap().reg;
                self.asm_fmt.get_fmt_reg(&num, &Size::DD)
            }
            inst::Inst::Str { .. } => {
                self.data_map
                    .iter()
                    .find(|v| &v.0 == parent_id)
                    .unwrap()
                    .1
                    .clone()
            }
            inst::Inst::Mov { name, src, .. } => {
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
            inst::Inst::Block(name) => {
                name.to_string()
            }
            inst::Inst::ExpectJmp(name) => {
                name.to_string()
            }
            inst::Inst::Struct(struct_node) => {
                crate::gen_struct_asm!(self, struct_node);
            }
            inst::Inst::MemoryValue(inst::MemoryInst::Memory { kind, size, .. }) => {
                if kind == &inst::MemoryKind::Static {
                    // 静的領域の変数: データセクションに置いたラベルを参照する
                    self.data_map
                        .iter()
                        .find(|v| &v.0 == parent_id)
                        .expect("static var label not found")
                        .1
                        .clone()
                } else {
                    // スタック領域の変数: %rbpからのオフセットを参照する
                    self.asm_fmt.fmt_ref_operand(
                        &"%rbp".to_string(),
                        &size.to_bytes(),
                    )
                }
            }
            inst::Inst::RefStruct { src, size } => {
                self.asm_fmt.fmt_ref_operand(
                    &self.asm_fmt.get_fmt_reg(
                        &self.var_hash_map.get(src).expect(src).reg,
                        &Size::DQ
                    ),
                    &size,
                )
            }
            t => {
                if let Some(result) = self.last_inst_idx
                    .iter()
                    .find(|i| &i.0 == parent_id)
                {
                    // レジスタの文字列を取得
                    self.asm_fmt.get_fmt_reg(&result.1, &Size::DD)
                } else {
                    panic!("{:?}", t);
                }
            }
        }
    }

    pub(super) fn format_expr_inst(
        &mut self,
        expr: &inst::ExprInst,
    ) -> String {
        let key = match expr.kind {
            inst::ExprKind::Add => "add",
            inst::ExprKind::Sub => "sub",
            inst::ExprKind::Mul => "mul",
            inst::ExprKind::Div => "div",
            inst::ExprKind::LessThen => "cmp_l",
            inst::ExprKind::GreaterThen => "cmp_g",
        };

        let formated = self.asm_fmt
            .get_opcode_tmpl(key)
            .replace("{dst}", &self.get_reg(Some(&self.reg_idx), &Size::DD))
            .replace("{src1}", &self.extract_operand_text(&expr.ls))
            .replace("{src2}", &self.extract_operand_text(&expr.rs))
            .to_string();

        if let Some(ref name) = self.reserved_label_name.take() {
            // ラベルの予約があるかつフォーマット中の文字列に"{label}"
            // がない場合はシステムエラー
            if formated.find("{label}").is_some() {
                formated.replace("{label}", name)
            } else {
                panic!("system err");
            }
        } else {
            formated
        }
    }
}

