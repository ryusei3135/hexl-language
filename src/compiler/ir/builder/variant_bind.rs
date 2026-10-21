//! `cond`の条件式の中だけで使える、バリアントのパターン束縛
//! (`node::Expr::VariantBind`)を、次のように展開するAPIを提供する
//!
//! ```text
//! cond M(val) := value { body } | { else_body }
//!
//!   ↓ 展開
//!
//! if value.label == tag_of(M) {     // `variant_bind_cond`
//!     val: ty = value.value         // `variant_bind_define_var`
//!     body
//! } else {
//!     else_body
//! }
//! ```
//!
//! これは`Some(val) == value`のパターンマッチと同じ意味になる。
//! - `value.label`: バリアント型の値の先頭にあるタグ(`tagged`のときだけ存在する)
//! - `value.value`: タグの後ろにあるペイロード(メンバー`M`の中身)
//!
//! 条件(`value.label == tag`)は条件式の位置、`val`の定義は
//! 条件が真のときの本体の先頭(本体のスコープの中)に置く。
//! そのため、`val`は本体の中でだけ使える。

use super::*;
use err_factory::expr_node;

/// `M(val) := value`の展開に必要な情報
/// (`variant_bind_resolve`で、型の定義から求める)
pub(super) struct VariantBindInfo {
    /// `value`の変数の名前
    src: String,
    /// `value`がポインタ(`V*`)なら`true`。その場合、`value.label`/`value.value`は
    /// ポインタが指す先(`RefStructPtr`)から読む
    via_ptr: bool,
    /// `M`のタグ(定義順のインデックス)
    tag: usize,
    /// `value.value`(ペイロード)の、バリアント型の先頭からのオフセット
    payload_pos: usize,
    /// `value.value`の型
    payload_size: types::Size,
    /// `val`の名前
    bind: String,
    /// `val`の型(`:=`のときは`M`の中身の型)
    ty: node::TyNode,
    /// `val`が不変か、`mut`/`const`か
    var_attr: parse::VarMutAttr,
}

impl IR {
    /// `M(val) := value`が、どのバリアント型のどのメンバーを指すかを調べる
    ///
    /// `value`には変数(`Expr::Var`)だけを渡せる。バリアント型のタグとペイロードは、
    /// 変数の置かれた場所から直接読み出すため
    fn variant_bind_resolve(
        &mut self,
        bind: &node::VariantBind,
    ) -> Result<VariantBindInfo, err::ErrKind> {
        let node::ExprNode::Var(src) = &(*bind.value).node else {
            panic!(
                "`{}({}) := ..`の`:=`の右辺には、バリアント型の変数を渡してください: {:?}",
                bind.variant, bind.bind, bind.value
            );
        };
        // `value`の変数を使ったことを記録(`src/ir/counter_mnger.rs`)
        self.count_var_use(src);

        let via_ptr = matches!(
            self.var_tree.get_ty_node(src),
            Some(node::TyNode::Pointer { .. })
        );
        // ポインタ型の変数なら、指す先のバリアント型の名前が返る
        let variant_name = self.var_tree.get_ty_name(src)?;
        let variant_def = self.variant_tree.get_variant(&variant_name)?;

        // `A::M(val) := value`の`A`が、`value`の型と同じか確認する
        // (ジェネリクスの`Option$int`は、`Option`として扱う)
        if let Some(ty) = &bind.variant_ty {
            let same = &variant_name == ty
                || variant_name
                    .strip_prefix(ty.as_str())
                    .is_some_and(|suffix| suffix.starts_with('$'));
            if !same {
                panic!(
                    "`{}::{}({}) := {:?}`の`{}`は、右辺の型 `{}` と一致しません",
                    ty, bind.variant, bind.bind, bind.value, ty, variant_name
                );
            }
        }

        if variant_def.is_plain_enum() {
            panic!(
                "バリアント型 `{}` は全てのメンバーが型なしなので、`{}({}) := ..`のように中身を取り出せません",
                variant_name, bind.variant, bind.bind
            );
        }
        if !variant_def.is_tagged() {
            panic!(
                "バリアント型 `{}` は`unsafe`でタグがないので、`{}({}) := ..`のようにメンバーを判定できません",
                variant_name, bind.variant, bind.bind
            );
        }

        let tag = variant_def.tag_of(&bind.variant).ok_or_else(|| {
            expr_node::this_variant_field_is_undefined(&bind.variant).unwrap_err()
        })?;
        let Some(field_ty) = variant_def.fields[tag].ty.clone() else {
            panic!(
                "バリアント型 `{}` のメンバー `{}` は型がないので、`{}({}) := ..`のように中身を取り出せません",
                variant_name, bind.variant, bind.variant, bind.bind
            );
        };
        // `M(val): ty = value`は、`ty`が`M`の中身の型と同じでなければならない
        if let Some(ty) = &bind.ty {
            if ty != &field_ty {
                panic!(
                    "`{}({}): {:?} = ..`の型が、バリアント型 `{}` のメンバー `{}` の型({:?})と一致しません",
                    bind.variant, bind.bind, ty, variant_name, bind.variant, field_ty
                );
            }
        }

        Ok(VariantBindInfo {
            src: src.clone(),
            via_ptr,
            tag,
            payload_pos: self.size_of_variant(&variant_def).variant_payload_offset(),
            payload_size: self.try_size_or_emit_struct(&field_ty)?,
            bind: bind.bind.clone(),
            ty: field_ty,
            var_attr: bind.var_attr.clone(),
        })
    }

