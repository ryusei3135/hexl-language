use super::*;


pub fn parse_expr(
        tokens: Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, err_kind::ErrorsKind> {
    let mut node = parse_trim(tokens.clone(), index)?;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenAdd => {
                *index += 1;
                let right = parse_trim(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeAdd,
                );
            }
            token::TokenKind::TokenSub => {
                *index += 1;
                let right = parse_trim(tokens.clone(), index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeSub,
                );
            }
            token::TokenKind::TokenSpace => {
                *index += 1;
            }
            _ => {
                break;
            }
        }
    }

    return Ok(node);
}
