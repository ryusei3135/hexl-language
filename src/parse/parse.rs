use crate::token::token;

use crate::parse::expr;
use crate::parse::semantic;
use crate::manager::global_state::func_manager;



pub struct Parser {
    pub brace_depth: i32,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            brace_depth: 0,
        }
    }

    pub fn make_node(
            &mut self,
            token: Vec<token::Token>
    ) {
        let mut index: i32 = 0;

        while token.len() > index as usize {
            match token[index as usize].kind {
                token::TokenKind::TokenNum => {
                    let node = expr::parse_expr::parse_expr(token.clone(), &mut index);
                    func_manager().add_func_calcul_node(node);
                },
                token::TokenKind::TokenFuncStart => {
                    let func_node = semantic::func::make_func_header(token.clone(), &mut index);
                    func_manager().add_func(func_node);
                },
                token::TokenKind::TokenUsePackage => {
                    let package_node = semantic::package::make_use_package_node(token.clone(), &mut index);
                    func_manager().add_func_calcul_node(package_node);
                },
                token::TokenKind::TokenNewVar => {
                    let node = expr::parse_def::parse_var_def(token.clone(), &mut index);
                    func_manager().add_func_calcul_node(node);
                },
                token::TokenKind::TokenName => {
                    let node = expr::parse_assign::parse_assign(token.clone(), &mut index);
                    func_manager().add_func_calcul_node(node);
                },
                token::TokenKind::TokenSpace => {
                    index += 1;
                    continue;
                },
                token::TokenKind::TokenLBrace => self.brace_depth += 1,
                token::TokenKind::TokenRBrace => self.brace_depth -= 1,
                _ => {
                    println!("{:?}", token[index as usize].kind);
                }
            }

            index += 1;
        }
    }
}
