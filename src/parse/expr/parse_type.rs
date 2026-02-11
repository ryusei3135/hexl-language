use super::*;

/// # 型のノード
/// - valueに型の名前
/// - node_type は node::NodeKind::NodeType
/// - left && right はNone,
/// - block はSome

pub fn parse_type_node(
        tokens: &Vec<token::Token>,
        index: &mut usize
) -> Result<node::CalculNode, parse_err::ParseErrs> {
    let mut starts_with_less_than: bool = false;
    let mut ident_seen_name: bool = false;
    let mut node = resp::handler::make_null_node();

    while tokens.len() > *index {
        if starts_with_less_than {
            match tokens[*index].kind {
                token::TokenKind::TokenGreaterThan => {
                    if !ident_seen_name {
                        eprintln!("banana [syntax err]: line {}", tokens[0].line);
                        eprintln!("expected type after variable name");
                        return Err(parse_err::ParseErrs::TypeSpecUnspecified);
                    }
                    *index += 1;
                    break;
                },
                token::TokenKind::TokenLessThan => {
                    //  start_with_less_thanがtrueになって、
                    //  このブロックが実行されているため、もし <がまた来た場合構文エラー
                    eprintln!("banana [syntax err]: line {}", tokens[0].line);
                    eprintln!("unexpected symbol `<`");
                    return Err(parse_err::ParseErrs::TypeSpecAngleBracketForbid);
                },
                token::TokenKind::TokenName => {
                    //  変数の名前が来た
                    ident_seen_name = true;
                    node.value = tokens[*index].lexeme.clone();
                    node.block = Some(tokens[0].line.clone());
                },
                _ => {
                    //
                },
            }
        } else {
            if tokens[*index].kind == token::TokenKind::TokenLessThan {
                node.node_type = node::NodeKind::NodeType;
                starts_with_less_than = true;
            }
        }

        *index += 1;
    }

    Ok(node)
}

#[cfg(test)]
mod tests {
    use crate::token::tokenizer;
    use crate::token::token::Token;
    use super::*;

    fn make_token(txt: String) -> Vec<Token> {
        let mut lexer = tokenizer::Tokenizer::new();
        lexer.make_token(txt, 0)
    }

    /// 正常な型のトークンが正しいか
    #[test]
    fn check_use_type_node() {
        let token_data = make_token("<i32>name".to_string());

        assert_eq!(
            parse_type_node(&token_data, &mut 0).unwrap(),
            node::CalculNode {
                value: "i32".to_string(),
                node_type: node::NodeKind::NodeType,
                left_node: None,
                right_node: None,
                block: Some(0),
            }
        );
    }

    /// 型の<..>の..に何も入れないとエラーになるかを調べる
    #[test]
    fn check_err_type_node() {
        let token_data = make_token("<>name".to_string());

        assert_eq!(
            parse_type_node(&token_data, &mut 0),
            Err(parse_err::ParseErrs::TypeSpecUnspecified),
        );
    }
}
