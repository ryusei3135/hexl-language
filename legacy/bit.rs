


pub fn toggle_bit(x: &mut u8, bit: u8) {
    *x ^= 1 << bit;
}

pub fn is_bit_set(x: u8, bit: u8) -> bool {
    (x & (1 << bit)) != 0
}