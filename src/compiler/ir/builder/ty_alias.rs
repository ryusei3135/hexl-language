//! 型エイリアス(`type T = BaseTy`)の展開
//!
//! IRを作る前に、`Group1Node`全体の型(`TyNode`)に含まれるエイリアス名を
//! 元の型に置き換える。これにより、`types.rs`の`size_of`や構造体の型の照合など
//! 後続の処理は、エイリアスの存在を一切意識しなくてよい。
//!
//! - エイリアスの連鎖(`type A = B` / `type B = int`)は`int`まで展開する
//! - エイリアスの循環(`type A = B` / `type B = A`、`type T = T`)はエラー
//! - 同じ名前のエイリアスの重複定義はエラー
//! - 構造体の初期化(`Alias { .. }`)や`Alias::Member`のように、型名が
//!   文字列のまま使われている場所も、エイリアスの先が名前だけの型なら置き換える
use std::collections::HashMap;

use super::*;
use crate::compiler::ast::{StmtNode, TyNode};

type AliasMap = HashMap<String, ast::TyNode>;

/// エイリアスを展開したノードの列を返す。エイリアスの宣言自体は残す
/// (`builder`は`TypeAlias`を無視する)
pub(super) fn expand_type_aliases(
    nodes: &[ast::Group1Node],
) -> Result<Vec<ast::Group1Node>, err::ErrKind> {
    let map = map_alias(nodes)?;
    let mut out = nodes.to_vec();
    if map.is_empty() {
        return Ok(out);
    }
    for n in out.iter_mut() {
        group1(n, &map);
    }
    Ok(out)
}

