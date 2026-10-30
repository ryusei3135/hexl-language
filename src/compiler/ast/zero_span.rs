//! テスト専用: ノードに含まれるすべての `span` の `line` と `pos` を 0 にする
//!
//! `StructDefine` / `FuncDefine` / `TypeAlias` などは `span` も含めて
//! `PartialEq` で比較されるため、パース結果と期待値をテストで比べるときに
//! `parse(..).zeroed_spans()` のように使う。
//!
//! `Span` の作り方を変えたい場合は `zeroed` だけを直せばよい。

use super::*;
use crate::compiler::err::Span;
use std::rc::Rc;

/// `line`と`pos`が0の`Span`(`test_utils::span_at(0)`と同じもの)
#[inline(always)]
fn zeroed() -> Rc<Span> {
    Rc::new(Span::new(0, 0, String::new(), &Rc::new("<test>".to_string())))
}

/// 自身と、中に含まれるすべてのノードの`span`を0にする
pub trait ZeroSpan {
    fn zero_span(&mut self);

    /// `zero_span`して自身を返す(メソッドチェーン用)
    fn zeroed_spans(mut self) -> Self
    where
        Self: Sized,
    {
        self.zero_span();
        self
    }
}

// ===== 汎用 =====

impl ZeroSpan for Rc<Span> {
    #[inline(always)]
    fn zero_span(&mut self) {
        *self = zeroed();
    }
}

impl<T: ZeroSpan> ZeroSpan for Vec<T> {
    fn zero_span(&mut self) {
        self.iter_mut().for_each(ZeroSpan::zero_span);
    }
}

impl<T: ZeroSpan> ZeroSpan for Option<T> {
    fn zero_span(&mut self) {
        if let Some(v) = self {
            v.zero_span();
        }
    }
}

impl<T: ZeroSpan> ZeroSpan for Box<T> {
    fn zero_span(&mut self) {
        (**self).zero_span();
    }
}

// ===== 式 =====

impl ZeroSpan for Expr {
    fn zero_span(&mut self) {
        self.span.zero_span();
        self.node.zero_span();
    }
}

impl ZeroSpan for ExprNode {
    fn zero_span(&mut self) {
        match self {
            Self::RangeNode(_)
            | Self::Number(_)
            | Self::Float(_)
            | Self::Str(_)
            | Self::Sizeof(_)
            | Self::Var(_)
            | Self::EnumVariant { .. } => {}

            Self::GetAddress(e)
            | Self::ConnectAddr(e)
            | Self::Inc(e)
            | Self::Dec(e)
            | Self::Inversion(e)
            | Self::CmpNot(e)
            | Self::LeftInc(e)
            | Self::LeftDec(e)
            | Self::Cast((_, e))
            | Self::Sign((_, e)) => e.zero_span(),

            Self::Add((l, r))
            | Self::Sub((l, r))
            | Self::Mul((l, r))
            | Self::Div((l, r))
            | Self::Surplus((l, r))
            | Self::CmpAnd((l, r))
            | Self::CmpOr((l, r))
            | Self::LessThen((l, r))
            | Self::GreaterThen((l, r))
            | Self::LessThenEq((l, r))
            | Self::GreaterThenEq((l, r))
            | Self::AndCalcul((l, r))
            | Self::XorCalcul((l, r))
            | Self::OrCalcul((l, r))
            | Self::LShift((l, r))
            | Self::RShift((l, r))
            | Self::Equal((l, r))
            | Self::NotEq((l, r)) => {
                l.zero_span();
                r.zero_span();
            }

            Self::Ternary((cond, then, els)) => {
                cond.zero_span();
                then.zero_span();
                els.zero_span();
            }

            Self::CallFunc(info) => info.args.zero_span(),
            Self::Assign(assign) => {
                assign.dst.zero_span();
                assign.value.zero_span();
            }
            Self::Match {
                pattern,
                arms,
                arm_else,
            } => {
                pattern.zero_span();
                arms.zero_span();
                arm_else.zero_span();
            }
            Self::Loop { pattern, body } => {
                pattern.zero_span();
                body.zero_span();
            }
            Self::InitStruct { fields, .. } | Self::InitVariant { fields, .. } => {
                fields.values_mut().for_each(ZeroSpan::zero_span);
            }
            Self::Array(items) => items.zero_span(),
            Self::RefArray {
                dst,
                index,
                else_idx,
                ..
            } => {
                dst.zero_span();
                index.zero_span();
                else_idx.zero_span();
            }
            Self::DefVar(def) => def.value.zero_span(),
            Self::VariantBind(bind) => bind.value.zero_span(),
            Self::Scope { target, .. } | Self::Member { target, .. } => target.zero_span(),
            Self::PtrMember {
                index,
                else_idx,
                target,
                ..
            } => {
                index.zero_span();
                else_idx.zero_span();
                target.zero_span();
            }
        }
    }
}

