//! hexl コンパイラ本体。
//!
//! `main.rs` は `run()` を呼ぶだけにして、モジュール宣言・コマンドライン
//! オプションの解釈・ビルド処理・終了コードの決定はすべてここに置く。

mod asm_setting;
mod assembler;
mod compiler;
mod macros;
mod dump;

// 他モジュールが `crate::err` / `crate::parse` / `crate::node` などの形で
// 参照している可能性があるため、元の `use` はそのまま残している。
#[allow(unused_imports)]
use compiler::{
    parse::{self, node},
    ir,
    lex,
    asm_gen,
    err,
};

use std::{error::Error, fs, process};

// ---------------------------------------------------------------------------
// コマンドラインオプション
// ---------------------------------------------------------------------------

pub enum OptFlags {
    FmtAsm,
    SetFile,
}

pub struct OptSettings {
    pub fmt_name: Option<String>,
    pub file_name: Option<String>,
    /// `dump` オプションが指定されたか
    pub dump: bool,
    /// `dump <file>` 形式で指定されたか（dump のみ行いビルドしない）
    pub dump_only: bool,

    opt_flags: Option<OptFlags>,
}

impl OptSettings {
    pub fn new(first_flag: OptFlags) -> Self {
        Self {
            fmt_name: None,
            file_name: None,
            dump: false,
            dump_only: false,

            opt_flags: Some(first_flag),
        }
    }

    pub fn new_file(&self, file_name: &str) -> Self {
        Self {
            fmt_name: self.fmt_name.clone(),
            file_name: Some(file_name.to_owned()),
            dump: self.dump,
            dump_only: self.dump_only,
            opt_flags: Some(OptFlags::SetFile),
        }
    }

    /// 値を渡す、フラグがすでに立っているなら成功
    pub fn set_value(&mut self, value: String) -> Result<(), err::opt::OptErrs> {
        if let Some(flag) = self.opt_flags.take() {
            let _ = match flag {
                OptFlags::FmtAsm => self.fmt_name.insert(value),
                OptFlags::SetFile => self.file_name.insert(value),
            };
            Ok(())
        } else {
            Err(err::opt::OptErrs::OptFlags)
        }
    }

    /// フラグを立てる
    /// もしフラグがすでに立っている場合はエラーになる
    pub fn set_flag(&mut self, flag_name: OptFlags) -> Result<(), err::opt::OptErrs> {
        if self.opt_flags.is_none() {
            let _ = self.opt_flags.insert(flag_name);
            Ok(())
        } else {
            Err(err::opt::OptErrs::OptFlags)
        }
    }
}

/// オプション管理
///
/// ここでは設定を集めるだけで、`dump` などの実処理は行わない（`run` が行う）。
pub fn mng_opt_cmd(args: &[&str]) -> OptSettings {
    let mut settings = OptSettings::new(OptFlags::SetFile);

    for (index, opt) in args.iter().enumerate() {
        // 1以下の数はオプションをつけれない
        match &index {
            0 => continue,
            1 => {
                // `dump <file>` 形式: 次の引数がファイル名になる
                // (opt_flags は初期状態の SetFile のまま残るので、
                //  次の引数が file_name に入る)
                if *opt == "dump" {
                    settings.dump = true;
                    settings.dump_only = true;
                    continue;
                }
                if let Err(ref e) = settings.set_value(opt.to_string()) {
                    eprintln!("警告: コマンドライン引数`{}`を無視しました: {}", opt, e);
                }
                continue;
            }
            _ => {}
        }

        match *opt {
            "dump" => settings.dump = true,
            "-f" => {
                if let Err(e) = settings.set_flag(OptFlags::FmtAsm) {
                    eprintln!("警告: オプション`-f`を無視しました: {}", e);
                }
            }
            // フラグ以外の文字
            _ => {
                if let Err(e) = settings.set_value(opt.to_string()) {
                    eprintln!("警告: コマンドライン引数`{}`を無視しました: {}", opt, e);
                }
            }
        }
    }
    settings
}

// ---------------------------------------------------------------------------
// ビルド
// ---------------------------------------------------------------------------

