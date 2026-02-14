use super::*;



pub fn parse_comper_op(
        tokens: Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, err_kind::ErrorsKind> {
    let mut node = parse_expr::parse_expr(tokens.clone(), index)?;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenEqTo => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeEqTo,
                    &tokens[0].line
                );
            },
            token::TokenKind::TokenNotEqTo => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeNotEqTo,
                    &tokens[0].line
                );
            },
            token::TokenKind::TokenLessThanOrEqualTo => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeLessThanOrEqualTo,
                    &tokens[0].line
                );
            }
            token::TokenKind::TokenGreaterThanOrEqualTo => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeGreaterThanOrEqualTo,
                    &tokens[0].line
                );
            }
            token::TokenKind::TokenLessThan => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeLessThan,
                    &tokens[0].line
                );
            }
            token::TokenKind::TokenGreaterThan => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeGreaterThan,
                    &tokens[0].line
                );
            }
            token::TokenKind::TokenSpace => {
                *index += 1;
            },
            _ => {
                break;
            }
        }
    }

    Ok(node)
}
