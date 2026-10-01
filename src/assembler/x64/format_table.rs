//! 出力ファイル形式の対応表
//!
//! # 出力形式を追加するには
//!
//! 1. 新しい形式のバイナリを作る関数を書く
//!    (`elf.rs` / `pe.rs` と同じ形: `pub fn write_xxx(&Emitter) -> Result<Vec<u8>, String>`)。
//!    新しいファイル (例: `macho.rs`) にして、`mod.rs` に `pub mod macho;` を足す。
//! 2. 下の `FORMATS` に **1行足す**。
//!
//! ```text
//! OutputFormat { mode: "-c", os: Some("mac"), file_name: "a.out",
//!                executable: true, write: macho::write_executable },
//! ```
//!
//! `emitter_x64` (`mod.rs`) 側の変更は不要。
//! エラーメッセージに出る「指定できるモード/OS」の一覧もこの表から作られる。

use super::emitter::Emitter;
use super::{elf, pe};

pub(super) struct OutputFormat {
    /// `emitter_x64` の `mode` 引数 (`"-o"` / `"-c"`)
    pub mode: &'static str,
    /// `emitter_x64` の `os` 引数。`None` はOSに依存しない形式
    pub os: Option<&'static str>,
    /// 書き出すファイル名
    pub file_name: &'static str,
    /// 書き出した後に実行権限を付けるか
    pub executable: bool,
    /// エミッタの結果からファイルの中身を作る関数
    pub write: fn(&Emitter) -> Result<Vec<u8>, String>,
}

pub(super) static FORMATS: &[OutputFormat; 3] = &[
    // 再配置可能オブジェクトファイル (ELF固定。`os`には依存しない)
    OutputFormat {
        mode: "-o",
        os: None,
        file_name: "a.o",
        executable: false,
        write: elf::write_object,
    },
    // Linux 用の実行ファイル
    OutputFormat {
        mode: "-c",
        os: Some("linux"),
        file_name: "a.elf",
        executable: true,
        write: elf::write_executable,
    },
    // Windows (PE32+) 用の実行ファイル
    OutputFormat {
        mode: "-c",
        os: Some("win"),
        file_name: "a.exe",
        executable: true,
        write: pe::write_executable,
    },
];

/// `mode` / `os` に対応する出力形式を表から探す
pub(super) fn find(mode: &str, os: &str) -> Result<&'static OutputFormat, String> {
    let candidates: Vec<&'static OutputFormat> =
        FORMATS.iter().filter(|f| f.mode == mode).collect();

    if candidates.is_empty() {
        return Err(format!(
            "不明なモードです: {} ({} を指定してください)",
            mode,
            join_quoted(unique(FORMATS.iter().map(|f| f.mode)))
        ));
    }

    // OSに依存しない形式ならそのまま使う
    if let Some(format) = candidates.iter().find(|f| f.os.is_none()) {
        return Ok(*format);
    }

    candidates
        .iter()
        .find(|f| f.os == Some(os))
        .copied()
        .ok_or_else(|| {
            format!(
                "不明なOSです: {} ({} を指定してください)",
                os,
                join_quoted(unique(candidates.iter().filter_map(|f| f.os)))
            )
        })
}

fn unique<'a>(items: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();

    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }

    out
}

fn join_quoted(items: Vec<&str>) -> String {
    items
        .iter()
        .map(|s| format!("\"{}\"", s))
        .collect::<Vec<_>>()
        .join(" または ")
}
