//! 条件付きコンパイル(`#if` / `#elif` / `#else` / `#endif`)

use super::Preprocessor;

/// `#if` / `#elif` / `#else` の入れ子1段分の状態。
pub(super) struct CondFrame {
    /// 外側のスコープ自体が有効かどうか
    parent_active: bool,
    /// 今いる枝(`#if` / `#elif` / `#else`)の条件が成り立っているか
    condition: bool,
    /// これまでの枝のどれかが既に採用されたか(以降の `#elif` / `#else` は無効)
    taken: bool,
    /// すでに `#else` を通過したか
    in_else: bool,
}

impl CondFrame {
    fn active(&self) -> bool {
        self.parent_active && self.condition
    }
}

/// 今いる位置が有効な枝の中か(`#if`の外なら有効)
pub(super) fn is_active(stack: &[CondFrame]) -> bool {
    stack.last().map(CondFrame::active).unwrap_or(true)
}

impl Preprocessor {
    /// `#if #A` / `#elif #A`の条件が成り立つか(`A`が定義済みか)
    fn eval_cond(&self, rest_of_line: &str) -> bool {
        self.is_defined(cond_target(rest_of_line))
    }

    /// `#if #NAME`: 入れ子を1段積む(`#if #A` のように、条件は `#NAME` の形で書く)
    pub(super) fn cond_if(
        &self,
        stack: &mut Vec<CondFrame>,
        rest_of_line: &str,
        parent_active: bool,
    ) {
        let condition = self.eval_cond(rest_of_line);
        stack.push(CondFrame {
            parent_active,
            condition,
            taken: condition,
            in_else: false,
        });
    }

    /// `#elif #NAME`: `#else`の後ろに書くと panic する
    pub(super) fn cond_elif(&self, stack: &mut [CondFrame], rest_of_line: &str) {
        let condition = self.eval_cond(rest_of_line);
        if let Some(frame) = stack.last_mut() {
            if frame.in_else {
                panic!("`#elif` after `#else`");
            }
            // 既にどれかの枝を採用していたら、条件に関係なく無効
            frame.condition = !frame.taken && condition;
            frame.taken |= frame.condition;
        }
    }
}

/// `#else`
pub(super) fn cond_else(stack: &mut [CondFrame]) {
    if let Some(frame) = stack.last_mut() {
        frame.condition = !frame.taken;
        frame.taken = true;
        frame.in_else = true;
    }
}

/// `#endif`: 入れ子を1段戻す
pub(super) fn cond_endif(stack: &mut Vec<CondFrame>) {
    stack.pop();
}

/// `#if #NAME` / `#elif #NAME` の条件部分(`#NAME`)から `NAME` を取り出す
fn cond_target(rest_of_line: &str) -> &str {
    rest_of_line.trim().trim_start_matches('#')
}
