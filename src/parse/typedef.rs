use super::{Parser, *};


/// 構造体、列挙型を定義するノードの生成
impl Parser {
    /// `struct name { mem: ty, mem2: ty2 }` を解析する
    /// 呼び出し時は current_tkn() が `KeyWordStruct` であることを想定する
    pub(super) fn struct_node(&mut self) -> Result<node::Group1Node, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn()? else {
            panic!("構造体の名前が必要です");
        };

        match self.next_tkn()? {
            lex::Tkn::LBrace => {},
            t => panic!("{:?}", t),
        }

        let fields = self.define_struct_fields()?;
        Ok(node::StructDefine::new(name, fields))
    }

    /// `enum name { mem, mem2 }` を解析する
    /// 呼び出し時は current_tkn() が `KeyWordEnum` であることを想定する
    pub(super) fn enum_node(&mut self) -> Result<node::Group1Node, err::ErrKind> {
        let lex::Tkn::Name(name) = self.next_tkn()? else {
            panic!("列挙型の名前が必要です");
        };

        match self.next_tkn()? {
            lex::Tkn::LBrace => {},
            t => panic!("{:?}", t),
        }

        let variants = self.define_enum_variants()?;
        Ok(node::EnumDefine::new(name, variants))
    }

    /// 構造体のメンバ定義を解析する
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace` を指す
    fn define_struct_fields(&mut self) -> Result<Vec<node::StructField>, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return Err(err::ErrKind::NotFoundTkn(lex::Tkn::LBrace));
        }

        let mut fields = Vec::<node::StructField>::new();

        loop {
            match self.next_tkn()? {
                lex::Tkn::Name(name) => {
                    // ":" へ移動
                    self.next_tkn()?;
                    // 型名の解析（型名の後ろまでトークンを進める）
                    let ty = self.define_ty_node()?;

                    fields.push(node::StructField {
                        name: name.clone(),
                        ty,
                    });

                    match self.current_tkn() {
                        lex::Tkn::RBrace => break,
                        lex::Tkn::Comma => {},
                        t => panic!("{:?}", t),
                    }
                }
                lex::Tkn::RBrace => break,
                t => panic!("{:?}", t),
            }
        }

        Ok(fields)
    }

    /// 列挙型のメンバ定義を解析する
    /// 呼び出し時、終了時ともに current_tkn() は `LBrace` / `RBrace` を指す
    fn define_enum_variants(&mut self) -> Result<Vec<String>, err::ErrKind> {
        if self.current_tkn() != &lex::Tkn::LBrace {
            return Err(err::ErrKind::NotFoundTkn(lex::Tkn::LBrace));
        }

        let mut variants = Vec::<String>::new();

        loop {
            match self.next_tkn()? {
                lex::Tkn::Name(name) => {
                    variants.push(name);

                    match self.next_tkn()? {
                        lex::Tkn::RBrace => break,
                        lex::Tkn::Comma => {},
                        t => panic!("{:?}", t),
                    }
                }
                lex::Tkn::RBrace => break,
                t => panic!("{:?}", t),
            }
        }

        Ok(variants)
    }
}
