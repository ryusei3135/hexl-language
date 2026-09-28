//! レジスタの定義表
//!
//! # レジスタを追加するには
//!
//! 下の `define_regs!` に **1行足すだけ** です。他のファイルは変更不要です。
//!
//! ```text
//! バリアント名 => ("アセンブリ上の名前", レジスタ番号(0-15), ビット幅),
//! ```
//!
//! - `enum Reg` / `Reg::id()` / `Reg::bits()` / `Reg::from_name()` は
//!   すべてこの表から自動生成される。
//! - アセンブリ文字列 -> `Reg` の変換 (`convert.rs`) は `Reg::from_name` を使う。

macro_rules! define_regs {
    ( $( $variant:ident => ($name:literal, $id:literal, $bits:literal) ),* $(,)? ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Reg {
            $( $variant ),*
        }

        impl Reg {
            /// ModRM / REX に埋め込むレジスタ番号 (0-15)
            pub fn id(self) -> u8 {
                match self {
                    $( Reg::$variant => $id ),*
                }
            }

            /// レジスタのビット幅 (8 / 16 / 32 / 64)
            pub fn bits(self) -> u8 {
                match self {
                    $( Reg::$variant => $bits ),*
                }
            }

            /// アセンブリ上の名前 (`%rax` など) から `Reg` を引く
            pub fn from_name(name: &str) -> Option<Reg> {
                match name {
                    $( $name => Some(Reg::$variant), )*
                    _ => None,
                }
            }

            /// REX プレフィックスが必要な拡張レジスタ (`%r8`-`%r15` 系) か
            pub fn is_extended(self) -> bool {
                self.id() >= 8
            }
        }
    };
}

define_regs! {
    // 8 bit
    Al => ("%al", 0, 8),
    Cl => ("%cl", 1, 8),
    Dl => ("%dl", 2, 8),
    Bl => ("%bl", 3, 8),
    Spl => ("%spl", 4, 8),
    Bpl => ("%bpl", 5, 8),
    Sil => ("%sil", 6, 8),
    Dil => ("%dil", 7, 8),
    R8b => ("%r8b", 8, 8),
    R9b => ("%r9b", 9, 8),
    R10b => ("%r10b", 10, 8),
    R11b => ("%r11b", 11, 8),
    R12b => ("%r12b", 12, 8),
    R13b => ("%r13b", 13, 8),
    R14b => ("%r14b", 14, 8),
    R15b => ("%r15b", 15, 8),

    // 16 bit
    Ax => ("%ax", 0, 16),
    Cx => ("%cx", 1, 16),
    Dx => ("%dx", 2, 16),
    Bx => ("%bx", 3, 16),
    Sp => ("%sp", 4, 16),
    Bp => ("%bp", 5, 16),
    Si => ("%si", 6, 16),
    Di => ("%di", 7, 16),
    R8w => ("%r8w", 8, 16),
    R9w => ("%r9w", 9, 16),
    R10w => ("%r10w", 10, 16),
    R11w => ("%r11w", 11, 16),
    R12w => ("%r12w", 12, 16),
    R13w => ("%r13w", 13, 16),
    R14w => ("%r14w", 14, 16),
    R15w => ("%r15w", 15, 16),

    // 32 bit
    Eax => ("%eax", 0, 32),
    Ecx => ("%ecx", 1, 32),
    Edx => ("%edx", 2, 32),
    Ebx => ("%ebx", 3, 32),
    Esp => ("%esp", 4, 32),
    Ebp => ("%ebp", 5, 32),
    Esi => ("%esi", 6, 32),
    Edi => ("%edi", 7, 32),
    R8d => ("%r8d", 8, 32),
    R9d => ("%r9d", 9, 32),
    R10d => ("%r10d", 10, 32),
    R11d => ("%r11d", 11, 32),
    R12d => ("%r12d", 12, 32),
    R13d => ("%r13d", 13, 32),
    R14d => ("%r14d", 14, 32),
    R15d => ("%r15d", 15, 32),

    // 64 bit
    Rax => ("%rax", 0, 64),
    Rcx => ("%rcx", 1, 64),
    Rdx => ("%rdx", 2, 64),
    Rbx => ("%rbx", 3, 64),
    Rsp => ("%rsp", 4, 64),
    Rbp => ("%rbp", 5, 64),
    Rsi => ("%rsi", 6, 64),
    Rdi => ("%rdi", 7, 64),
    R8 => ("%r8", 8, 64),
    R9 => ("%r9", 9, 64),
    R10 => ("%r10", 10, 64),
    R11 => ("%r11", 11, 64),
    R12 => ("%r12", 12, 64),
    R13 => ("%r13", 13, 64),
    R14 => ("%r14", 14, 64),
    R15 => ("%r15", 15, 64),

    // 命令ポインタ (RIP相対アドレッシングでのみ使う)
    Rip => ("%rip", 5, 64),
}
