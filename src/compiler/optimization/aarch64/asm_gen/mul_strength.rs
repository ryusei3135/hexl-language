//! AArch64向け: 定数との乗算(`x * C`)の強度削減(strength reduction)。
//!
//! `mul`(レイテンシ3〜4)の代わりに、シフト付きオペランドを持つ
//! `add`/`sub`と`lsl`だけで作れる定数を、最大2命令で置き換える。
//! x64の`lea`相当は、AArch64では`add d, x, x, lsl #k`で表せる。
//!
//! | 定数 `C`                    | 置き換え                                  | 命令数 |
//! |-----------------------------|-------------------------------------------|--------|
//! | `2^k`  (k >= 1)             | `lsl d, x, #k`                            | 1      |
//! | `2^k + 1`  (3, 5, 9, 17...) | `add d, x, x, lsl #k`                     | 1      |
//! | `(2^k + 1) << s`(6, 10, 12..)| `add d, x, x, lsl #k` → `lsl d, d, #s`   | 2      |
//! | `2^k - 1`  (7, 15, 31...)   | `sub d, x, x, lsl #k` → `neg d, d`        | 2      |
//!
//! 上記以外(`0`・`1`・負数・`11`・`13`など)は`None`を返し、通常の`mul`に任せる。
//! 結果は`bits`ビット幅で切り捨てられる(2の補数で同じ値になる)ので、
//! 符号付き/符号なしのどちらにも使える。

/// 定数との乗算の置き換え方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MulPlan {
    /// `x * 2^k` (`k >= 1`) → `lsl d, x, #k`
    Shift(u32),
    /// `x * (2^k + 1)` → `add d, x, x, lsl #k`
    AddShift { k: u32 },
    /// `x * ((2^k + 1) << s)` → `add d, x, x, lsl #k` → `lsl d, d, #s`
    AddShiftShift { k: u32, s: u32 },
    /// `x * (2^k - 1)` → `sub d, x, x, lsl #k`(= `-C*x`)→ `neg d, d`
    SubShift { k: u32 },
}

impl MulPlan {
    /// 置き換えた後の命令数
    pub fn inst_count(&self) -> u32 {
        match self {
            Self::Shift(_) | Self::AddShift { .. } => 1,
            Self::AddShiftShift { .. } | Self::SubShift { .. } => 2,
        }
    }
}

fn is_pow2(v: i128) -> bool {
    v > 0 && v & (v - 1) == 0
}

/// `x * constant`(`x`は`bits`ビットの整数)の置き換え方を返す。
///
/// `bits`は、乗算の結果の型の幅(`8`/`16`/`32`/`64`)。
/// `lsl`のシフト量は幅未満でなければならない(32bitレジスタでは`mod 32`に
/// 丸められて結果が変わる)ので、そのような定数は置き換えない。
/// 定数が`bits`ビットに収まらない場合も置き換えない。
pub fn plan_mul(constant: i128, bits: u32) -> Option<MulPlan> {
    if constant <= 1 || bits == 0 || bits > 64 || constant >= (1i128 << bits) {
        return None;
    }
    // 2の累乗
    if is_pow2(constant) {
        return Some(MulPlan::Shift(constant.trailing_zeros()));
    }
    // 下位の0を外して、奇数 m と シフト量 s に分ける: C = m << s
    let s = constant.trailing_zeros();
    let m = constant >> s;
    // m = 2^k + 1
    if is_pow2(m - 1) {
        let k = (m - 1).trailing_zeros();
        if k < bits && s < bits {
            return Some(if s == 0 {
                MulPlan::AddShift { k }
            } else {
                MulPlan::AddShiftShift { k, s }
            });
        }
        return None;
    }
    // C = 2^k - 1(s == 0 のときだけ。`(2^k-1)<<s`は3命令になるので対象外)
    if s == 0 && is_pow2(m + 1) {
        let k = (m + 1).trailing_zeros();
        if k < bits {
            return Some(MulPlan::SubShift { k });
        }
    }
    None
}

/// レジスタ名。`bits`が64なら`x`、それ以外は`w`のレジスタを使う。
pub fn reg_name(num: u32, bits: u32) -> String {
    format!("{}{}", if bits == 64 { 'x' } else { 'w' }, num)
}

