use super::*;


//  変数に値を代入するノードを作成
pub fn parse_assign(
        tokens: &Vec<token::Token>,
        index: &mut usize
) -> Result<(node::CalculNode, bool), err_kind::ErrorsKind> {
    let mut first_var_name: bool = false;
    let mut defined_var: bool = false;
    // 変数の名前を設定
    let mut node = resp::handler::make_value_node(
        &tokens[0],
        node::NodeKind::NodeVarName,
    );

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenAssign => {
                return if first_var_name {
                    *index += 1;
                    let right = parse_comper::parse_comper_op(tokens.clone(), index)?;
                    Ok(
                        (resp::handler::make_operator_node(
                            node,
                            right,
                            node::NodeKind::NodeAssignVar),
                        defined_var,)
                    )
                } else {
                    Err(err_kind::ErrorsKind::VarMissingAssignmentTarget)
                };
            },
            token::TokenKind::TokenVarDefine => {
                return if first_var_name {
                    defined_var = true;
                    *index += 1;
                    let right = parse_comper::parse_comper_op(tokens.clone(), index)?;
                    node.node_type = node::NodeKind::NodeDefVar;
                    Ok(
                        (resp::handler::make_operator_node(
                            node,
                            right,
                            node::NodeKind::NodeAssignVar),
                        defined_var,)
                    )
                } else {
                    Err(err_kind::ErrorsKind::VarMissingAssignmentTarget)
                };
            }
            token::TokenKind::TokenSpace => {
                *index += 1;
            },
            token::TokenKind::TokenName => {
                if !first_var_name {
                    let current_token = tokens[*index].clone();
                    node = parse_factor::call_value_node(
                        tokens.clone(),
                        current_token,
                        index
                    )?;
                    // 変数に値を代入するノードに設定
                    first_var_name = true;
                } else {
                    Err(err_kind::ErrorsKind::VarMultipleVariableNames)?
                }
            }
            _ => {
                break;
            }
        }
    }

    return Ok((node, defined_var));
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::tokenizer;
    use crate::token::token::Token;

    fn make_token(txt: String) -> Vec<Token> {
        let mut lexer = tokenizer::Tokenizer::new();
        lexer.make_token(&txt, 0)
    }

    #[test]
    fn check_ok_assign_node() {
        let token = make_token("a = 10".to_string());

        let node = resp::handler::make_operator_node(
            resp::handler::convert_value_to_node("a".to_string(), node::NodeKind::NodeCallVar),
            resp::handler::convert_value_to_node("10".to_string(), node::NodeKind::NodeNum),
            node::NodeKind::NodeAssignVar);

        assert_eq!(
            parse_assign(&token, &mut 0),
            Ok((node, false))
        );
    }

    #[test]
    fn check_multiple_var_names() {
        let token = make_token("a a = 10".to_string());
        assert_eq!(
            parse_assign(&token, &mut 0),
            Err(err_kind::ErrorsKind::VarMultipleVariableNames)
        );
    }

    #[test]
    fn check_miss_assignment_target() {
        let token = make_token("= 10".to_string());
        assert_eq!(
            parse_assign(&token, &mut 0),
            Err(err_kind::ErrorsKind::VarMissingAssignmentTarget)
        );
    }
}
