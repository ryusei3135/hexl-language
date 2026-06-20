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


pub struct AsmEmitter { 
    asm_text: String,
    data_sec_text: String,
    data_idx: usize,
    reg_idx: usize,
    // フォーマット
    reg_format: Option<Reg>,
    op_format: Option<HashMap<String, OperandInfo>>,

    curr_inst: Vec<inst::Inst>,
    data_map: Vec<(usize, String)>,
    last_inst_idx: Vec<(usize, usize)>,
    var_hash_map: HashMap<String, usize>,
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

            curr_inst: Vec::new(),
            data_map: Vec::new(),
            last_inst_idx: Vec::new(),
            var_hash_map: HashMap::new(),
        }
    }

    pub fn to_asm_text(&mut self, func_tree: &mut FuncTree, asm_format: &AsmFormat) -> String {
        self.asm_text = String::new();
        let _ = self.reg_format.insert(asm_format.reg.clone());
        let _ = self.op_format.insert(asm_format.op.clone());

        for func in func_tree.func.iter_mut() {
            self.asm_text.push_str(func.0);
            self.asm_text.push('\n');
            self.curr_inst = mem::take(&mut func.1.tree);

            for node in self.curr_inst.iter() {
                println!("{:?}", node);// =============================
                match &node {
                    inst::Inst::Str { dst, value } => {
                        self.data_sec_text.push_str(
                            &format!("M{}: db {}\n", self.data_idx.to_string(), value)
                        );
                        self.data_map.push((*dst, format!("M{}", self.data_idx.to_string())));
                        self.data_idx += 1;
                    }
                    inst::Inst::Expr(expr) => {
                        let asm = self.format_expr_inst(&asm_format, &expr);

                        self.asm_text.push_str(&asm);

                        self.last_inst_idx.push(
                            (expr.dst, self.reg_idx)
                        );
                        self.reg_idx += 1;
                    }
                    inst::Inst::Num{ .. } => {
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
                    inst::Inst::Mov { name, src, .. } => {
                        let template = &asm_format.op.get("mov").unwrap().template;
                        let formated = template
                            .replace("{dst}", &self.reg_format.as_ref().unwrap().dd[self.reg_idx])
                            .replace("{src}", &self.extract_operand_text(&src));
                        if let Some(var_name) = name {
                            self.var_hash_map.insert(var_name.clone(), self.reg_idx);
                        }
                        self.reg_idx += 1;
                        self.asm_text.push_str(&formated);
                    }
                    t => panic!("{:?}", t),
                }
            }

            self.var_hash_map = HashMap::new();
            self.reg_idx = 0;
        }
        println!("{}\n{}", self.data_sec_text, self.asm_text);
        mem::take(&mut self.asm_text)
    }

    /// 渡された情報を、設定したアセンブリ言語のフォーマット
    /// 通りに加工する。
    fn format_line(
        &self,
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
        &self,
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
            inst::Inst::Mov { name, .. } => {
                if let Some(var_name) = name {
                    let reg_num = self.var_hash_map.get(&*var_name).unwrap();
                    self.reg_format.as_ref().unwrap().dd[*reg_num].as_str()
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
        &self,
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
