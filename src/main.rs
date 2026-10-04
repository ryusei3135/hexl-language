// Cargo.toml の [package] name（ライブラリ名）に合わせること
use hexl::run;

fn main() -> std::process::ExitCode {
    run(std::env::args())
}