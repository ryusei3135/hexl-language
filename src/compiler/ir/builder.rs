mod err_factory;
mod member;
mod node_process;
mod preproc;
mod proc_fn_info;
mod scope;
mod variant_bind;
/// ここに処理が分割されている
mod generater;

use std::rc::Rc;

use super::*;
use crate::compiler::ir::types::*;
use crate::compiler::{err::*, models::Body, parse};

const NEXT_VAR_NODE_IS_GET_ADDR: bool = true;

impl IR {
    pub fn new() -> Self {
        Self {
            var_tree: def_tree::VarTree::new(),
            extern_funcs: Vec::new(),
            id_counter: 0,
            func_ret_ty: None,
            ir_tree: Vec::new(),
            pattern_labels: 0,
            loop_labels: Vec::new(),
            scope_states: Vec::new(),
            expr_counter: 0,
            // メゾットの処理中`true`
            this_is_self: false,
            // 関数の情報を初期化
            func_tree: def_tree::FuncTree::new(),
            extern_func_tree: Vec::new(),
            public_func_tree: Vec::new(),
            define_meta_data: Vec::new(),
            struct_tree: def_tree::StructTree::new(),
            variant_tree: def_tree::VariantTree::new(),
            stk_counter: 0,
            current_span: Rc::new(Span::unknown()),
            constract_flag: checker::ConstractFlags::new(),
            var_counter: counter_mnger::VarCounter::new(),
            expr_is_ptr_val: false,
            ternatry: None,
        }
    }

    pub(crate) fn builder<M: ResultMod>(
        &mut self,
        nodes: &[node::Group1Node],
        #[cfg(not(test))] settings: &crate::OptSettings,
    ) -> Result<Vec<def_tree::FnDefMetaData>, err::ErrKind> {
        // 構造体・バリアント型は、定義された場所より前で使われる場合があるので
        // 先に全て登録しておく(前方参照に対応するため)
        for node in nodes {
            match node {
                // 構造体を登録
                node::Group1Node::StructDefine(info) => {
                    self.struct_tree.add(info);
                }
                node::Group1Node::VariantDefine(info) => {
                    self.variant_tree.add(info);
                }
                node::Group1Node::FuncDefine(info) => {
                    // 関数本体を生成する前にシグネチャを登録し、
                    // 前方参照とジェネリック関数の呼び出しを解決する
                    self.func_tree.declare(info);
                }
                _ => {}
            }
        }

        for node in nodes {
            self.expr_counter = 0;
            match node {
                node::Group1Node::FuncDefine(info) => {
                    let _ = self.ini_def_fn_info::<M>(&info)?;
                }
                #[cfg(not(test))]
                node::Group1Node::Include(path) => {
                    self.include_proc(path, settings)?;
                }
                node::Group1Node::StructDefine(info) => {
                    // 構造体の情報を登録
                    self.struct_tree.add(info);
                    // 構造体の中に定義されているメゾットを、
                    // 通常の関数としてIRへ展開する
                    let _ = self.expand_struct_methods::<M>(info)?;
                }
                node::Group1Node::VariantDefine(info) => {
                    // バリアント型の情報を登録
                    self.variant_tree.add(info);
                    // バリアント型の中に定義されているメゾットを、
                    // 通常の関数としてIRへ展開する
                    let _ = self.expand_variant_methods::<M>(info)?;
                }
                _ => {}
            }
        }
        Ok(mem::take(&mut self.define_meta_data))
    }
}

#[cfg(test)]
fn test_span() -> std::rc::Rc<err::Span> {
    std::rc::Rc::new(err::Span::unknown())
}

#[cfg(test)]
fn parse_test_source(src: &str) -> Vec<node::Group1Node> {
    let mut lexer = crate::compiler::lex::Lexer::new();
    lexer.analy(&src.to_string()).unwrap();
    let file_name = std::rc::Rc::new("<test>".to_string());
    let err_pool = std::rc::Rc::new(std::cell::RefCell::new(crate::err::err_pool::ErrPool::new()));
    let mut parser = parse::Parser::new(&file_name, err_pool);
    parser
        .parser(&lexer.gen_tkns, &Vec::<String>::new())
        .unwrap()
        .to_vec()
}