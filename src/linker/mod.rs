//! リンカ本体のモジュール群。
//!
//! * [`elf`]   — ELF64 オブジェクトファイルのパーサ
//! * [`error`] — エラー型
//! * `link`    — セクション配置・シンボル解決・リロケーション・出力 (`Linker`)
//!
//! 一番手軽な入口は、オブジェクトファイルのパスを渡すだけの [`linker`] 関数。
//! オプション (エントリ名・ロードアドレス) を変えたい場合や、メモリ上のオブジェクトを
//! 渡したい場合は [`Linker`] を直接使う。

pub mod elf;
pub mod error;
mod link;

use std::path::Path;

pub use error::{LinkError, Result, UndefinedRef};
pub use link::{
    LinkOptions, LinkOutput, Linker, OutputSection, OutputSymbol, DEFAULT_BASE_ADDRESS,
};

/// オブジェクトファイルのパスを受け取り、すべてを読み込んでリンクする。
///
/// 既定のオプション (エントリ `_start`、ロードアドレス `0x400000`) を使う。
/// パスは渡した順にリンクされる。結果は [`LinkOutput`] として返り、
/// ファイルへ書き出すには [`LinkOutput::write_to_file`] を呼ぶ。
///
/// ```no_run
/// # fn main() -> Result<(), objlink::LinkError> {
/// let output = objlink::linker(["main.o", "util.o"])?;
/// output.write_to_file("a.out")?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// 読み込めないファイル、ELF として不正な入力、未対応の構造、シンボル解決の失敗、
/// リロケーションのオーバーフローなど、リンクに失敗したときは [`LinkError`] を返す。

pub fn linker<I, P>(paths: I) -> Result<LinkOutput>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut linker = Linker::new();
    for path in paths {
        linker.add_object_file(path)?;
    }
    linker.link()
}
