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
    let formated_node: node::CalculNode = node::CalculNode {
        value: node.left_node.unwrap().value.clone(),
        node_type: node::NodeKind::NodeDefVar,
        left_node: Some(Box::new(var_type_node)),
        right_node: node.right_node,
        block: None
    };

    return formated_node;
}

pub fn parse_var_def(tokens: Vec<token::Token>, index: &mut usize) -> node::CalculNode {
    let mut node = resp::handler::convert_value_to_node(
        "[*null*]".to_string(),
        node::NodeKind::NodeNull
    );

    while tokens.len() > *index {
        match tokens[*index].kind {
            // token::TokenKind::TokenNewVar => {
            //     eprintln!("banana [syntax err]: line {}", tokens[0].line);
            //     eprintln!("unexpected keyword `let`");
            //     panic!("");
            // },
            token::TokenKind::TokenLessThan => {
                // 変数の型
                node = parse_type::parse_type_node(tokens.clone(), index);
                continue;
            },
            token::TokenKind::TokenName => {
                let var_node = parse_assign::parse_assign(tokens.clone(), index);
                return if var_node.1 {
                    format_assign_var_node(var_node.0, node)
                } else {
                    var_node.0
                };
            },
            _ => {
                //
            },
        }
        *index += 1;
    }

    node
}
