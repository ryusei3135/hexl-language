//! `IR`が持つ型情報(`struct_tree`/`variant_tree`)を使った、型のサイズ計算

use super::*;

impl IR {
    /// `TyNode`を`Size`へ変換する
    ///
    /// `try_into`が`Err`を返した場合(構造体などのユーザー定義の型)は、
    /// `struct_tree`を参照して`Size::emit_struct_ty_node`で構造体の型を作成する
    pub(in crate::compiler::ir) fn try_size_or_emit_struct(
        &self,
        ty: &TyNode,
    ) -> Result<types::Size, err::ErrKind> {
        // バリアント型は`struct_tree`では解決できないので先に調べる。
        // `Self*`は`Option$int*`のようなポインタ型に解決されているので、
        // ポインタ・配列・契約・参照の内側の型もたどって調べる
        if self.ty_base_is_variant(ty) {
            return Ok(self.size_of(ty));
        }
        types::Size::try_from_or_emit_struct(&mut |name| self.struct_tree.get_struct_size(name), ty)
            .map_err(|error| {
                if error.span().is_none() {
                    error.with_span(std::rc::Rc::clone(&self.current_span))
                } else {
                    error
                }
            })
    }

    /// `ty`の包みを外した、いちばん内側の型がバリアント型か
    /// (`Option$int` / `Option$int*` / `[Option$int 4]` / `Option$int* must=free`)
    fn ty_base_is_variant(&self, ty: &TyNode) -> bool {
        match ty {
            TyNode::Ty(name) | TyNode::SelfTy(name) => self.variant_tree.contains_key(name),
            TyNode::Pointer { ty_name, .. } => self.ty_base_is_variant(ty_name),
            TyNode::Stack { name, .. } | TyNode::Static { name, .. } => {
                self.ty_base_is_variant(name)
            }
            TyNode::RefTy(inner) | TyNode::RefContract(inner) => self.ty_base_is_variant(inner),
            TyNode::ConstractMust(inner) | TyNode::ConstractOf(inner) => {
                self.ty_base_is_variant(&inner.unwrap_ty())
            }
            TyNode::Auto => false,
        }
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
            TyNode::RefTy(inner) | TyNode::RefContract(inner) => self.size_of(inner),
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
    pub(in crate::compiler::ir) fn byte_size_of(
        &self,
        ty: &TyNode,
        span: &std::rc::Rc<err::Span>,
    ) -> Result<usize, err::ErrKind> {
        self.byte_size_of_inner(ty, &mut Vec::new(), span)
    }

    fn byte_size_of_inner(
        &self,
        ty: &TyNode,
        // 値として展開中の構造体/バリアント名(自己参照の検出用)
        visiting: &mut Vec<String>,
        span: &std::rc::Rc<err::Span>,
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
                        None,
                        std::rc::Rc::clone(span)
                    ));
                }

                // 構造体
                if let Some(struct_def) = self.struct_tree.get(name) {
                    visiting.push(name.clone());
                    let mut total = 0usize;
                    for field in struct_def.fields.iter() {
                        match self.byte_size_of_inner(&field.ty, visiting, &field.span) {
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
                        match self.byte_size_of_inner(field_ty, visiting, &field.span) {
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
                    None,
                    std::rc::Rc::clone(span)
                ))
            }
            // ポインタは指す先に関わらず固定サイズ(構造体への再帰もここで止まる)
            TyNode::Pointer { .. } => Ok(POINTER_BYTES),
            // スタック/静的領域は、要素のサイズ × 長さ
            TyNode::Stack { name, len } | TyNode::Static { name, len } => {
                let elem = self.byte_size_of_inner(name, visiting, span)?;
                Ok(if *len >= 1 { elem * len } else { elem })
            }
            TyNode::RefTy(inner) | TyNode::RefContract(inner) => {
                self.byte_size_of_inner(inner, visiting, span)
            }
            TyNode::SelfTy(name) => {
                self.byte_size_of_inner(&TyNode::Ty(name.to_string()), visiting, span)
            }
            TyNode::ConstractMust(inner) | TyNode::ConstractOf(inner) => {
                self.byte_size_of_inner(&inner.unwrap_ty(), visiting, span)
            }
        }
    }
}
