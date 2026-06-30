use super::*;


// struct Reg
//   db: Vec<String>,
//   dw: Vec<String>,
//   dd: Vec<String>,
//   dq: Vec<String>,

// struct OperandInfo
//    len: usize,
//    template: String,

// struct AsmFormat {
//    reg: Reg,
//    op: HashMap<String, OperandInfo>,

#[derive(Debug)]
struct VarIndexInfo {
    pub reg: usize,
    pub index: usize,
}

pub struct AsmEmitter { 
    pub(super) asm_text: String,
    data_sec_text: String,
    data_idx: usize,
    reg_idx: usize,
    expr_vars: Vec<String>,

    pub(super) asm_fmt: mng_fmt::MngAsmFmt, 

    curr_inst: Vec<inst::Inst>,
    // (親のid, 変数の名前)
    data_map: Vec<(usize, String)>,
    last_inst_idx: Vec<(usize, usize)>,
    var_hash_map: HashMap<String, VarIndexInfo>,
}

impl AsmEmitter {
    pub fn new(asm_setting: AsmSetting) -> Self {
        let mut me = Self {
            asm_text: String::new(),
            data_sec_text: String::new(),
            data_idx: 0,
            reg_idx: 0,
            expr_vars: Vec::new(),

            asm_fmt: mng_fmt::MngAsmFmt::new(asm_setting), 
            curr_inst: Vec::new(),
            data_map: Vec::new(),
            last_inst_idx: Vec::new(),
            var_hash_map: HashMap::new(),
        };
        me.data_sec_text.push_str(&me.asm_fmt.get_section_fmt("data"));
        me
    }

    pub fn to_asm_text(&mut self, func_tree: &mut FuncTree) -> String {
        self.asm_text = String::new();

        for func in func_tree.func.clone().iter_mut() {
            // 新しく関数の作成、
            self.asm_text.push_str(&format!("{}:\n", func.0));
            self.curr_inst = mem::take(&mut func.1.body);

            for node in self.curr_inst.clone().iter() {
                match &node {
                    inst::Inst::Str { dst, value } => {
                        self.data_sec_text.push_str(
                            &format!("M{}: db \"{}\"\n", self.data_idx.to_string(), value)
                        );
                        self.data_map.push((*dst, format!("M{}", self.data_idx.to_string())));
                        self.data_idx += 1;
                    }
                    inst::Inst::Expr(expr) => {
                        let asm = self.format_expr_inst(&expr);

                        self.asm_text.push_str(&asm);

                        self.last_inst_idx.push(
                            (expr.dst, self.reg_idx)
                        );
                        self.reg_idx += 1;
                    }
                    inst::Inst::Num{ .. } => {
                    }
                    inst::Inst::Block(name) => {
                        self.asm_text.push_str(&format!("{}:\n", name));
                    }
                    inst::Inst::Comple{name, nodes} => {
                        self.deploy_inline_asm(&name, &nodes);
                    }
                    inst::Inst::Jmp(name) => {
                        self.asm_text.push_str(&format!("jmp {}\n", name));
                    }
                    inst::Inst::AssignVar { name, value } => {
                        self.update_value_info(&name, &value);

                        let current_reg = self.reg_idx;

                        if self.expr_vars.iter().find(|v| v.as_str() == name.as_str()).is_some() {
                            self.update_value_reg(&name, &current_reg);
                            println!("{:?}", self.var_hash_map);
                        }
                    }
                    inst::Inst::Ret(idx) => {
                        self.format_line(
                            "mov",
                            Some(&0),
                            &idx,
                            None
                        );
                        self.asm_text.push_str("ret\n");
                    }
                    inst::Inst::ExpectJmp(name) => {
                        self.asm_text = self.asm_text.replace("{label}", name);
                    }
                    inst::Inst::Mov { name, dst, src } => {
                        if self.data_map.iter().find(|v| &v.0 == src).is_some() {
                            // 子のノードがstaticりょいきの値なので、変数名だけ登録する
                            self.insert_var_info(&name.as_ref().unwrap(), &dst, self.reg_idx);
                        } else {
                            // レジスタに置く変数
                            let formated = self.format_line("mov", Some(dst), src, None);

                            self.reg_idx += 1;
                            self.asm_text.push_str(&formated);

                            if let Some(var_name) = name {
                                self.insert_var_info(&var_name, &dst, self.reg_idx);
                                let current_reg = self.reg_idx;

                                if self.expr_vars.iter().find(|v| v.as_str() == var_name.as_str()).is_some() {
                                    self.update_value_reg(&var_name, &current_reg);
                                }
                            }
                        }
                    }
                    inst::Inst::CallFunc(meta_data) => {
                        self.emit_call_func(&meta_data);
                    }
                    inst::Inst::Param(param) => {
                        let reg_num = self.asm_fmt.get_fmt_param::<usize>(&param.num);
                        self.insert_var_info(
                            &param.name, &param.dst, reg_num
                        );
                    }
                }
            }

            self.var_hash_map = HashMap::new();
            self.reg_idx = 0;
        }
        format!(
            "{}\n{}\n{}",
            self.data_sec_text,
            self.asm_fmt.get_section_fmt("text"),
            self.asm_text
        )
    }

    #[inline(always)]
    pub(super) fn get_static_var_name(&mut self, name: &String) -> String {
        let index = self.var_hash_map.get(name).unwrap().index;
        self.extract_operand_text(&index).to_string()
    }

    #[inline(always)]
    fn insert_var_info(&mut self, name: &String, index: &usize, reg_idx: usize) {
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
    fn update_value_info(&mut self, name: &String, index: &usize) {
        self.var_hash_map.get_mut(name).unwrap().index = *index;
    }

    #[inline(always)]
    fn update_value_reg(&mut self, name: &String, reg: &usize) {
        self.var_hash_map.get_mut(name).unwrap().reg = *reg;
    }

    /// 渡された情報を、設定したアセンブリ言語のフォーマット
    /// 通りに加工する。
    fn format_line(
        &mut self,
        opcode: &str,
        dst: Option<&usize>,
        src1: &usize,
        src2: Option<&usize>
    ) -> String {
        let formated = self.asm_fmt
            .get_opcode_tmpl(opcode)
            .replace("{dst}", &self.get_reg(dst))
            .replace("{src1}", &self.extract_operand_text(src1));

        if let Some(src2_id) = src2 {
            formated.replace("{src2}", &self.extract_operand_text(src2_id))
        } else {
            formated
        }
    }

    /// ## 引数
    /// - reg_idx これは必ずusizeで無ければいけない、
    fn get_reg(&self, reg_idx: Option<&usize>) -> String {
        let num = 
            if reg_idx.is_none() {
                self.reg_idx
            } else {
                *reg_idx.unwrap()
            };
        self.asm_fmt.get_fmt_reg(&num, &Size::DD)
    }

    fn extract_operand_text(
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
            inst::Inst::Str { .. } => {
                self.data_map
                    .iter()
                    .find(|v| &v.0 == parent_id)
                    .unwrap()
                    .1
                    .clone()
            }
            inst::Inst::Mov { name, src, .. } => {
                if let Some(var_name) = name {

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
                } else {
                    panic!();
                } 
            }
            inst::Inst::Block(name) => {
                name.to_string()
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

    fn format_expr_inst(
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

        self.asm_fmt
            .get_opcode_tmpl(key)
            .replace("{dst}", &self.get_reg(Some(&self.reg_idx)))
            .replace("{src1}", &self.extract_operand_text(&expr.ls))
            .replace("{src2}", &self.extract_operand_text(&expr.rs))
            .to_string()
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_func_args() {
        //
    }
}
