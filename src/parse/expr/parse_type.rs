//  変数や引数の型、関数の戻り値の型
//  を決めるトークンをノードに変換する
use crate::token::token;
use crate::parse::node;
use crate::parse::resp;

pub fn parse_type_node(tokens: Vec<token::Token>, index: &mut i32) -> node::CalculNode {
    let mut starts_with_less_than: bool = false;
    let mut ident_seen_name: bool = false;
    let mut node = resp::handler::make_null_node();
    node.node_type = node::NodeKind::NodeType;

    while tokens.len() > *index as usize {
        if starts_with_less_than {
            match tokens[*index as usize].kind {
                token::TokenKind::TokenGreaterThan => {
                    if !ident_seen_name {
                        eprintln!("banana [syntax err]: line {}", tokens[0].line);
                        eprintln!("expected type after variable name");
                        panic!("");
                    }
                    *index += 1;
                    break;
                },
                token::TokenKind::TokenLessThan => {
                    //  start_with_less_thanがtrueになって、
                    //  このブロックが実行されているため、もし <がまた来た場合構文エラー
                    eprintln!("banana [syntax err]: line {}", tokens[0].line);
                    eprintln!("unexpected symbol `<`");
                    panic!("");
                },
                token::TokenKind::TokenName => {
                    //  変数の名前が来た
                    ident_seen_name = true;
                    node.value = tokens[*index as usize].lexeme.clone();
                },
                _ => {
                    //
                },
            }
        } else {
            if tokens[*index as usize].kind == token::TokenKind::TokenLessThan {
                starts_with_less_than = true;
            }
        }

        *index += 1;
    }

    node
}