impl ZeroSpan for MatchArm {
    fn zero_span(&mut self) {
        self.pattern.zero_span();
        self.body.zero_span();
        self.span.zero_span();
    }
}

impl ZeroSpan for InlineAsm {
    fn zero_span(&mut self) {
        self.operands.zero_span();
        self.span.zero_span();
    }
}

// ===== 文 =====

impl ZeroSpan for StmtNode {
    fn zero_span(&mut self) {
        match self {
            Self::Return(expr) => expr.zero_span(),
            Self::Continue => {}
            Self::Break(span) => span.zero_span(),
        }
    }
}

impl ZeroSpan for Group2Node {
    fn zero_span(&mut self) {
        match self {
            Self::Stmt(stmt) => stmt.zero_span(),
            Self::Expr(expr) => expr.zero_span(),
            Self::CompleSyntax((_, asms)) => asms.zero_span(),
            Self::Include(path) => path.zero_span(),
            Self::Line(_) => {}
        }
    }
}

/// `span`に加えて、`line`フィールド(何行目か)も0にする
impl ZeroSpan for Group2Info {
    fn zero_span(&mut self) {
        self.line = 0;
        self.node.zero_span();
        self.span.zero_span();
    }
}

impl ZeroSpan for ModPath {
    fn zero_span(&mut self) {
        self.span.zero_span();
    }
}

// ===== 定義 =====

impl ZeroSpan for ArgsNode {
    fn zero_span(&mut self) {
        self.span.zero_span();
    }
}

impl ZeroSpan for FuncDefine {
    fn zero_span(&mut self) {
        self.params.zero_span();
        self.body.zero_span();
        self.span.zero_span();
    }
}

impl ZeroSpan for TypeAlias {
    fn zero_span(&mut self) {
        self.span.zero_span();
    }
}

impl ZeroSpan for StructField {
    fn zero_span(&mut self) {
        self.span.zero_span();
    }
}

impl ZeroSpan for StructDefine {
    fn zero_span(&mut self) {
        self.fields.zero_span();
        self.methods.zero_span();
        self.span.zero_span();
    }
}

impl ZeroSpan for VariantField {
    fn zero_span(&mut self) {
        self.span.zero_span();
    }
}

impl ZeroSpan for VariantFieldKind {
    fn zero_span(&mut self) {
        match self {
            Self::Field(field) => field.zero_span(),
            Self::Method(node) => node.zero_span(),
        }
    }
}

impl ZeroSpan for VariantDefine {
    fn zero_span(&mut self) {
        self.fields.zero_span();
        self.methods.zero_span();
        self.span.zero_span();
    }
}

impl ZeroSpan for Group1Node {
    fn zero_span(&mut self) {
        match self {
            Self::FuncDefine(node) => node.zero_span(),
            Self::StructDefine(node) => node.zero_span(),
            Self::VariantDefine(node) => node.zero_span(),
            Self::TypeAlias(node) => node.zero_span(),
            Self::Include(path) => path.zero_span(),
            Self::Line(_) => {}
        }
    }
}
