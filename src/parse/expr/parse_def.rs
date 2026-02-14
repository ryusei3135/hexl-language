use super::*;

//  変数を上書きするノードを変数を宣言するノードに変換
fn format_assign_var_node(
        node: node::CalculNode,
        var_type_node: Option<node::CalculNode>
) -> node::CalculNode {
    let formated_node: node::CalculNode = node::CalculNode {
        value: node.left_node.unwrap().value.clone(),
        node_type: node::NodeKind::NodeDefVar,
        left_node:
            if let Some(t) = var_type_node {
                Some(Box::new(t))
            } else {
                None
            },
        right_node: node.right_node,
        block: None
    };

    return formated_node;
}

/// "mut"や"i32"などの複数のトークンがあるときに、
/// すべてを組み合わせる関数
/// # 引数
/// - var_status
///     - "mut"や"imm"などの情報
/// - node
///     - 変数の型
/// # semantic/funcで使用
pub fn fotmat_multiple_type(
        var_status: &Option<node::NodeKind>,
        node: &Option<Box<node::CalculNode>>,
        token: &token::Token,
) -> Option<node::CalculNode> {
    if let Some(m) = var_status {
        Some(
            node::CalculNode {
                value: String::new(),
                node_type: m.clone(),
                left_node:
                    if let Some(n) = node {
                        Some(Box::new(*n.clone()))
                    } else {
                        None
                    },
                right_node: None,
                block: Some(token.line.clone()),
            }
        )
    } else {
        if let Some(n) = node {
            Some(*n.clone())
        } else {
            None
        }
    }
}

pub fn parse_var_def(
        tokens: &Vec<token::Token>,
        index: &mut usize,
) -> Result<node::CalculNode, err_kind::ErrorsKind> {
    // 変数の型が定義されている状態 <type>nameのときtypeがあるなら
    // 必ず変数の名前を存在する
    let mut existe_var_name: bool = false;
    let mut var_status: Option<node::NodeKind> = None;
    let mut node: Option<Box<node::CalculNode>> = None;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenVarMut => {
                if var_status == None {
                    var_status = Some(node::NodeKind::NodeMut);
                } else {
                    Err(err_kind::ErrorsKind::MultipleMutabilitySpecifiers)?;
                }
            }
            token::TokenKind::TokenVarImm => {
                if var_status == None {
                    var_status = Some(node::NodeKind::NodeImm);
                } else {
                    Err(err_kind::ErrorsKind::MultipleMutabilitySpecifiers)?;
                }
            }
            token::TokenKind::TokenLessThan => {
                existe_var_name = true;
                // 変数の型
                node = Some(Box::new(parse_type::parse_type_node(tokens, index)?));
                continue;
            }
            token::TokenKind::TokenName => {
                let var_node = parse_assign::parse_assign(&tokens, index)?;

                return Ok(
                    if var_node.1 {
                        format_assign_var_node(
                            var_node.0,
                            fotmat_multiple_type(
                                &var_status,
                                &node,
                                &tokens[0],
                            ),
                        )
                    } else {
                        var_node.0
                    }
                );
            },
            token::TokenKind::TokenSpace => {}
            _ => {
                if existe_var_name {
                    Err(err_kind::ErrorsKind::VarMissingVarNameAfterType)?
                } else {
                    panic!("{:?} parse def", tokens[*index].kind);
                }
            },
        }
        *index += 1;
    }

    Ok(*node.unwrap())
}

#[cfg(test)]
mod tests {
    use crate::token::tokenizer;
    use super::*;

    fn make_mut_token(
            var_type: &Option<node::CalculNode>,
            is_mut: &node::NodeKind
    ) -> Option<Box<node::CalculNode>> {
        Some(Box::new(node::CalculNode {
            value: String::new(),
            node_type: is_mut.clone(),
            left_node:
                if let Some(n) = var_type {
                    Some(Box::new(n.clone()))
                } else {
                    None
                },
            right_node: None,
            block: Some(0),
        }))
    }

    fn make_type_node() -> node::CalculNode {
        node::CalculNode {
            value: "i32".to_string(),
            node_type: node::NodeKind::NodeType,
            left_node: None,
            right_node: None,
            block: Some(0),
        }
    }

    #[test]
    fn test_mut_var() {
        let mut lexer = tokenizer::Tokenizer::new();
        let tokens = lexer.make_token(&"mut i := 0".to_string(), 0);
        // 可変ののみ
        assert_eq!(
            parse_var_def(&tokens, &mut 0),
            Ok(node::CalculNode {
                value: "i".to_string(),
                node_type: node::NodeKind::NodeDefVar,
                left_node: make_mut_token(&None, &node::NodeKind::NodeMut),
                right_node: Some(Box::new(resp::handler::convert_value_to_node("0".to_string(), node::NodeKind::NodeNum))),
                block: None,
            })
        );
        // 可変かつ型の指定がある
        let tokens = lexer.make_token(&"mut <i32>i := 0".to_string(), 0);
        assert_eq!(
            parse_var_def(&tokens, &mut 0),
            Ok(node::CalculNode {
                value: "i".to_string(),
                node_type: node::NodeKind::NodeDefVar,
                left_node: make_mut_token(&Some(make_type_node()), &node::NodeKind::NodeMut),
                right_node: Some(Box::new(resp::handler::convert_value_to_node("0".to_string(), node::NodeKind::NodeNum))),
                block: None,
            })
        );
    }

    #[test]
    fn test_imm_var() {
        let mut lexer = tokenizer::Tokenizer::new();
        let tokens = lexer.make_token(&"imm i := 0".to_string(), 0);
        // 可変ののみ
        assert_eq!(
            parse_var_def(&tokens, &mut 0),
            Ok(node::CalculNode {
                value: "i".to_string(),
                node_type: node::NodeKind::NodeDefVar,
                left_node: make_mut_token(&None, &node::NodeKind::NodeImm),
                right_node: Some(Box::new(resp::handler::convert_value_to_node("0".to_string(), node::NodeKind::NodeNum))),
                block: None,
            })
        );
        // 可変かつ型の指定がある
        let tokens = lexer.make_token(&"imm <i32>i := 0".to_string(), 0);
        assert_eq!(
            parse_var_def(&tokens, &mut 0),
            Ok(node::CalculNode {
                value: "i".to_string(),
                node_type: node::NodeKind::NodeDefVar,
                left_node: make_mut_token(&Some(make_type_node()), &node::NodeKind::NodeImm),
                right_node: Some(Box::new(resp::handler::convert_value_to_node("0".to_string(), node::NodeKind::NodeNum))),
                block: None,
            })
        );
    }
}
