use super::*;

//  変数を上書きするノードを変数を宣言するノードに変換
fn format_assign_var_node(
        node: node::CalculNode,
        var_type_node: node::CalculNode
) -> node::CalculNode {
    let formated_node: node::CalculNode = node::CalculNode {
        value: node.left_node.unwrap().value.clone(),
        node_type: node::NodeKind::NodeDefVar,
        left_node: Some(Box::new(var_type_node)),
        right_node: node.right_node,
        block: None
    };

    return formated_node;
}

pub fn parse_var_def(
        tokens: &Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, parse_err::ParseErrs> {
    // 変数の型が定義されている状態 <type>nameのときtypeがあるなら
    // 必ず変数の名前を存在する
    let mut existe_var_name: bool = false;
    let mut node = resp::handler::convert_value_to_node(
        "[*null*]".to_string(),
        node::NodeKind::NodeNull
    );

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenLessThan => {
                existe_var_name = true;
                // 変数の型
                node = parse_type::parse_type_node(tokens, index)?;
                continue;
            },
            token::TokenKind::TokenName => {
                let var_node = parse_assign::parse_assign(&tokens, index)?;
                return Ok(
                    if var_node.1 {
                        format_assign_var_node(var_node.0, node)
                    } else {
                        var_node.0
                    }
                );
            },
            _ => {
                if existe_var_name {
                    Err(parse_err::ParseErrs::VarMissingVarNameAfterType)?
                }
                panic!("{:?} parse def", tokens[*index].kind);
            },
        }
        *index += 1;
    }

    Ok(node)
}