/// `dst = src * constant` のアセンブリ(GNU形式)を返す。置き換えられなければ`None`。
///
/// `dst`と`src`は同じレジスタでもよい(一時レジスタは使わない)。
/// `dst`/`src`は`x0`/`w0`のようなレジスタ名。
pub fn emit_mul(dst: &str, src: &str, constant: i128, bits: u32) -> Option<Vec<String>> {
    let plan = plan_mul(constant, bits)?;
    Some(match plan {
        MulPlan::Shift(k) => vec![format!("lsl {dst}, {src}, #{k}")],
        MulPlan::AddShift { k } => vec![format!("add {dst}, {src}, {src}, lsl #{k}")],
        MulPlan::AddShiftShift { k, s } => vec![
            format!("add {dst}, {src}, {src}, lsl #{k}"),
            format!("lsl {dst}, {dst}, #{s}"),
        ],
        MulPlan::SubShift { k } => vec![
            format!("sub {dst}, {src}, {src}, lsl #{k}"),
            format!("neg {dst}, {dst}"),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 生成した命令列を、小さなインタプリタで実行する(`dst`と`src`は別レジスタ/同一)
    fn run(asm: &[String], x: u64, bits: u32) -> u64 {
        let mask: u64 = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
        let mut src = x & mask;
        let mut dst = src;
        for line in asm {
            let line = line.replace(',', " ").replace('#', "");
            let t: Vec<&str> = line.split_whitespace().collect();
            let val = |name: &str, d: u64, s: u64| if name == "d" { d } else { s };
            match t[0] {
                "lsl" => dst = (val(t[2], dst, src) << t[3].parse::<u32>().unwrap()) & mask,
                "add" => {
                    let sh = t[5].parse::<u32>().unwrap();
                    dst = (val(t[2], dst, src)
                        .wrapping_add(val(t[3], dst, src) << sh))
                        & mask
                }
                "sub" => {
                    let sh = t[5].parse::<u32>().unwrap();
                    dst = (val(t[2], dst, src)
                        .wrapping_sub(val(t[3], dst, src) << sh))
                        & mask
                }
                "neg" => dst = dst.wrapping_neg() & mask,
                other => panic!("{other}"),
            }
            src = src & mask;
        }
        dst
    }

    #[test]
    fn plans() {
        assert_eq!(plan_mul(2, 32), Some(MulPlan::Shift(1)));
        assert_eq!(plan_mul(8, 32), Some(MulPlan::Shift(3)));
        assert_eq!(plan_mul(3, 32), Some(MulPlan::AddShift { k: 1 }));
        assert_eq!(plan_mul(5, 64), Some(MulPlan::AddShift { k: 2 }));
        assert_eq!(plan_mul(17, 64), Some(MulPlan::AddShift { k: 4 }));
        assert_eq!(plan_mul(6, 32), Some(MulPlan::AddShiftShift { k: 1, s: 1 }));
        assert_eq!(plan_mul(10, 32), Some(MulPlan::AddShiftShift { k: 2, s: 1 }));
        assert_eq!(plan_mul(7, 32), Some(MulPlan::SubShift { k: 3 }));
        assert_eq!(plan_mul(15, 32), Some(MulPlan::SubShift { k: 4 }));
    }

    #[test]
    fn shift_count_must_be_smaller_than_width() {
        assert_eq!(plan_mul(1 << 32, 32), None);
        assert_eq!(plan_mul(1 << 8, 8), None);
        assert_eq!(plan_mul(128, 8), Some(MulPlan::Shift(7)));
        assert_eq!(plan_mul(1 << 63, 64), Some(MulPlan::Shift(63)));
    }

    #[test]
    fn others_are_not_optimized() {
        for c in [-8, -1, 0, 1, 11, 13, 14, 100] {
            assert_eq!(plan_mul(c, 32), None, "{}", c);
        }
    }

    /// 全ての置き換え可能な定数で、`mul`と同じ結果になる(幅ごとに検証)
    #[test]
    fn matches_real_multiplication() {
        let xs: [u64; 8] = [0, 1, 2, 3, 0x7f, 0x80, 0xdead_beef, u64::MAX];
        for bits in [8u32, 16, 32, 64] {
            let mask: u64 = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
            for c in 2i128..2000 {
                let Some(asm) = emit_mul("d", "s", c, bits) else { continue };
                for &x in &xs {
                    let want = (x & mask).wrapping_mul(c as u64) & mask;
                    assert_eq!(run(&asm, x, bits), want, "c={c} bits={bits} x={x:#x}");
                }
            }
            for k in 1..bits {
                let c = 1i128 << k;
                let asm = emit_mul("d", "s", c, bits).unwrap();
                assert_eq!(run(&asm, 3, bits), 3u64.wrapping_mul(c as u64) & mask);
            }
        }
    }

    #[test]
    fn emit_text() {
        assert_eq!(emit_mul("x0", "x1", 7, 64).unwrap(), ["sub x0, x1, x1, lsl #3", "neg x0, x0"]);
        assert_eq!(emit_mul("w0", "w0", 5, 32).unwrap(), ["add w0, w0, w0, lsl #2"]);
        assert_eq!(reg_name(3, 64), "x3");
        assert_eq!(reg_name(3, 32), "w3");
    }
}
