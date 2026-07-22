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
        return Err(err::PreprocErrs::$name.build(&$self.current_line()));
    };
}

/// src/lex/pathで使う
#[macro_export]
macro_rules! scope_node {
    ($self:tt, $target:ident, $result:ident) => {
        // "::"をスキップ
        let _ = $self.next_tkn()?;
        let mut path_node = Vec::<String>::new();
        loop {
            if let lex::Tkn::Name(name) = $self.next_tkn()? {
                path_node.push(name);
            } else {
                panic!();
            }

            if !matches!($self.next_tkn_ref()?, lex::Tkn::$target) {
                let expr =
                    $self.expr_define_var(
                        path_node
                        .last()
                        .unwrap()
                        .to_string()
                    )?;
                let node = node::Group2Node::$result {
                    scope: path_node.clone(),
                    target: Box::new(
                        expr
                    )
                };
                return Ok(node);
            }
            let _ = $self.next_tkn()?;
        }
    };
}
