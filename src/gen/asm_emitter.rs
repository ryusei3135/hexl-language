use super::*;
use crate::{
    *,
    ir::inst,
};
use std::mem;


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


struct VarIndexInfo {
    reg: usize,
    index: usize,
}

pub struct AsmEmitter { 
    pub(super) asm_text: String,
    data_sec_text: String,
    data_idx: usize,
    reg_idx: usize,
    // フォーマット
    reg_format: Option<Reg>,
    op_format: Option<HashMap<String, OperandInfo>>,
    pub(super) asm_setting: Option<AsmSetting>,

    curr_inst: Vec<inst::Inst>,
    // (親のid, 変数の名前)
    data_map: Vec<(usize, String)>,
    last_inst_idx: Vec<(usize, usize)>,
    var_hash_map: HashMap<String, VarIndexInfo>,
}

impl AsmEmitter {
    pub fn new() -> Self {
        Self {
            asm_text: String::new(),
            data_sec_text: String::from("section data\n"),
            data_idx: 0,
            reg_idx: 0,
            
            reg_format: None,
            op_format: None,
            asm_setting: None,

            curr_inst: Vec::new(),
            data_map: Vec::new(),
            last_inst_idx: Vec::new(),
            var_hash_map: HashMap::new(),
        }
    }

    pub fn to_asm_text(&mut self, func_tree: &mut FuncTree, asm_setting: AsmSetting) -> String {
        self.asm_text = String::new();

        // === アセンブラのフォーマットの設定 ===
        let _ = self.asm_setting.insert(asm_setting);
        let default = self.asm_setting.as_ref().unwrap().get_default_format();
        let _ = self.reg_format.insert(default.reg.clone());
        let _ = self.op_format.insert(default.op.clone());

        for func in func_tree.func.clone().iter_mut() {
            // 新しく関数の作成、
            self.asm_text.push_str(func.0);
            self.asm_text.push('\n');
            self.curr_inst = mem::take(&mut func.1.tree);

            for node in self.curr_inst.clone().iter() {
                match &node {
                    inst::Inst::Str { dst, value } => {
                        self.data_sec_text.push_str(
                            &format!("M{}: db {}\n", self.data_idx.to_string(), value)
                        );
                        self.data_map.push((*dst, format!("M{}", self.data_idx.to_string())));
                        self.data_idx += 1;
                    }
                    inst::Inst::Expr(expr) => {
                        let asm = self.format_expr_inst(&default, &expr);

                        self.asm_text.push_str(&asm);

                        self.last_inst_idx.push(
                            (expr.dst, self.reg_idx)
                        );
                        self.reg_idx += 1;
                    }
                    inst::Inst::Num{ .. } => {
                    }
                    inst::Inst::Comple{name, nodes} => {
                        self.deploy_inline_asm(&name, &nodes);
                    }
                    inst::Inst::Ret(idx) => {
                        self.format_line(
                            &"mov".to_string(),
                            Some(&0),
                            &idx,
                            None
                        );
                        self.asm_text.push_str("ret\n");
                    }
                    inst::Inst::Mov { name, dst, src } => {
                        if self.data_map.iter().find(|v| &v.0 == src).is_some() {
                            // 子のノードがstaticりょいきの値なので、変数名だけ登録する
                            self.insert_var_info(&name.as_ref().unwrap(), &dst);
                        } else {
                            // レジスタに置く変数
                            let formated = default
                                .op.get("mov")
                                .unwrap()
                                .template
                                .replace("{dst}", &self.reg_format.as_ref().unwrap().dd[self.reg_idx])
                                .replace("{src}", &self.extract_operand_text(&src));
                            if let Some(var_name) = name {
                                self.insert_var_info(&var_name, &dst);
                            }
                            self.reg_idx += 1;
                            self.asm_text.push_str(&formated);
                        }
                    }
                    t => panic!("{:?}", t),
                }
            }

            self.var_hash_map = HashMap::new();
            self.reg_idx = 0;
        }
        format!("{}\nsection text\n{}", self.data_sec_text, self.asm_text)
    }

    #[inline(always)]
    pub(super) fn get_static_var_name(&mut self, name: &String) -> String {
        let index = self.var_hash_map.get(name).unwrap().index;
        self.extract_operand_text(&index).to_string()
    }

    #[inline(always)]
    fn insert_var_info(&mut self, name: &String, index: &usize) {
        self.var_hash_map.insert(
            name.clone(),
            VarIndexInfo {
                reg: self.reg_idx,
                index: *index,
            }
        );
    }

    /// 渡された情報を、設定したアセンブリ言語のフォーマット
    /// 通りに加工する。
    fn format_line(
        &mut self,
        opcode: &String,
        dst: Option<&usize>,
        src1: &usize,
        src2: Option<&usize>
    ) -> String {
        let formated = self.op_format
            .as_ref()
            .unwrap()
            .get(opcode)
            .unwrap()
            .template
            .replace("{dst}", self.get_reg(dst))
            .replace("{src1}", self.extract_operand_text(src1));

        if let Some(src2_id) = src2 {
            formated.replace("{src2}", self.extract_operand_text(src2_id))
        } else {
            formated
        }
    }

    /// ## 引数
    /// - reg_idx これは必ずusizeで無ければいけない、
    fn get_reg(&self, reg_idx: Option<&usize>) -> &String {
        &self.reg_format
            .as_ref()
            .unwrap()
            .dd[
                if reg_idx.is_none() {
                    self.reg_idx
                } else {
                    *reg_idx.unwrap()
                }
            ]
    }

    fn extract_operand_text(
        &mut self,
        parent_id: &usize,
    ) -> &str {
        match &self.curr_inst[*parent_id] {
            inst::Inst::Num {  value, .. } => {
                value
            }
            inst::Inst::Str { .. } => {
                self.data_map
                    .iter()
                    .find(|v| &v.0 == parent_id)
                    .unwrap()
                    .1
                    .as_str()
            }
            inst::Inst::Mov { name, src, .. } => {
                if let Some(var_name) = name {
                    let reg_num = self.var_hash_map.get(&*var_name).unwrap().reg;
                    
                    if let Some(static_var) = self.data_map.iter().find(|v| &v.0 == src) {
                        // static領域の変数を返す:
                        //self.var_hash_map.entry(var_name.to_string()).or_insert(0);
                        &static_var.1
                    } else {
                        self.reg_idx = reg_num.clone();
                        &self.reg_format.as_ref().unwrap().dd[reg_num.clone()]
                    }
                } else {
                    panic!();
                } 
            }
            t => {
                if let Some(result) = self.last_inst_idx
                    .iter()
                    .find(|i| &i.0 == parent_id)
                {
                    self.reg_format
                        .as_ref()
                        .unwrap()
                        .dd[result.1]
                        .as_str()
                } else {
                    panic!("{:?}", t);
                }
            }
        }
    }

    fn format_expr_inst(
        &mut self,
        format: &AsmFormat,
        expr: &inst::ExprInst,
    ) -> String {
        let key = match expr.kind {
            inst::ExprKind::Add => "add",
            inst::ExprKind::Sub => "sub",
            inst::ExprKind::Mul => "mul",
            inst::ExprKind::Div => "div",
        };

        format.op.get(key)
            .unwrap()
            .template
            .replace("{dst}", &self.reg_format.as_ref().unwrap().dd[self.reg_idx])
            .replace("{src1}", &self.extract_operand_text(&expr.ls))
            .replace("{src2}", &self.extract_operand_text(&expr.rs))
    }
}
