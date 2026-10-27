//! hexl コンパイラ本体。
//!
//! `main.rs` は `run()` を呼ぶだけにして、モジュール宣言・コマンドライン
//! オプションの解釈・ビルド処理・終了コードの決定はすべてここに置く。
use std::cell::RefCell;
use std::rc::Rc;

mod asm_setting;
mod assembler;
mod compiler;
mod dump;
mod linker;
mod macros;
mod updater;

#[cfg(target_os = "windows")]
const DEFAULT: &str = "windows";
/// windows 以外は linux 扱い
#[cfg(not(target_os = "windows"))]
const DEFAULT: &str = "linux";

// 他モジュールが `crate::err` / `crate::parse` / `crate::node` などの形で
// 参照している可能性があるため、元の `use` はそのまま残している。
#[allow(unused_imports)]
use compiler::{Lexer, Parser, asm_gen, ast, err, ir};

use std::{error::Error, fs, process, sync::Mutex};

use crate::compiler::{err::Span, stmt_node::StmtNode::Continue};

// ---------------------------------------------------------------------------
// コマンドラインオプション
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum OptFlags {
    FmtAsm,
    SetFile,
}

#[derive(Debug)]
pub struct OptSettings {
    pub fmt_name: Option<String>,
    pub file_name: Option<String>,
    /// `dump` オプションが指定されたか
    pub dump: bool,
    /// `dump <file>` 形式で指定されたか（dump のみ行いビルドしない）
    pub dump_only: bool,
    pub os: Option<String>,
    pub emit_asm: bool,

    opt_flags: Option<OptFlags>,
}

impl OptSettings {
    pub fn new(first_flag: OptFlags) -> Self {
        Self {
            fmt_name: None,
            file_name: None,
            dump: false,
            dump_only: false,
            os: None,
            emit_asm: false,

            opt_flags: Some(first_flag),
        }
    }

