//! リンカ本体のモジュール群。
//!
//! * [`elf`]   — ELF64 オブジェクトファイルのパーサ (リンカ内部表現も兼ねる)
//! * [`coff`]  — COFF (x86-64) オブジェクトファイルのパーサ。内部表現へ変換する
//! * [`error`] — エラー型
//! * `pe`      — PE32+ ヘッダ・import テーブルの生成
//! * `link`    — セクション配置・シンボル解決・リロケーション・出力 (`Linker`)
//!
//! 入力は ELF64 / COFF を内容から自動判定する。出力は ELF (既定) か PE32+ で、
//! [`LinkOptions::format`] で選ぶ。
//! 一番手軽な入口は、オブジェクトファイルのパスを渡すだけの [`linker`] 関数。
//! オプション (エントリ名・ロードアドレス) を変えたい場合や、メモリ上のオブジェクトを
//! 渡したい場合は [`Linker`] を直接使う。

pub mod coff;
pub mod elf;
pub mod error;
mod link;
mod pe;

use std::path::Path;

pub use error::LinkResult;
pub use link::{LinkOptions, LinkOutput, Linker};
pub use pe::DllImport;

/// オブジェクトファイルのパスを受け取り、すべてを読み込んでリンクする。
///
/// 既定のオプション (エントリ `_start`、ロードアドレス `0x400000`) を使う。
/// パスは渡した順にリンクされる。結果は [`LinkOutput`] として返り、
/// ファイルへ書き出すには [`LinkOutput::write_to_file`] を呼ぶ。
///
/// # Errors
///
/// 読み込めないファイル、ELF / COFF として不正な入力、未対応の構造、シンボル解決の失敗、
/// リロケーションのオーバーフローなど、リンクに失敗したときは [`LinkError`] を返す。

pub fn linker<I, P>(paths: I) -> LinkResult<LinkOutput>
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

pub fn linker_with_options<I, P>(paths: I, options: LinkOptions) -> LinkResult<LinkOutput>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut linker = Linker::with_options(options);
    for path in paths {
        linker.add_object_file(path)?;
    }
    linker.link()
}
