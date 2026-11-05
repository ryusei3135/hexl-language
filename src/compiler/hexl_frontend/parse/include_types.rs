//! `#include`先の公開型ノードを呼び出し側のASTへ取り込む。

use super::*;
use crate::compiler::hexl_frontend::include_list::IncludeStage;
use crate::compiler::hexl_frontend::include_root;
use std::rc::Rc;

impl Parser {
    fn type_reference(node: &Group1Node) -> Group1Node {
        match node {
            Group1Node::StructDefine(def) => {
                let mut def = def.clone();
                Self::make_method_declarations(&mut def.methods);
                Group1Node::StructDefine(def)
            }
            Group1Node::VariantDefine(def) => {
                let mut def = def.clone();
                Self::make_method_declarations(&mut def.methods);
                Group1Node::VariantDefine(def)
            }
            Group1Node::TypeAlias(def) => Group1Node::TypeAlias(def.clone()),
            _ => unreachable!("only type definitions are imported"),
        }
    }

    fn make_method_declarations(methods: &mut [Group1Node]) {
        for method in methods {
            if let Group1Node::FuncDefine(def) = method {
                *def = def.declaration_only();
            }
        }
    }

    pub(super) fn collect_included_types(&mut self) -> Result<(), err::ErrKind> {
        let Some(tkns) = self.tkns.as_ref() else {
            return crate::err_at!(self.tkn_is_eof(Vec::new()));
        };
        let mut heads = Vec::<usize>::new();
        let mut depth = 0usize;
        for (i, t) in tkns.iter().enumerate() {
            match &t.tkn {
                lex::Tkn::LBrace => depth += 1,
                lex::Tkn::RBrace => depth = depth.saturating_sub(1),
                lex::Tkn::CompleSyn
                    if depth == 0
                        && matches!(
                            tkns.get(i + 1).map(|t| &t.tkn),
                            Some(lex::Tkn::Name(n)) if n == "include"
                        ) =>
                {
                    heads.push(i)
                }
                _ => {}
            }
        }

        let saved_idx = self.idx;
        let mut paths = Vec::<ModPath>::new();
        for head in heads {
            self.idx = head;
            match self.comple_syntax() {
                Ok(Group2Node::Include(path)) => paths.push(path),
                Ok(_) => {}
                Err(e) => {
                    self.idx = saved_idx;
                    return Err(e);
                }
            }
        }
        self.idx = saved_idx;

        for path in paths {
            self.collect_types_of_include(&path)?;
        }
        Ok(())
    }