/// 全てのエイリアスを、他のエイリアスを含まない型へ解決した表を作る
fn map_alias(nodes: &[ast::Group1Node]) -> Result<AliasMap, err::ErrKind> {
    let mut map: AliasMap = HashMap::new();
    for n in nodes {
        if let ast::Group1Node::TypeAlias(a) = n {
            if map.insert(a.name.clone(), a.ty.clone()).is_some() {
                // 重複定義。専用のエラー種別があれば差し替える
                return Err(err::ErrKind::UnexpectedToken);
            }
        }
    }

    // 連鎖を、変化しなくなるまで展開する。連鎖の長さはエイリアスの数以下なので、
    // それ以上繰り返しても安定しないものは循環している
    for _ in 0..=map.len() {
        let snapshot = map.clone();
        let lookup = |name: &str| snapshot.get(name).cloned();
        let mut changed = false;
        for ty in map.values_mut() {
            let next = ty.expand_alias(&lookup);
            if next != *ty {
                *ty = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // 展開後も自分自身を含むエイリアス名が残っていれば循環
    for (name, ty) in &map {
        let leaf = ty.get_ty_str_name();
        if map.contains_key(&leaf) {
            return Err(crate::GenUndefErrResult!(
                UndefVarTy,
                name.to_string(),
                None
            ));
        }
    }
    Ok(map)
}

fn ty(t: &mut TyNode, map: &AliasMap) {
    *t = t.expand_alias(&|name: &str| map.get(name).cloned());
}

/// 型名だけが文字列で使われている場所用。
/// エイリアスの先が名前だけの型(`Ty`)のときだけ置き換える
fn name(n: &mut String, map: &AliasMap) {
    if let Some(TyNode::Ty(target)) = map.get(n.as_str()) {
        *n = target.clone();
    }
}

fn group1(node: &mut ast::Group1Node, map: &AliasMap) {
    match node {
        ast::Group1Node::FuncDefine(f) => {
            f.temp_ty.iter_mut().for_each(|t| ty(t, map));
            f.params.iter_mut().for_each(|p| ty(&mut p.ty, map));
            ty(&mut f.ret_ty, map);
            for g in f.body.iter_mut() {
                group2(&mut g.node, map);
            }
        }
        ast::Group1Node::StructDefine(s) => {
            s.fields.iter_mut().for_each(|f| ty(&mut f.ty, map));
            s.methods.iter_mut().for_each(|m| group1(m, map));
        }
        ast::Group1Node::VariantDefine(v) => {
            for f in v.fields.iter_mut() {
                if let Some(t) = f.ty.as_mut() {
                    ty(t, map);
                }
            }
            v.methods.iter_mut().for_each(|m| group1(m, map));
        }
        ast::Group1Node::TypeAlias(_) | ast::Group1Node::Include(_) | ast::Group1Node::Line(_) => {}
    }
}

fn body(b: &mut Body, map: &AliasMap) {
    for g in b.iter_mut() {
        group2(&mut g.node, map);
    }
}

fn group2(node: &mut ast::Group2Node, map: &AliasMap) {
    match node {
        ast::Group2Node::Expr(e) | ast::Group2Node::Stmt(StmtNode::Return(e)) => expr(e, map),
        _ => {}
    }
}

fn expr(e: &mut ast::Expr, map: &AliasMap) {
    use ast::ExprNode as E;

    // 子が2つの二項演算など
    macro_rules! pair {
        ($p:expr) => {{
            expr(&mut $p.0, map);
            expr(&mut $p.1, map);
        }};
    }

    match &mut e.node {
        E::Cast((t, v)) => {
            ty(t, map);
            expr(v, map);
        }
        E::DefVar(d) => {
            ty(&mut d.ty, map);
            expr(&mut d.value, map);
        }
        E::VariantBind(b) => {
            if let Some(t) = b.ty.as_mut() {
                ty(t, map);
            }
            if let Some(n) = b.variant_ty.as_mut() {
                name(n, map);
            }
            expr(&mut b.value, map);
        }
        E::CallFunc(c) => {
            c.temp_ty.iter_mut().for_each(|t| ty(t, map));
            c.args.iter_mut().for_each(|a| expr(a, map));
        }
        E::InitStruct {
            name: n, fields, ..
        }
        | E::InitVariant {
            name: n, fields, ..
        } => {
            name(n, map);
            fields.values_mut().for_each(|v| expr(v, map));
        }
        E::EnumVariant { name: n, .. } => name(n, map),

        E::GetAddress(a)
        | E::ConnectAddr(a)
        | E::Inc(a)
        | E::Dec(a)
        | E::Inversion(a)
        | E::CmpNot(a)
        | E::LeftInc(a)
        | E::LeftDec(a) => expr(a, map),
        E::Sign((_, a)) => expr(a, map),

        E::Assign(a) => {
            expr(&mut a.dst, map);
            expr(&mut a.value, map);
        }
        E::Add(p)
        | E::Sub(p)
        | E::Mul(p)
        | E::Div(p)
        | E::Surplus(p)
        | E::CmpAnd(p)
        | E::CmpOr(p)
        | E::LessThen(p)
        | E::GreaterThen(p)
        | E::LessThenEq(p)
        | E::GreaterThenEq(p)
        | E::AndCalcul(p)
        | E::XorCalcul(p)
        | E::OrCalcul(p)
        | E::LShift(p)
        | E::RShift(p)
        | E::Equal(p)
        | E::NotEq(p) => pair!(p),
        E::Ternary((c, a, b)) => {
            expr(c, map);
            expr(a, map);
            expr(b, map);
        }

        E::Match {
            pattern,
            arms,
            arm_else,
        } => {
            if let Some(p) = pattern {
                expr(p, map);
            }
            for arm in arms.iter_mut() {
                expr(&mut arm.pattern, map);
                body(&mut arm.body, map);
            }
            if let Some(b) = arm_else {
                body(b, map);
            }
        }
        E::Loop { pattern, body: b } => {
            if let Some(p) = pattern {
                expr(p, map);
            }
            body(b, map);
        }
        E::Array(items) => items.iter_mut().for_each(|i| expr(i, map)),
        E::RefArray {
            dst,
            index,
            else_idx,
            ..
        } => {
            expr(dst, map);
            expr(index, map);
            if let Some(x) = else_idx {
                expr(x, map);
            }
        }
        E::Scope { target, .. } | E::Member { target, .. } => expr(target, map),
        E::PtrMember {
            index,
            else_idx,
            target,
            ..
        } => {
            if let Some(x) = index {
                expr(x, map);
            }
            if let Some(x) = else_idx {
                expr(x, map);
            }
            expr(target, map);
        }

        E::RangeNode(_) | E::Number(_) | E::Float(_) | E::Str(_) | E::Var(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::TyNode;

    fn alias(name: &str, base: TyNode) -> ast::Group1Node {
        ast::TypeAlias::new(name.to_string(), base, test_span())
    }

    fn int() -> TyNode {
        TyNode::Ty("int".to_string())
    }

    #[test]
    fn chain_is_fully_expanded() {
        let nodes = vec![alias("A", TyNode::Ty("B".into())), alias("B", int())];
        let map = map_alias(&nodes).unwrap();
        assert_eq!(map["A"], int());
        assert_eq!(map["B"], int());
    }

    #[test]
    fn pointer_inside_is_expanded() {
        let nodes = vec![alias("T", int())];
        let map = map_alias(&nodes).unwrap();
        let mut t = TyNode::Pointer {
            is_const: false,
            ty_name: Box::new(TyNode::Ty("T".into())),
            range: None,
        };
        ty(&mut t, &map);
        assert_eq!(
            t,
            TyNode::Pointer {
                is_const: false,
                ty_name: Box::new(int()),
                range: None
            }
        );
    }

    #[test]
    fn cycle_is_error() {
        let nodes = vec![
            alias("A", TyNode::Ty("B".into())),
            alias("B", TyNode::Ty("A".into())),
        ];
        assert!(map_alias(&nodes).is_err());
        assert!(map_alias(&[alias("T", TyNode::Ty("T".into()))]).is_err());
    }

    #[test]
    fn duplicate_is_error() {
        let nodes = vec![alias("T", int()), alias("T", int())];
        assert!(map_alias(&nodes).is_err());
    }
}
