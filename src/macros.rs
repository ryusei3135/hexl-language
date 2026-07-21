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
