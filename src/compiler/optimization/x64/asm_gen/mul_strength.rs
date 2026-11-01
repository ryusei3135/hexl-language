//! 定数との乗算(`x * C`)の強度削減(strength reduction)。
//!
//! `imul`を使わず、より軽い命令に置き換える方針だけを決める。
//! 実際のアセンブリの生成は`asm_gen/asm_emitter/mul_opt.rs`が行う。
//!
//! | 定数 `C`            | 置き換え                                   |
//! |---------------------|--------------------------------------------|
//! | `2`                 | `add`(`x + x`)                             |
//! | `4, 8, 16, ...`     | シフト(`shl $k`)                           |
//! | `3` / `5` / `9`     | `lea (r,r,2)` / `lea (r,r,4)` / `lea (r,r,8)` |
//! | `7`                 | `lea 0(,r,8)`と`sub`(`8x - x`)              |
//!
//! 上記以外(`0`・`1`・負数・`6`・`10`など)は`None`を返し、
//! 通常の乗算の生成に任せる。

/// 定数との乗算の置き換え方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MulPlan {
    /// `x * 2` → `x + x`
    SelfAdd,
    /// `x * 2^k` (`k >= 2`) → `x << k`
    Shift(u32),
    /// `x * 3/5/9` → `lea (x,x,scale)`。`scale`は`2`/`4`/`8`
    Lea { scale: u8 },
    /// `x * 7` → `lea 0(,x,8)`で`8x`を作り、`x`を引く
    Lea7,
}

/// `x * constant`(`x`は`bits`ビットの整数)の置き換え方を返す。
///
/// `bits`は、乗算の結果の型の幅(`8`/`16`/`32`/`64`)。
/// シフト量が幅以上になると、`shl`はシフト量を丸めて(32bitなら`mod 32`)
/// しまい結果が変わるので、そのような定数は置き換えない。
pub fn plan_mul(constant: i128, bits: u32) -> Option<MulPlan> {
    if constant <= 1 {
        return None;
    }
    // 2の累乗
    if constant & (constant - 1) == 0 {
        let k = constant.trailing_zeros();
        if k >= bits {
            return None;
        }
        return Some(if k == 1 {
            MulPlan::SelfAdd
        } else {
            MulPlan::Shift(k)
        });
    }
    match constant {
        3 => Some(MulPlan::Lea { scale: 2 }),
        5 => Some(MulPlan::Lea { scale: 4 }),
        9 => Some(MulPlan::Lea { scale: 8 }),
        7 => Some(MulPlan::Lea7),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_is_add() {
        assert_eq!(plan_mul(2, 32), Some(MulPlan::SelfAdd));
        assert_eq!(plan_mul(2, 8), Some(MulPlan::SelfAdd));
    }

    #[test]
    fn power_of_two_is_shift() {
        assert_eq!(plan_mul(4, 32), Some(MulPlan::Shift(2)));
        assert_eq!(plan_mul(8, 32), Some(MulPlan::Shift(3)));
        assert_eq!(plan_mul(1 << 30, 32), Some(MulPlan::Shift(30)));
        assert_eq!(plan_mul(1 << 62, 64), Some(MulPlan::Shift(62)));
    }

    #[test]
    fn shift_count_must_be_smaller_than_width() {
        assert_eq!(plan_mul(1 << 32, 32), None);
        assert_eq!(plan_mul(1 << 8, 8), None);
        assert_eq!(plan_mul(128, 8), Some(MulPlan::Shift(7)));
        assert_eq!(plan_mul(1 << 63, 64), Some(MulPlan::Shift(63)));
    }

    #[test]
    fn lea_constants() {
        assert_eq!(plan_mul(3, 32), Some(MulPlan::Lea { scale: 2 }));
        assert_eq!(plan_mul(5, 64), Some(MulPlan::Lea { scale: 4 }));
        assert_eq!(plan_mul(9, 16), Some(MulPlan::Lea { scale: 8 }));
        assert_eq!(plan_mul(7, 32), Some(MulPlan::Lea7));
    }

    #[test]
    fn others_are_not_optimized() {
        for c in [-8, -1, 0, 1, 6, 10, 11, 12, 100] {
            assert_eq!(plan_mul(c, 32), None, "{}", c);
        }
    }
}
