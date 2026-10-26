use clap::{Parser, Subcommand};
use self_update::cargo_crate_version;
use std::env;

// 戻り値: 更新が実行された場合は Ok(true)、すでに最新の場合は Ok(false)
pub fn handle_update(target_version: Option<String>) -> Result<bool, Box<dyn std::error::Error>> {
    let target = self_update::get_target();

    // 展開先のディレクトリを取得
    let mut current_exe_dir = env::current_exe()?;
    current_exe_dir.pop(); // バイナリ名を除去して親フォルダパスにする

    let mut builder = self_update::backends::github::Update::configure();
    
    builder
        .repo_owner("ryusei3135")
        .repo_name("hexl-language")
        .bin_install_path(&current_exe_dir)
        .show_download_progress(true)
        .show_output(false)
        .current_version(cargo_crate_version!())
        .target(&target);

    // 引数でバージョンが指定されている場合は、そのバージョンをターゲットに設定する
    if let Some(ref version) = target_version {
        builder.target_version_tag(&format!("v{}", version.trim_start_matches('v')));
    }
    // `.bin_name` を敢えて「アーカイブ内のルート（バイナリ名）」にしつつ、
    // 全体を展開するために、以下のようにビルドを組み立てます。
    let mut binding = self_update::backends::github::Update::configure();
    let status = binding
        .repo_owner("ryusei3135")
        .repo_name("hexl-language")
        .bin_name("hexl") // アーカイブ内の対象バイナリ名
        .show_download_progress(true)
        .show_output(false)
        .current_version(cargo_crate_version!())
        .target(&target);

    // バージョン指定の反映
    let mut status_builder = status;
    if let Some(version) = target_version {
        status_builder.target_version_tag(&format!("v{}", version.trim_start_matches('v')));
    }
    // self_update の通常アップデート（バイナリの置き換え）を実行
    let release_status = status_builder.build()?.update()?;
    Ok(release_status.updated())
}
