//use crate::ir::inst;

/// ブロックやラベルなどジャンプ系のノードを生成する
/// ときに使うマクロ
/// ir/builder.rs
#[macro_export]
macro_rules! push_jmp_code {
    ($tree:expr, $variant:ident, $value:expr) => {
        $tree.ir_tree.push(inst::Inst::$variant(format!("L{}", $value)));
        $tree.id_counter += 1;
    };
}

/// プリプロセッサの構文エラーの戻り値を生成
#[macro_export]
macro_rules! preproc_err {
    ($self:tt, $name:ident) => {
        return Err(err::PreprocErrs::$name.build($self.build_err_span()));
    };
}

/// src/lex/pathで使う
#[macro_export]
macro_rules! scope_node {
    ($self:tt, $target:ident, $result:ident, $start:expr) => {
        // "::"をスキップ
        let _ = $self.next_tkn(vec![])?;
        let mut path_node = Vec::<String>::new();
        path_node.push($start.to_string());
        loop {
            // スコープのノードを作成
            if let lex::Tkn::Name(name) = $self.next_tkn(vec!["name"])? {
                path_node.push(name);
            } else {
                panic!();
            }

            // スコープやメゾットでなくなったので、ノードを作成
            if !matches!($self.next_tkn_ref(vec![])?, lex::Tkn::$target) {
                let expr =
                    match 
                    $self.expr_define_var(
                        path_node
                        .last()
                        .unwrap()
                        .to_string()
                    )?
                    {
                        node::Expr::Assign { .. } => {
                            node::Expr::Var(path_node.last().unwrap().to_string())
                        }
                        n => n,
                    };
                path_node.pop().unwrap();
                let node = node::Expr::$result {
                    scope: path_node.clone(),
                    target: Box::new(
                        expr
                    )
                };
                return Ok(node);
            }
            let _ = $self.next_tkn(vec![])?;
        }
    };
}

/// asm_settingで使う
/// 構造体のアセンブリ言語を生成する処理を展開する
#[macro_export]
macro_rules! gen_struct_asm {
    ($self:tt, $struct_node:path) => {
        let mut struct_txt = String::new();
        let mut add_size = 0;
        for member in $struct_node.clone().iter() {
            let inst::MemoryInst::Member{ value_idx, size, .. } = member else {
                panic!();
            };

            let value = $self.extract_operand_text(&value_idx).to_string();
            // このメンバー分を足した「累積」サイズ
            // (これが、このメンバーの`%rbp`からのオフセットになる)
            add_size += size.to_bytes();

            struct_txt.push_str(
                $self.asm_fmt.get_fmt_struct_member(
                    value,
                    &size,
                    // 既に使用していたスタックのサイズ + ここまでの
                    // メンバーの累積サイズ = このメンバーの正しいオフセット
                    &($self.stk_use_counter + add_size)
                ).as_str()
            );
        }
        $self.stk_use_counter += add_size;
        return struct_txt;
    };
}


#[macro_export]
macro_rules! mov_size_fmt {
    ($self:tt, $size:ident) => {
        $self.fmt.fmt.op_size.$size.clone()
    };
}
