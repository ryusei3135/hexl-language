//! `#include "dir/file"` の基準になる `ROOT` ディレクトリの管理。
//!
//! `#include`のパスは、`#include`を書いたファイルの場所ではなく、
//! **最初にコンパイルを始めたファイルのあるディレクトリ(`ROOT`)** が基準になる。
//! 入れ子の`#include`の中でも、常に`ROOT/dir/file`として解決される。
//!
//! - `Lexer::set_source_file`が、最も外側のファイルのときだけ`ROOT`を設定する。
//! - `#include`で取り込んだファイルを`crate::build`する間は`enter_include`の
//!   ガードを持つ。その間の`set_source_file`は`ROOT`を書き換えない。

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

/// `#include`に書かれたパス(`dir/file`)を`ROOT/dir/file`にする。
/// 絶対パスはそのまま返す。
pub fn resolve(rel: &str) -> String {
    root().join(rel).to_string_lossy().into_owned()
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
        // 外に出たら、次の最外ファイルで更新できる
        set_root_from_file(Path::new("other/main.hexl"));
        assert_eq!(
            resolve("x.hexl"),
            Path::new("other").join("x.hexl").to_string_lossy()
        );
    }
}
