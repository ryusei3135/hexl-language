use super::variable::VariableManager;

/// スタックとスコープを管理
impl VariableManager {
    /// 新しくスタック領域を作成
    pub fn make_new_stack(&mut self) {
        if self.local_scope.is_empty() {
            panic!("[system err] scope is not found");
        } else {
            self.local_scope.last_mut().unwrap().push(self.region_stack_index.len());
            self.region_stack_index.push(Vec::<usize>::new());
        }
    }
    /// スタック領域を削除
    pub fn remove_stack(&mut self) {
        if self.local_scope.last().unwrap().len() > 0 {
            if let Some(index_vec) = self.region_stack_index.pop() {
                for index in index_vec.iter().rev() {
                    self.variables_info_vec.remove(*index);
                }
            }
            self.local_scope.last_mut().unwrap().pop();
        } else {
            panic!("[system err] No stack space in scope");
        }
    }
    /// 新しくスコープを作成
    pub fn make_scope(&mut self) {
        self.local_scope.push(Vec::<usize>::new());
    }
    /// スコープ内にあるすべてのスタック領域を削除し自身を削除する
    pub fn remove_scope(&mut self) {
        for _ in self.local_scope.last().unwrap().clone() {
            self.remove_stack();
        }
        self.local_scope.pop();
    }
}
