use super::*;
use crate::compiler::ir::{def_tree, types::Size};
use super::tracking_contract_len::{VarLenFn, contract_mem_len, var_len_from_tree};

/// 定数式の評価結果。
///
/// `ExprNode` に真偽値リテラルが存在しないため、評価結果はASTとは別に保持する。
#[derive(Clone, Debug, PartialEq)]
pub enum ValKind {
    Num(i128),
    Float(f64),
    Bool(bool),
}

impl ValKind {
    fn as_num(&self) -> Option<i128> {
        match self {
            Self::Num(value) => Some(*value),
            _ => None,
        }
    }

    fn as_float(&self) -> Option<f64> {
        match self {
            Self::Num(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            Self::Bool(_) => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

/// 式が定数式であれば、その値を返す。
///
/// 評価できない式、型が合わない式、整数オーバーフロー、0除算などの場合は
/// `None` を返す。通常の式までコンパイルエラーにしないため、ここでは panic しない。
pub fn constant_folding(expr: &ast::Expr, var_len: VarLenFn) -> Option<ValKind> {
    tracking_expr_node(expr, var_len)
}

fn eval_pair<F>(
    pair: &(Box<ast::Expr>, Box<ast::Expr>),
    var_len: VarLenFn,
    op: F,
) -> Option<ValKind>
where
    F: FnOnce(ValKind, ValKind) -> Option<ValKind>,
{
    let left = tracking_expr_node(&pair.0, var_len)?;
    let right = tracking_expr_node(&pair.1, var_len)?;
    op(left, right)
}

fn compare_values<F>(left: ValKind, right: ValKind, compare: F) -> Option<ValKind>
where
    F: FnOnce(std::cmp::Ordering) -> bool,
{
    use std::cmp::Ordering;

    let ordering = match (left, right) {
        (ValKind::Num(left), ValKind::Num(right)) => left.cmp(&right),
        (ValKind::Float(left), ValKind::Float(right)) => left.partial_cmp(&right)?,
        (ValKind::Num(left), ValKind::Float(right)) => (left as f64).partial_cmp(&right)?,
        (ValKind::Float(left), ValKind::Num(right)) => left.partial_cmp(&(right as f64))?,
        _ => return None,
    };

    // Orderingを受け取って比較することで、各比較演算の実装を共通化する。
    let _ = Ordering::Equal;
    Some(ValKind::Bool(compare(ordering)))
}

fn tracking_expr_node(expr: &ast::Expr, var_len: VarLenFn) -> Option<ValKind> {
    use ValKind::{Bool, Float, Num};
    use ast::ExprNode;

    match &expr.node {
        ExprNode::Number(value) => Some(Num(value.parse::<i128>().unwrap())),
        ExprNode::Float(value) => Some(Float(value.parse::<f64>().unwrap())),

        // 単項演算
        ExprNode::Sign((is_positive, value)) => {
            let value = tracking_expr_node(value, var_len)?;
            match value {
                // `+a`は`a`のまま、`-a`は符号を反転する(`desugar_unary`と同じ)
                Num(value) => Some(Num(if *is_positive {
                    value
                } else {
                    value.checked_neg()?
                })),
                Float(value) => Some(Float(if *is_positive { value } else { -value })),
                Bool(_) => None,
            }
        }
        // `~a`は、型の幅(符号付きは-1、符号なしは型の最大値とのxor)で結果が変わる
        // (`desugar_unary`)ので、型の分からないここでは畳み込まない
        ExprNode::Inversion(_) => None,
        ExprNode::CmpNot(value) => match tracking_expr_node(value, var_len)? {
            Bool(value) => Some(Bool(!value)),
            _ => None,
        },

        // 算術演算
        ExprNode::Add(pair) => eval_pair(pair, var_len, |left, right| match (left, right) {
            (Num(left), Num(right)) => Some(Num(left.checked_add(right)?)),
            (left, right) => Some(Float(left.as_float()? + right.as_float()?)),
        }),
        ExprNode::Sub(pair) => eval_pair(pair, var_len, |left, right| match (left, right) {
            (Num(left), Num(right)) => Some(Num(left.checked_sub(right)?)),
            (left, right) => Some(Float(left.as_float()? - right.as_float()?)),
        }),
        ExprNode::Mul(pair) => eval_pair(pair, var_len, |left, right| match (left, right) {
            (Num(left), Num(right)) => Some(Num(left.checked_mul(right)?)),
            (left, right) => Some(Float(left.as_float()? * right.as_float()?)),
        }),
        ExprNode::Div(pair) => eval_pair(pair, var_len, |left, right| match (left, right) {
            (Num(_), Num(0)) => None,
            (Num(left), Num(right)) => Some(Num(left.checked_div(right)?)),
            (left, right) => {
                let right = right.as_float()?;
                if right == 0.0 {
                    None
                } else {
                    Some(Float(left.as_float()? / right))
                }
            }
        }),
        ExprNode::Surplus(pair) => eval_pair(pair, var_len, |left, right| match (left, right) {
            (Num(_), Num(0)) => None,
            (Num(left), Num(right)) => Some(Num(left.checked_rem(right)?)),
            (left, right) => {
                let right = right.as_float()?;
                if right == 0.0 {
                    None
                } else {
                    Some(Float(left.as_float()? % right))
                }
            }
        }),

        // ビット演算は整数だけを対象にする。
        ExprNode::AndCalcul(pair) => eval_pair(pair, var_len, |left, right| {
            Some(Num(left.as_num()? & right.as_num()?))
        }),
        ExprNode::XorCalcul(pair) => eval_pair(pair, var_len, |left, right| {
            Some(Num(left.as_num()? ^ right.as_num()?))
        }),
        ExprNode::OrCalcul(pair) => eval_pair(pair, var_len, |left, right| {
            Some(Num(left.as_num()? | right.as_num()?))
        }),
        ExprNode::LShift(pair) => eval_pair(pair, var_len, |left, right| {
            let left = left.as_num()?;
            let shift = u32::try_from(right.as_num()?).ok().filter(|s| *s < 64)?;
            Some(Num(left.checked_shl(shift)?))
        }),
        ExprNode::RShift(pair) => eval_pair(pair, var_len, |left, right| {
            let left = left.as_num()?;
            let shift = u32::try_from(right.as_num()?).ok().filter(|s| *s < 64)?;
            Some(Num(left.checked_shr(shift)?))
        }),

        // 論理演算。左辺だけで結果が決まる場合は右辺を評価しない。
        ExprNode::CmpAnd(pair) => {
            let left = tracking_expr_node(&pair.0, var_len)?.as_bool()?;
            if !left {
                Some(Bool(false))
            } else {
                Some(Bool(tracking_expr_node(&pair.1, var_len)?.as_bool()?))
            }
        }
        ExprNode::CmpOr(pair) => {
            let left = tracking_expr_node(&pair.0, var_len)?.as_bool()?;
            if left {
                Some(Bool(true))
            } else {
                Some(Bool(tracking_expr_node(&pair.1, var_len)?.as_bool()?))
            }
        }

        // 比較演算
        ExprNode::LessThen(pair) => eval_pair(pair, var_len, |left, right| {
            compare_values(left, right, |order| order == std::cmp::Ordering::Less)
        }),
        ExprNode::LessThenEq(pair) => eval_pair(pair, var_len, |left, right| {
            compare_values(left, right, |order| order != std::cmp::Ordering::Greater)
        }),
        ExprNode::GreaterThen(pair) => eval_pair(pair, var_len, |left, right| {
            compare_values(left, right, |order| order == std::cmp::Ordering::Greater)
        }),
        ExprNode::GreaterThenEq(pair) => eval_pair(pair, var_len, |left, right| {
            compare_values(left, right, |order| order != std::cmp::Ordering::Less)
        }),
        ExprNode::Equal(pair) => eval_pair(pair, var_len, |left, right| {
            let equal = match (left, right) {
                (Num(left), Num(right)) => left == right,
                (Float(left), Float(right)) => left == right,
                (Num(left), Float(right)) => left as f64 == right,
                (Float(left), Num(right)) => left == right as f64,
                (Bool(left), Bool(right)) => left == right,
                _ => return None,
            };
            Some(Bool(equal))
        }),
        ExprNode::NotEq(pair) => eval_pair(pair, var_len, |left, right| {
            let equal = match (left, right) {
                (Num(left), Num(right)) => left == right,
                (Float(left), Float(right)) => left == right,
                (Num(left), Float(right)) => left as f64 == right,
                (Float(left), Num(right)) => left == right as f64,
                (Bool(left), Bool(right)) => left == right,
                _ => return None,
            };
            Some(Bool(!equal))
        }),

        // 条件式の条件が定数なら、選択された側だけを評価する。
        ExprNode::Ternary((condition, then_expr, else_expr)) => {
            if tracking_expr_node(condition, var_len)?.as_bool()? {
                tracking_expr_node(then_expr, var_len)
            } else {
                tracking_expr_node(else_expr, var_len)
            }
        }
        // `Str`/`Array`のリテラルは要素数、変数は変数の情報から長さを求める。
        // 長さが分からない場合は`0`
        ExprNode::GetContractMemLen(var) => Some(Num(contract_mem_len(var, var_len))),

        // これらは値を安全に決定できない、または式の評価自体に副作用がある。
        ExprNode::RangeNode(_)
        | ExprNode::GetAddress(_)
        | ExprNode::ConnectAddr(_)
        | ExprNode::Cast(_)
        | ExprNode::Str(_)
        | ExprNode::Var(_)
        | ExprNode::CallFunc(_)
        | ExprNode::Assign(_)
        | ExprNode::Inc(_)
        | ExprNode::Dec(_)
        | ExprNode::LeftInc(_)
        | ExprNode::LeftDec(_)
        | ExprNode::Sizeof(_)
        | ExprNode::Match { .. }
        | ExprNode::Loop { .. }
        | ExprNode::InitStruct { .. }
        | ExprNode::InitVariant { .. }
        | ExprNode::EnumVariant { .. }
        | ExprNode::Array(_)
        | ExprNode::RefArray { .. }
        | ExprNode::DefVar(_)
        | ExprNode::VariantBind(_)
        | ExprNode::Scope { .. }
        | ExprNode::Member { .. }
        | ExprNode::PtrMember { .. } => None,
    }
}

/// 整数`value`が、型`size`の範囲に収まるか
fn int_fits(value: i128, size: &Size) -> bool {
    match size {
        Size::DB => i8::try_from(value).is_ok(),
        Size::DW => i16::try_from(value).is_ok(),
        Size::DD => i32::try_from(value).is_ok(),
        Size::DQ => i64::try_from(value).is_ok(),
        Size::UDB => u8::try_from(value).is_ok(),
        Size::UDW => u16::try_from(value).is_ok(),
        Size::UDD => u32::try_from(value).is_ok(),
        Size::UDQ => u64::try_from(value).is_ok(),
        Size::Bool => matches!(value, 0 | 1),
        _ => false,
    }
}

/// 浮動小数点数を、リテラルの文字列にする。
/// 指数表記(`1e21`)や`inf`/`NaN`になる値は、アセンブラ側で扱えないので`None`
fn float_literal(value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let text = format!("{:?}", value);
    if text.contains(['e', 'E']) {
        return None;
    }
    Some(text)
}

/// 定数式`expr`を、期待する型`expect`のリテラル(`Number`/`Float`)に置き換えた式を返す。
///
/// 畳み込まない(`None`)場合:
/// - すでにリテラルだけの式、定数式でない式、評価できない式(0除算・オーバーフローなど)
/// - 結果が`expect`の範囲に収まらない(実行時にラップする結果と食い違わないよう、
///   実行時の計算に任せる)
/// - 整数の型に浮動小数点数の結果を入れる、`f32`に入れる(丸めが実行時と変わる)
/// - `expect`が整数/`bool`/`f64`以外(ポインタ・構造体など)
///
/// 比較・論理演算の結果(`Bool`)は、`1`/`0`の整数にする。
pub fn fold_expr(
    expr: &ast::Expr,
    expect: &Size,
    var_tree: &def_tree::VarTree,
) -> Option<ast::Expr> {
    fold_expr_with(expr, expect, &|name| var_len_from_tree(name, var_tree))
}

/// `fold_expr`の本体。変数の長さを引く関数`var_len`を直接受け取る
/// (`VarTree`を用意せずにテストできる)
pub fn fold_expr_with(expr: &ast::Expr, expect: &Size, var_len: VarLenFn) -> Option<ast::Expr> {
    use ast::ExprNode;

    // リテラルは、そのままIRにできる
    if matches!(expr.node, ExprNode::Number(_) | ExprNode::Float(_)) {
        return None;
    }

    let node = match (constant_folding(expr, var_len)?, expect) {
        (ValKind::Bool(value), size) if !size.is_float() => {
            let value = i128::from(value);
            int_fits(value, size).then(|| ExprNode::Number(value.to_string()))?
        }
        (ValKind::Num(value), size) if !size.is_float() => {
            int_fits(value, size).then(|| ExprNode::Number(value.to_string()))?
        }
        (ValKind::Num(value), Size::F64) => ExprNode::Float(float_literal(value as f64)?),
        (ValKind::Float(value), Size::F64) => ExprNode::Float(float_literal(value)?),
        _ => return None,
    };
    Some(node.with_span(std::rc::Rc::clone(&expr.span)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::ExprNode;
    use std::rc::Rc;

    /// 変数の長さが何も分からない
    fn no_var(_: &str) -> Option<i128> {
        None
    }

    fn sp() -> Rc<crate::compiler::err::Span> {
        Rc::new(crate::compiler::err::Span::unknown())
    }

    fn num(v: &str) -> ast::Expr {
        ExprNode::Number(v.to_string()).with_span(sp())
    }

    fn flt(v: &str) -> ast::Expr {
        ExprNode::Float(v.to_string()).with_span(sp())
    }

    fn bin(
        f: fn((Box<ast::Expr>, Box<ast::Expr>)) -> ExprNode,
        a: ast::Expr,
        b: ast::Expr,
    ) -> ast::Expr {
        f(ast::Expr::wrap(a, b)).with_span(sp())
    }

    #[test]
    fn folds_nested_arithmetic() {
        // 1 + 2 * 3
        let e = bin(
            ExprNode::Add,
            num("1"),
            bin(ExprNode::Mul, num("2"), num("3")),
        );
        assert_eq!(constant_folding(&e, &no_var), Some(ValKind::Num(7)));
        let folded = fold_expr_with(&e, &Size::DD, &no_var).unwrap();
        assert_eq!(folded.node, ExprNode::Number("7".to_string()));
    }

    #[test]
    fn unary_plus_is_identity() {
        let e = ExprNode::Sign((
            true,
            Box::new(ExprNode::Sign((false, Box::new(num("5")))).with_span(sp())),
        ))
        .with_span(sp());
        assert_eq!(constant_folding(&e, &no_var), Some(ValKind::Num(-5)));
    }

    #[test]
    fn comparison_becomes_0_or_1() {
        let e = bin(ExprNode::LessThen, num("1"), num("2"));
        let folded = fold_expr_with(&e, &Size::DD, &no_var).unwrap();
        assert_eq!(folded.node, ExprNode::Number("1".to_string()));
    }

    #[test]
    fn does_not_fold_out_of_range_or_div_zero() {
        // int(i32)に収まらない
        let big = bin(ExprNode::Add, num("2147483647"), num("1"));
        assert!(fold_expr_with(&big, &Size::DD, &no_var).is_none());
        assert!(fold_expr_with(&big, &Size::DQ, &no_var).is_some());
        // 符号なしに負の値
        let neg = bin(ExprNode::Sub, num("1"), num("2"));
        assert!(fold_expr_with(&neg, &Size::UDD, &no_var).is_none());
        // 0除算
        let div0 = bin(ExprNode::Div, num("1"), num("0"));
        assert!(fold_expr_with(&div0, &Size::DD, &no_var).is_none());
    }

    #[test]
    fn float_only_into_f64() {
        let e = bin(ExprNode::Add, flt("1.5"), flt("2.0"));
        assert_eq!(
            fold_expr_with(&e, &Size::F64, &no_var).unwrap().node,
            ExprNode::Float("3.5".to_string())
        );
        assert!(fold_expr_with(&e, &Size::F32, &no_var).is_none());
        assert!(fold_expr_with(&e, &Size::DD, &no_var).is_none());
    }

    #[test]
    fn literal_and_variable_are_left_alone() {
        assert!(fold_expr_with(&num("1"), &Size::DD, &no_var).is_none());
        let v = bin(
            ExprNode::Add,
            ExprNode::Var("a".to_string()).with_span(sp()),
            num("1"),
        );
        assert!(fold_expr_with(&v, &Size::DD, &no_var).is_none());
    }
}