//! 使用回数が0回の変数(と、その初期値だけを構成するノード)の検出

use super::*;

impl UsedRegManager {
    /// 生成しない変数を定義する`Inst::Mov`のノードid(`body`の添字)を集める
    ///
    /// ## 生成しない変数
    /// 1. 使用回数が0回の変数
    /// 2. 生成しない変数の初期値の中で参照される変数のうち、
    ///    使用回数のすべてがその初期値の中にあるもの。
    ///
    /// ## 対象外
    /// - 初期値の式に関数の呼び出し・代入・inlineアセンブラがある変数:
    ///   生成しないと副作用が消えてしまうため
    /// - 同じ名前の変数への再代入(`Inst::AssignVar`)がある変数:
    ///   再代入のために変数の情報が必要なため
    ///
    /// 使用回数は、`var_counter`に出てこない変数を0回として扱う
    ///
    /// 戻り値には、生成しない変数の`Inst::Mov`に加えて、その初期値の式だけを
    /// 構成するノード(`Inst::Expr`・`Inst::Float`・`Inst::Str`)のidも含める
    /// (`gen_node_asm`はこれらも生成しない。残すと、式が未登録の変数を参照したり、
    /// 使われない式の命令・定数が出力されたりする)
    pub(in crate::compiler::asm_gen) fn unused_var_nodes(
        var_counter: &counter_mnger::VarCounter,
        body: &[inst::Inst],
    ) -> std::collections::HashSet<usize> {
        use std::collections::{HashMap, HashSet};

        let mut counts: HashMap<String, usize> = HashMap::new();
        for (name, count) in var_counter.vars_by_priority() {
            *counts.entry(name.to_string()).or_insert(0) += Self::count_value(count);
        }
        let assigned: HashSet<&str> = body
            .iter()
            .filter_map(|node| match node {
                inst::Inst::AssignVar { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();

        let mut removed: HashSet<usize> = HashSet::new();
        // 生成しない変数の式の中で使われている回数(変数の名前ごと)
        let mut dead_uses: HashMap<String, usize> = HashMap::new();

        loop {
            let mut changed = false;
            for (idx, node) in body.iter().enumerate() {
                let inst::Inst::Mov {
                    name: Some(name),
                    src,
                    ..
                } = node
                else {
                    continue;
                };
                if removed.contains(&idx) || assigned.contains(name.as_str()) {
                    continue;
                }
                // 生成する(生成しない式の外で使われている)変数が残っている
                let uses = counts.get(name.as_str()).copied().unwrap_or(0);
                if uses > dead_uses.get(name.as_str()).copied().unwrap_or(0) {
                    continue;
                }
                let mut refs = Vec::new();
                let mut dead_nodes = Vec::new();
                if !Self::collect_pure_expr_vars(
                    body,
                    *src,
                    &mut HashSet::new(),
                    &mut refs,
                    &mut dead_nodes,
                ) {
                    continue;
                }
                removed.insert(idx);
                removed.extend(dead_nodes);
                for var in refs {
                    *dead_uses.entry(var).or_insert(0) += 1;
                }
                changed = true;
            }
            if !changed {
                break;
            }
        }
        removed
    }

    /// 式`id`が副作用の無い式(関数の呼び出し・代入・inlineアセンブラを
    /// 含まない)なら`true`を返し、式の中で使われている変数(`Inst::Mov`のノード)
    /// の名前を`vars`へ追加する
    ///
    /// 式を構成するノード(`Inst::Expr`・`Inst::Float`・`Inst::Str`)のidは
    /// `nodes`へ追加する(変数が生成されないとき、これらも生成しない)。
    ///
    /// 同じノードに複数回たどり着いても、1回だけ数える(`visited`)。
    /// 数え漏らす分には、変数を生成する側(安全側)に倒れるだけなので、
    /// 数え過ぎないことを優先している
    fn collect_pure_expr_vars(
        body: &[inst::Inst],
        id: usize,
        visited: &mut std::collections::HashSet<usize>,
        vars: &mut Vec<String>,
        nodes: &mut Vec<usize>,
    ) -> bool {
        if !visited.insert(id) {
            return true;
        }
        let Some(node) = body.get(id) else {
            return true;
        };
        match node {
            // 変数の使用(中身の式はたどらない)
            inst::Inst::Mov {
                name: Some(name), ..
            } => {
                vars.push(name.clone());
                true
            }
            // 式(`fv + f2`など)。オペランドも副作用が無ければ、式のノードごと
            // 生成しない(`nodes`に加える)
            inst::Inst::Expr(expr) => {
                if matches!(expr.kind, inst::ExprKind::Inc | inst::ExprKind::Dec) {
                    return false;
                }
                // 三項演算子は、条件(ひとつ前のノード。`asm_emitter/ternary.rs`)も
                // 一緒に消さないと、生成しない変数を参照する`cmp`だけが残ってしまう
                if expr.kind == inst::ExprKind::Ternary {
                    if let Some(cond) = id.checked_sub(1) {
                        if !Self::collect_pure_expr_vars(body, cond, visited, vars, nodes) {
                            return false;
                        }
                    }
                }
                if !Self::collect_pure_expr_vars(body, expr.ls, visited, vars, nodes)
                    || !Self::collect_pure_expr_vars(body, expr.rs, visited, vars, nodes)
                {
                    return false;
                }
                nodes.push(id);
                true
            }
            // キャスト: 元の値が副作用の無い式なら、キャストのノードごと生成しない
            inst::Inst::Cast { src, .. } => {
                if !Self::collect_pure_expr_vars(body, *src, visited, vars, nodes) {
                    return false;
                }
                nodes.push(id);
                true
            }
            // 浮動小数点・文字列のリテラル(定数を`.data`へ出力するノード)
            inst::Inst::Float { .. } | inst::Inst::Str { .. } => {
                nodes.push(id);
                true
            }
            inst::Inst::GetAddress(inner) | inst::Inst::Pointer(inner) => {
                Self::collect_pure_expr_vars(body, *inner, visited, vars, nodes)
            }
            inst::Inst::InitArr(ids) => ids
                .iter()
                .all(|item| Self::collect_pure_expr_vars(body, *item, visited, vars, nodes)),
            // 副作用がある
            inst::Inst::CallFunc(..) | inst::Inst::AssignVar { .. } | inst::Inst::Comple { .. } => {
                false
            }
            _ => true,
        }
    }
}
