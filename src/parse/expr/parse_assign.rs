use crate::parse::node;
use crate::token::token;

use crate::parse::expr::{parse_comper, parse_factor};
use crate::parse::resp;


//  変数に値を代入するノードを作成
pub fn parse_assign(tokens: Vec<token::Token>, index: &mut usize) -> (node::CalculNode, bool) {
    let mut first_var_name: bool = false;
    let mut defined_var: bool = false;
    let mut node = resp::handler::make_value_node(
        &tokens[0],
        node::NodeKind::NodeVarName,
    );

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenAssign => {
                if first_var_name {
                    node.node_type = node::NodeKind::NodeVarName;
                    *index += 1;
                    let right = parse_comper::parse_comper_op(tokens.clone(), index);
                    node = resp::handler::make_operator_node(
                        node,
                        right,
                        node::NodeKind::NodeAssignVar,
                    );
                }
            },
            token::TokenKind::TokenVarDefine => {
                if first_var_name {
                    defined_var = true;
                    node.node_type = node::NodeKind::NodeVarName;
                    *index += 1;
                    let right = parse_comper::parse_comper_op(tokens.clone(), index);
                    node = resp::handler::make_operator_node(
                        node,
                        right,
                        node::NodeKind::NodeAssignVar,
                    );
                }
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
                    );
                    first_var_name = true;
                } else {
                    break;
                }
            }
            _ => {
                break;
            }
        }
    }

    return (node, defined_var);
}