    pub fn new_file(&self, file_name: &str) -> Self {
        Self {
            fmt_name: self.fmt_name.clone(),
            file_name: Some(file_name.to_owned()),
            dump: self.dump,
            dump_only: self.dump_only,
            os: None,
            emit_asm: false,
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
                match *opt {
                    "dump" => {
                        settings.dump = true;
                        settings.dump_only = true;
                        continue;
                    }
                    "update" => {
                        println!("アップデートを確認中...");
                        match updater::handle_update(None) {
                            Ok(true) => {
                                println!("更新が成功しました。新しい機能をお楽しみください！");
                                std::process::exit(0);
                            }
                            Ok(false) => {
                                // すでに最新
                                std::process::exit(0);
                            }
                            Err(e) => {
                                eprintln!("アップデートに失敗しました: {}", e);
                                std::process::exit(1);
                            }
                        }
                        Continue;
                    }
                    _ => {}
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
            "--linux" => settings.os = Some("linux".to_string()),
            "--windows" => settings.os = Some("windows".to_string()),
            "-S" => settings.emit_asm = true,
            val => {
                if val.ends_with(".hexl") {
                    settings.file_name = Some(val.to_string());
                } else {
                    panic!();
                }
            } // // フラグ以外の文字
              // _ => {
              //     if let Err(e) = settings.set_value(opt.to_string()) {
              //         eprintln!("警告: コマンドライン引数`{}`を無視しました: {}", opt, e);
              //     }
              // }
        }
    }
    settings
}

// ---------------------------------------------------------------------------
// 生成したオブジェクトファイル(バイナリ)のパスの記憶
// ---------------------------------------------------------------------------

/// `#include` は IR 生成中に `crate::build` を再帰的に呼ぶため、
/// 取り込んだファイルごとに生成された再配置可能オブジェクトファイル(`.o`)の
/// パスもここへ集める。
/// 依存先のファイルほど先に登録される (メインのファイルは最後)。
static GENERATED_BIN: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// 生成したバイナリのパスを記憶する。同じパスが登録済みなら何もしない
/// (同じファイルを複数箇所から `#include` した場合の重複を避けるため)。
fn remember_binary(path: String) {
    let mut list = GENERATED_BIN.lock().unwrap_or_else(|e| e.into_inner());
    if !list.contains(&path) {
        list.push(path);
    }
}

/// 記憶しているバイナリのパスを全て取り出す (記憶は空になる)
fn take_generated_bin() -> Vec<String> {
    let mut list = GENERATED_BIN.lock().unwrap_or_else(|e| e.into_inner());
    std::mem::take(&mut *list)
}

/// 記憶した全てのオブジェクトファイルのパスを、登録順に出力する
fn print_binaries() {
    let paths = take_generated_bin();
    if paths.is_empty() {
        return;
    }
    println!("生成したオブジェクトファイル:");
    let output = linker::linker(paths).unwrap();
    output.write_to_file("a.out").unwrap();
    // for path in &paths {
    //     println!("  {}", path);
    // }
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
pub fn build(settings: &OptSettings) -> Result<Vec<ir::def_tree::FnDefMetaData>, Box<dyn Error>> {
    // 初期化
    let file_name = Rc::new(
        settings
            .file_name
            .clone()
            .ok_or_else(|| "ファイル名が指定されていません".to_string())?,
    );

    let content = fs::read_to_string((*file_name).as_str())
        .map_err(|e| format!("ファイル`{}`を読み込めません: {}", file_name, e))?;

    // `--linux` / `--windows` で選んだOS(未指定なら実行環境のOS)
    let os: String = if let Some(os) = &settings.os {
        os.clone()
    } else {
        DEFAULT.to_string()
    };

    // エラーメッセージを蓄積する
    let mut err_pool = Rc::new(RefCell::new(err::err_pool::ErrPool::new()));
    // c言語を処理
    let nodes = if let Some(ref file_path) = settings.file_name {
        if file_path.starts_with(".c") | file_path.starts_with(".h") {
            compiler::c_frontend::emit_c_node(
                &file_path,
                &Rc::new(settings.file_name.as_ref().unwrap().to_string()),
            )
        } else {
            // 選んだOSのシンボル(`#linux` / `#windows`)だけを定義して、
            // `#if #windows` / `#elif #linux` などの条件をこのOSで切り替える
            let mut lexer = Lexer::with_os(&os, &settings.file_name.as_ref().unwrap());
            let mut parser = Parser::new(&file_name, Rc::clone(&err_pool));

            // `#include "file"` の相対パスは、このファイルのあるディレクトリが基準
            lexer.set_source_file(Rc::clone(&file_name));

            // アセンブリ言語のデータを作成
            lexer.analy(&content)?;
            let c_file_list: &[String] = &lexer.include_c_file_list;

            parser.parser(&lexer.gen_tkns, c_file_list)?.to_vec()
        }
    } else {
        // 選んだOSのシンボル(`#linux` / `#windows`)だけを定義して、
        // `#if #windows` / `#elif #linux` などの条件をこのOSで切り替える
        let mut lexer = Lexer::with_os(
            &os,
            &Rc::new(settings.file_name.as_ref().unwrap().to_string()),
        );
        let mut parser = Parser::new(&file_name, Rc::clone(&err_pool));

        // `#include "file"` の相対パスは、このファイルのあるディレクトリが基準
        lexer.set_source_file(Rc::clone(&file_name));

        // アセンブリ言語のデータを作成
        lexer.analy(&content)?;
        let c_file_list: &[String] = &lexer.include_c_file_list;

        parser.parser(&lexer.gen_tkns, c_file_list)?.to_vec()
    };
    let mut ir_builder = ir::IR::new(&err_pool);

    let func_def_meta_data = ir_builder
        .builder::<ir::PanicErr>(
            &nodes,
            #[cfg(not(test))]
            Rc::clone(&err_pool),
            #[cfg(not(test))]
            &settings,
        )
        .map_err(|e| format!("IRの生成に失敗しました: {:?}", e))?;
    // dbg!(&ir_builder);

    let asm_text = asm_setting::gen_asm_text(
        ir_builder.func_tree,
        &ir_builder.extern_funcs,
        &ir_builder.public_func_tree,
        settings.fmt_name.as_deref(),
        os.as_str(),
        &err_pool,
    )
    // `AsmSettingError` は Display で日本語メッセージになる。`run` がそれを表示する
    .map_err(|e| format!("アセンブリコードの生成に失敗しました: {}", e))?;

    // エラーメッセージを表示
    err_pool.borrow().view();

    if settings.emit_asm {
        // 出力先のアセンブリ言語のファイル
        let asm_file = file_name.replace(".hexl", "");
        fs::write(format!("{}.s", asm_file), &asm_text)
            .map_err(|e| format!("ファイル`{}.s`へ書き込めません: {}", asm_file, e))?;
    }

    // 再配置可能オブジェクトファイル(ELF, `-o`)を生成する。
    // `#include` したファイルも同じ関数を通るので、ファイルごとに
    // `<ファイル名>.o` が作られる (出力名を分けないと互いに上書きしてしまう)。
    // `.s` は先に書き出してあるので、失敗しても内容は確認できる。
    let obj_file = file_name.replace(".hexl", ".o");
    let obj_path =
        assembler::x64::emitter_x64_to("-o", &asm_text, &os, Some(&obj_file)).map_err(|e| {
            format!(
                "オブジェクトファイル`{}`の生成に失敗しました: {}",
                obj_file, e
            )
        })?;

    // `#include` したファイルの分も含め、生成したオブジェクトファイルのパスを記憶する
    remember_binary(obj_path);

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

    // 設定ファイルが読めない場合は panic せず、メッセージを表示して終了する
    if let Err(e) = asm_setting::load_setting() {
        eprintln!("エラー: {}", e);
        return process::ExitCode::FAILURE;
    }

    // 前回の実行で記憶したものが残らないようにする
    take_generated_bin();

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
        Ok(_) => {
            // `#include` したファイルも含め、生成した全てのバイナリのパスを出力する
            print_binaries();
            process::ExitCode::SUCCESS
        }
        Err(e) => {
            take_generated_bin();
            eprintln!("エラー: {}", e);
            process::ExitCode::FAILURE
        }
    }
}
