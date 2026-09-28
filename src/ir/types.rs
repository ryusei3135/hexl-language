use super::*;
use crate::err::undef::UndefKind;
use crate::parse::node::TyNode;
use std::convert::TryFrom;

#[derive(Clone, Debug, PartialEq)]
pub enum Size {
    DB,
    DW,
    DD,
    DQ,
    Struct(Vec<Box<(String, Size)>>),
    Pointer {
        ty: Box<Size>,
        is_const: bool,
        range: Option<(usize, usize)>,
    },
    Array {
        size: Box<Size>,
        len: usize,
    },
    Void,
}

impl TryFrom<&TyNode> for Size {
    type Error = err::ErrKind;
    /// 組み込みの型(byte/u16/int/u64)のみを解決する
    /// 構造体や列挙型などのユーザー定義の型を解決する場合は
    /// `builder::IR::size_of` を使用する
    fn try_from(ty: &TyNode) -> Result<Self, Self::Error> {
        let size_ty = match ty {
            node::TyNode::Ty(ty_name) => embe_ty_sort(ty_name)?,
            // スタック/静的領域の型は、要素の型と同じサイズを持つ
            node::TyNode::Stack { name, .. } | node::TyNode::Static { name, .. } => {
                node::TyNode::Ty(name.to_owned()).try_into()?
            }
            node::TyNode::Pointer {
                is_const,
                ty_name,
                range,
            } => Self::Pointer {
                ty: Box::new((*ty_name.to_owned()).try_into()?),
                is_const: is_const.clone(),
                range: range.clone(),
            },
            node::TyNode::SelfTy(..) => Self::DQ,
            // 契約(`must`/`of`)はコンパイル時にだけ意味を持つ情報なので、
            // サイズとしては内側の型と全く同じものとして扱う
            node::TyNode::ConstractMust(ty) | node::TyNode::ConstractOf(ty) => {
                ty.unwrap_ty().try_into()?
            }
            t => panic!("{:?}", t),
        };
        Ok(size_ty)
    }
}

impl TryFrom<TyNode> for Size {
    type Error = err::ErrKind;
    /// 組み込みの型(byte/u16/int/u64)のみを解決する
    /// 構造体や列挙型などのユーザー定義の型を解決する場合は
    /// `builder::IR::size_of` を使用する
    fn try_from(ty: TyNode) -> Result<Self, Self::Error> {
        let size_ty = match ty {
            node::TyNode::Ty(ty_name) => embe_ty_sort(&ty_name)?,
            // スタック/静的領域の型は、要素の型と同じサイズを持つ
            node::TyNode::Stack { name, .. } | node::TyNode::Static { name, .. } => {
                node::TyNode::Ty(name.to_owned()).try_into()?
            }
            node::TyNode::Pointer {
                is_const,
                ty_name,
                range,
            } => Self::Pointer {
                ty: Box::new((*ty_name).try_into()?),
                is_const: is_const,
                range: range,
            },
            node::TyNode::SelfTy(..) => Self::DQ,
            // 契約(`must`/`of`)はコンパイル時にだけ意味を持つ情報なので、
            // サイズとしては内側の型と全く同じものとして扱う
            node::TyNode::ConstractMust(ty) | node::TyNode::ConstractOf(ty) => {
                ty.unwrap_ty().try_into()?
            }
            t => panic!("{:?}", t),
        };
        Ok(size_ty)
    }
}

impl Size {
    /// 構造体の型を作成する
    pub fn emit_struct_ty_node(
        f: &mut impl FnMut(&str) -> Result<node::StructDefine, err::ErrKind>,
        ty: &node::TyNode,
    ) -> Result<Self, err::ErrKind> {
        let binding = ty.get_ty_str_name();
        let target = f(&binding)?;
        let mut struct_ty = Vec::new();

        for field in target.fields.iter() {
            let ty = Box::new((field.name.clone(), (&field.ty).try_into().unwrap()));
            struct_ty.push(ty);
        }

        Ok(Self::Struct(struct_ty))
    }

    /// `TyNode`を`Size`へ変換し、変換に失敗(`Err`)した場合は
    /// `emit_struct_ty_node`で構造体の型として解決し直す
    ///
    /// `try_into`は組み込み型しか解決できないため、構造体などの
    /// ユーザー定義の型では`Err`が返ってくる。その場合に`f`
    /// (構造体の定義を名前から取得する関数)を使って構造体の型を作成する
    pub fn try_from_or_emit_struct(
        f: &mut impl FnMut(&str) -> Result<node::StructDefine, err::ErrKind>,
        ty: &node::TyNode,
    ) -> Result<Self, err::ErrKind> {
        match Self::try_from(ty) {
            Ok(size) => Ok(size),
            Err(_) => Self::emit_struct_ty_node(f, ty),
        }
    }

