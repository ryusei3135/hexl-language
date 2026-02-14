use super::*;


pub fn parse_trim(
        tokens: Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, err_kind::ErrorsKind> {
    let mut node = parse_factor(&tokens, index)?;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenMul => {
                *index += 1;
                let right = parse_factor(&tokens, index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeMul,
                    &tokens[0].line
                );
            }
            token::TokenKind::TokenDiv => {
                *index += 1;
                let right = parse_factor(&tokens, index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeDiv,
                    &tokens[0].line
                );
            }
            token::TokenKind::TokenModulo => {
                *index += 1;
                let right = parse_factor(&tokens, index)?;
                node = resp::handler::make_operator_node(
                    node,
                    right,
                    node::NodeKind::NodeModulo,
                    &tokens[0].line
                );
            }
            token::TokenKind::TokenSpace => {
                *index += 1;
            }
            _ => break,
        }
    }

    return Ok(node);
}