    /// `value`の先頭(`pos`バイト目)から`size`の値を読む命令を作る
    fn variant_ref_inst(info: &VariantBindInfo, pos: usize, size: types::Size) -> inst::Inst {
        if info.via_ptr {
            inst::Inst::RefStructPtr {
                src: info.src.clone(),
                pos,
                size,
            }
        } else {
            inst::Inst::RefStruct {
                src: info.src.clone(),
                pos,
                size,
            }
        }
    }

    /// 条件式: `value.label == tag_of(M)`のIRを生成する
    ///
    /// 呼び出し元が、この前に`ExpectJmp(arm)`を積んでおく
    /// (他の条件式と同じ。最後に積まれた比較が真なら`arm`へジャンプする)
    pub(super) fn variant_bind_cond(
        &mut self,
        bind: &node::VariantBind,
    ) -> Result<VariantBindInfo, err::ErrKind> {
        let info = self.variant_bind_resolve(bind)?;
        // `value.label`
        let label_idx = self.push_inst(Self::variant_ref_inst(&info, 0, types::Size::DD));
        // `value.label == tag`
        self.push_cmp_with_num(label_idx, info.tag, inst::ExprKind::Equal);
        Ok(info)
    }

    /// 条件が真のときの本体の先頭: `val: ty = value.value`を定義する
    ///
    /// アームのスコープ(`begin_scope`)の中で呼び出す
    /// (`end_block_scope`で`val`の定義が消える)
    pub(super) fn variant_bind_define_var(
        &mut self,
        info: &VariantBindInfo,
    ) -> Result<(), err::ErrKind> {
        // `value.value`
        let value_idx = self.push_inst(Self::variant_ref_inst(
            info,
            info.payload_pos,
            info.payload_size.clone(),
        ));
        // `val`を、通常の変数定義(`def_var_node`)と同じように登録する
        let _ = self.var_tree.push::<'l'>(
            &info.bind,
            self.id_counter,
            &info.ty,
            &info.var_attr,
            Rc::clone(&self.current_span),
            || self.constract_flag.put_var_def(&info.ty),
        )?;
        self.push_inst(inst::Inst::Mov {
            name: Some(info.bind.clone()),
            size: info.payload_size.clone(),
            dst: self.id_counter,
            src: value_idx,
        });
        Ok(())
    }
}

impl IR {
    /// `Type::Mem(value)`が、バリアント型の初期化だった場合、`InitVariant`に直す
    ///
    /// パーサーは、`Type::Mem(..)`がバリアント型の初期化か、`Type`のメゾットの呼び出しかを、
    /// 「その時点までに読んだバリアント型の定義」で判断する。
    /// そのため、バリアント型が関数より後ろに定義されている場合や、
    /// ジェネリクス関数の中などでは、バリアント型の初期化が`Scope`+`CallFunc`として
    /// 渡されてくる。IRはバリアント型の定義を全て集めてから処理するので、ここで直す。
    ///
    /// `Type`がバリアント型で、`Mem`がそのメンバー、かつ引数が1つのときだけ直す
    pub(super) fn scope_call_to_variant_init(&self, expr: &node::Expr) -> node::Expr {
        let node::ExprNode::Scope { scope, target } = &expr.node else {
            return expr.clone();
        };
        let (Some(type_name), node::ExprNode::CallFunc(call)) = (scope.last(), &(**target).node)
        else {
            return expr.clone();
        };
        let is_ctor = scope.len() == 1
            && call.temp_ty.is_empty()
            && call.args.len() == 1
            && self
                .variant_tree
                .find_by_base_name(type_name)
                .is_some_and(|def| def.find_field(&call.name).is_some());
        if !is_ctor {
            return expr.clone();
        }
        let mut fields = HashMap::new();
        fields.insert(call.name.clone(), Box::new(call.args[0].clone()));
        node::Expr::new(
            node::ExprNode::InitVariant {
                is_self: false,
                name: type_name.clone(),
                fields,
            },
            Rc::clone(&expr.span),
        )
    }
}
