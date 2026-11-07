use super::*;
use crate::compiler::ast::{StructDefine, TyNode, VariantDefine};
use std::convert::TryFrom;

#[derive(Clone, Debug, PartialEq)]
pub enum Size {
    DB,
    DW,
    DD,
    DQ,
    UDB,
    UDW,
    UDD,
    UDQ,
    F32,
    F64,
    Bool,
    Struct(Vec<Box<(String, Size)>>),
    /// バリアント型
    /// - `fields`: メンバー名と型(型なしメンバーは`Void`)。定義順がタグの値
    /// - `tagged`: 先頭にタグ(`VARIANT_TAG_BYTES`バイト)を持つか
    Variant {
        name: String,
        fields: Vec<Box<(String, Size)>>,
        tagged: bool,
    },
    Pointer {
        ty: Box<Size>,
        is_const: bool,
        range: Option<(usize, usize)>,
    },
    Array {
        size: Box<Size>,
        len: usize,
    },
    GetAddr(Box<Size>),
    Void,
}

impl TryFrom<&TyNode> for Size {
    type Error = err::ErrKind;
    /// 組み込みの型(byte/u16/int/u64)のみを解決する
    /// 構造体や列挙型などのユーザー定義の型を解決する場合は
    /// `builder::IR::size_of` を使用する
    fn try_from(ty: &TyNode) -> Result<Self, Self::Error> {
        let size_ty = match ty {
            TyNode::Ty(ty_name) => embe_ty_sort(ty_name)?,
            // スタック/静的領域の型は、要素の型と同じサイズを持つ
            TyNode::Stack { name, len } | TyNode::Static { name, len } => {
                if *len == 1 {
                    name.as_ref().try_into()?
                } else {
                    Self::Array {
                        size: Box::new(name.as_ref().try_into()?),
                        len: *len,
                    }
                }
            }
            TyNode::Pointer {
                is_const,
                ty_name,
                range,
            } => Self::Pointer {
                ty: Box::new((*ty_name.to_owned()).try_into()?),
                is_const: is_const.clone(),
                range: range.clone(),
            },
            TyNode::SelfTy(..) => Self::DQ,
            // 契約(`must`/`of`)はコンパイル時にだけ意味を持つ情報なので、
            // サイズとしては内側の型と全く同じものとして扱う
            TyNode::ConstractMust(ty) | TyNode::ConstractOf(ty) => ty.unwrap_ty().try_into()?,
            // 契約を借りる型(`RefContract`)も、サイズは内側の型と同じ
            TyNode::RefContract(inner) => inner.as_ref().try_into()?,
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
            TyNode::Ty(ty_name) => embe_ty_sort(&ty_name)?,
            // スタック/静的領域の型は、要素の型と同じサイズを持つ
            TyNode::Stack { name, len } | TyNode::Static { name, len } => {
                if len == 1 {
                    (*name).try_into()?
                } else {
                    Self::Array {
                        size: Box::new((*name).try_into()?),
                        len,
                    }
                }
            }
            TyNode::Pointer {
                is_const,
                ty_name,
                range,
            } => Self::Pointer {
                ty: Box::new((*ty_name).try_into()?),
                is_const: is_const,
                range: range,
            },
            TyNode::SelfTy(..) => Self::DQ,
            // 契約(`must`/`of`)はコンパイル時にだけ意味を持つ情報なので、
            // サイズとしては内側の型と全く同じものとして扱う
            TyNode::ConstractMust(ty) | TyNode::ConstractOf(ty) => ty.unwrap_ty().try_into()?,
            // 契約を借りる型(`RefContract`)も、サイズは内側の型と同じ
            TyNode::RefContract(inner) => (*inner).try_into()?,
            t => panic!("{:?}", t),
        };
        Ok(size_ty)
    }
}

/// バリアント型のタグのバイト数(型なしメンバーだけの場合の`DD`と同じ)
pub const VARIANT_TAG_BYTES: usize = 4;

impl Size {
    /// バリアント型のペイロード(タグを除いた領域)のバイト数。
    /// 型なしメンバー(`Void`)は0バイトとして扱う
    pub(crate) fn payload_bytes(&self) -> usize {
        match self {
            Self::Void => 0,
            t => t.to_bytes(),
        }
    }

    /// バリアント型のタグを除いた、メンバーの値を置く位置のオフセット
    pub(crate) fn variant_payload_offset(&self) -> usize {
        match self {
            Self::Variant { tagged: true, .. } => VARIANT_TAG_BYTES,
            Self::Variant { tagged: false, .. } => 0,
            t => panic!(
                "variant_payload_offset: バリアント型ではありません: {:?}",
                t
            ),
        }
    }