    fn collect_types_of_include(&mut self, path: &ModPath) -> Result<(), err::ErrKind> {
        let full_path = include_root::resolve_for(path, &path.gen_path());
        let file = match &path.kind {
            ImportKind::Auto if std::path::Path::new(&full_path).exists() => full_path,
            ImportKind::Auto | ImportKind::Func(_) => return Ok(()),
            ImportKind::Module(_) | ImportKind::Glob => full_path,
        };

        if file.ends_with(".c") || file.ends_with(".h") || !std::path::Path::new(&file).exists() {
            return Ok(());
        }
        if !self.include_list.insert(IncludeStage::Type, file.clone()) {
            return Ok(());
        }

        let Ok(content) = std::fs::read_to_string(&file) else {
            return Ok(());
        };
        let file_name = Rc::new(file.clone());
        let mut lexer = lex::Lexer::new(&file_name);
        if lexer.analy(&content).is_err() {
            return Ok(());
        }

        let mut parser = Parser::new(&file_name, Rc::clone(&self.err_pool));
        // 子へは`Type`の段だけを渡す(親の`Include`の記録で、子の`#include`が消えないように)
        parser.include_list = self.include_list.only(IncludeStage::Type);
        let nodes = parser.parser(&lexer.gen_tkns, &[])?.to_vec();
        self.include_list
            .merge(&parser.include_list, IncludeStage::Type);

        self.included_type_nodes.extend(
            nodes
                .iter()
                .filter(|node| {
                    node.is_public()
                        && matches!(
                            node,
                            Group1Node::StructDefine(_)
                                | Group1Node::VariantDefine(_)
                                | Group1Node::TypeAlias(_)
                        )
                })
                .map(Self::type_reference),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn parse_source(source: &str) -> Vec<Group1Node> {
        let dir = std::env::temp_dir().join(format!(
            "hexl_include_types_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("nested.hexl"),
            "#include types=\"types.hexl\"\npub struct Nested { value: int }\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("types.hexl"),
            "#include nested=\"nested.hexl\"\n\
             pub struct Item { value: int pub get(self: Self): int { ret 0 } }\n\
             pub variant State { Ready }\n\
             pub type Count = int\n\
             struct Private { value: int }\n\
             pub get(): int { ret 0 }\n",
        )
        .unwrap();
        let main_path = dir.join("main.hexl");
        let file_name = Rc::new(main_path.to_string_lossy().into_owned());
        include_root::set_root_from_file(&main_path);
        let mut lexer = lex::Lexer::new(&file_name);
        lexer.set_source_file(Rc::clone(&file_name));
        lexer.analy(&source).unwrap();
        let mut parser = Parser::new(&file_name, Rc::new(RefCell::new(err_pool::ErrPool::new())));
        let nodes = parser.parser(&lexer.gen_tkns, &[]).unwrap().to_vec();
        std::fs::remove_dir_all(dir).unwrap();
        nodes
    }

    fn parse_with_include(include: &str) -> Vec<Group1Node> {
        parse_source(&format!("{include}\n"))
    }

    fn imported_type_names(nodes: &[Group1Node]) -> Vec<String> {
        nodes
            .iter()
            .filter_map(|node| match node {
                Group1Node::StructDefine(def) => Some(def.name.clone()),
                Group1Node::VariantDefine(def) => Some(def.name.clone()),
                Group1Node::TypeAlias(def) => Some(def.name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn module_include_imports_only_public_type_nodes() {
        let nodes = parse_with_include("#include types=\"types.hexl\"");
        let names = imported_type_names(&nodes);
        assert!(names.contains(&"Item".to_string()));
        assert!(names.contains(&"Nested".to_string()));
        assert!(names.contains(&"State".to_string()));
        assert!(names.contains(&"Count".to_string()));
        assert!(!names.contains(&"Private".to_string()));

        let item = nodes.iter().find_map(|node| match node {
            Group1Node::StructDefine(def) if def.name == "Item" => Some(def),
            _ => None,
        });
        let method = item.unwrap().methods.iter().find_map(|node| match node {
            Group1Node::FuncDefine(def) if def.name == "get" => Some(def),
            _ => None,
        });
        let method = method.unwrap();
        assert!(method.is_declaration_only());
        assert!(method.body.is_empty());
    }

    #[test]
    fn local_struct_method_body_is_registered_as_a_definition() {
        let nodes = parse_source("pub struct Item { pub get(self: Self): int { ret 0 } }\n");
        let method = nodes.iter().find_map(|node| match node {
            Group1Node::StructDefine(def) if def.name == "Item" => {
                def.methods.iter().find_map(|method| match method {
                    Group1Node::FuncDefine(def) if def.name == "get" => Some(def),
                    _ => None,
                })
            }
            _ => None,
        });

        let method = method.expect("the local method should be parsed");
        assert!(!method.is_declaration_only());
        assert!(!method.body.is_empty());
    }

    #[test]
    fn glob_include_imports_types_but_function_include_does_not() {
        let glob = parse_with_include("#include \"types.hexl\"::*");
        assert!(imported_type_names(&glob).contains(&"Item".to_string()));

        let function = parse_with_include("#include \"types.hexl\"::get");
        assert!(imported_type_names(&function).is_empty());
    }

    #[test]
    fn local_type_definition_takes_precedence_over_imported_type() {
        let nodes = parse_source("#include types=\"types.hexl\"\nstruct Item { local: int }\n");
        let local = nodes.iter().find_map(|node| match node {
            Group1Node::StructDefine(def) if def.name == "Item" => Some(def),
            _ => None,
        });
        assert_eq!(local.unwrap().fields[0].name, "local");
    }
}
