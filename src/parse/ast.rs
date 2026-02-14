use super::*;


pub struct Parser<'a> {
    pub brace_depth: i32,
    pub all_info: &'a mut node::AllInfo,
}

impl<'a> Parser<'a> {
    pub fn new(all_info: &'a mut node::AllInfo) -> Self {
        Parser {
            brace_depth: 0,
            all_info: all_info,
        }
    }

    pub fn make_node(
            &mut self,
            token: Vec<token::Token>
    ) -> Result<(), err_kind::ErrorsKind>{
        let mut index: usize = 0;
        // 式などのノードを作成する関数を呼び出すときは、indexを何もせずに
        // そのままで渡すこと
        while token.len() > index {
            match token[index].kind {
                token::TokenKind::TokenNum => {
                    let node = expr::parse_expr::parse_expr(token.clone(), &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                }
                token::TokenKind::TokenBoolTrue => {
                    let node = expr::parse_expr::parse_expr(token.clone(), &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                }
                token::TokenKind::TokenBoolFalse => {
                    let node = expr::parse_expr::parse_expr(token.clone(), &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                }
                token::TokenKind::TokenFloat => {
                    let node = expr::parse_expr::parse_expr(token.clone(), &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                }
                token::TokenKind::TokenFuncStart => {
                    let func_node = semantic::func::make_func_header(token.clone(), &mut index)?;
                    self.brace_depth = 0;
                    self.all_info.func_info.add_func(func_node.clone())?;
                    continue;
                }
                token::TokenKind::TokenUsePackage => {
                    let package_node = semantic::package_node::make_use_package_node(token.clone(), &mut index);
                    load::load_native_lib(
                        self.all_info,
                        &package_node
                    );
                    break;
                }
                // token::TokenKind::TokenNewVar => {
                //     let node = expr::parse_def::parse_var_def(token.clone(), &mut index);
                // }
                token::TokenKind::TokenName => {
                    let node = expr::parse_def::parse_var_def(&token, &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                }
                token::TokenKind::TokenLessThan => {
                    let node = expr::parse_def::parse_var_def(&token, &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                }
                token::TokenKind::TokenVarMut => {
                    let node = expr::parse_def::parse_var_def(&token, &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                }
                token::TokenKind::TokenVarImm => {
                    let node = expr::parse_def::parse_var_def(&token, &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                }
                token::TokenKind::TokenSpace => {}
                token::TokenKind::TokenRet => {
                    let node = semantic::ret::make_ret_node(token.clone(), &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                }
                token::TokenKind::TokenLBrace => {
                    self.brace_depth += 1;
                }
                token::TokenKind::TokenRBrace => {
                    self.brace_depth -= 1;
                    index += 1;
                    if let Some(node) = make_if_else_node(token.clone(), &mut index)? {
                        self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                    }
                    continue;
                }
                token::TokenKind::TokenIf => {
                    let node = make_if_node(token.clone(), &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                    continue;
                }
                token::TokenKind::TokenFor => {
                    let node = make_for_node(&token, &mut index)?;
                    self.all_info.func_info.add_func_calcul_node(node.clone(), self.brace_depth.clone());
                    continue;
                }
                _ => {
                    println!("{:?} token ast", token[index].kind);
                }
            }

            index += 1;
        }
        Ok(())
    }
}
