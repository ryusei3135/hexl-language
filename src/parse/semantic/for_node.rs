use super::*;

/// for var in valueのノードを作成
/// # 引数
/// - first_token 最初に来たトークンの情報
/// - tokens 一行のトークンの情報
/// - index トークン"in"の次の場所を可変参照
fn make_token_in_node(
        first_token: &(token::TokenKind, &String),
        tokens: &Vec<token::Token>,
        index: &mut usize,
) -> Result<node::CalculNode, ()> {
    // 最初に変数のトークンが来ていたかつ現在のトークンが"in"
    // なら変数に代入しながら値を更新するノードを作成
    if first_token.0 == token::TokenKind::TokenName {
        *index += 1;
        return Ok(
            node::CalculNode {
                value: first_token.1.clone(),
                node_type: node::NodeKind::NodeIn,
                left_node: Some(Box::new(expr::parse_expr::parse_expr(tokens.clone(), index))),
                right_node: None,
                block: Some(tokens[0].line),
            }
        );
    }
    Err(())
}

/// for文のノードを作成
pub fn make_for_node(
        tokens: &Vec<token::Token>,
        index: &mut usize
) -> node::CalculNode {
    *index += 1;
    let mut for_loop_node = resp::handler::make_null_node();
    let mut first_token: (token::TokenKind, &String) = (token::TokenKind::TokenEOF, &String::new());

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenNum => {
                first_token = (token::TokenKind::TokenNum, &tokens[*index].lexeme);
                for_loop_node = expr::parse_expr::parse_expr(tokens.clone(), index);
                continue;
            }
            token::TokenKind::TokenLBrace => {
                break;
            }
            token::TokenKind::TokenName => {
                if first_token.0 == token::TokenKind::TokenEOF {
                    first_token = (token::TokenKind::TokenName, &tokens[*index].lexeme);
                } else {
                    println!("syntax err for semantic");
                    std::process::exit(1);
                }
            }
            token::TokenKind::TokenIn => {
                for_loop_node = make_token_in_node(&first_token, tokens, index).unwrap();
                break;
            }
            token::TokenKind::TokenSpace => {}
            _ => {}
        }
        *index += 1;
    }

    node::CalculNode {
        value: String::new(),
        node_type: node::NodeKind::NodeFor,
        left_node: Some(Box::new(for_loop_node)),
        right_node: None,
        block: Some(tokens[0].line),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::token;

    fn make_tokens(token_txt: &str) -> Vec<token::token::Token> {
        let mut tokenizer = token::tokenizer::Tokenizer::new();
        tokenizer.make_token(token_txt.to_string(), 0)
    }

    fn in_token_node(for_token: &Vec<token::token::Token>, index: &mut usize) -> node::CalculNode {
        node::CalculNode {
            value: "a".to_string(),
            node_type: node::NodeKind::NodeIn,
            left_node: Some(Box::new(expr::parse_expr::parse_expr(for_token.clone(), index))),
            right_node: None,
            block: Some(0),
        }
    }

    #[test]
    fn check_make_token_in_node() {
        let for_token = make_tokens("for a in 10 {");
        assert_eq!(
            make_token_in_node(
                &(token::token::TokenKind::TokenName, &"a".to_string()),
                &for_token,
                // トークン"in"の次の場所を代入
                // for_tokenの"in"の次は、5
                &mut 5,
            ).unwrap(),
            in_token_node(&for_token, &mut 5),
        );
    }

    #[test]
    fn check_range_for_node() {
        let for_token = make_tokens("for a in 10 {");
        let mut index: usize = 0;
        assert_eq!(
            make_for_node(&for_token, &mut index),
            node::CalculNode {
                value: String::new(),
                node_type: node::NodeKind::NodeFor,
                left_node: Some(Box::new(in_token_node(&for_token, &mut 5))),
                right_node: None,
                block: Some(for_token[0].line),
            }
        );
        assert_eq!(for_token[index].kind, token::token::TokenKind::TokenLBrace);
    }
}
