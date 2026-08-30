use super::*;

impl IR {
    /// `mod::func()`や`StructName.method()`のような、スコープを伴う
    /// 呼び出しを処理する
    ///
    /// ## `var_name`について
    /// この呼び出し(`Point.new()`など)の結果を格納する予定の
    /// 「元の変数名」があれば、それをここに渡す
    /// (例: `let p = Point.new();`や`p = Point.new();`の`p`)。
    /// - `Some`の場合: 構造体の初期化のために確保する一時的な
    ///   スタック領域を表す変数を、コンパイラ内部の合成名
    ///   (`__self_N`)ではなく、この`var_name`でそのまま
    ///   `var_tree`に登録する
    /// - `None`の場合: 対応する変数名が存在しない文脈
    ///   (関数の引数や構造体フィールドの初期化式として直接
    ///   使われた場合など)から呼ばれているので、内部的な
    ///   仮の名前を使う
    pub fn scope_node(
        &mut self,
        scope: &Vec<String>,
        target: Box<node::Expr>,
        var_name: Option<&String>,
    ) -> inst::Inst {
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
                    // 参照できるようにするため`var_tree`に登録しておく。
                    // 呼び出し元から「元の変数名」(`var_name`)が
                    // 渡されている場合は、コンパイラ内部の合成名
                    // (`__self_N`)を作らず、そのままその名前で登録する
                    // (以後、その変数名で参照した際に、この一時領域を
                    //  指すようにするため)。対応する変数名が無い場合
                    // のみ、内部的な仮の名前を使う
                    let tmp_name = match var_name {
                        Some(name) => name.clone(),
                        None => format!("$self_area_{}", self_idx),
                    };
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
            self.gen_call_fn_ir(scope.last(), &call_func_node)
        } else {
            panic!();
        }
    }
}
