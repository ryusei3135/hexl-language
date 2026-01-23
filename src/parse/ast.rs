use super::*;


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
        // 式などのノードを作成する関数を呼び出すときは、indexを何もせずに
        // そのままで渡すこと
        while token.len() > index as usize {
            match token[index as usize].kind {
                token::TokenKind::TokenNum => {
                    let node = expr::parse_expr::parse_expr(token.clone(), &mut index);
                    func_manager().add_func_calcul_node(node, self.brace_depth.clone());
                }
                token::TokenKind::TokenFuncStart => {
                    let func_node = semantic::func::make_func_header(token.clone(), &mut index);
                    func_manager().add_func(func_node);
                    continue;
                }
                token::TokenKind::TokenUsePackage => {
                    let package_node = semantic::package_node::make_use_package_node(token.clone(), &mut index);
                    load::load_native_lib(package_node);
                    break;
                }
                token::TokenKind::TokenNewVar => {
                    let node = expr::parse_def::parse_var_def(token.clone(), &mut index);
                    func_manager().add_func_calcul_node(node, self.brace_depth.clone());
                }
                token::TokenKind::TokenName => {
                    let node = expr::parse_assign::parse_assign(token.clone(), &mut index);
                    func_manager().add_func_calcul_node(node, self.brace_depth.clone());
                }
                token::TokenKind::TokenSpace => {}
                token::TokenKind::TokenRet => {
                    let node = semantic::ret::make_ret_node(token.clone(), &mut index);
                    func_manager().add_func_calcul_node(node, self.brace_depth.clone());
                }
                token::TokenKind::TokenLBrace => self.brace_depth += 1,
                token::TokenKind::TokenRBrace => {
                    self.brace_depth -= 1;
                    index += 1;
                    if let Some(node) = make_if_else_node(token.clone(), &mut index) {
                        func_manager().add_func_calcul_node(node, self.brace_depth.clone());
                    }
                    continue;
                }
                token::TokenKind::TokenIf => {
                    let node = make_if_node(token.clone(), &mut index);
                    func_manager().add_func_calcul_node(node, self.brace_depth.clone());
                    continue;
                }
                token::TokenKind::TokenFor => {
                    let node = make_for_node(token.clone(), &mut index);
                    println!("{:?} ::", node);
                    func_manager().add_func_calcul_node(node, self.brace_depth.clone());
                    continue;
                }
                _ => {
                    println!("{:?}", token[index as usize].kind);
                }
            }

            index += 1;
        }
    }
}