    /// 構造体の型を作成する
    pub(in crate::compiler::ir) fn emit_struct_ty_node(
        f: &mut impl FnMut(&str) -> Result<StructDefine, err::ErrKind>,
        ty: &TyNode,
    ) -> Result<Self, err::ErrKind> {
        let binding = ty.get_ty_str_name();
        let target = f(&binding)?;
        let mut struct_ty = Vec::new();

        for field in target.fields.iter() {
            let size = Self::try_from_or_emit_struct(f, &field.ty)
                .map_err(|error| error.with_span(std::rc::Rc::clone(&field.span)))?;
            let ty = Box::new((field.name.clone(), size));
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
    pub(super) fn try_from_or_emit_struct(
        f: &mut impl FnMut(&str) -> Result<StructDefine, err::ErrKind>,
        ty: &TyNode,
    ) -> Result<Self, err::ErrKind> {
        match Self::try_from(ty) {
            Ok(size) => Ok(size),
            Err(_) => Self::emit_struct_ty_node(f, ty),
        }
    }

    pub(in crate::compiler) fn is_pointer(&self) -> Option<Size> {
        if let Self::Pointer { ty, .. } = self {
            Some(*ty.clone())
        } else {
            None
        }
    }

    /// ポインタ型を作成する
    pub(in crate::compiler::ir) fn build_ptr_ty(size: Self, range: Option<(usize, usize)>) -> Self {
        Self::Pointer {
            ty: Box::new(size),
            is_const: false,
            range,
        }
    }

    /// 組み込み型かどうかを判定する
    pub(in crate::compiler::ir) fn is_builtin_ty_name(name: &str) -> bool {
        matches!(
            name,
            "byte"
                | "char"
                | "i16"
                | "int"
                | "i64"
                | "ubyte"
                | "u16"
                | "uint"
                | "u64"
                | "float"
                | "f64"
                | "bool"
        )
    }

    pub(crate) fn is_float(&self) -> bool {
        match self {
            Self::F32 | Self::F64 => true,
            _ => false,
        }
    }

    pub(crate) fn is_unsigned(&self) -> bool {
        match self {
            Self::UDD | Self::UDB | Self::UDW | Self::UDQ => true,
            _ => false,
        }
    }

    /// このサイズがバイト単位で何バイトかを返す
    pub(crate) fn to_bytes(&self) -> usize {
        match self {
            Self::DB | Self::UDB | Self::Bool => 1,
            Self::DW | Self::UDW => 2,
            Self::DD | Self::UDD | Self::F32 => 4,
            Self::DQ | Self::UDQ | Self::F64 => 8,
            Self::Array { size, len } => size.to_bytes() * len,
            // ポインタ自身は指す先に関わらず8バイト(指す先のサイズは`is_pointer()`の`ty`で取る)。
            // 以前は指す先のサイズを返していたため、構造体のポインタ型メンバーが4バイトと
            // 数えられ、次のメンバーと領域が重なっていた
            Self::Pointer { .. } => 8,
            Self::Struct(struct_size) => {
                let mut size_counter = 0;
                for mem in struct_size.iter() {
                    size_counter += mem.1.to_bytes();
                }
                size_counter
            }
            Self::Variant { fields, tagged, .. } => {
                let payload = fields
                    .iter()
                    .map(|mem| mem.1.payload_bytes())
                    .max()
                    .unwrap_or(0);
                if *tagged {
                    VARIANT_TAG_BYTES + payload
                } else {
                    payload
                }
            }
            Self::GetAddr(..) => 8,
            Self::Void => 0,
        }
    }

    #[inline(always)]
    pub(crate) fn wrap_dst_size(&self) -> Option<Size> {
        Some(self.clone())
    }

    #[inline(always)]
    pub(in crate::compiler::ir) fn wrap_ok<E>(self) -> Result<Self, E> {
        Ok(self)
    }

    /// 型をポインタを取得する型でwrap
    pub(in crate::compiler::ir) fn wrap_get_addr(&self) -> Self {
        Self::GetAddr(Box::new(self.clone()))
    }

    pub(in crate::compiler) fn extract_variant_largest_size(&self) -> &Size {
        if let Self::Variant { fields, .. } = self {
            let idx = fields
                .iter()
                .enumerate()
                .max_by_key(|(_, val)| (*val).1.to_bytes());
            &(*fields[idx.unwrap().0]).1
        } else {
            panic!();
        }
    }
}

#[inline(always)]
pub(in crate::compiler::ir) fn embe_ty_sort(ty_name: &str) -> Result<Size, err::ErrKind> {
    match ty_name {
        "byte" | "char" => Size::DB,
        "i16" => Size::DW,
        "int" => Size::DD,
        "i64" => Size::DQ,
        "ubyte" => Size::UDB,
        "u16" => Size::UDW,
        "uint" => Size::UDD,
        "u64" => Size::UDQ,
        "float" => Size::F32,
        "f64" => Size::F64,
        "bool" => Size::Bool,
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

/// `IR`が持つ型情報を使った、型のサイズ計算(`impl IR`)
mod ir_size;
