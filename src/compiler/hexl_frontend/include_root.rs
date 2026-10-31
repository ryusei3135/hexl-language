//! `#include "dir/file"` の基準になる `ROOT` ディレクトリの管理。
//!
//! `#include`のパスは、`#include`を書いたファイルの場所ではなく、
//! **最初にコンパイルを始めたファイルのあるディレクトリ(`ROOT`)** が基準になる。
//! 入れ子の`#include`の中でも、常に`ROOT/dir/file`として解決される。
//!
//! - `Lexer::set_source_file`が、最も外側のファイルのときだけ`ROOT`を設定する。
//! - `#include`で取り込んだファイルを`crate::build`する間は`enter_include`の
//!   ガードを持つ。その間の`set_source_file`は`ROOT`を書き換えない。
//!
//! ## 標準ライブラリ(`std`)
//! `#include std::"os/vec.hexl"`(`ModPath::is_std`)は`ROOT`ではなく、
//! **hexl本体(実行ファイル)のあるディレクトリ**の`library/`を基準にする
//! (-> `<実行ファイルのディレクトリ>/library/os/vec.hexl`)。入れ子の`#include`の中でも同じ。
//! `#include "std/os/vec.hexl"`のような文字列だけの書き方は、ただのパスなので`ROOT`が基準。

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

thread_local! {
    static ROOT: RefCell<PathBuf> = RefCell::new(PathBuf::new());
    static DEPTH: Cell<usize> = Cell::new(0);
}

/// `#include`で取り込んだファイルを処理している間だけ持つガード
pub struct IncludeGuard;

impl Drop for IncludeGuard {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// `#include`の先を`build`する直前に呼ぶ。戻り値のガードが生きている間は、
/// `ROOT`は変わらない。
pub fn enter_include() -> IncludeGuard {
    DEPTH.with(|d| d.set(d.get() + 1));
    IncludeGuard
}

/// 最も外側のファイル(`#include`の先ではないファイル)のときだけ、
/// そのファイルのあるディレクトリを`ROOT`にする。
pub fn set_root_from_file(file: &Path) {
    if DEPTH.with(Cell::get) == 0 {
        let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
        ROOT.with(|r| *r.borrow_mut() = dir);
    }
}

/// 現在の`ROOT`ディレクトリ(未設定なら空=実行時のカレント)
pub fn root() -> PathBuf {
    ROOT.with(|r| r.borrow().clone())
}

/// 標準ライブラリのディレクトリ名(hexl本体のあるディレクトリの下)
const STD_DIR: &str = "library";

/// hexl本体(実行ファイル)のあるディレクトリ。取得できなければ`None`
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

/// `resolve_std`の本体。`exe_dir`を引数にして、テストできるようにしている
fn resolve_std_in(rel: &str, exe_dir: Option<&Path>) -> String {
    // 実行ファイルの場所が分からないときは、カレントの`library/`を探す
    let base = exe_dir.map(Path::to_path_buf).unwrap_or_default();
    let lib = base.join(STD_DIR);
    // `cargo run`では実行ファイルが`target/debug/`にあり、その隣に`library/`が
    // 無いので、無ければビルドしたソースのプロジェクト直下の`library/`を使う
    let lib = if lib.is_dir() {
        lib
    } else {
        let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join(STD_DIR);
        if dev.is_dir() { dev } else { lib }
    };
    lib.join(rel).to_string_lossy().into_owned()
}

/// 標準ライブラリのパス(`os/vec.hexl`)を`<hexl本体のディレクトリ>/library/os/vec.hexl`にする
pub fn resolve_std(rel: &str) -> String {
    resolve_std_in(rel, exe_dir().as_deref())
}

/// `#include`に書かれたパス(`dir/file`)を`ROOT/dir/file`にする。
/// 絶対パスはそのまま返す。
pub fn resolve(rel: &str) -> String {
    root().join(rel).to_string_lossy().into_owned()
}

/// `#include`の`path`(`rel`はそのファイルのパス)を解決する。
/// `std::"..."`なら標準ライブラリ、それ以外は`ROOT`が基準
pub fn resolve_for(path: &crate::compiler::ast::ModPath, rel: &str) -> String {
    if path.is_std {
        resolve_std(rel)
    } else {
        resolve(rel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_is_relative_to_root_even_when_nested() {
        set_root_from_file(Path::new("proj/src/main.hexl"));
        assert_eq!(
            resolve("lib/a.hexl"),
            Path::new("proj/src").join("lib/a.hexl").to_string_lossy()
        );
        {
            let _g = enter_include();
            // 取り込み先のファイルを処理中でも、ROOTは変わらない
            set_root_from_file(Path::new("proj/src/lib/a.hexl"));
            assert_eq!(
                resolve("lib/b.hexl"),
                Path::new("proj/src").join("lib/b.hexl").to_string_lossy()
            );
        }
        // 標準ライブラリは、ROOTに関係なく`<hexl本体のディレクトリ>/library/...`
        assert_eq!(
            resolve_std_in("os/vec.hexl", Some(Path::new("/opt/hexl"))),
            Path::new("/opt/hexl")
                .join("library")
                .join("os/vec.hexl")
                .to_string_lossy()
        );
        // `"std/..."`は、ただの`ROOT`基準のパス
        assert_eq!(
            resolve("std/os/vec.hexl"),
            Path::new("proj/src")
                .join("std/os/vec.hexl")
                .to_string_lossy()
        );
        // 外に出たら、次の最外ファイルで更新できる
        set_root_from_file(Path::new("other/main.hexl"));
        assert_eq!(
            resolve("x.hexl"),
            Path::new("other").join("x.hexl").to_string_lossy()
        );
    }
}