    pub fn is_pointer(&self) -> Option<Size> {
        if let Self::Pointer { ty, .. } = self {
            Some(*ty.clone())
        } else {
            None
        }
    }

    /// ポインタ型を作成する
    pub fn build_ptr_ty(ty: &node::TyNode, range: Option<(usize, usize)>) -> Self {
        Self::Pointer {
            ty: Box::new(ty.try_into().unwrap()),
            is_const: false,
            range,
        }
    }

    /// 組み込み型かどうかを判定する
    pub fn is_builtin_ty_name(name: &str) -> bool {
        matches!(name, "byte" | "i16" | "int" | "i64")
    }

    /// このサイズがバイト単位で何バイトかを返す
    pub fn to_bytes(&self) -> usize {
        match self {
            Self::DB => 1,
            Self::DW => 2,
            Self::DD => 4,
            Self::DQ => 8,
            Self::Array { size, len } => size.to_bytes() * len,
            Self::Pointer { ty, .. } => (*ty).to_bytes(),
            Self::Struct(struct_size) => {
                let mut size_counter = 0;
                for mem in struct_size.iter() {
                    size_counter += mem.1.to_bytes();
                }
                size_counter
            }
            Self::Void => panic!(),
        }
    }

    #[inline(always)]
    pub fn wrap_dst_size(&self) -> crate::asm_gen::SelfPtrInfo {
        Some(self.clone())
    }

    #[inline(always)]
    pub fn wrap_ok<E>(self) -> Result<Self, E> {
        Ok(self)
    }
}

#[inline(always)]
fn embe_ty_sort(ty_name: &str) -> Result<Size, err::ErrKind> {
    match ty_name {
        "byte" => Size::DB,
        "i16" => Size::DW,
        "int" => Size::DD,
        "i64" => Size::DQ,
        _ => {
            return Err(crate::GenUndefErrResult!(
                UndefVarTy,
                ty_name.to_string(),
                None
            ));
        }
    }
    .wrap_ok::<err::ErrKind>()
}

impl IR {
    /// `TyNode`を`Size`へ変換する
    ///
    /// `try_into`が`Err`を返した場合(構造体などのユーザー定義の型)は、
    /// `struct_tree`を参照して`Size::emit_struct_ty_node`で構造体の型を作成する
    pub(super) fn try_size_or_emit_struct(
        &self,
        ty: &node::TyNode,
    ) -> Result<types::Size, err::ErrKind> {
        types::Size::try_from_or_emit_struct(&mut |name| self.struct_tree.get_struct_size(name), ty)
    }

    /// 型からサイズを求める
    ///
    /// 組み込み型(`byte`/`u16`/`int`/`u64`)は`types::Size::new`と
    /// 同じ結果を返すが、構造体・列挙型などのユーザー定義の型名が渡された
    /// 場合は`struct_tree`/`enum_tree`を参照して解決する
    pub(super) fn size_of(&self, ty: &node::TyNode) -> types::Size {
        match ty {
            node::TyNode::Ty(name) => {
                if types::Size::is_builtin_ty_name(name) {
                    return ty.try_into().unwrap();
                }

                if self.enum_tree.contains_key(name) {
                    // 列挙型は現在、タグ(整数値)として扱う
                    return types::Size::DD;
                }

                if let Some(struct_def) = self.struct_tree.get(&name) {
                    let fields = struct_def
                        .fields
                        .iter()
                        .map(|field| Box::new((field.name.clone(), self.size_of(&field.ty))))
                        .collect();
                    return types::Size::Struct(fields);
                }

                panic!("未定義の型です: {}", name);
            }
            node::TyNode::Pointer {
                is_const,
                ty_name,
                range,
            } => types::Size::Pointer {
                ty: Box::new(self.size_of(ty_name)),
                is_const: is_const.clone(),
                range: range.clone(),
            },
            // スタック/静的領域の型は、要素の型と同じサイズを持つ
            node::TyNode::Stack { name, len } | node::TyNode::Static { name, len } => {
                let size = self.size_of(&node::TyNode::Ty(name.clone()));
                // 配列の作成
                if len >= &1 {
                    types::Size::Array {
                        size: Box::new(size),
                        len: *len,
                    }
                } else {
                    size
                }
            }
            node::TyNode::RefTy(inner) => self.size_of(inner),
            // `Self`はIRへ変換する前に、実際の構造体の型
            // (`node::TyNode::Ty`)やポインタ型へ解決されている必要がある
            node::TyNode::SelfTy(name) => self.size_of(&node::TyNode::Ty(name.to_string())),
            node::TyNode::ConstractMust(ty) | node::TyNode::ConstractOf(ty) => {
                self.size_of(&ty.unwrap_ty())
            }
        }
    }
}
