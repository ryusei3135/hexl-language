use crate::parse::node;
use crate::token::token;

use crate::parse::expr::parse_comper;
use crate::parse::resp;


//  変数に値を代入するノードを作成
pub fn parse_assign(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut first_var_name: bool = false;
    let mut node = resp::handler::make_value_node(
        &tokens[0],
        node::NodeKind::NodeVarName,
    );

    while tokens.len() > *index as usize {
        match tokens[*index as usize].kind {
            token::TokenKind::TokenAssign => {
                if first_var_name {
                    *index += 1;
                    let right = parse_comper::parse_comper_op(tokens.clone(), index);
                    node = resp::handler::make_operator_node(
                        node,
                        right,
                        node::NodeKind::NodeAssignVar,
                    );
                }
            },
            token::TokenKind::TokenSpace => {
                *index += 1;
            },
            token::TokenKind::TokenName => {
                if !first_var_name {
                    node = resp::handler::make_value_node(
                        &tokens[*index as usize],
                        node::NodeKind::NodeVarName,
                    );
                    first_var_name = true;
                    *index += 1;
                } else {
                    break;
                }
            }
            _ => {
                break;
            }
        }
    }

    return node;
}
