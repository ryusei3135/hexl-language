//  変数を宣言するノードを作成
use crate::token::token;
use crate::parse::node;
use crate::parse::expr::parse_assign;
use crate::parse::expr::parse_type;
use crate::parse::resp;


//  変数を上書きするノードを変数を宣言するノードに変換
fn format_assign_var_node(
        node: node::CalculNode, 
        var_type_node: node::CalculNode
) -> node::CalculNode {
    let mut formated_node: node::CalculNode = node::CalculNode {
        value: node.left_node.unwrap().value.clone(),
        node_type: node::NodeKind::NodeDefVar,
        left_node: Some(Box::new(var_type_node)),
        right_node: node.right_node
    };

    return formated_node;
}


pub fn parse_var_def(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut starts_with_new_var: bool = false;
    let mut node = resp::handler::convert_value_to_node(
        "[*null*]".to_string(),
        node::NodeKind::NodeNull
    );

    while tokens.len() > *index as usize {
        if starts_with_new_var {
            //  変数を宣言するノード作成成
            match tokens[*index as usize].kind {
                token::TokenKind::TokenNewVar => {
                    eprintln!("banana [syntax err]: line {}", tokens[0].line);
                    eprintln!("unexpected keyword `let`");
                    panic!("");
                },
                token::TokenKind::TokenLessThan => {
                    // 変数の型
                    node = parse_type::parse_type_node(tokens.clone(), index);
                    continue;
                },
                token::TokenKind::TokenName => {
                    node = format_assign_var_node(
                        parse_assign::parse_assign(tokens.clone(), index),
                        node,
                    );
                    return node;
                },
                _ => {
                    //
                },
            }
        } else {
            match tokens[*index as usize].kind {
                //  変数を宣言
                token::TokenKind::TokenNewVar => {
                    starts_with_new_var = true;
                },
                _ => {
                    //
                },
            }
        }

        *index += 1;
    }

    node
}
