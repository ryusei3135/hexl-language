use clap::{Parser, Subcommand};
use self_update::cargo_crate_version;

// 戻り値: 更新が実行された場合は Ok(true)、すでに最新の場合は Ok(false)
pub fn handle_update() -> Result<bool, Box<dyn std::error::Error>> {
    let target = self_update::get_target();

    let status = self_update::backends::github::Update::configure()
        .repo_owner("ryusei3135") 
        .repo_name("hexl-language") 
        .bin_name("hexl") 
        .show_download_progress(true)          // ターミナルに進捗バーを表示
        .show_output(false)
        .current_version(cargo_crate_version!())
        .target(&target)
        .build()?
        .update()?;

    Ok(status.updated())
}