/// もらった情報で、データを生成
/// ## 戻り値
/// 関数の戻り値は呼び出し元に現在の公開されている
/// 関数の情報の配列を返す
///
/// ## Errors
/// - ファイル名が指定されていない場合
/// - ソースファイルの読み込みに失敗した場合
/// - 字句解析・構文解析・IR生成のいずれかに失敗した場合
/// - アセンブリ言語ファイルの書き込みに失敗した場合
pub fn build(
    settings: &OptSettings,
) -> Result<Vec<ir::def_tree::FnDefMetaData>, Box<dyn Error>> {
    // 初期化
    let file_name = settings
        .file_name
        .as_ref()
        .ok_or_else(|| "ファイル名が指定されていません".to_string())?;

    let content = fs::read_to_string(file_name)
        .map_err(|e| format!("ファイル`{}`を読み込めません: {}", file_name, e))?;

    let mut lexer = lex::Lexer::new();
    let mut parser = parse::Parser::new();
    let mut ir_builder = ir::IR::new();

    // アセンブリ言語のデータを作成
    lexer.analy(&content)?;

    let nodes = parser.parser(&lexer.gen_tkns)?;

    let func_def_meta_data = ir_builder
        .builder(
            &nodes,
            #[cfg(not(test))]
            &settings,
        )
        .map_err(|e| format!("IRの生成に失敗しました: {:?}", e))?;
    // dbg!(&ir_builder);

    let asm_text = asm_setting::gen_asm_text(
        ir_builder.func_tree,
        &ir_builder.extern_funcs,
        &ir_builder.public_func_tree,
        &settings.fmt_name,
    );

    assembler::x64::emitter_x64("-c", &asm_text, "linux");

    // 出力先のアセンブリ言語のファイル
    let asm_file = file_name.replace(".hexl", "");
    fs::write(format!("{}.s", asm_file), asm_text)
        .map_err(|e| format!("ファイル`{}.s`へ書き込めません: {}", asm_file, e))?;

    Ok(func_def_meta_data)
}

// ---------------------------------------------------------------------------
// dump
// ---------------------------------------------------------------------------

/// `dump` オプション指定時に `.text` を表示する。成功したら true を返す。
/// 失敗しても警告のみで、呼び出し側が続行するかどうかを決める。
fn dump_text(settings: &OptSettings) -> bool {
    let file_name = match settings.file_name.as_deref() {
        Some(name) => name,
        None => {
            eprintln!("警告: dump するファイル名が指定されていません");
            return false;
        }
    };

    match dump::load::dump(file_name, "text") {
        Ok(_) => true,
        Err(e) => {
            eprintln!("警告: `{}`のdumpに失敗しました: {}", file_name, e);
            false
        }
    }
}

// ---------------------------------------------------------------------------
// エントリポイント
// ---------------------------------------------------------------------------

/// コマンドライン引数（先頭はプログラム名）を受け取って全体を実行し、終了コードを返す。
///
/// ```text
/// hexl dump a.out        # a.out の .text を表示して終了
/// hexl main.hexl dump    # dump してからビルド
/// hexl main.hexl         # ビルドのみ
/// ```
pub fn run(args: impl IntoIterator<Item = String>) -> process::ExitCode {
    let args_vec: Vec<String> = args.into_iter().collect();
    // 各 String への参照（&str）を集めた Vec を作る
    let args_refs: Vec<&str> = args_vec.iter().map(String::as_str).collect();

    // オプションなどの設定
    let settings = mng_opt_cmd(&args_refs);

    asm_setting::load_setting();

    // `hexl dump <file>`: dump だけ行って終了
    if settings.dump_only {
        return if dump_text(&settings) {
            process::ExitCode::SUCCESS
        } else {
            process::ExitCode::FAILURE
        };
    }

    // `hexl <file> dump`: dump してからビルド
    if settings.dump {
        dump_text(&settings);
    }

    match build(&settings) {
        Ok(_) => process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("エラー: {}", e);
            process::ExitCode::FAILURE
        }
    }
}