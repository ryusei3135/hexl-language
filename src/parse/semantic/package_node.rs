use crate::token::token;
use crate::parse::node;
use crate::parse::resp;



struct PackageState {
    allow_name: bool,
    first_dir: bool,
    package_node: node::CalculNode,
    package_dir: String,
    add_dir: String,
}

impl PackageState {
    pub fn new() -> Self {
        Self {
            allow_name: true,
            first_dir: false,
            package_node: resp::handler::make_null_node(),
            package_dir: "[*null*]".to_string(),
            add_dir: "[*null*]".to_string(),
        }
    }

    pub fn build_node(&self) -> node::CalculNode {
        resp::handler::make_operator_node(
            self.package_node.clone(),
            node::CalculNode {
                value: self.package_dir.clone(),
                node_type: node::NodeKind::NodeNull,
                left_node: None,
                right_node: None,
                block: None,
            },
            node::NodeKind::NodeUsePackage,
            &0
        )
    }
}

fn make_lib_name(state: &mut PackageState, token: token::Token) {
    if state.allow_name {
        state.allow_name = false;
        if !state.first_dir {
            match token.lexeme.as_str() {
                "std" => {
                    //  標準ライブラリのディレクトリは、lib/std/にある
                    state.package_node.value = "./extern_lib/std/".to_string();
                    state.package_node.node_type = node::NodeKind::NodeStd;
                    state.package_dir = "./extern_lib/std/".to_string();
                }
                "src" => {
                    state.package_node.value = "./".to_string();
                    state.package_node.node_type = node::NodeKind::NodeSrc;
                    state.package_dir = "./".to_string();
                }
                _ => panic!("head err use"),
            }
            state.first_dir = true;
        } else {
            if state.add_dir == "[*null*]".to_string() {
                state.add_dir = token.lexeme.clone();
            } else {
                state.package_dir.push_str(&(state.add_dir.clone() + "/"));
                state.add_dir = token.lexeme.clone();
            }
            state.package_node.value.push_str(&token.lexeme);
        }
    } else {
        panic!("[syntax err]: line -> {} use ::", token.line);
    }
}

pub fn make_use_package_node(tokens: Vec<token::Token>, index: &mut usize) -> node::CalculNode {
    if tokens[*index].kind != token::TokenKind::TokenUsePackage {
        eprintln!("[system err]: [file]: parse/semantic/package.rs");
        eprintln!("[func]: make_use_package_node");
        panic!("");
    }
    if tokens.len() > (*index) + 1 {
        *index += 1;
    } else {
        println!("[use err]: -> `{}` Incomplete Import Path", tokens[*index].line);
        panic!("");
    }

    let mut state = PackageState::new();

    while tokens.len() > *index {
        match tokens[*index].kind {
            token::TokenKind::TokenName => make_lib_name(&mut state, tokens[*index].clone()),
            token::TokenKind::TokenScope => {
                if state.allow_name {
                    println!("[syntax err]: line -> {}", tokens[*index].line);
                    panic!("");
                } else {
                    state.allow_name = true;
                }
            }
            token::TokenKind::TokenSpace => {}
            _ => {}
        }
        *index += 1;
    }

    state.build_node()
}
