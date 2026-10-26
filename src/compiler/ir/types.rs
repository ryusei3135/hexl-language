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
            Self::Pointer { ty, .. } => (*ty).to_bytes(),
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
fn embe_ty_sort(ty_name: &str) -> Result<Size, err::ErrKind> {
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

impl IR {
    /// `TyNode`を`Size`へ変換する
    ///
    /// `try_into`が`Err`を返した場合(構造体などのユーザー定義の型)は、
    /// `struct_tree`を参照して`Size::emit_struct_ty_node`で構造体の型を作成する
    pub(in crate::compiler::ir) fn try_size_or_emit_struct(
        &self,
        ty: &TyNode,
    ) -> Result<types::Size, err::ErrKind> {
        // バリアント型は`struct_tree`では解決できないので先に調べる
        if let TyNode::Ty(name) = ty {
            if self.variant_tree.contains_key(name) {
                return Ok(self.size_of(ty));
            }
        }
        types::Size::try_from_or_emit_struct(&mut |name| self.struct_tree.get_struct_size(name), ty)
    }

    /// バリアント型の定義から`types::Size::Variant`を作る
    pub(in crate::compiler::ir) fn size_of_variant(
        &self,
        variant_def: &VariantDefine,
    ) -> types::Size {
        let fields = variant_def
            .fields
            .iter()
            .map(|field| {
                let size = match &field.ty {
                    Some(ty) => self.size_of(ty),
                    // 型なしメンバー(`A::Name`)は値を持たない
                    None => types::Size::Void,
                };
                Box::new((field.name.clone(), size))
            })
            .collect();
        types::Size::Variant {
            name: variant_def.name.clone(),
            fields,
            tagged: variant_def.is_tagged(),
        }
    }

    /// 型からサイズを求める
    ///
    /// 組み込み型(`byte`/`u16`/`int`/`u64`)は`types::Size::new`と
    /// 同じ結果を返すが、構造体・バリアント型などのユーザー定義の型名が渡された
    /// 場合は`struct_tree`/`variant_tree`を参照して解決する
    pub(in crate::compiler::ir) fn size_of(&self, ty: &TyNode) -> types::Size {
        match ty {
            TyNode::Auto => panic!(),
            TyNode::Ty(name) => {
                if types::Size::is_builtin_ty_name(name) {
                    return ty.try_into().unwrap();
                }

                if let Some(struct_def) = self.struct_tree.get(&name) {
                    let fields = struct_def
                        .fields
                        .iter()
                        .map(|field| Box::new((field.name.clone(), self.size_of(&field.ty))))
                        .collect();
                    return types::Size::Struct(fields);
                }

                if let Some(variant_def) = self.variant_tree.get(name) {
                    // 全メンバーが型なし(旧列挙型相当)の場合は、
                    // タグ(整数値)だけで表せるので`DD`として扱う
                    if variant_def.is_plain_enum() {
                        return types::Size::DD;
                    }
                    return self.size_of_variant(&variant_def.clone());
                }

                panic!(
                    "未定義の型です: {} {:?} {:?}",
                    name, self.variant_tree, self.struct_tree
                );
            }
            TyNode::Pointer {
                is_const,
                ty_name,
                range,
            } => types::Size::Pointer {
                ty: Box::new(self.size_of(ty_name)),
                is_const: is_const.clone(),
                range: range.clone(),
            },
            // スタック/静的領域の型は、要素の型と同じサイズを持つ
            TyNode::Stack { name, len } | TyNode::Static { name, len } => {
                let size = self.size_of(&name);
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
            TyNode::RefTy(inner) => self.size_of(inner),
            // `Self`はIRへ変換する前に、実際の構造体の型
            // (`TyNode::Ty`)やポインタ型へ解決されている必要がある
            TyNode::SelfTy(name) => self.size_of(&TyNode::Ty(name.to_string())),
            TyNode::ConstractMust(ty) | TyNode::ConstractOf(ty) => self.size_of(&ty.unwrap_ty()),
        }
    }

    /// `TyNode`のバイトサイズを`usize`で返す
    ///
    /// - 組み込み型は`Size::try_from`で`Size`へ変換して`to_bytes`で求める
    /// - 組み込み型でなければ、`struct_tree`/`variant_tree`の定義から求める
    ///   - 構造体: 全フィールドのサイズの合計
    ///   - バリアント: (タグがあれば`VARIANT_TAG_BYTES`) + 最大のペイロード。
    ///     全メンバーが型なしの場合はタグだけ(`size_of`の`DD`と同じ)
    /// - ポインタは指す先に関わらず`POINTER_BYTES`(8)バイト
    ///
    /// `size_of`と違い、未定義の型や自己参照する型でもpanicせず`Err`を返す
    pub(in crate::compiler::ir) fn byte_size_of(&self, ty: &TyNode) -> Result<usize, err::ErrKind> {
        self.byte_size_of_inner(ty, &mut Vec::new())
    }

    fn byte_size_of_inner(
        &self,
        ty: &TyNode,
        // 値として展開中の構造体/バリアント名(自己参照の検出用)
        visiting: &mut Vec<String>,
    ) -> Result<usize, err::ErrKind> {
        const POINTER_BYTES: usize = 8;

        match ty {
            TyNode::Auto => panic!(),
            TyNode::Ty(name) => {
                // 組み込み型
                if types::Size::is_builtin_ty_name(name) {
                    return Ok(types::Size::try_from(ty)?.to_bytes());
                }

                // ポインタを介さずに自分自身を含む型は、サイズが決まらない
                if visiting.iter().any(|v| v == name) {
                    return Err(crate::GenUndefErrResult!(
                        UndefStruct,
                        format!("{name} は自分自身を直接含んでいるため、サイズを求められません"),
                        None
                    ));
                }

                // 構造体
                if let Some(struct_def) = self.struct_tree.get(name) {
                    visiting.push(name.clone());
                    let mut total = 0usize;
                    for field in struct_def.fields.iter() {
                        match self.byte_size_of_inner(&field.ty, visiting) {
                            Ok(n) => total += n,
                            Err(e) => {
                                visiting.pop();
                                return Err(e);
                            }
                        }
                    }
                    visiting.pop();
                    return Ok(total);
                }

                // バリアント型
                if let Some(variant_def) = self.variant_tree.get(name) {
                    // 全メンバーが型なしならタグ(整数値)だけ
                    if variant_def.is_plain_enum() {
                        return Ok(VARIANT_TAG_BYTES);
                    }
                    visiting.push(name.clone());
                    let mut payload = 0usize;
                    for field in variant_def.fields.iter() {
                        // 型なしメンバーはペイロード0バイト
                        let Some(field_ty) = &field.ty else { continue };
                        match self.byte_size_of_inner(field_ty, visiting) {
                            Ok(n) => payload = payload.max(n),
                            Err(e) => {
                                visiting.pop();
                                return Err(e);
                            }
                        }
                    }
                    visiting.pop();
                    let tag = if variant_def.is_tagged() {
                        VARIANT_TAG_BYTES
                    } else {
                        0
                    };
                    return Ok(tag + payload);
                }

                Err(crate::GenUndefErrResult!(
                    UndefVarTy,
                    name.to_string(),
                    None
                ))
            }
            // ポインタは指す先に関わらず固定サイズ(構造体への再帰もここで止まる)
            TyNode::Pointer { .. } => Ok(POINTER_BYTES),
            // スタック/静的領域は、要素のサイズ × 長さ
            TyNode::Stack { name, len } | TyNode::Static { name, len } => {
                let elem = self.byte_size_of_inner(name, visiting)?;
                Ok(if *len >= 1 { elem * len } else { elem })
            }
            TyNode::RefTy(inner) => self.byte_size_of_inner(inner, visiting),
            TyNode::SelfTy(name) => {
                self.byte_size_of_inner(&TyNode::Ty(name.to_string()), visiting)
            }
            TyNode::ConstractMust(inner) | TyNode::ConstractOf(inner) => {
                self.byte_size_of_inner(&inner.unwrap_ty(), visiting)
            }
        }
    }
}