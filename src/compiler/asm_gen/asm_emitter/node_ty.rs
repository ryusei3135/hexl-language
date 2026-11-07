//! IRのノードの型・種類(構造体/メモリ上の値など)を調べる処理

use super::*;

impl AsmEmitter {
    /// IRの値IDから、その式が生成する値の型をたどって取得する。
    pub(in crate::compiler::asm_gen) fn get_expr_ty(&self, node_idx: usize) -> Size {
        match self.fn_state.curr_inst[node_idx] {
            // `Set<比較>`の結果は、8bitのレジスタ(`setcc`)のまま(`asm_emitter/set_cc.rs`)
            inst::Inst::Expr(ref expr) if Self::is_set_cc_kind(&expr.kind) => Size::DB,
            inst::Inst::Expr(ref expr) => self.get_expr_ty(expr.ls),
            inst::Inst::Pointer(inner) => match self.get_expr_ty(inner) {
                Size::Pointer { ty, .. } => *ty,
                ty => ty,
            },
            inst::Inst::GetAddress(..) => Size::DQ,
            inst::Inst::InsertArr { ref name, .. } => self.arr_elem_ty(name),
            inst::Inst::Cast { ref after, .. } => after.clone(),
            inst::Inst::AssignVar { value, .. } => self.get_expr_ty(value),
            inst::Inst::InitArr(ref ids) => {
                let size = ids
                    .first()
                    .map(|id| self.get_expr_ty(*id))
                    .unwrap_or(Size::Void);
                Size::Array {
                    size: Box::new(size),
                    len: ids.len(),
                }
            }
            inst::Inst::CallFunc(..) => Size::DQ,
            ref node => node
                .get_param_ty(&self.fn_state.curr_inst)
                .unwrap_or_else(|| panic!("cannot determine expression type: {:?}", node)),
        }
    }

    /// `idx`が(直接、あるいは`GetAddress`/`Pointer`でラップされた先に)
    /// `Inst::Struct`を指している場合、その`Inst::Struct`自身のidxを返す。
    /// `format_line`が構造体の生成を特別扱いする際、ラップされた
    /// ノードの内側までたどれるようにするためのヘルパー
    pub(in crate::compiler::asm_gen) fn resolve_struct_idx(&self, idx: usize) -> Option<usize> {
        match self.fn_state.curr_inst[idx] {
            inst::Inst::Struct { .. } => Some(idx),
            inst::Inst::GetAddress(inner)
            | inst::Inst::Pointer(inner)
            | inst::Inst::Mov { src: inner, .. } => self.resolve_struct_idx(inner),
            _ => None,
        }
    }

    /// `resolve_struct_idx`のバリアント版
    pub(super) fn resolve_variant_idx(&self, idx: usize) -> Option<usize> {
        match self.fn_state.curr_inst[idx] {
            inst::Inst::Variant { .. } => Some(idx),
            inst::Inst::GetAddress(inner)
            | inst::Inst::Pointer(inner)
            | inst::Inst::Mov { src: inner, .. } => self.resolve_variant_idx(inner),
            _ => None,
        }
    }

    pub(super) fn check_node_is_struct(&self, node_idx: usize) -> bool {
        match &self.fn_state.curr_inst[node_idx] {
            inst::Inst::RefStruct { .. } | inst::Inst::RefStructPtr { .. } => true,
            _ => false,
        }
    }

    /// 渡されたノードがスタック/静的領域の変数(`MemoryValue`)を
    /// 参照している場合、そのサイズを返す。
    /// これは`mov`命令に付けるサイズ接尾辞(`movl`など)を
    /// 決定するために使う。
    #[inline(always)]
    pub(in crate::compiler::asm_gen) fn check_node_is_mem_val(
        &self,
        node_idx: usize,
    ) -> Option<Size> {
        self.check_node_is_mem_val_inner(node_idx, false)
    }

    /// `check_node_is_mem_val`の実装本体
    /// ## 引数
    /// - is_deref
    ///     現在たどっている経路が`Inst::Pointer`(`*ptr`のような
    ///     参照先の読み書き)を経由しているかどうか。
    ///     `Inst::MemoryValue`にたどり着いたとき、これが`true`なら
    ///     ポインタが指す「参照先」の型のサイズ(`ty`)を、`false`なら
    ///     ポインタ変数「自身」の値のサイズ(常に8byte)を返す。
    fn check_node_is_mem_val_inner(&self, node_idx: usize, is_deref: bool) -> Option<Size> {
        if self
            .fn_state
            .spilled_expr_results
            .iter()
            .any(|(id, _)| *id == node_idx)
        {
            return Some(Self::value_reg_size(&self.get_expr_ty(node_idx)));
        }
        match self.fn_state.curr_inst[node_idx] {
            inst::Inst::MemoryValue(inst::MemoryInst::Memory { ref size, .. }) => match size {
                // `*ptr`のように参照先を読み書きする場合のみ、
                // 参照先の型`ty`のサイズを使う
                Size::Pointer { ty, .. } if is_deref => Some(*(*ty).clone()),
                // それ以外(ポインタ変数自身の値をそのまま読む場合)は、
                // ポインタの実際のサイズ(常に8byte)を使う
                Size::Pointer { .. } => Some(Size::DQ),
                other => Some(other.clone()),
            },
            inst::Inst::Pointer(inner) => {
                // ここから先は「参照先」をたどる経路になる
                self.check_node_is_mem_val_inner(inner, true)
            }
            inst::Inst::GetAddress(inner) => {
                // アドレスを求める経路では参照先をたどっているわけ
                // ではないので、`is_deref`はそのまま引き継ぐ
                self.check_node_is_mem_val_inner(inner, is_deref)
            }
            inst::Inst::InsertArr { ref name, .. } => self.var_size(name),
            _ => None,
        }
    }
}
