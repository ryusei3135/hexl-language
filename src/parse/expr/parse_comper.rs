use super::*;



pub fn parse_comper_op(
        tokens: Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, parse_err::ParseErrs> {
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
                );
            },
            token::TokenKind::TokenNotEqTo => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeNotEqTo,
                );
            },
            token::TokenKind::TokenLessThanOrEqualTo => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeLessThanOrEqualTo,
                );
            }
            token::TokenKind::TokenGreaterThanOrEqualTo => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeGreaterThanOrEqualTo,
                );
            }
            token::TokenKind::TokenLessThan => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeLessThan,
                );
            }
            token::TokenKind::TokenGreaterThan => {
                *index += 1;
                let right = parse_expr::parse_expr(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeGreaterThan,
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
