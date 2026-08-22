use super::*;

impl IR {
    pub fn scope_node(&mut self, scope: &Vec<String>, target: Box<node::Expr>) -> inst::Inst {
        if let node::Expr::CallFunc(mut call_func_node) = *target {
            if self.expr_counter != 1 {
                // `self.struct_tree.get(..)` は `&self` の借用を返すため、
                // すぐ下で `&mut self` を要求する関数(`init_struct_node`など)
                // を呼べるように、必要な情報だけを先にcloneして借用を切る
                if let Some(struct_info) = self.struct_tree.get(scope.last().unwrap()).cloned() {
                    // フィールドの値は呼び出し元では分からないので、
                    // ここではまだ何も初期化せず、構造体が収まる分の
                    // 「空」のスタック領域だけを確保する(実際の値は、
                    // この後呼び出す関数の中で`self`経由で書き込まれる想定)
                    let mut size = 0;
                    for field in &struct_info.fields {
                        // 確保するスタックを増やす
                        self.stack_counter(&field.ty);
                        size += self.size_of(&field.ty).to_bytes();
                    }

                    // 確保したスタックのノードをirに追加、targetの
                    // 引数の値に、自分自身のポインタを入れる、
                    self.ir_tree.push(inst::Inst::Stacks { size });
                    self.id_counter += 1;

                    // `GetPtr.stk`は`%rbp`からの「バイトオフセット」として
                    // `gen`側でそのまま使われる(`fmt_ref_operand("%rbp", &stk)`)。
                    // 以前はここに`self.id_counter`(命令が生成された順番を
                    // 表すID)を渡してしまっていたため、実際のスタック上の
                    // 位置とは無関係な値がオフセットとして出力され、
                    // `lea -0(%rbp), %rdi`のようにたまたま小さい数字に
                    // なった場合だけ「それらしく」見える壊れたアセンブリが
                    // 生成されていた。
                    // 直前の`for field in &struct_info.fields`ループで
                    // `self.stack_counter(&field.ty)`によりこの構造体の
                    // サイズ分を`self.stk_counter`へ積み終えているため、
                    // ここでの`self.stk_counter`が、この構造体が実際に
                    // 置かれる`%rbp`からの正しい累積オフセットになる
                    self.ir_tree.push(inst::Inst::GetPtr {
                        size,
                        stk: self.stk_counter,
                    });
                    let self_idx = self.id_counter;
                    self.id_counter += 1;

                    // 今確保したスタックの実体を、後から`GetAddress`で
                    // 参照できるようにするため、コンパイラが内部で使う
                    // 一時変数名として`var_tree`に登録しておく
                    let tmp_name = format!("__self_{}", self_idx);
                    self.var_tree.push::<'l'>(
                        &tmp_name,
                        &self_idx,
                        &node::TyNode::Ty(struct_info.name.clone()),
                    );

                    // メゾットの第一引数(`self`)として、今確保した
                    // スタックへのポインタを暗黙的に先頭へ渡す
                    call_func_node.args.insert(
                        0,
                        node::Expr::GetAddress(Box::new(node::Expr::Var(tmp_name))),
                    );
                }
            }
            self.gen_call_func_ir(scope.last(), &call_func_node)
        } else {
            panic!();
        }
    }
}
