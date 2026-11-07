//! 構造体/バリアントの定義を名前から引くための木(`StructTree`/`VariantTree`)
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct StructTree {
    tree: HashMap<String, StructDefine>,
}

impl StructTree {
    pub(in crate::compiler::ir) fn new() -> Self {
        Self {
            tree: HashMap::new(),
        }
    }

    pub(in crate::compiler::ir) fn add(&mut self, info: &StructDefine) {
        self.tree.insert(info.name.to_string(), info.clone());
    }

    pub(in crate::compiler::ir) fn get(&self, name: &str) -> Option<&StructDefine> {
        self.tree.get(name)
    }

    /// 任意の構造体を指定し、その構造体にあるメンバー
    /// のバイトの位置を取得し返す関数
    pub(in crate::compiler::ir) fn get_pos(
        &self,
        // 構造体の名前
        name: &str,
        field_name: &str,
    ) -> usize {
        let mut byte_counter = 0;
        if let Some(struct_def) = self.tree.get(name) {
            for ref member in struct_def.fields.iter() {
                let ty = types::Size::try_from_or_emit_struct(
                    &mut |name| self.get_struct_size(name),
                    &member.ty,
                )
                .unwrap();
                byte_counter += ty.to_bytes();
                if member.name == field_name {
                    return byte_counter;
                }
            }
        } else {
            return types::embe_ty_sort(name).unwrap().to_bytes();
        }

        panic!();
    }

    pub(in crate::compiler::ir) fn get_mem_size(
        &self,
        name: &str,
        field_name: &str,
    ) -> types::Size {
        let ty = self
            .tree
            .get(name)
            .expect(format!("struct name, {}", name).as_str())
            .fields
            .iter()
            .find(|v| &v.name == field_name)
            .unwrap()
            .ty
            .clone();
        types::Size::try_from_or_emit_struct(&mut |name| self.get_struct_size(name), &ty).unwrap()
    }

    pub(in crate::compiler::ir) fn get_struct_size(
        &self,
        name: &str,
    ) -> Result<StructDefine, err::ErrKind> {
        if let Some(target) = self.tree.get(name) {
            Ok(target.clone())
        } else {
            Err(crate::GenUndefErrResult!(
                UndefStruct,
                name.to_string(),
                None
            ))
        }
    }
}

/// バリアント型の定義を名前から引くための木
#[derive(Clone, Debug, PartialEq)]
pub struct VariantTree {
    tree: HashMap<String, VariantDefine>,
}

impl VariantTree {
    pub(in crate::compiler::ir) fn new() -> Self {
        Self {
            tree: HashMap::new(),
        }
    }

    pub(in crate::compiler::ir) fn add(&mut self, info: &VariantDefine) {
        self.tree.insert(info.name.to_string(), info.clone());
    }

    pub(in crate::compiler::ir) fn get(&self, name: &str) -> Option<&VariantDefine> {
        self.tree.get(name)
    }

    pub(in crate::compiler::ir) fn contains_key(&self, name: &str) -> bool {
        self.tree.contains_key(name)
    }

    /// 名前から、バリアント型の定義を探す。
    /// ジェネリクスなバリアント型(`Option`)は、型引数ごとに`Option$int`のような
    /// 名前で登録されているので、完全一致が無ければ`名前$`で始まるものを探す
    pub(in crate::compiler::ir) fn find_by_base_name(&self, name: &str) -> Option<&VariantDefine> {
        self.tree.get(name).or_else(|| {
            self.tree
                .iter()
                .filter(|(key, _)| {
                    key.strip_prefix(name)
                        .is_some_and(|suffix| suffix.starts_with('$'))
                })
                .map(|(_, def)| def)
                .next()
        })
    }

    // /// メンバーの定義順のインデックス(タグの値)を取得する
    // pub(in crate::compiler::ir)
    // fn get_tag(&self, name: &str, field_name: &str) -> Option<usize> {
    //     self.tree.get(name)?.tag_of(field_name)
    // }

    pub(in crate::compiler::ir) fn get_variant(
        &self,
        name: &str,
    ) -> Result<VariantDefine, err::ErrKind> {
        if let Some(target) = self.tree.get(name) {
            Ok(target.clone())
        } else {
            Err(crate::GenUndefErrResult!(
                UndefVariant,
                name.to_string(),
                None
            ))
        }
    }
}
