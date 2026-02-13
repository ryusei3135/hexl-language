use super::*;


pub fn make_if_node(
        tokens: Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, err_kind::ErrorsKind> {
    let mut if_node = resp::handler::make_null_node();

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenSpace => {},
            token::TokenKind::TokenIf => if_node.node_type = node::NodeKind::NodeIf,
            token::TokenKind::TokenLBrace => {
                break;
            }
            _ => {
                if_node.left_node = Some(Box::new(parse_comper_op(tokens.clone(), index)?));
                continue;
            }
        }
        *index += 1;
    }

    Ok(if_node)
}

pub fn make_if_else_node(
        tokens: Vec<token::Token>,
        index: &mut usize
) -> Result<Option<node::CalculNode>, err_kind::ErrorsKind> {
    let mut if_else_node = resp::handler::make_null_node();
    if_else_node.node_type = node::NodeKind::NodeIfElse;

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenSpace => {},
            token::TokenKind::TokenElse => {
                return Ok(Some(make_else_node(tokens.clone(), index)));
            },
            token::TokenKind::TokenLBrace => {
                break;
            }
            _ => {
                if_else_node.left_node = Some(Box::new(parse_comper_op(tokens.clone(), index)?));
                continue;
            }
        }
        *index += 1;
    }

    if let None = if_else_node.left_node {
        return Ok(None);
    }

    Ok(Some(if_else_node))
}

pub fn make_else_node(tokens: Vec<token::Token>, index: &mut usize) -> node::CalculNode {
    let mut else_node = resp::handler::make_null_node();

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenSpace => {},
            token::TokenKind::TokenElse => else_node = resp::handler::make_true_node(node::NodeKind::NodeElse),
            token::TokenKind::TokenLBrace => {
                break;
            }
            _ => {
                panic!("[syntax err] else");
            }
        }
        *index += 1;
    }

    else_node
}
